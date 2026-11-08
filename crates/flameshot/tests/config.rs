// SPDX-License-Identifier: GPL-3.0-or-later
use std::process::Command;
#[test]
fn config_cli_updates_only_requested_settings_and_rejects_bad_values() {
    let temp = tempfile::tempdir().unwrap();
    let directory = temp.path().join("flameshot");
    std::fs::create_dir(&directory).unwrap();
    let file = directory.join("flameshot.ini");
    let original = "; preserved\n[General]\nbuttons=@Variant(\\0)\n[Shortcuts]\nTYPE_SAVE=Ctrl+S\n";
    std::fs::write(&file, original).unwrap();
    let run = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_flameshot"))
            .env("XDG_CONFIG_HOME", temp.path())
            .args(args)
            .output()
            .unwrap()
    };
    let result = run(&[
        "config",
        "--filename",
        "%F-custom",
        "--trayicon",
        "false",
        "--notifications",
        "false",
        "--showhelp",
        "false",
    ]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let saved = std::fs::read_to_string(&file).unwrap();
    let (general, shortcuts) = original.split_once("[Shortcuts]").unwrap();
    assert!(saved.starts_with(general));
    assert!(saved.ends_with(&format!("[Shortcuts]{shortcuts}")));
    assert_eq!(saved.matches("[General]").count(), 1);
    assert_eq!(saved.matches("[Shortcuts]").count(), 1);
    let ini = flameshot_core::config::IniDocument::parse(saved.clone());
    assert_eq!(
        ini.string("General", "disabledTrayIcon")
            .unwrap()
            .as_deref(),
        Some("true")
    );
    assert_eq!(
        ini.string("General", "filenamePattern").unwrap().as_deref(),
        Some("%F-custom")
    );
    assert!(
        !run(&["config", "--filename", "../outside"])
            .status
            .success()
    );
    assert_eq!(std::fs::read_to_string(file).unwrap(), saved);
}

#[test]
fn autostart_cli_uses_isolated_legacy_desktop_entry() {
    let temp = tempfile::tempdir().unwrap();
    for enabled in ["true", "true", "false", "false"] {
        let result = Command::new(env!("CARGO_BIN_EXE_flameshot"))
            .env("XDG_CONFIG_HOME", temp.path())
            .args(["config", "--autostart", enabled])
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(
            temp.path().join("autostart/Flameshot.desktop").exists(),
            enabled == "true"
        );
        let ini = flameshot_core::config::IniDocument::parse(
            std::fs::read_to_string(temp.path().join("flameshot/flameshot.ini")).unwrap(),
        );
        assert_eq!(
            ini.string("General", "startupLaunch").unwrap().as_deref(),
            Some(enabled)
        );
    }
}

#[test]
fn config_check_is_headless_read_only_and_has_useful_exit_status() {
    let temp = tempfile::tempdir().unwrap();
    let run = || {
        Command::new(env!("CARGO_BIN_EXE_flameshot"))
            .env("XDG_CONFIG_HOME", temp.path())
            .env_remove("DISPLAY")
            .env_remove("WAYLAND_DISPLAY")
            .args(["config", "--check", "--trayicon", "false"])
            .output()
            .unwrap()
    };
    let missing = run();
    assert!(missing.status.success(), "{:?}", missing);
    let file = temp.path().join("flameshot/flameshot.ini");
    assert!(!file.exists());
    std::fs::create_dir(file.parent().unwrap()).unwrap();
    let source = "[General]\narrowStyle=5\ncustomUnrecognized=true\n[Shortcuts]\nTYPE_SAVE=F8\nTYPE_COPY=F8\n";
    std::fs::write(&file, source).unwrap();
    let invalid = run();
    assert_eq!(invalid.status.code(), Some(1));
    let error = String::from_utf8_lossy(&invalid.stderr);
    for expected in ["arrowStyle", "customUnrecognized", "conflicts"] {
        assert!(error.contains(expected), "{error}");
    }
    assert!(invalid.stdout.is_empty());
    assert_eq!(std::fs::read_to_string(&file).unwrap(), source);
    let valid = "[General]\narrowStyle=1\nreverseArrow=true\n[Shortcuts]\nTYPE_ACCEPT=Ctrl+S\n";
    std::fs::write(&file, valid).unwrap();
    let result = run();
    assert!(result.status.success());
    assert_eq!(result.stdout, b"No errors detected.\n");
    assert_eq!(std::fs::read_to_string(file).unwrap(), valid);
}

#[test]
fn legacy_color_options_are_headless_atomic_and_preserve_unknown_settings() {
    let temp = tempfile::tempdir().unwrap();
    let file = temp.path().join("flameshot/flameshot.ini");
    std::fs::create_dir(file.parent().unwrap()).unwrap();
    std::fs::write(&file, "; preserved\n[General]\nprivateValue=keep\n").unwrap();
    let run = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_flameshot"))
            .env("XDG_CONFIG_HOME", temp.path())
            .env_remove("DISPLAY")
            .env_remove("WAYLAND_DISPLAY")
            .args(args)
            .output()
            .unwrap()
    };
    let result = run(&["config", "-m", "green", "--contrastcolor", "#ff112233"]);
    assert!(result.status.success(), "{:?}", result);
    let source = std::fs::read_to_string(&file).unwrap();
    let ini = flameshot_core::config::IniDocument::parse(source.clone());
    assert_eq!(
        ini.string("General", "uiColor").unwrap().as_deref(),
        Some("#008000")
    );
    assert_eq!(
        ini.string("General", "contrastUiColor").unwrap().as_deref(),
        Some("#112233")
    );
    assert!(source.contains("privateValue=keep"));
    for args in [
        vec![
            "config",
            "--maincolor",
            "#11223344",
            "--filename",
            "changed",
        ],
        vec!["config", "-k", "not-a-color"],
    ] {
        let result = run(&args);
        assert_eq!(result.status.code(), Some(0));
        assert!(!result.stderr.is_empty());
        assert!(result.stdout.is_empty());
        assert_eq!(std::fs::read_to_string(&file).unwrap(), source);
    }
}

#[test]
fn configuration_check_rejects_oversized_values_without_modifying_the_file() {
    let temp = tempfile::tempdir().unwrap();
    let directory = temp.path().join("flameshot");
    std::fs::create_dir(&directory).unwrap();
    let path = directory.join("flameshot.ini");
    let source = "[General]\ndrawThickness=2147483648\n";
    std::fs::write(&path, source).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_flameshot"))
        .env("XDG_CONFIG_HOME", temp.path())
        .env("DBUS_SESSION_BUS_ADDRESS", "invalid-test-address")
        .env_remove("DISPLAY")
        .env_remove("WAYLAND_DISPLAY")
        .args(["config", "--check"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1), "{:?}", output.stderr);
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("drawThickness"));
    assert!(String::from_utf8_lossy(&output.stderr).contains("2147483647"));
    assert_eq!(std::fs::read_to_string(&path).unwrap(), source);
}

#[test]
fn concurrent_cli_updates_preserve_disjoint_settings() {
    let temp = tempfile::tempdir().unwrap();
    let directory = temp.path().join("flameshot");
    std::fs::create_dir(&directory).unwrap();
    let path = directory.join("flameshot.ini");
    let opaque = "[Other]\nopaque=@Variant(\\0keep)\n";
    std::fs::write(&path, format!("; retained\n[General]\n{opaque}")).unwrap();
    let updates = [
        ("--filename", "%F-concurrent"),
        ("--maincolor", "#123456"),
        ("--contrastcolor", "#654321"),
        ("--notifications", "false"),
        ("--trayicon", "false"),
        ("--showhelp", "false"),
    ];
    let barrier = std::sync::Barrier::new(updates.len());
    std::thread::scope(|scope| {
        for (flag, value) in updates {
            let barrier = &barrier;
            let config = temp.path();
            scope.spawn(move || {
                barrier.wait();
                let output = Command::new(env!("CARGO_BIN_EXE_flameshot"))
                    .env("XDG_CONFIG_HOME", config)
                    .args(["config", flag, value])
                    .output()
                    .unwrap();
                assert!(output.status.success(), "{flag}: {:?}", output.stderr);
            });
        }
    });
    let saved = std::fs::read_to_string(path).unwrap();
    assert!(saved.starts_with("; retained\n") && saved.ends_with(opaque));
    assert_eq!(saved.matches("[General]").count(), 1);
    let ini = flameshot_core::config::IniDocument::parse(saved);
    for (key, value) in [
        ("filenamePattern", "%F-concurrent"),
        ("uiColor", "#123456"),
        ("contrastUiColor", "#654321"),
        ("showDesktopNotification", "false"),
        ("disabledTrayIcon", "true"),
        ("showHelp", "false"),
    ] {
        assert_eq!(
            ini.string("General", key).unwrap().as_deref(),
            Some(value),
            "{key}"
        );
    }
}

#[test]
fn waiting_writer_reads_configuration_after_acquiring_the_lock() {
    use std::time::{Duration, Instant};
    struct ChildGuard(std::process::Child);
    impl Drop for ChildGuard {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    let temp = tempfile::tempdir().unwrap();
    let directory = temp.path().join("flameshot");
    std::fs::create_dir(&directory).unwrap();
    let path = directory.join("flameshot.ini");
    let lock_path = directory.join("flameshot.ini.rust-lock");
    std::fs::write(&path, "[General]\nshowHelp=true\n").unwrap();
    let lock = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&lock_path)
        .unwrap();
    lock.lock().unwrap();
    let mut writer = ChildGuard(
        Command::new(env!("CARGO_BIN_EXE_flameshot"))
            .env("XDG_CONFIG_HOME", temp.path())
            .args(["config", "--filename", "from-writer"])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .unwrap(),
    );
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        assert!(
            writer.0.try_wait().unwrap().is_none(),
            "Writer bypassed the held lock"
        );
        let descriptors = std::fs::read_dir(format!("/proc/{}/fd", writer.0.id())).unwrap();
        if descriptors
            .filter_map(Result::ok)
            .any(|entry| std::fs::read_link(entry.path()).is_ok_and(|p| p == lock_path))
        {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "Writer did not open the configuration lock"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    std::fs::write(&path, "[General]\nshowHelp=false\n; latest change\n").unwrap();
    drop(lock);
    loop {
        if let Some(status) = writer.0.try_wait().unwrap() {
            assert!(status.success());
            break;
        }
        assert!(
            Instant::now() < deadline,
            "Writer did not finish after lock release"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    let source = std::fs::read_to_string(path).unwrap();
    let ini = flameshot_core::config::IniDocument::parse(source.clone());
    assert_eq!(
        ini.string("General", "showHelp").unwrap().as_deref(),
        Some("false")
    );
    assert_eq!(
        ini.string("General", "filenamePattern").unwrap().as_deref(),
        Some("from-writer")
    );
    assert!(source.contains("; latest change\n"));
}

#[test]
fn failed_autostart_config_write_preserves_custom_desktop_entry() {
    use std::os::unix::fs::PermissionsExt;
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir(root.path().join("autostart")).unwrap();
    std::fs::create_dir(root.path().join("flameshot")).unwrap();
    let path = root.path().join("autostart/Flameshot.desktop");
    let original = "[Desktop Entry]\nExec=flameshot --custom-argument\nX-Custom=keep\n";
    std::fs::write(&path, original).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o640)).unwrap();
    let ini = root.path().join("flameshot/flameshot.ini");
    let invalid = "[General]\njpegQuality=999\n";
    std::fs::write(&ini, invalid).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_flameshot"))
        .env("XDG_CONFIG_HOME", root.path())
        .args(["config", "--autostart", "false"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stderr).contains("jpegQuality"));
    assert_eq!(std::fs::read_to_string(&path).unwrap(), original);
    assert_eq!(path.metadata().unwrap().permissions().mode() & 0o777, 0o640);
    assert_eq!(std::fs::read_to_string(ini).unwrap(), invalid);
}

#[test]
fn repeated_config_options_keep_first_values_and_later_errors_write_nothing() {
    let root = tempfile::tempdir().unwrap();
    let run = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_flameshot"))
            .env("XDG_CONFIG_HOME", root.path())
            .env("DBUS_SESSION_BUS_ADDRESS", "not-a-valid-dbus-address")
            .env_remove("DISPLAY")
            .env_remove("WAYLAND_DISPLAY")
            .args(args)
            .output()
            .unwrap()
    };
    let result = run(&[
        "config",
        "--filename",
        "first",
        "-f",
        "later",
        "--maincolor",
        "#123",
        "-m",
        "red",
        "--notifications",
        "false",
        "-n",
        "true",
    ]);
    assert!(result.status.success(), "{:?}", result.stderr);
    let path = root.path().join("flameshot/flameshot.ini");
    let saved = std::fs::read_to_string(&path).unwrap();
    let ini = flameshot_core::config::IniDocument::parse(saved.clone());
    for (key, expected) in [
        ("filenamePattern", "first"),
        ("uiColor", "#112233"),
        ("showDesktopNotification", "false"),
    ] {
        assert_eq!(
            ini.string("General", key).unwrap().as_deref(),
            Some(expected)
        );
    }
    for args in [
        vec![
            "config",
            "--filename",
            "changed",
            "-m",
            "red",
            "--maincolor",
            "invalid",
        ],
        vec![
            "config",
            "--filename",
            "changed",
            "-n",
            "true",
            "--notifications",
            "invalid",
        ],
    ] {
        let result = run(&args);
        assert_eq!(result.status.code(), Some(0));
        assert!(!result.stderr.is_empty());
        assert!(result.stdout.is_empty());
        assert!(result.stdout.is_empty());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), saved);
    }
}
