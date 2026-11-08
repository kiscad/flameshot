// SPDX-License-Identifier: GPL-3.0-or-later
use std::{collections::HashMap, process::Command};
use zbus::{
    Connection,
    message::Header,
    zvariant::{OwnedObjectPath, OwnedValue, Value},
};

struct Portal(String, std::sync::Arc<std::sync::atomic::AtomicUsize>);
#[zbus::interface(name = "org.freedesktop.portal.Screenshot")]
impl Portal {
    async fn screenshot(
        &self,
        _parent: &str,
        options: HashMap<String, OwnedValue>,
        #[zbus(header)] header: Header<'_>,
        #[zbus(connection)] connection: &Connection,
    ) -> OwnedObjectPath {
        self.1.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let sender = header
            .sender()
            .unwrap()
            .as_str()
            .trim_start_matches(':')
            .replace('.', "_");
        let token = <&str>::try_from(options.get("handle_token").unwrap()).unwrap();
        let path: OwnedObjectPath =
            format!("/org/freedesktop/portal/desktop/request/{sender}/{token}")
                .try_into()
                .unwrap();
        connection
            .emit_signal(
                header.sender().map(|name| name.as_str()),
                path.clone(),
                "org.freedesktop.portal.Request",
                "Response",
                &(
                    0_u32,
                    HashMap::from([("uri", Value::from(self.0.as_str()))]),
                ),
            )
            .await
            .unwrap();
        path
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires private D-Bus"]
async fn full_outputs_only_png_on_stdout_and_saves_without_overwrite() {
    assert_eq!(
        std::env::var("FLAMESHOT_TEST_PRIVATE_BUS").as_deref(),
        Ok("1")
    );
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source.png");
    let expected = image::RgbaImage::from_fn(9, 7, |x, y| {
        image::Rgba([x as u8 * 20, y as u8 * 30, 150, 255])
    });
    expected.save(&source).unwrap();
    let calls = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let _service = zbus::connection::Builder::session()
        .unwrap()
        .name("org.freedesktop.portal.Desktop")
        .unwrap()
        .serve_at(
            "/org/freedesktop/portal/desktop",
            Portal(format!("file://{}", source.display()), calls.clone()),
        )
        .unwrap()
        .build()
        .await
        .unwrap();
    let destination = temp.path().join("capture.png");
    for _ in 0..2 {
        let result = Command::new(env!("CARGO_BIN_EXE_flameshot"))
            .env("XDG_CONFIG_HOME", temp.path().join("config"))
            .args(["full", "--raw", "--path"])
            .arg(&destination)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(
            image::load_from_memory(&result.stdout).unwrap().to_rgba8(),
            expected
        );
    }
    for filename in ["capture.png", "capture_1.png"] {
        assert_eq!(
            image::open(temp.path().join(filename)).unwrap().to_rgba8(),
            expected
        );
    }
    assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 2);
    let config = temp.path().join("config/flameshot");
    std::fs::create_dir_all(&config).unwrap();
    std::fs::write(
        config.join("flameshot.ini"),
        "[General]\nuseX11LegacyScreenshot=true\n",
    )
    .unwrap();
    let native = Command::new(env!("CARGO_BIN_EXE_flameshot"))
        .env("XDG_CONFIG_HOME", temp.path().join("config"))
        .env("XDG_SESSION_TYPE", "x11")
        .env_remove("DISPLAY")
        .env_remove("WAYLAND_DISPLAY")
        .args(["full", "--raw"])
        .output()
        .unwrap();
    assert_eq!(native.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&native.stderr).contains("Cannot connect to X11"));
    assert_eq!(
        calls.load(std::sync::atomic::Ordering::SeqCst),
        2,
        "Forced X11 contacted the portal"
    );
    let wayland = Command::new(env!("CARGO_BIN_EXE_flameshot"))
        .env("XDG_CONFIG_HOME", temp.path().join("config"))
        .env("XDG_SESSION_TYPE", "wayland")
        .env_remove("DISPLAY")
        .args(["full", "--raw"])
        .output()
        .unwrap();
    assert!(wayland.status.success(), "{:?}", wayland);
    assert_eq!(
        image::load_from_memory(&wayland.stdout).unwrap().to_rgba8(),
        expected
    );
    assert_eq!(
        calls.load(std::sync::atomic::Ordering::SeqCst),
        3,
        "Wayland did not use the portal"
    );
    assert!(
        source.exists(),
        "Portal-provided file must not be blindly deleted"
    );
    for invalid in [
        "drawThickness=2147483648",
        "unknown=value",
        "savePath=",
        "[Shortcuts]\nTYPE_COPY=Ctrl+NoSuchKey",
    ] {
        let text = format!("[General]\nuseX11LegacyScreenshot=true\n{invalid}\n");
        let path = config.join("flameshot.ini");
        std::fs::write(&path, &text).unwrap();
        let result = Command::new(env!("CARGO_BIN_EXE_flameshot"))
            .env("XDG_CONFIG_HOME", temp.path().join("config"))
            .env("XDG_SESSION_TYPE", "x11")
            .env_remove("DISPLAY")
            .env_remove("WAYLAND_DISPLAY")
            .args(["full", "--raw"])
            .output()
            .unwrap();
        assert!(result.status.success(), "{result:?}");
        assert_eq!(
            image::load_from_memory(&result.stdout).unwrap().to_rgba8(),
            expected
        );
        let stderr = String::from_utf8_lossy(&result.stderr);
        assert_eq!(
            stderr
                .matches("The configuration contains an error.")
                .count(),
            1,
            "{stderr}"
        );
        assert_eq!(std::fs::read_to_string(path).unwrap(), text);
    }
    assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 7);
}
