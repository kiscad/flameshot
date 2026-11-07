// SPDX-License-Identifier: GPL-3.0-or-later
use std::{
    collections::HashMap,
    os::unix::fs::MetadataExt,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU32, Ordering},
    },
    time::Duration,
};
use zbus::{
    Connection,
    message::Header,
    zvariant::{OwnedFd, OwnedObjectPath, OwnedValue},
};

struct Portal {
    status: Arc<AtomicU32>,
    closed: Arc<AtomicU32>,
    inode: u64,
    device: u64,
    images: Arc<Mutex<Vec<std::path::PathBuf>>>,
}
struct Request(Arc<AtomicU32>);
#[zbus::interface(name = "org.freedesktop.portal.Request")]
impl Request {
    fn close(&self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}
#[zbus::interface(name = "org.freedesktop.portal.OpenURI")]
impl Portal {
    async fn open_file(
        &self,
        parent: &str,
        fd: OwnedFd,
        options: HashMap<String, OwnedValue>,
        #[zbus(header)] header: Header<'_>,
        #[zbus(connection)] connection: &Connection,
    ) -> zbus::fdo::Result<OwnedObjectPath> {
        assert!(parent.is_empty());
        let file = std::fs::File::from(std::os::fd::OwnedFd::from(fd));
        let metadata = file.metadata().unwrap();
        let ask = bool::try_from(options.get("ask").unwrap()).unwrap();
        assert_eq!(
            bool::try_from(options.get("writable").unwrap()).unwrap(),
            ask
        );
        if ask {
            use std::os::fd::AsRawFd;
            let path = std::fs::read_link(format!("/proc/self/fd/{}", file.as_raw_fd())).unwrap();
            let decoded = image::ImageReader::new(std::io::BufReader::new(&file))
                .with_guessed_format()
                .unwrap()
                .decode()
                .unwrap();
            assert_eq!((decoded.width(), decoded.height()), (80, 60));
            self.images.lock().unwrap().push(path);
        } else {
            assert!(metadata.is_dir());
            assert_eq!((metadata.dev(), metadata.ino()), (self.device, self.inode));
        }
        let sender = header
            .sender()
            .unwrap()
            .as_str()
            .trim_start_matches(':')
            .replace('.', "_");
        let token = <&str>::try_from(options.get("handle_token").unwrap()).unwrap();
        let status = self.status.load(Ordering::SeqCst);
        let suffix = if status == 4 { "_unexpected" } else { "" };
        let path: OwnedObjectPath =
            format!("/org/freedesktop/portal/desktop/request/{sender}/{token}{suffix}")
                .try_into()
                .unwrap();
        connection
            .object_server()
            .at(path.clone(), Request(self.closed.clone()))
            .await
            .unwrap();
        if status != 3 && status != 4 {
            connection
                .emit_signal(
                    header.sender().map(|name| name.as_str()),
                    path.clone(),
                    "org.freedesktop.portal.Request",
                    "Response",
                    &(status, HashMap::<String, OwnedValue>::new()),
                )
                .await
                .unwrap();
        }
        Ok(path)
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires private D-Bus"]
async fn directory_portal_fd_lifecycle_and_sandbox_cli_dispatch() {
    assert_eq!(
        std::env::var("FLAMESHOT_TEST_PRIVATE_BUS").as_deref(),
        Ok("1")
    );
    let root = tempfile::tempdir().unwrap();
    let directory = root.path().join("截图 folder $(literal)");
    std::fs::create_dir(&directory).unwrap();
    let metadata = directory.metadata().unwrap();
    let status = Arc::new(AtomicU32::new(0));
    let closed = Arc::new(AtomicU32::new(0));
    let images = Arc::new(Mutex::new(Vec::new()));
    let _server = zbus::connection::Builder::session()
        .unwrap()
        .name("org.freedesktop.portal.Desktop")
        .unwrap()
        .serve_at(
            "/org/freedesktop/portal/desktop",
            Portal {
                status: status.clone(),
                closed: closed.clone(),
                inode: metadata.ino(),
                device: metadata.dev(),
                images: images.clone(),
            },
        )
        .unwrap()
        .build()
        .await
        .unwrap();
    let client = Connection::session().await.unwrap();
    use flameshot_platform::open_portal::open_directory;
    let timeout = Duration::from_secs(2);
    assert!(open_directory(&client, &directory, timeout).await.unwrap());
    status.store(1, Ordering::SeqCst);
    assert!(!open_directory(&client, &directory, timeout).await.unwrap());
    for (code, message) in [(2, "rejected"), (5, "Invalid")] {
        status.store(code, Ordering::SeqCst);
        assert!(
            open_directory(&client, &directory, timeout)
                .await
                .unwrap_err()
                .to_string()
                .contains(message)
        );
    }
    status.store(3, Ordering::SeqCst);
    assert!(
        open_directory(&client, &directory, Duration::from_millis(100))
            .await
            .unwrap_err()
            .to_string()
            .contains("timed out")
    );
    assert_eq!(closed.load(Ordering::SeqCst), 1);
    status.store(4, Ordering::SeqCst);
    assert!(
        open_directory(&client, &directory, timeout)
            .await
            .unwrap_err()
            .to_string()
            .contains("handle token")
    );
    assert_eq!(closed.load(Ordering::SeqCst), 2);
    let config = root.path().join("flameshot");
    std::fs::create_dir(&config).unwrap();
    std::fs::write(
        config.join("flameshot.ini"),
        format!(
            "[General]\ncheckForUpdates=false\nsavePath={}\n",
            directory.display()
        ),
    )
    .unwrap();
    for sandbox in ["FLATPAK_ID", "SNAP"] {
        for code in [0, 1, 2] {
            status.store(code, Ordering::SeqCst);
            let mut command = std::process::Command::new(env!("CARGO_BIN_EXE_flameshot"));
            command
                .arg("internal-open-save-path")
                .env("XDG_CONFIG_HOME", root.path())
                .env_remove("FLATPAK_ID")
                .env_remove("SNAP")
                .env(sandbox, "isolated-test")
                .env("PATH", "/nonexistent/flameshot-test-tools")
                .env_remove("DISPLAY")
                .env_remove("WAYLAND_DISPLAY");
            let output = tokio::task::spawn_blocking(move || command.output().unwrap())
                .await
                .unwrap();
            assert_eq!(
                output.status.code(),
                Some(if code == 2 { 1 } else { 0 }),
                "{:?}",
                output.stderr
            );
            assert!(output.stdout.is_empty());
            if code == 2 {
                assert!(String::from_utf8_lossy(&output.stderr).contains("rejected"));
            }
            if code == 1 {
                assert!(output.stderr.is_empty(), "Cancellation should be quiet");
            }
        }
    }
    if let Ok(tools) = std::env::var("FLAMESHOT_TEST_PRIVATE_DISPLAY_TOOLS") {
        exercise_open_with(root.path(), &tools, &status, &images).await;
    }
    let file = root.path().join("not-a-directory");
    std::fs::write(&file, "data").unwrap();
    assert!(
        open_directory(&client, &file, timeout)
            .await
            .unwrap_err()
            .to_string()
            .contains("not a directory")
    );
    assert!(
        open_directory(&client, &root.path().join("missing"), timeout)
            .await
            .is_err()
    );
}

struct ChildGuard(std::process::Child);
impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
async fn exercise_open_with(
    root: &std::path::Path,
    tools: &str,
    status: &AtomicU32,
    images: &Mutex<Vec<std::path::PathBuf>>,
) {
    let xdotool = std::path::Path::new(tools).join("usr/bin/xdotool");
    let tool = |args: &[&str]| {
        std::process::Command::new(&xdotool)
            .args(args)
            .output()
            .unwrap()
    };
    for (code, keep) in [(0, false), (1, false), (0, true), (2, false)] {
        status.store(code, Ordering::SeqCst);
        images.lock().unwrap().clear();
        std::fs::write(root.join("flameshot/flameshot.ini"), format!("[General]\ncheckForUpdates=false\nshowStartupLaunchMessage=false\nshowDesktopNotification=false\nuseX11LegacyScreenshot=true\nkeepOpenAppLauncher={keep}\n")).unwrap();
        let mut child = ChildGuard(
            std::process::Command::new(env!("CARGO_BIN_EXE_flameshot"))
                .args(["gui", "--region", "80x60+10+20", "--raw"])
                .env("FLATPAK_ID", "isolated-test")
                .env("XDG_CONFIG_HOME", root)
                .env("XDG_CACHE_HOME", root)
                .env("TMPDIR", root)
                .env("LC_ALL", "C")
                .env("PATH", "/nonexistent/flameshot-test-tools")
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn()
                .unwrap(),
        );
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        let window = loop {
            let output = tool(&["search", "--onlyvisible", "--name", "^Flameshot$"]);
            if output.status.success() {
                break String::from_utf8(output.stdout)
                    .unwrap()
                    .lines()
                    .last()
                    .unwrap()
                    .to_owned();
            }
            assert!(child.0.try_wait().unwrap().is_none(), "Editor exited early");
            assert!(std::time::Instant::now() < deadline, "Missing editor");
            tokio::time::sleep(Duration::from_millis(50)).await;
        };
        assert!(tool(&["windowfocus", "--sync", &window]).status.success());
        tokio::time::sleep(Duration::from_millis(300)).await;
        assert!(tool(&["key", "ctrl+o"]).status.success());
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        let path = loop {
            if let Some(path) = images.lock().unwrap().first().cloned() {
                break path;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "Open With did not call the app chooser"
            );
            tokio::time::sleep(Duration::from_millis(50)).await;
        };
        tokio::time::sleep(Duration::from_millis(500)).await;
        if code != 0 || keep {
            assert!(
                child.0.try_wait().unwrap().is_none(),
                "Cancellation/keep-open/error discarded editor"
            );
            if keep || code == 2 {
                assert!(tool(&["key", "Escape"]).status.success());
                tokio::time::sleep(Duration::from_millis(250)).await;
            }
            if code != 0 {
                assert!(tool(&["key", "Return"]).status.success());
            }
        }
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        loop {
            if let Some(exit) = child.0.try_wait().unwrap() {
                assert!(exit.success(), "{exit}");
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "Editor failed to complete"
            );
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        if code == 0 {
            assert!(
                path.exists(),
                "Accepted screenshot disappeared before consumer could open it"
            );
            std::fs::remove_file(path).unwrap();
        } else {
            assert!(!path.exists(), "Unaccepted screenshot was not cleaned up");
        }
    }
}
