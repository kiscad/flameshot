// SPDX-License-Identifier: GPL-3.0-or-later
use std::{
    collections::HashMap,
    process::{Child, Command, Stdio},
    time::Duration,
};
use zbus::{
    Connection, Proxy,
    zvariant::{OwnedObjectPath, OwnedValue, Structure, Value},
};
struct ChildGuard(Child);
impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
struct Watcher(tokio::sync::mpsc::UnboundedSender<String>);
#[zbus::interface(name = "org.kde.StatusNotifierWatcher")]
impl Watcher {
    fn register_status_notifier_item(&self, service: &str) {
        self.0.send(service.into()).unwrap();
    }
    #[zbus(property)]
    fn is_status_notifier_host_registered(&self) -> bool {
        true
    }
    #[zbus(property)]
    fn protocol_version(&self) -> i32 {
        0
    }
    #[zbus(property)]
    fn registered_status_notifier_items(&self) -> Vec<String> {
        Vec::new()
    }
}

struct Notifications(tokio::sync::mpsc::UnboundedSender<String>);
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
    ) -> u32 {
        let _ = self.0.send(body.into());
        1
    }
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires private D-Bus"]
async fn daemon_protocol_single_instance_tray_icon_and_quit() {
    assert_eq!(
        std::env::var("FLAMESHOT_TEST_PRIVATE_BUS").as_deref(),
        Ok("1")
    );
    let (sender, mut registered) = tokio::sync::mpsc::unbounded_channel();
    let _watcher = zbus::connection::Builder::session()
        .unwrap()
        .name("org.kde.StatusNotifierWatcher")
        .unwrap()
        .serve_at("/StatusNotifierWatcher", Watcher(sender))
        .unwrap()
        .build()
        .await
        .unwrap();
    let (notify_sender, mut notifications) = tokio::sync::mpsc::unbounded_channel();
    let _notifications = zbus::connection::Builder::session()
        .unwrap()
        .name("org.freedesktop.Notifications")
        .unwrap()
        .serve_at(
            "/org/freedesktop/Notifications",
            Notifications(notify_sender),
        )
        .unwrap()
        .build()
        .await
        .unwrap();
    let temp = tempfile::tempdir().unwrap();
    std::fs::create_dir(temp.path().join("flameshot")).unwrap();
    std::fs::write(
        temp.path().join("flameshot/flameshot.ini"),
        "[General]\ncheckForUpdates=false\nshowDesktopNotification=false\n",
    )
    .unwrap();
    let mut daemon = ChildGuard(
        Command::new(env!("CARGO_BIN_EXE_flameshot"))
            .env("XDG_CONFIG_HOME", temp.path())
            .env("LANG", "C")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .spawn()
            .unwrap(),
    );
    let item_name = tokio::time::timeout(Duration::from_secs(10), registered.recv())
        .await
        .unwrap()
        .unwrap();
    let startup = tokio::time::timeout(Duration::from_secs(5), notifications.recv())
        .await
        .unwrap()
        .unwrap();
    assert!(startup.contains("Hello, I'm here!"));
    let connection = Connection::session().await.unwrap();
    let bus = zbus::fdo::DBusProxy::new(&connection).await.unwrap();
    let service = zbus::names::BusName::try_from("org.flameshot.Flameshot").unwrap();
    let owner = bus.get_name_owner(service.clone()).await.unwrap();
    let introspection = Proxy::new(
        &connection,
        "org.flameshot.Flameshot",
        "/",
        "org.freedesktop.DBus.Introspectable",
    )
    .await
    .unwrap();
    let xml: String = introspection.call("Introspect", &()).await.unwrap();
    for name in [
        "captureScreen",
        "attachScreenshotToClipboard",
        "attachTextToClipboard",
        "attachPin",
    ] {
        assert!(xml.contains(&format!("name=\"{name}\"")), "{xml}");
    }
    let proxy = Proxy::new(
        &connection,
        "org.flameshot.Flameshot",
        "/",
        "org.flameshot.Flameshot",
    )
    .await
    .unwrap();
    for method in ["attachScreenshotToClipboard", "attachPin"] {
        let error = proxy
            .call::<_, _, ()>(method, &(vec![0_u8, 1, 2],))
            .await
            .unwrap_err();
        assert!(
            matches!(error,zbus::Error::MethodError(ref name,_,_) if name.as_str()=="org.freedesktop.DBus.Error.InvalidArgs")
        );
    }
    let mut duplicate = ChildGuard(
        Command::new(env!("CARGO_BIN_EXE_flameshot"))
            .env("XDG_CONFIG_HOME", temp.path())
            .spawn()
            .unwrap(),
    );
    let status = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if let Some(status) = duplicate.0.try_wait().unwrap() {
                break status;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    })
    .await
    .unwrap();
    assert!(status.success());
    assert_eq!(bus.get_name_owner(service).await.unwrap(), owner);
    let item = Proxy::new(
        &connection,
        item_name.as_str(),
        "/StatusNotifierItem",
        "org.kde.StatusNotifierItem",
    )
    .await
    .unwrap();
    assert_eq!(
        item.get_property::<String>("Title").await.unwrap(),
        "Flameshot"
    );
    let icons: Vec<(i32, i32, Vec<u8>)> = item.get_property("IconPixmap").await.unwrap();
    assert!(icons.iter().any(|(w, h, data)| *w == 32
        && *h == 32
        && data.len() == 4096
        && data.iter().any(|v| *v != 0)));
    let menu_path: OwnedObjectPath = item.get_property("Menu").await.unwrap();
    let menu = Proxy::new(
        &connection,
        item_name.as_str(),
        menu_path.as_str(),
        "com.canonical.dbusmenu",
    )
    .await
    .unwrap();
    type Layout = (i32, HashMap<String, OwnedValue>, Vec<OwnedValue>);
    let (_, (_, _, children)): (u32, Layout) = menu
        .call("GetLayout", &(0_i32, -1_i32, vec!["label"]))
        .await
        .unwrap();
    let mut labels = Vec::new();
    let mut quit = None;
    for child in children {
        let structure = Structure::try_from(child).unwrap();
        let mut fields = structure.into_fields();
        let id: i32 = fields.remove(0).downcast().unwrap();
        let properties: HashMap<String, OwnedValue> = fields.remove(0).downcast().unwrap();
        if let Some(label) = properties.get("label") {
            let label = <&str>::try_from(label).unwrap().to_string();
            if label == "Quit" {
                quit = Some(id);
            }
            labels.push(label);
        }
    }
    assert!(labels.contains(&"Take Screenshot".into()));
    assert!(labels.contains(&"Configuration".into()));
    assert!(labels.contains(&"Open Launcher".into()));
    menu.call::<_, _, ()>(
        "Event",
        &(quit.unwrap(), "clicked", Value::from(0_i32), 0_u32),
    )
    .await
    .unwrap();
    let status = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if let Some(status) = daemon.0.try_wait().unwrap() {
                break status;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    })
    .await
    .unwrap();
    assert!(status.success());
    assert!(
        notifications.try_recv().is_err(),
        "Duplicate daemon repeated startup notification"
    );
    std::fs::write(
        temp.path().join("flameshot/flameshot.ini"),
        "[General]\ncheckForUpdates=false\nshowStartupLaunchMessage=false\n",
    )
    .unwrap();
    let _quiet = ChildGuard(
        Command::new(env!("CARGO_BIN_EXE_flameshot"))
            .env("XDG_CONFIG_HOME", temp.path())
            .env("LANG", "C")
            .env("FLATPAK_ID", "org.flameshot.Flameshot")
            .spawn()
            .unwrap(),
    );
    let sandbox_item = tokio::time::timeout(Duration::from_secs(5), registered.recv())
        .await
        .unwrap()
        .unwrap();
    assert!(
        sandbox_item.starts_with(':'),
        "Sandbox tray should use its unique bus name"
    );
    assert!(
        tokio::time::timeout(Duration::from_millis(200), notifications.recv())
            .await
            .is_err(),
        "Disabled startup message was delivered"
    );
}
