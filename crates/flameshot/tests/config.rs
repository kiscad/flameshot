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
    assert!(saved.starts_with(original));
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
