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
    let chooser_status = Arc::new(AtomicU32::new(0));
    let chosen_uris = Arc::new(Mutex::new(Vec::<String>::new()));
    let chooser_calls = Arc::new(AtomicU32::new(0));
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
        .serve_at(
            "/org/freedesktop/portal/desktop",
            Chooser {
                status: chooser_status.clone(),
                closed: closed.clone(),
                uris: chosen_uris.clone(),
                calls: chooser_calls.clone(),
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
    exercise_save_protocol(&client, root.path(), &chooser_status, &chosen_uris, &closed).await;
    if let Ok(tools) = std::env::var("FLAMESHOT_TEST_PRIVATE_DISPLAY_TOOLS") {
        exercise_open_with(root.path(), &tools, &status, &images).await;
        exercise_save_ui(
            root.path(),
            &tools,
            &chooser_status,
            &chosen_uris,
            &chooser_calls,
        )
        .await;
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

struct Chooser {
    status: Arc<AtomicU32>,
    closed: Arc<AtomicU32>,
    uris: Arc<Mutex<Vec<String>>>,
    calls: Arc<AtomicU32>,
}
#[zbus::interface(name = "org.freedesktop.portal.FileChooser")]
impl Chooser {
    async fn save_file(
        &self,
        parent: &str,
        title: &str,
        options: HashMap<String, OwnedValue>,
        #[zbus(header)] header: Header<'_>,
        #[zbus(connection)] connection: &Connection,
    ) -> zbus::fdo::Result<OwnedObjectPath> {
        assert!(parent.is_empty());
        assert!(!title.is_empty());
        let folder: Vec<u8> = options
            .get("current_folder")
            .unwrap()
            .try_clone()
            .unwrap()
            .try_into()
            .unwrap();
        assert_eq!(folder.last(), Some(&0));
        assert_eq!(folder.first(), Some(&b'/'));
        assert!(
            !<&str>::try_from(options.get("current_name").unwrap())
                .unwrap()
                .is_empty()
        );
        type Filter = (String, Vec<(u32, String)>);
        let filters: Vec<Filter> = options
            .get("filters")
            .unwrap()
            .try_clone()
            .unwrap()
            .try_into()
            .unwrap();
        let current: Filter = options
            .get("current_filter")
            .unwrap()
            .try_clone()
            .unwrap()
            .try_into()
            .unwrap();
        assert!(filters.contains(&current));
        assert!(filters.contains(&("PNG".into(), vec![(0, "*.[pP][nN][gG]".into())])));
        let jpeg = filters
            .iter()
            .find(|filter| filter.0 == "JPG")
            .unwrap()
            .clone();
        let sender = header
            .sender()
            .unwrap()
            .as_str()
            .trim_start_matches(':')
            .replace('.', "_");
        let token = <&str>::try_from(options.get("handle_token").unwrap()).unwrap();
        let status = self.status.load(Ordering::SeqCst);
        let suffix = if status == 4 { "_wrong" } else { "" };
        let path: OwnedObjectPath =
            format!("/org/freedesktop/portal/desktop/request/{sender}/{token}{suffix}")
                .try_into()
                .unwrap();
        connection
            .object_server()
            .at(path.clone(), Request(self.closed.clone()))
            .await
            .unwrap();
        self.calls.fetch_add(1, Ordering::SeqCst);
        if status != 3 && status != 4 {
            let results = HashMap::from([
                (
                    "uris",
                    zbus::zvariant::Value::from(self.uris.lock().unwrap().clone()),
                ),
                ("current_filter", zbus::zvariant::Value::from(jpeg)),
            ]);
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
        }
        Ok(path)
    }
}
async fn exercise_save_protocol(
    connection: &Connection,
    root: &std::path::Path,
    status: &AtomicU32,
    uris: &Mutex<Vec<String>>,
    closed: &AtomicU32,
) {
    use flameshot_platform::file_chooser::save_file;
    let suggested = root.join("suggested.png");
    let timeout = Duration::from_secs(2);
    *uris.lock().unwrap() = vec![format!(
        "file://{}/selected%20%E6%88%AA%E5%9B%BE",
        root.display()
    )];
    let selected = save_file(connection, "", "Save", &suggested, &["png", "jpg"], timeout)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(selected.path, root.join("selected 截图"));
    assert_eq!(selected.extension, "jpg");
    for invalid in [
        vec![],
        vec!["https://example.org/file.png".into()],
        vec!["file://remote/file.png".into()],
        vec!["file:///unexpected?query".into()],
        vec!["file:///unexpected#fragment".into()],
        vec!["file:///one".into(), "file:///two".into()],
    ] {
        *uris.lock().unwrap() = invalid;
        assert!(
            save_file(connection, "", "Save", &suggested, &["png", "jpg"], timeout)
                .await
                .is_err()
        );
    }
    status.store(1, Ordering::SeqCst);
    assert!(
        save_file(connection, "", "Save", &suggested, &["png", "jpg"], timeout)
            .await
            .unwrap()
            .is_none()
    );
    for code in [2, 5] {
        status.store(code, Ordering::SeqCst);
        assert!(
            save_file(connection, "", "Save", &suggested, &["png", "jpg"], timeout)
                .await
                .is_err()
        );
    }
    let before = closed.load(Ordering::SeqCst);
    for code in [3, 4] {
        status.store(code, Ordering::SeqCst);
        assert!(
            save_file(
                connection,
                "",
                "Save",
                &suggested,
                &["png", "jpg"],
                Duration::from_millis(100)
            )
            .await
            .is_err()
        );
    }
    assert_eq!(closed.load(Ordering::SeqCst), before + 2);
}
async fn exercise_save_ui(
    root: &std::path::Path,
    tools: &str,
    status: &AtomicU32,
    uris: &Mutex<Vec<String>>,
    calls: &AtomicU32,
) {
    let xdotool = std::path::Path::new(tools).join("usr/bin/xdotool");
    let tool = |args: &[&str]| {
        std::process::Command::new(&xdotool)
            .args(args)
            .output()
            .unwrap()
    };
    let mut cases = vec![0, 1, 2, 6];
    if std::env::var_os("FLAMESHOT_TEST_DOCUMENT_PORTAL").is_some() {
        cases.extend([7, 8]);
    }
    for code in cases {
        status.store(if code >= 6 { 0 } else { code }, Ordering::SeqCst);
        let destination = root.join(format!("save-{code}")); // No suffix: honor returned JPG filter without renaming.
        if code != 8 {
            std::fs::write(&destination, b"old file").unwrap();
        }
        let chosen = if code == 6 {
            root.join("missing/file.png")
        } else {
            destination.clone()
        };
        let chosen = if code >= 7 {
            use std::os::unix::ffi::OsStringExt;
            let connection = Connection::session().await.unwrap();
            let documents = zbus::Proxy::new(
                &connection,
                "org.freedesktop.portal.Documents",
                "/org/freedesktop/portal/documents",
                "org.freedesktop.portal.Documents",
            )
            .await
            .unwrap();
            let descriptor = rustix::fs::open(
                if code == 8 { root } else { &destination },
                rustix::fs::OFlags::PATH | rustix::fs::OFlags::CLOEXEC,
                rustix::fs::Mode::empty(),
            )
            .unwrap();
            let id: String = if code == 8 {
                let name = format!("save-{code}\0").into_bytes();
                documents
                    .call(
                        "AddNamed",
                        &(zbus::zvariant::Fd::from(&descriptor), name, false, false),
                    )
                    .await
                    .unwrap()
            } else {
                documents
                    .call(
                        "Add",
                        &(zbus::zvariant::Fd::from(&descriptor), false, false),
                    )
                    .await
                    .unwrap()
            };
            documents
                .call::<_, _, ()>(
                    "GrantPermissions",
                    &(
                        id.as_str(),
                        "org.flameshot.Flameshot",
                        vec!["read", "write", "delete"],
                    ),
                )
                .await
                .unwrap();
            let mut mount: Vec<u8> = documents.call("GetMountPoint", &()).await.unwrap();
            if mount.last() == Some(&0) {
                mount.pop();
            }
            std::path::PathBuf::from(std::ffi::OsString::from_vec(mount))
                .join("by-app/org.flameshot.Flameshot")
                .join(id)
                .join(destination.file_name().unwrap())
        } else {
            chosen
        };
        *uris.lock().unwrap() = vec![format!("file://{}", chosen.display())];
        let before = calls.load(Ordering::SeqCst);
        std::fs::write(root.join("flameshot/flameshot.ini"), format!("[General]\ncheckForUpdates=false\nshowDesktopNotification=false\nuseX11LegacyScreenshot=true\nsavePath={}\n",root.display())).unwrap();
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
            assert!(child.0.try_wait().unwrap().is_none());
            assert!(std::time::Instant::now() < deadline);
            tokio::time::sleep(Duration::from_millis(50)).await;
        };
        assert!(tool(&["windowfocus", "--sync", &window]).status.success());
        tokio::time::sleep(Duration::from_millis(300)).await;
        assert!(tool(&["key", "ctrl+s"]).status.success());
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while calls.load(Ordering::SeqCst) == before {
            assert!(std::time::Instant::now() < deadline);
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        tokio::time::sleep(Duration::from_millis(400)).await;
        if matches!(code, 1 | 2 | 6) {
            assert!(
                child.0.try_wait().unwrap().is_none(),
                "Save cancellation/error discarded editor"
            );
            assert_eq!(std::fs::read(&destination).unwrap(), b"old file");
            if code == 6 {
                // Retry after a write error, using the same selection.
                *uris.lock().unwrap() = vec![format!("file://{}", destination.display())];
                assert!(tool(&["key", "Return"]).status.success());
            } else {
                if code == 2 {
                    assert!(tool(&["key", "Escape"]).status.success());
                    tokio::time::sleep(Duration::from_millis(200)).await;
                }
                assert!(tool(&["key", "Return"]).status.success());
            }
        }
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        loop {
            if let Some(exit) = child.0.try_wait().unwrap() {
                assert!(exit.success());
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "Save editor did not finish"
            );
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        if matches!(code, 0 | 6 | 7 | 8) {
            let data = std::fs::read(&destination).unwrap();
            assert_eq!(
                image::guess_format(&data).unwrap(),
                image::ImageFormat::Jpeg
            );
            let decoded = image::load_from_memory(&data).unwrap();
            assert_eq!((decoded.width(), decoded.height()), (80, 60));
            assert!(!destination.with_extension("jpg").exists());
            if code >= 7 {
                println!(
                    "PASS: real document-portal {} publishes to the host file",
                    if code == 7 { "replacement" } else { "creation" }
                );
            }
        } else {
            assert_eq!(std::fs::read(&destination).unwrap(), b"old file");
        }
    }
}
