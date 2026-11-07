// SPDX-License-Identifier: GPL-3.0-or-later
use std::{collections::HashMap, process::Command};
use zbus::{
    Connection,
    message::Header,
    zvariant::{OwnedObjectPath, OwnedValue, Value},
};
struct Notifications(
    std::sync::mpsc::Sender<(String, bool)>,
    std::sync::Arc<std::sync::atomic::AtomicBool>,
    std::sync::Arc<std::sync::atomic::AtomicBool>,
    std::sync::Arc<tokio::sync::Semaphore>,
);
#[zbus::interface(name = "org.freedesktop.Notifications")]
impl Notifications {
    #[allow(clippy::too_many_arguments)]
    async fn notify(
        &self,
        _app: &str,
        _replaces: u32,
        _icon: &str,
        _title: &str,
        body: &str,
        _actions: Vec<String>,
        _hints: HashMap<String, OwnedValue>,
        _timeout: i32,
        #[zbus(header)] header: Header<'_>,
    ) -> zbus::fdo::Result<u32> {
        self.0
            .send((
                body.into(),
                header
                    .primary()
                    .flags()
                    .contains(zbus::message::Flags::NoReplyExpected),
            ))
            .unwrap();
        if self.2.load(std::sync::atomic::Ordering::SeqCst) {
            self.3.acquire().await.unwrap().forget();
        }
        if self.1.load(std::sync::atomic::Ordering::SeqCst) {
            Err(zbus::fdo::Error::Failed(
                "Synthetic notification failure".into(),
            ))
        } else {
            Ok(1)
        }
    }
}
struct CapturePortal(std::sync::Arc<std::sync::Mutex<Option<String>>>);
#[zbus::interface(name = "org.freedesktop.portal.Screenshot")]
impl CapturePortal {
    async fn screenshot(
        &self,
        _parent: &str,
        options: HashMap<String, OwnedValue>,
        #[zbus(header)] header: Header<'_>,
        #[zbus(connection)] connection: &Connection,
    ) -> OwnedObjectPath {
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
        let uri = self.0.lock().unwrap().clone();
        let (status, results) = if let Some(uri) = uri {
            (0_u32, HashMap::from([("uri", Value::from(uri))]))
        } else {
            (1_u32, HashMap::new())
        };
        connection
            .emit_signal(
                header.sender().map(|name| name.as_str()),
                path.clone(),
                "org.freedesktop.portal.Request",
                "Response",
                &(status, results),
            )
            .await
            .unwrap();
        path
    }
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires private D-Bus"]
async fn capture_notifications_are_nonblocking_and_preserve_preferences_and_outputs() {
    assert_eq!(
        std::env::var("FLAMESHOT_TEST_PRIVATE_BUS").as_deref(),
        Ok("1")
    );
    let (sender, received) = std::sync::mpsc::channel();
    let reject = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let hold = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let gate = std::sync::Arc::new(tokio::sync::Semaphore::new(0));
    let source_uri = std::sync::Arc::new(std::sync::Mutex::new(None));
    let _notifications = zbus::connection::Builder::session()
        .unwrap()
        .name("org.freedesktop.Notifications")
        .unwrap()
        .serve_at(
            "/org/freedesktop/Notifications",
            Notifications(sender, reject.clone(), hold.clone(), gate.clone()),
        )
        .unwrap()
        .build()
        .await
        .unwrap();
    let _portal = zbus::connection::Builder::session()
        .unwrap()
        .name("org.freedesktop.portal.Desktop")
        .unwrap()
        .serve_at(
            "/org/freedesktop/portal/desktop",
            CapturePortal(source_uri.clone()),
        )
        .unwrap()
        .build()
        .await
        .unwrap();
    let temp = tempfile::tempdir().unwrap();
    let config = temp.path().join("flameshot");
    std::fs::create_dir(&config).unwrap();
    for (notifications, abort) in [(true, true), (true, false), (false, true), (false, false)] {
        std::fs::write(config.join("flameshot.ini"),format!("[General]\nshowDesktopNotification={notifications}\nshowAbortNotification={abort}\ncheckForUpdates=false\n")).unwrap();
        let result = Command::new(env!("CARGO_BIN_EXE_flameshot"))
            .env("XDG_CONFIG_HOME", temp.path())
            .env("XDG_SESSION_TYPE", "wayland")
            .env_remove("DISPLAY")
            .args(["full", "--raw"])
            .output()
            .unwrap();
        assert_eq!(result.status.code(), Some(2));
        assert!(result.stdout.is_empty());
        if notifications && abort {
            assert_eq!(
                received
                    .recv_timeout(std::time::Duration::from_secs(2))
                    .unwrap(),
                ("Screenshot aborted.".into(), true)
            );
        } else {
            assert!(received.try_recv().is_err());
        }
    }
    reject.store(true, std::sync::atomic::Ordering::SeqCst);
    std::fs::write(
        config.join("flameshot.ini"),
        "[General]\nshowDesktopNotification=true\nshowAbortNotification=true\ncheckForUpdates=false\n",
    )
    .unwrap();
    let failed_notification = Command::new(env!("CARGO_BIN_EXE_flameshot"))
        .env("XDG_CONFIG_HOME", temp.path())
        .env("XDG_SESSION_TYPE", "wayland")
        .env_remove("DISPLAY")
        .args(["full", "--raw"])
        .output()
        .unwrap();
    assert_eq!(failed_notification.status.code(), Some(2));
    assert!(failed_notification.stdout.is_empty());
    assert_eq!(
        received
            .recv_timeout(std::time::Duration::from_secs(2))
            .unwrap(),
        ("Screenshot aborted.".into(), true)
    );
    reject.store(false, std::sync::atomic::Ordering::SeqCst);
    hold.store(true, std::sync::atomic::Ordering::SeqCst);
    let source = temp.path().join("source.png");
    let pixels = image::RgbaImage::from_pixel(9, 7, image::Rgba([20, 50, 190, 255]));
    pixels.save(&source).unwrap();
    for success in [false, true] {
        *source_uri.lock().unwrap() = success.then(|| format!("file://{}", source.display()));
        let destination = temp.path().join("saved.png");
        let mut command = Command::new(env!("CARGO_BIN_EXE_flameshot"));
        command
            .env("XDG_CONFIG_HOME", temp.path())
            .env("XDG_CACHE_HOME", temp.path())
            .env("XDG_DATA_HOME", temp.path())
            .env("XDG_SESSION_TYPE", "wayland")
            .env_remove("DISPLAY")
            .args(["full", "--raw"])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());
        if success {
            command.arg("--path").arg(&destination);
        }
        let mut child = command.spawn().unwrap();
        let finished = tokio::time::timeout(std::time::Duration::from_secs(1), async {
            loop {
                if child.try_wait().unwrap().is_some() {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
        })
        .await;
        if finished.is_err() {
            child.kill().unwrap();
        }
        let result = child.wait_with_output().unwrap();
        assert!(
            finished.is_ok(),
            "CLI waited for notification handler completion"
        );
        assert_eq!(result.status.code(), Some(if success { 0 } else { 2 }));
        let body = if success {
            "Capture saved"
        } else {
            "Screenshot aborted."
        };
        assert_eq!(
            received
                .recv_timeout(std::time::Duration::from_secs(2))
                .unwrap(),
            (body.into(), true)
        );
        if success {
            assert_eq!(
                image::load_from_memory(&result.stdout).unwrap().to_rgba8(),
                pixels
            );
            assert_eq!(image::open(destination).unwrap().to_rgba8(), pixels);
        } else {
            assert!(result.stdout.is_empty());
        }
        gate.add_permits(1);
    }
}
