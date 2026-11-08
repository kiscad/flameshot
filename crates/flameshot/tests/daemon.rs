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
#[derive(Debug, PartialEq, Eq)]
struct MethodContract {
    arguments: Vec<std::collections::BTreeMap<String, String>>,
    annotations: std::collections::BTreeMap<String, String>,
}
fn interface_contract(xml: &str) -> std::collections::BTreeMap<String, MethodContract> {
    let document = roxmltree::Document::parse_with_options(
        xml,
        roxmltree::ParsingOptions {
            allow_dtd: true,
            ..Default::default()
        },
    )
    .unwrap();
    let interface = document
        .descendants()
        .find(|node| {
            node.has_tag_name("interface")
                && node.attribute("name") == Some("org.flameshot.Flameshot")
        })
        .unwrap();
    let mut methods = std::collections::BTreeMap::new();
    for method in interface.children().filter(|node| node.is_element()) {
        assert!(
            method.has_tag_name("method"),
            "Unexpected interface member: {xml}"
        );
        let mut contract = MethodContract {
            arguments: Vec::new(),
            annotations: Default::default(),
        };
        for child in method.children().filter(|node| node.is_element()) {
            match child.tag_name().name() {
                "arg" => contract.arguments.push(
                    child
                        .attributes()
                        .map(|attr| (attr.name().to_owned(), attr.value().to_owned()))
                        .collect(),
                ),
                "annotation" => assert!(
                    contract
                        .annotations
                        .insert(
                            child.attribute("name").unwrap().into(),
                            child.attribute("value").unwrap().into()
                        )
                        .is_none(),
                    "Duplicate annotation: {xml}"
                ),
                _ => panic!("Unexpected method metadata: {xml}"),
            }
        }
        assert!(
            methods
                .insert(method.attribute("name").unwrap().to_owned(), contract)
                .is_none(),
            "Duplicate method: {xml}"
        );
    }
    methods
}

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

struct Notifications {
    messages: tokio::sync::mpsc::UnboundedSender<String>,
    delivery: Option<(
        std::sync::Arc<tokio::sync::Semaphore>,
        tokio::sync::mpsc::UnboundedSender<bool>,
    )>,
}
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
        #[zbus(header)] header: zbus::message::Header<'_>,
    ) -> zbus::fdo::Result<u32> {
        assert!(
            header
                .primary()
                .flags()
                .contains(zbus::message::Flags::NoReplyExpected),
            "Best-effort notification requested a reply"
        );
        let _ = self.messages.send(body.into());
        if body.starts_with("clipboard-order")
            && let Some((gate, observations)) = &self.delivery
        {
            observations
                .send(
                    header
                        .primary()
                        .flags()
                        .contains(zbus::message::Flags::NoReplyExpected),
                )
                .unwrap();
            gate.acquire().await.unwrap().forget();
            if body.ends_with("reject") {
                return Err(zbus::fdo::Error::Failed(
                    "Synthetic notification failure".into(),
                ));
            }
        }
        Ok(1)
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
    let (notification_flag_sender, mut notification_flags) = tokio::sync::mpsc::unbounded_channel();
    let notification_gate = std::sync::Arc::new(tokio::sync::Semaphore::new(0));
    let delivery_probe = std::env::var_os("FLAMESHOT_TEST_PRIVATE_DISPLAY_TOOLS")
        .map(|_| (notification_gate.clone(), notification_flag_sender));
    let _notifications = zbus::connection::Builder::session()
        .unwrap()
        .name("org.freedesktop.Notifications")
        .unwrap()
        .serve_at(
            "/org/freedesktop/Notifications",
            Notifications {
                messages: notify_sender,
                delivery: delivery_probe,
            },
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
    // Keep tray dispatch isolated from the user's file manager and MIME setup.
    let launcher_dir = temp.path().join("launchers");
    std::fs::create_dir(&launcher_dir).unwrap();
    let receipt = temp.path().join("opened-directory");
    let gio = launcher_dir.join("gio");
    std::fs::write(&gio, format!(
        "#!/usr/bin/python3\nimport pathlib,sys\np=pathlib.Path({:?})\np.with_suffix('.tmp').write_text('\\n'.join(sys.argv[1:]))\np.with_suffix('.tmp').replace(p)\n", receipt.to_str().unwrap()
    )).unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&gio, std::fs::Permissions::from_mode(0o700)).unwrap();
    let mut daemon = ChildGuard(
        Command::new(env!("CARGO_BIN_EXE_flameshot"))
            .env("PATH", &launcher_dir)
            .env("XDG_CONFIG_HOME", temp.path())
            .env("XDG_CACHE_HOME", temp.path())
            .env("LANG", "C")
            .env("LC_ALL", "C")
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
    assert_eq!(
        interface_contract(&xml),
        interface_contract(include_str!("fixtures/qt-dbus-interface.xml")),
        "Public method signatures or annotations differ from the Qt reference",
    );
    assert_eq!(
        interface_contract(include_str!(
            "../../../data/dbus/org.flameshot.Flameshot.xml"
        )),
        interface_contract(&xml),
        "Installed interface description differs from runtime introspection",
    );
    let properties = Proxy::new(
        &connection,
        "org.flameshot.Flameshot",
        "/",
        "org.freedesktop.DBus.Properties",
    )
    .await
    .unwrap();
    let empty: HashMap<String, OwnedValue> = properties
        .call("GetAll", &("org.flameshot.Flameshot",))
        .await
        .unwrap();
    assert!(empty.is_empty());
    let missing = properties
        .call::<_, _, OwnedValue>("Get", &("org.flameshot.Flameshot", "missing"))
        .await
        .unwrap_err();
    assert!(
        matches!(missing, zbus::Error::MethodError(ref name,_,_) if name.as_str()=="org.freedesktop.DBus.Error.UnknownProperty")
    );
    let missing = properties
        .call::<_, _, ()>(
            "Set",
            &("org.flameshot.Flameshot", "missing", Value::from(1_u32)),
        )
        .await
        .unwrap_err();
    assert!(
        matches!(missing, zbus::Error::MethodError(ref name,_,_) if name.as_str()=="org.freedesktop.DBus.Error.UnknownProperty")
    );
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
            .env("XDG_CACHE_HOME", temp.path())
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
    let _: bool = menu.call("AboutToShow", &(0_i32,)).await.unwrap();
    type Layout = (i32, HashMap<String, OwnedValue>, Vec<OwnedValue>);
    let (_, (_, _, children)): (u32, Layout) = menu
        .call("GetLayout", &(0_i32, -1_i32, vec!["label"]))
        .await
        .unwrap();
    let mut labels = Vec::new();
    let mut quit = None;
    let mut open_save = None;
    let mut monitor_items = Vec::new();
    for child in children {
        let structure = Structure::try_from(child).unwrap();
        let mut fields = structure.into_fields();
        let id: i32 = fields.remove(0).downcast().unwrap();
        let properties: HashMap<String, OwnedValue> = fields.remove(0).downcast().unwrap();
        if let Some(label) = properties.get("label") {
            let label = <&str>::try_from(label).unwrap().to_string();
            if label == "Select Screen" {
                let entries: Vec<OwnedValue> = fields.remove(0).downcast().unwrap();
                for entry in entries {
                    let mut fields = Structure::try_from(entry).unwrap().into_fields();
                    let id: i32 = fields.remove(0).downcast().unwrap();
                    let properties: HashMap<String, OwnedValue> =
                        fields.remove(0).downcast().unwrap();
                    monitor_items.push((
                        id,
                        <&str>::try_from(properties.get("label").unwrap())
                            .unwrap()
                            .to_owned(),
                    ));
                }
            }
            if label == "_Open Save Path" {
                open_save = Some(id);
            }
            if label == "_Quit" {
                quit = Some(id);
            }
            labels.push(label);
        }
    }
    assert!(labels.contains(&"_Take Screenshot".into()));
    assert!(labels.contains(&"_Configuration".into()));
    assert!(labels.contains(&"_About".into()));
    assert!(labels.contains(&"_Open Save Path".into()));
    assert!(labels.contains(&"_Open Launcher".into()));
    if let Ok(tools) = std::env::var("FLAMESHOT_TEST_PRIVATE_DISPLAY_TOOLS") {
        let xclip = std::path::Path::new(&tools).join("usr/bin/xclip");
        let read_clipboard = || {
            let output = Command::new(&xclip)
                .args(["-selection", "clipboard", "-t", "UTF8_STRING", "-o"])
                .output()
                .unwrap();
            assert!(output.status.success(), "{:?}", output.stderr);
            output.stdout
        };
        let config = temp.path().join("flameshot/flameshot.ini");
        std::fs::write(
            &config,
            "[General]\ncheckForUpdates=false\nshowDesktopNotification=true\n",
        )
        .unwrap();
        for notification in ["clipboard-order-held", "clipboard-order-reject"] {
            proxy
                .call::<_, _, ()>("attachTextToClipboard", &("before notification", ""))
                .await
                .unwrap();
            tokio::time::timeout(
                Duration::from_secs(1),
                proxy.call::<_, _, ()>(
                    "attachTextToClipboard",
                    &("after notification", notification),
                ),
            )
            .await
            .expect("Copy waited for notification handling")
            .unwrap();
            assert!(
                notification_flags.recv().await.unwrap(),
                "Notification requested a reply"
            );
            assert_eq!(notifications.recv().await.unwrap(), notification);
            assert_eq!(read_clipboard(), b"after notification");
            notification_gate.add_permits(1);
        }
        let first = async {
            proxy
                .call::<_, _, ()>(
                    "attachTextToClipboard",
                    &("first pending", "clipboard-order-held"),
                )
                .await
                .unwrap();
        };
        let second = async {
            assert!(notification_flags.recv().await.unwrap());
            proxy
                .call::<_, _, ()>("attachTextToClipboard", &("second queued", ""))
                .await
                .unwrap();
        };
        tokio::time::timeout(Duration::from_secs(1), async {
            tokio::join!(first, second);
        })
        .await
        .unwrap();
        assert_eq!(notifications.recv().await.unwrap(), "clipboard-order-held");
        assert_eq!(
            read_clipboard(),
            b"second queued",
            "An older request overwrote newer text"
        );
        notification_gate.add_permits(1);
        let expected_image = image::RgbaImage::from_pixel(3, 2, image::Rgba([9, 80, 220, 255]));
        let wire_image = flameshot_core::qt_wire::encode_clipboard(&expected_image).unwrap();
        let first = async {
            proxy
                .call::<_, _, ()>(
                    "attachTextToClipboard",
                    &("older text before image", "clipboard-order-held"),
                )
                .await
                .unwrap();
        };
        let second = async {
            assert!(notification_flags.recv().await.unwrap());
            proxy
                .call::<_, _, ()>("attachScreenshotToClipboard", &(wire_image,))
                .await
                .unwrap();
        };
        tokio::time::timeout(Duration::from_secs(1), async {
            tokio::join!(first, second);
        })
        .await
        .unwrap();
        assert_eq!(notifications.recv().await.unwrap(), "clipboard-order-held");
        let copied_image = Command::new(&xclip)
            .args(["-selection", "clipboard", "-t", "image/png", "-o"])
            .output()
            .unwrap();
        assert!(copied_image.status.success(), "{:?}", copied_image.stderr);
        assert_eq!(
            image::load_from_memory(&copied_image.stdout)
                .unwrap()
                .to_rgba8(),
            expected_image
        );
        notification_gate.add_permits(1);
        // Q_NOREPLY clients must still trigger the clipboard side effect.
        proxy
            .call_noreply("attachTextToClipboard", &("legacy no-reply text", ""))
            .await
            .unwrap();
        tokio::time::timeout(Duration::from_secs(3), async {
            while read_clipboard() != b"legacy no-reply text" {
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await
        .unwrap();
        std::fs::write(
            &config,
            "[General]\ncheckForUpdates=false\nshowDesktopNotification=false\n",
        )
        .unwrap();
        proxy
            .call::<_, _, ()>(
                "attachTextToClipboard",
                &("disabled notification", "clipboard-order-disabled"),
            )
            .await
            .unwrap();
        assert_eq!(read_clipboard(), b"disabled notification");
        assert!(notification_flags.try_recv().is_err());
        assert!(notifications.try_recv().is_err());
        std::fs::write(&config, "[General]\ncheckForUpdates=false\nshowDesktopNotification=true\nautoCloseIdleDaemon=true\n").unwrap();
        let external = flameshot_platform::clipboard::copy(
            b"external owner".to_vec(),
            "text/plain;charset=utf-8",
            false,
        )
        .unwrap();
        tokio::time::timeout(
            Duration::from_secs(1),
            proxy.call::<_, _, ()>(
                "attachTextToClipboard",
                &("copied while notification pending", "clipboard-order-held"),
            ),
        )
        .await
        .expect("Copy waited for notification handling")
        .unwrap();
        assert!(notification_flags.recv().await.unwrap());
        assert_eq!(notifications.recv().await.unwrap(), "clipboard-order-held");
        tokio::time::sleep(Duration::from_millis(1400)).await;
        assert_eq!(read_clipboard(), b"copied while notification pending");
        assert!(daemon.0.try_wait().unwrap().is_none());
        notification_gate.add_permits(1);
        drop(external);
        std::fs::write(
            &config,
            "[General]\ncheckForUpdates=false\nshowDesktopNotification=false\n",
        )
        .unwrap();
        assert_eq!(monitor_items.len(), 2, "{monitor_items:?}");
        assert_eq!(monitor_items[1].1, "Monitor 2: right (512x768)");
        menu.call::<_, _, ()>(
            "Event",
            &(monitor_items[1].0, "clicked", Value::from(0_i32), 0_u32),
        )
        .await
        .unwrap();
        let xdotool = std::path::Path::new(&tools).join("usr/bin/xdotool");
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        let window = loop {
            let output = Command::new(&xdotool)
                .args(["search", "--onlyvisible", "--name", "^Flameshot$"])
                .output()
                .unwrap();
            if output.status.success() {
                break String::from_utf8(output.stdout)
                    .unwrap()
                    .lines()
                    .last()
                    .unwrap()
                    .to_owned();
            }
            assert!(
                std::time::Instant::now() < deadline,
                "Monitor action did not open an editor"
            );
            tokio::time::sleep(Duration::from_millis(50)).await;
        };
        assert!(
            Command::new(&xdotool)
                .args(["windowfocus", "--sync", &window])
                .status()
                .unwrap()
                .success()
        );
        tokio::time::sleep(Duration::from_millis(400)).await;
        assert!(
            Command::new(&xdotool)
                .args(["key", "ctrl+c"])
                .status()
                .unwrap()
                .success()
        );
        tokio::time::sleep(Duration::from_millis(400)).await;
        let copied = Command::new(std::path::Path::new(&tools).join("usr/bin/xclip"))
            .args(["-selection", "clipboard", "-t", "image/png", "-o"])
            .output()
            .unwrap();
        assert!(copied.status.success(), "{:?}", copied.stderr);
        let image = image::load_from_memory(&copied.stdout).unwrap();
        assert_eq!((image.width(), image.height()), (512, 768));
        assert_eq!(
            image.to_rgba8().get_pixel(256, 384).0,
            [255, 0, 0, 255],
            "Captured the wrong monitor"
        );
        assert!(
            Command::new("xrandr")
                .args(["--delmonitor", "right"])
                .status()
                .unwrap()
                .success()
        );
        let _: bool = menu.call("AboutToShow", &(0_i32,)).await.unwrap();
        let (_, (_, _, children)): (u32, Layout) = menu
            .call("GetLayout", &(0_i32, -1_i32, vec!["label"]))
            .await
            .unwrap();
        for child in children {
            let mut fields = Structure::try_from(child).unwrap().into_fields();
            let id: i32 = fields.remove(0).downcast().unwrap();
            let properties: HashMap<String, OwnedValue> = fields.remove(0).downcast().unwrap();
            if let Some(label) = properties.get("label") {
                let label = <&str>::try_from(label).unwrap();
                assert_ne!(
                    label, "Select Screen",
                    "Single remaining output retained monitor submenu"
                );
                if label == "_Quit" {
                    quit = Some(id);
                }
                if label == "_Open Save Path" {
                    open_save = Some(id);
                }
            }
        }
    }
    let saved = temp.path().join("截图 saved $(literal)");
    std::fs::create_dir(&saved).unwrap();
    std::fs::write(
        temp.path().join("flameshot/flameshot.ini"),
        format!(
            "[General]\ncheckForUpdates=false\nshowDesktopNotification=false\nsavePath={}\n",
            saved.display()
        ),
    )
    .unwrap();
    menu.call::<_, _, ()>(
        "Event",
        &(open_save.unwrap(), "clicked", Value::from(0_i32), 0_u32),
    )
    .await
    .unwrap();
    tokio::time::timeout(Duration::from_secs(5), async {
        while !receipt.exists() {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(
        std::fs::read_to_string(&receipt).unwrap(),
        format!("open\n{}", saved.display())
    );
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
        "[General]\ncheckForUpdates=false\nshowStartupLaunchMessage=false\nuiLanguage=zh_CN\n",
    )
    .unwrap();
    for (sandbox_key, sandbox_value) in [
        ("FLATPAK_ID", "org.flameshot.Flameshot"),
        ("SNAP", "/snap/flameshot/test"),
    ] {
        let _quiet = ChildGuard(
            Command::new(env!("CARGO_BIN_EXE_flameshot"))
                .env("XDG_CONFIG_HOME", temp.path())
                .env("XDG_CACHE_HOME", temp.path())
                .env("LANG", "C")
                .env("LC_ALL", "C")
                .env(sandbox_key, sandbox_value)
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
        let item = Proxy::new(
            &connection,
            sandbox_item.as_str(),
            "/StatusNotifierItem",
            "org.kde.StatusNotifierItem",
        )
        .await
        .unwrap();
        let path: OwnedObjectPath = item.get_property("Menu").await.unwrap();
        let menu = Proxy::new(
            &connection,
            sandbox_item.as_str(),
            path.as_str(),
            "com.canonical.dbusmenu",
        )
        .await
        .unwrap();
        let (_, (_, _, children)): (u32, Layout) = menu
            .call("GetLayout", &(0_i32, -1_i32, vec!["label"]))
            .await
            .unwrap();
        let labels: Vec<String> = children
            .into_iter()
            .filter_map(|child| {
                let mut fields = Structure::try_from(child).unwrap().into_fields();
                let properties: HashMap<String, OwnedValue> = fields.remove(1).downcast().unwrap();
                properties
                    .get("label")
                    .map(|value| <&str>::try_from(value).unwrap().to_owned())
            })
            .collect();
        for expected in [
            "进行截图(_T)",
            "打开启动器(_O)",
            "配置(_C)",
            "关于(_A)",
            "打开保存路径(_O)",
            "退出(_Q)",
        ] {
            assert!(labels.iter().any(|label| label == expected), "{labels:?}");
        }
        #[cfg(feature = "imgur")]
        assert!(labels.iter().any(|label| label == "最近的上传(_L)"));
        #[cfg(feature = "update-check")]
        assert!(labels.iter().any(|label| label == "检查更新"));

        assert!(
            tokio::time::timeout(Duration::from_millis(200), notifications.recv())
                .await
                .is_err(),
            "Disabled startup message was delivered"
        );
    }
    std::fs::write(
        temp.path().join("flameshot/flameshot.ini"),
        "[General]\ncheckForUpdates=false\nshowStartupLaunchMessage=true\nuiLanguage=zh_CN\n",
    )
    .unwrap();
    let _localized = ChildGuard(
        Command::new(env!("CARGO_BIN_EXE_flameshot"))
            .env("XDG_CONFIG_HOME", temp.path())
            .env("XDG_CACHE_HOME", temp.path())
            .env("LC_ALL", "C")
            .spawn()
            .unwrap(),
    );
    tokio::time::timeout(Duration::from_secs(5), registered.recv())
        .await
        .unwrap()
        .unwrap();
    let message = tokio::time::timeout(Duration::from_secs(5), notifications.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        message,
        "我已启动并在后台运行！点击托盘图标即可进行屏幕截图，右键点击托盘图标可查看更多选项。"
    );
}
