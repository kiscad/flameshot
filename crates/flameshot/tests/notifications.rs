// SPDX-License-Identifier: GPL-3.0-or-later
use std::{collections::HashMap, process::Command};
use zbus::{
    Connection,
    message::Header,
    zvariant::{OwnedObjectPath, OwnedValue, Value},
};
struct Notifications(
    std::sync::mpsc::Sender<String>,
    std::sync::Arc<std::sync::atomic::AtomicBool>,
);
#[zbus::interface(name = "org.freedesktop.Notifications")]
impl Notifications {
    #[allow(clippy::too_many_arguments)]
    fn notify(
        &self,
        _app: &str,
        _replaces: u32,
        _icon: &str,
        _title: &str,
        body: &str,
        _actions: Vec<String>,
        _hints: HashMap<String, OwnedValue>,
        _timeout: i32,
    ) -> zbus::fdo::Result<u32> {
        self.0.send(body.into()).unwrap();
        if self.1.load(std::sync::atomic::Ordering::SeqCst) {
            Err(zbus::fdo::Error::Failed(
                "Synthetic notification failure".into(),
            ))
        } else {
            Ok(1)
        }
    }
}
struct CancelPortal;
#[zbus::interface(name = "org.freedesktop.portal.Screenshot")]
impl CancelPortal {
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
        connection
            .emit_signal(
                header.sender().map(|name| name.as_str()),
                path.clone(),
                "org.freedesktop.portal.Request",
                "Response",
                &(1_u32, HashMap::<String, Value>::new()),
            )
            .await
            .unwrap();
        path
    }
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires private D-Bus"]
async fn cancellation_notification_respects_both_preferences_and_keeps_exit_status() {
    assert_eq!(
        std::env::var("FLAMESHOT_TEST_PRIVATE_BUS").as_deref(),
        Ok("1")
    );
    let (sender, received) = std::sync::mpsc::channel();
    let reject = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let _notifications = zbus::connection::Builder::session()
        .unwrap()
        .name("org.freedesktop.Notifications")
        .unwrap()
        .serve_at(
            "/org/freedesktop/Notifications",
            Notifications(sender, reject.clone()),
        )
        .unwrap()
        .build()
        .await
        .unwrap();
    let _portal = zbus::connection::Builder::session()
        .unwrap()
        .name("org.freedesktop.portal.Desktop")
        .unwrap()
        .serve_at("/org/freedesktop/portal/desktop", CancelPortal)
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
            assert_eq!(received.try_recv().unwrap(), "Screenshot aborted.");
        } else {
            assert!(received.try_recv().is_err());
        }
    }
    reject.store(true, std::sync::atomic::Ordering::SeqCst);
    std::fs::write(
        config.join("flameshot.ini"),
        "[General]\nshowDesktopNotification=true\nshowAbortNotification=true\n",
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
    assert_eq!(received.try_recv().unwrap(), "Screenshot aborted.");
}
