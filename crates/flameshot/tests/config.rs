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
    let result = run(&["config", "-m", "green", "--contrastcolor", "#11223344"]);
    assert!(result.status.success(), "{:?}", result);
    let source = std::fs::read_to_string(&file).unwrap();
    let ini = flameshot_core::config::IniDocument::parse(source.clone());
    assert_eq!(
        ini.string("General", "uiColor").unwrap().as_deref(),
        Some("green")
    );
    assert_eq!(
        ini.string("General", "contrastUiColor").unwrap().as_deref(),
        Some("#11223344")
    );
    assert!(source.contains("privateValue=keep"));
    for args in [
        vec!["config", "--maincolor", "#123", "--filename", "changed"],
        vec!["config", "-k", "not-a-color"],
    ] {
        let result = run(&args);
        assert_eq!(result.status.code(), Some(1));
        assert_eq!(std::fs::read_to_string(&file).unwrap(), source);
    }
}
