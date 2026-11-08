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
    zvariant::{OwnedFd, OwnedObjectPath, OwnedValue, Value},
};

struct Portal {
    status: Arc<AtomicU32>,
    closed: Arc<AtomicU32>,
    inode: u64,
    device: u64,
    images: Arc<Mutex<Vec<std::path::PathBuf>>>,
    urls: Arc<Mutex<Vec<String>>>,
}
struct Request(Arc<AtomicU32>);
#[zbus::interface(name = "org.freedesktop.portal.Request")]
impl Request {
    fn close(&self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}
struct StartingSaveRequest {
    closed: Arc<AtomicU32>,
    ready: Arc<std::sync::atomic::AtomicBool>,
    attempted: Arc<tokio::sync::Semaphore>,
}
#[zbus::interface(name = "org.freedesktop.portal.Request")]
impl StartingSaveRequest {
    fn close(&self) -> zbus::fdo::Result<()> {
        if !self.ready.load(Ordering::SeqCst) {
            self.attempted.add_permits(1);
            return Err(zbus::fdo::Error::UnknownObject(
                "Request is still starting".into(),
            ));
        }
        self.closed.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}
struct LateSaveRequest {
    closed: Arc<AtomicU32>,
    uris: Arc<Mutex<Vec<String>>>,
    path: OwnedObjectPath,
    filter: (String, Vec<(u32, String)>),
}
#[zbus::interface(name = "org.freedesktop.portal.Request")]
impl LateSaveRequest {
    async fn close(
        &self,
        #[zbus(connection)] connection: &Connection,
        #[zbus(header)] header: Header<'_>,
    ) {
        self.closed.fetch_add(1, Ordering::SeqCst);
        let uris = self.uris.lock().unwrap().clone();
        connection
            .emit_signal(
                header.sender().map(|sender| sender.as_str()),
                self.path.clone(),
                "org.freedesktop.portal.Request",
                "Response",
                &(
                    0_u32,
                    HashMap::from([
                        ("uris", Value::from(uris)),
                        ("current_filter", Value::from(self.filter.clone())),
                    ]),
                ),
            )
            .await
            .unwrap();
    }
}
#[zbus::interface(name = "org.freedesktop.portal.OpenURI")]
impl Portal {
    #[zbus(name = "OpenURI")]
    async fn open_uri(
        &self,
        parent: &str,
        uri: &str,
        options: HashMap<String, OwnedValue>,
        #[zbus(header)] header: Header<'_>,
        #[zbus(connection)] connection: &Connection,
    ) -> zbus::fdo::Result<OwnedObjectPath> {
        assert!(parent.is_empty());
        assert_eq!(options.len(), 1);
        self.urls.lock().unwrap().push(uri.to_owned());
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
    let urls = Arc::new(Mutex::new(Vec::new()));
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
                urls: urls.clone(),
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
    exercise_web_protocol(&client, &status, &closed, &urls).await;
    exercise_save_protocol(
        &client,
        root.path(),
        &chooser_status,
        &chosen_uris,
        &closed,
        &chooser_calls,
    )
    .await;
    if let Ok(tools) = std::env::var("FLAMESHOT_TEST_PRIVATE_DISPLAY_TOOLS") {
        exercise_open_with(root.path(), &tools, &status, &images).await;
        #[cfg(feature = "imgur")]
        exercise_history_links(root.path(), &tools, &status, &urls).await;
        exercise_save_ui(
            root.path(),
            &tools,
            &chooser_status,
            &chosen_uris,
            &chooser_calls,
            &closed,
        )
        .await;
        exercise_pin_save_ui(
            root.path(),
            &tools,
            &chooser_status,
            &chosen_uris,
            &chooser_calls,
            &closed,
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
        if matches!(status, 10 | 11) {
            let ready = Arc::new(std::sync::atomic::AtomicBool::new(false));
            let attempted = Arc::new(tokio::sync::Semaphore::new(0));
            connection
                .object_server()
                .at(
                    path.clone(),
                    StartingSaveRequest {
                        closed: self.closed.clone(),
                        ready: ready.clone(),
                        attempted: attempted.clone(),
                    },
                )
                .await
                .unwrap();
            self.calls.fetch_add(1, Ordering::SeqCst);
            attempted.acquire().await.unwrap().forget();
            ready.store(true, Ordering::SeqCst);
            if status == 11 {
                return Err(zbus::fdo::Error::Failed(
                    "Chooser initialization failed".into(),
                ));
            }
            return Ok(path);
        }
        if status == 12 {
            connection
                .object_server()
                .at(
                    path.clone(),
                    LateSaveRequest {
                        closed: self.closed.clone(),
                        uris: self.uris.clone(),
                        path: path.clone(),
                        filter: jpeg.clone(),
                    },
                )
                .await
                .unwrap();
        } else {
            connection
                .object_server()
                .at(path.clone(), Request(self.closed.clone()))
                .await
                .unwrap();
        }
        self.calls.fetch_add(1, Ordering::SeqCst);
        if !matches!(status, 3 | 4 | 12) {
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
    calls: &AtomicU32,
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
    use flameshot_platform::file_chooser::save_file_cancellable;
    *uris.lock().unwrap() = vec![format!(
        "file://{}",
        root.join("late-selection.png").display()
    )];
    let before_calls = calls.load(Ordering::SeqCst);
    assert!(
        save_file_cancellable(
            connection,
            "",
            "Save",
            &suggested,
            &["png", "jpg"],
            timeout,
            async {}
        )
        .await
        .unwrap()
        .is_none()
    );
    assert_eq!(
        calls.load(Ordering::SeqCst),
        before_calls,
        "Already-cancelled chooser was dispatched"
    );
    for mode in [3, 10, 11, 12] {
        status.store(mode, Ordering::SeqCst);
        let before_calls = calls.load(Ordering::SeqCst);
        let before_close = closed.load(Ordering::SeqCst);
        let cancel = async {
            while calls.load(Ordering::SeqCst) == before_calls {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        };
        assert!(
            save_file_cancellable(
                connection,
                "",
                "Save",
                &suggested,
                &["png", "jpg"],
                timeout,
                cancel
            )
            .await
            .unwrap()
            .is_none()
        );
        assert_eq!(
            closed.load(Ordering::SeqCst),
            before_close + 1,
            "Chooser cleanup failed for mode {mode}"
        );
    }
}
async fn exercise_save_ui(
    root: &std::path::Path,
    tools: &str,
    status: &AtomicU32,
    uris: &Mutex<Vec<String>>,
    calls: &AtomicU32,
    closed: &AtomicU32,
) {
    let xdotool = std::path::Path::new(tools).join("usr/bin/xdotool");
    let tool = |args: &[&str]| {
        std::process::Command::new(&xdotool)
            .args(args)
            .output()
            .unwrap()
    };
    let mut cases = vec![0, 1, 2, 6, 12];
    if std::env::var_os("FLAMESHOT_TEST_DOCUMENT_PORTAL").is_some() {
        cases.extend([7, 8]);
    }
    for code in cases {
        let before_close = closed.load(Ordering::SeqCst);
        status.store(
            if code == 12 {
                12
            } else if code >= 6 {
                0
            } else {
                code
            },
            Ordering::SeqCst,
        );
        let destination = root.join(format!("save-{code}")); // No suffix: honor returned JPG filter without renaming.
        if code != 8 {
            std::fs::write(&destination, b"old file").unwrap();
        }
        let chosen = if code == 6 {
            root.join("missing/file.png")
        } else {
            destination.clone()
        };
        let chosen = if matches!(code, 7 | 8) {
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
        if matches!(code, 1 | 2 | 6 | 12) {
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
                if matches!(code, 2 | 12) {
                    assert!(tool(&["key", "Escape"]).status.success());
                    tokio::time::sleep(Duration::from_millis(200)).await;
                    if code == 12 {
                        tokio::time::timeout(Duration::from_secs(3), async {
                            while closed.load(Ordering::SeqCst) == before_close {
                                tokio::time::sleep(Duration::from_millis(20)).await;
                            }
                        })
                        .await
                        .unwrap();
                        assert_eq!(closed.load(Ordering::SeqCst), before_close + 1);
                    }
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

async fn exercise_pin_save_ui(
    root: &std::path::Path,
    tools: &str,
    status: &AtomicU32,
    uris: &Mutex<Vec<String>>,
    calls: &AtomicU32,
    closed: &AtomicU32,
) {
    use std::io::Write;
    let xdotool = std::path::Path::new(tools).join("usr/bin/xdotool");
    let tool = |args: &[&str]| {
        let result = std::process::Command::new(&xdotool)
            .args(args)
            .output()
            .unwrap();
        assert!(result.status.success(), "{:?}: {:?}", args, result.stderr);
        result.stdout
    };
    let find = |title: &str| {
        let found = std::process::Command::new(&xdotool)
            .args(["search", "--onlyvisible", "--name", title])
            .output()
            .unwrap();
        found.status.success().then(|| {
            String::from_utf8(found.stdout)
                .unwrap()
                .lines()
                .last()
                .unwrap()
                .to_owned()
        })
    };
    for code in [0, 1, 2, 6, 12] {
        let before_close = closed.load(Ordering::SeqCst);
        status.store(if code == 6 { 0 } else { code }, Ordering::SeqCst);
        let destination = root.join(format!("pin-save-{code}"));
        std::fs::write(&destination, b"old pin file").unwrap();
        let chosen = if code == 6 {
            root.join("missing/pin.png")
        } else {
            destination.clone()
        };
        *uris.lock().unwrap() = vec![format!("file://{}", chosen.display())];
        std::fs::write(root.join("flameshot/flameshot.ini"),format!("[General]\ncheckForUpdates=false\nshowStartupLaunchMessage=false\nshowDesktopNotification=false\nsavePath={}\n",root.display())).unwrap();
        let image = image::RgbaImage::from_pixel(400, 300, image::Rgba([20, 40, 200, 255]));
        let data = flameshot_core::qt_wire::encode_pin(&flameshot_core::qt_wire::Pin {
            image,
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
            std::process::Command::new(env!("CARGO_BIN_EXE_flameshot"))
                .arg("internal-pin")
                .env("FLATPAK_ID", "isolated-test")
                .env("XDG_CONFIG_HOME", root)
                .env("XDG_CACHE_HOME", root)
                .env("XDG_DATA_HOME", root)
                .env("LC_ALL", "C")
                .env("PATH", "/nonexistent/flameshot-test-tools")
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::inherit())
                .spawn()
                .unwrap(),
        );
        child.0.stdin.take().unwrap().write_all(&data).unwrap();
        let window = tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                if let Some(window) = find("^flameshot-pin$") {
                    break window;
                }
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        })
        .await
        .unwrap();
        tool(&["windowfocus", "--sync", &window]);
        tokio::time::sleep(Duration::from_millis(300)).await;
        let before = calls.load(Ordering::SeqCst);
        tool(&["mousemove", "--window", &window, "20", "20"]);
        tool(&["click", "3"]);
        tokio::time::sleep(Duration::from_millis(200)).await;
        tool(&["key", "--delay", "150", "Down", "Down", "Return"]);
        tokio::time::timeout(Duration::from_secs(5), async {
            while calls.load(Ordering::SeqCst) == before {
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        })
        .await
        .unwrap();
        tokio::time::sleep(Duration::from_millis(300)).await;
        if matches!(code, 2 | 6 | 12) {
            assert_eq!(std::fs::read(&destination).unwrap(), b"old pin file");
            let dialog = find("^Save Screenshot$").expect("Pin error/retry dialog missing");
            tool(&["windowfocus", "--sync", &dialog]);
            if code == 6 {
                *uris.lock().unwrap() = vec![format!("file://{}", destination.display())];
                tool(&["key", "Return"]);
            } else if code == 12 {
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
                let window = dialog.parse::<u32>().unwrap();
                connection
                    .send_event(
                        false,
                        window,
                        EventMask::NO_EVENT,
                        ClientMessageEvent::new(
                            32,
                            window,
                            protocols,
                            [delete, x11rb::CURRENT_TIME, 0, 0, 0],
                        ),
                    )
                    .unwrap()
                    .check()
                    .unwrap();
                connection.flush().unwrap();
            } else {
                tool(&["key", "Escape"]);
            }
        }
        tokio::time::timeout(Duration::from_secs(5), async {
            while find("^Save Screenshot$").is_some() {
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        })
        .await
        .unwrap();
        assert!(
            child.0.try_wait().unwrap().is_none(),
            "Save result closed the pin"
        );
        if code == 12 {
            tokio::time::timeout(Duration::from_secs(3), async {
                while closed.load(Ordering::SeqCst) == before_close {
                    tokio::time::sleep(Duration::from_millis(20)).await;
                }
            })
            .await
            .unwrap();
            assert_eq!(closed.load(Ordering::SeqCst), before_close + 1);
        }

        if code == 0 || code == 6 {
            let data = std::fs::read(&destination).unwrap();
            assert_eq!(
                image::guess_format(&data).unwrap(),
                image::ImageFormat::Jpeg
            );
            let pixels = image::load_from_memory(&data).unwrap().to_rgba8();
            assert_eq!(pixels.dimensions(), (400, 300));
            assert!(
                pixels
                    .get_pixel(200, 150)
                    .0
                    .into_iter()
                    .zip([20, 40, 200, 255])
                    .all(|(a, b)| (i32::from(a) - b).abs() <= 3)
            );
            assert!(!destination.with_extension("jpg").exists());
        } else {
            assert_eq!(std::fs::read(&destination).unwrap(), b"old pin file");
        }
        tool(&["windowfocus", "--sync", &window]);
        tool(&["key", "Escape"]);
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                if let Some(status) = child.0.try_wait().unwrap() {
                    assert!(status.success());
                    break;
                }
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        })
        .await
        .unwrap();
    }
    println!(
        "PASS: pinned-image SaveFile accepts exact names/filters and retains images after cancel, rejection and write retry"
    );
}

async fn exercise_web_protocol(
    connection: &Connection,
    status: &AtomicU32,
    closed: &AtomicU32,
    urls: &Mutex<Vec<String>>,
) {
    use flameshot_platform::open_portal::open_web_uri;
    let uri = "https://example.org/release?q=a%20b#section";
    let timeout = Duration::from_secs(2);
    for code in [0, 1, 2, 5] {
        status.store(code, Ordering::SeqCst);
        let result = open_web_uri(connection, "", uri, timeout).await;
        match code {
            0 => assert!(result.unwrap()),
            1 => assert!(!result.unwrap()),
            _ => assert!(result.is_err()),
        }
        assert_eq!(urls.lock().unwrap().last().unwrap(), uri);
    }
    let before = closed.load(Ordering::SeqCst);
    for code in [3, 4] {
        status.store(code, Ordering::SeqCst);
        assert!(
            open_web_uri(connection, "", uri, Duration::from_millis(100))
                .await
                .is_err()
        );
    }
    assert_eq!(closed.load(Ordering::SeqCst), before + 2);
    let before = urls.lock().unwrap().len();
    for invalid in ["file:///tmp/image.png", "javascript:alert(1)", "not a url"] {
        assert!(
            open_web_uri(connection, "", invalid, timeout)
                .await
                .is_err()
        );
    }
    assert_eq!(urls.lock().unwrap().len(), before);
}
#[cfg(feature = "imgur")]
async fn exercise_history_links(
    root: &std::path::Path,
    tools: &str,
    status: &AtomicU32,
    urls: &Mutex<Vec<String>>,
) {
    let history = root.join("flameshot/history");
    std::fs::create_dir_all(&history).unwrap();
    let path = history.join("imgur-testtoken-abc.png");
    image::RgbaImage::new(80, 60).save(&path).unwrap();
    let original = std::fs::read(&path).unwrap();
    let xdotool = std::path::Path::new(tools).join("usr/bin/xdotool");
    let tool = |args: &[&str]| {
        std::process::Command::new(&xdotool)
            .args(args)
            .output()
            .unwrap()
    };
    let mut child = ChildGuard(
        std::process::Command::new(env!("CARGO_BIN_EXE_flameshot"))
            .arg("history")
            .env("FLATPAK_ID", "isolated-test")
            .env("XDG_CONFIG_HOME", root)
            .env("XDG_CACHE_HOME", root)
            .env("PATH", "/nonexistent/flameshot-test-tools")
            .env("LC_ALL", "C")
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .unwrap(),
    );
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    let window = loop {
        let output = tool(&[
            "search",
            "--onlyvisible",
            "--name",
            "^Flameshot Upload History$",
        ]);
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
    tokio::time::sleep(Duration::from_millis(400)).await;
    for code in [0, 1, 2] {
        status.store(code, Ordering::SeqCst);
        let before = urls.lock().unwrap().len();
        assert!(
            tool(&["mousemove", "--window", &window, "130", "58"])
                .status
                .success()
        );
        assert!(tool(&["click", "1"]).status.success());
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while urls.lock().unwrap().len() == before {
            assert!(
                std::time::Instant::now() < deadline,
                "History hyperlink did not reach the portal"
            );
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        assert_eq!(
            urls.lock().unwrap().last().unwrap(),
            "https://i.imgur.com/abc.png"
        );
        tokio::time::sleep(Duration::from_millis(300)).await;
        assert!(child.0.try_wait().unwrap().is_none());
    }
    assert!(tool(&["key", "Escape"]).status.success());
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(exit) = child.0.try_wait().unwrap() {
            assert!(exit.success());
            break;
        }
        assert!(std::time::Instant::now() < deadline);
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert_eq!(std::fs::read(path).unwrap(), original);
    println!(
        "PASS: actual sandbox history links use OpenURI and retain window/record after success, cancellation and rejection"
    );
}
