// SPDX-License-Identifier: GPL-3.0-or-later
use std::{
    io::Write,
    path::Path,
    process::{Child, Command, Stdio},
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::sync::{mpsc, oneshot};

struct ChildGuard(Child);
impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
struct Request {
    image: image::RgbaImage,
    complete: oneshot::Sender<bool>,
}
struct ClipboardService {
    requests: mpsc::UnboundedSender<Request>,
    leases: Arc<Mutex<Vec<flameshot_platform::clipboard::Lease>>>,
}
#[zbus::interface(name = "org.flameshot.Flameshot")]
impl ClipboardService {
    #[zbus(name = "attachScreenshotToClipboard")]
    async fn copy(&self, data: Vec<u8>) -> zbus::fdo::Result<()> {
        let image = flameshot_core::qt_wire::decode_clipboard(&data).unwrap();
        let (complete, finished) = oneshot::channel();
        self.requests
            .send(Request {
                image: image.clone(),
                complete,
            })
            .unwrap();
        if !finished.await.unwrap() {
            return Err(zbus::fdo::Error::Failed(
                "Synthetic clipboard rejection".into(),
            ));
        }
        let mut data = std::io::Cursor::new(Vec::new());
        image.write_to(&mut data, image::ImageFormat::Png).unwrap();
        let lease =
            flameshot_platform::clipboard::copy(data.into_inner(), "image/png", false).unwrap();
        self.leases.lock().unwrap().push(lease);
        Ok(())
    }
}
fn tool(root: &Path, args: &[&str]) -> String {
    let output = Command::new(root.join("usr/bin/xdotool"))
        .args(args)
        .output()
        .unwrap();
    assert!(output.status.success(), "{:?}: {:?}", args, output.stderr);
    String::from_utf8(output.stdout).unwrap()
}
async fn menu(root: &Path, window: &str, index: usize) {
    tool(root, &["mousemove", "--window", window, "20", "20"]);
    tool(root, &["click", "3"]);
    tokio::time::sleep(Duration::from_millis(200)).await;
    let mut args = vec!["key", "--delay", "150"];
    args.extend(std::iter::repeat_n("Down", index));
    args.push("Return");
    tool(root, &args);
    tokio::time::sleep(Duration::from_millis(200)).await;
}
fn geometry(root: &Path, window: &str) -> [u32; 4] {
    let text = tool(root, &["getwindowgeometry", "--shell", window]);
    let field = |key| {
        text.lines()
            .find_map(|line| line.strip_prefix(key))
            .unwrap()
            .parse()
            .unwrap()
    };
    [field("X="), field("Y="), field("WIDTH="), field("HEIGHT=")]
}
async fn size(root: &Path, window: &str, expected: [u32; 2]) {
    tokio::time::timeout(Duration::from_secs(5), async {
        while geometry(root, window)[2..] != expected {
            tokio::time::sleep(Duration::from_millis(30)).await;
        }
    })
    .await
    .unwrap();
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires a private D-Bus session and Xvfb display"]
async fn pin_copy_preserves_pixels_retries_failure_and_finishes_before_close() {
    assert_eq!(
        std::env::var("FLAMESHOT_TEST_PRIVATE_BUS").as_deref(),
        Ok("1")
    );
    let root =
        std::path::PathBuf::from(std::env::var_os("FLAMESHOT_TEST_PRIVATE_DISPLAY_TOOLS").unwrap());
    let temp = tempfile::tempdir().unwrap();
    std::fs::create_dir(temp.path().join("flameshot")).unwrap();
    std::fs::write(temp.path().join("flameshot/flameshot.ini"),"[General]\ncheckForUpdates=false\nshowStartupLaunchMessage=false\nshowDesktopNotification=false\nuseX11LegacyScreenshot=true\n").unwrap();
    let (requests, mut received) = mpsc::unbounded_channel();
    let leases = Arc::new(Mutex::new(Vec::new()));
    let _service = zbus::connection::Builder::session()
        .unwrap()
        .name("org.flameshot.Flameshot")
        .unwrap()
        .serve_at(
            "/",
            ClipboardService {
                requests,
                leases: leases.clone(),
            },
        )
        .unwrap()
        .build()
        .await
        .unwrap();
    let source = image::RgbaImage::from_fn(400, 300, |x, _| {
        image::Rgba(if x < 200 {
            [255, 0, 0, 255]
        } else {
            [0, 255, 0, 255]
        })
    });
    let expected = image::imageops::rotate90(&source);
    let wire = flameshot_core::qt_wire::encode_pin(&flameshot_core::qt_wire::Pin {
        image: source,
        geometry: flameshot_core::qt_wire::PinGeometry {
            x: 100,
            y: 100,
            width: 400,
            height: 300,
        },
        scale: 1.0,
    })
    .unwrap();
    let mut child = ChildGuard(
        Command::new(env!("CARGO_BIN_EXE_flameshot"))
            .arg("internal-pin")
            .env("XDG_CONFIG_HOME", temp.path())
            .env("XDG_CACHE_HOME", temp.path())
            .env("XDG_DATA_HOME", temp.path())
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .spawn()
            .unwrap(),
    );
    child.0.stdin.take().unwrap().write_all(&wire).unwrap();
    let window = tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let found = Command::new(root.join("usr/bin/xdotool"))
                .args(["search", "--onlyvisible", "--name", "^flameshot-pin$"])
                .output()
                .unwrap();
            if found.status.success() {
                break String::from_utf8(found.stdout)
                    .unwrap()
                    .lines()
                    .last()
                    .unwrap()
                    .to_owned();
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .unwrap();
    tool(&root, &["windowfocus", "--sync", &window]);
    tokio::time::sleep(Duration::from_millis(300)).await;
    menu(&root, &window, 2).await;
    size(&root, &window, [300, 400]).await;
    tool(&root, &["mousemove", "--window", &window, "150", "200"]);
    tool(&root, &["click", "4"]);
    size(&root, &window, [330, 440]).await;
    tool(&root, &["key", "5"]);
    let opacity_pixels = || {
        let screenshot = Command::new(env!("CARGO_BIN_EXE_flameshot"))
            .args(["full", "--raw"])
            .env("XDG_CONFIG_HOME", temp.path())
            .output()
            .unwrap();
        assert!(screenshot.status.success(), "{:?}", screenshot.stderr);
        let image = image::load_from_memory(&screenshot.stdout)
            .unwrap()
            .to_rgba8();
        let [x, y, w, h] = geometry(&root, &window);
        [
            image.get_pixel(x + w / 2, y + h / 4).0,
            image.get_pixel(x + w / 2, y + 3 * h / 4).0,
        ]
    };
    menu(&root, &window, 5).await;
    let pixels = opacity_pixels();
    assert!(
        (i32::from(pixels[0][0]) - 153).abs() <= 2 && (i32::from(pixels[1][1]) - 153).abs() <= 2,
        "{pixels:?}"
    );
    menu(&root, &window, 6).await;
    let pixels = opacity_pixels();
    assert!(
        (i32::from(pixels[0][0]) - 128).abs() <= 2 && (i32::from(pixels[1][1]) - 128).abs() <= 2,
        "{pixels:?}"
    );
    menu(&root, &window, 1).await;
    let request = tokio::time::timeout(Duration::from_secs(5), received.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        request.image, expected,
        "Zoom or opacity changed copied pixels"
    );
    request.complete.send(false).unwrap();
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(child.0.try_wait().unwrap().is_none());
    tool(&root, &["key", "Escape"]);
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert!(
        child.0.try_wait().unwrap().is_none(),
        "Dismissing the error closed the image"
    );
    menu(&root, &window, 1).await;
    let request = tokio::time::timeout(Duration::from_secs(5), received.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(request.image, expected, "Retry lost the pin image");
    tool(&root, &["key", "Escape"]);
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(
        child.0.try_wait().unwrap().is_none(),
        "Pin exited before clipboard handoff completed"
    );
    // Send the same WM_DELETE_WINDOW client message as an X11 window manager.
    // xdotool windowclose destroys the window and cannot test close negotiation.
    {
        use x11rb::{
            connection::Connection as _,
            protocol::xproto::{ClientMessageEvent, ConnectionExt as _, EventMask},
        };
        let (connection, _) = x11rb::connect(None).unwrap();
        let protocols = connection
            .intern_atom(false, b"WM_PROTOCOLS")
            .unwrap()
            .reply()
            .unwrap()
            .atom;
        let delete = connection
            .intern_atom(false, b"WM_DELETE_WINDOW")
            .unwrap()
            .reply()
            .unwrap()
            .atom;
        let id = window.parse::<u32>().unwrap();
        connection
            .send_event(
                false,
                id,
                EventMask::NO_EVENT,
                ClientMessageEvent::new(32, id, protocols, [delete, x11rb::CURRENT_TIME, 0, 0, 0]),
            )
            .unwrap();
        connection.flush().unwrap();
    }
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(
        child.0.try_wait().unwrap().is_none(),
        "Native close interrupted clipboard handoff"
    );
    request.complete.send(true).unwrap();
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if let Some(status) = child.0.try_wait().unwrap() {
                assert!(status.success());
                break;
            }
            tokio::time::sleep(Duration::from_millis(30)).await;
        }
    })
    .await
    .unwrap();
    let copied = Command::new(root.join("usr/bin/xclip"))
        .args(["-selection", "clipboard", "-t", "image/png", "-o"])
        .output()
        .unwrap();
    assert!(copied.status.success(), "{:?}", copied.stderr);
    assert_eq!(
        image::load_from_memory(&copied.stdout).unwrap().to_rgba8(),
        expected
    );
}
