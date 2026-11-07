// SPDX-License-Identifier: GPL-3.0-or-later
use std::process::Command;

#[test]
fn legacy_version_flag_and_aliases_do_not_start_desktop_services() {
    let temp = tempfile::tempdir().unwrap();
    for flag in ["-v", "-V", "--version"] {
        let output = Command::new(env!("CARGO_BIN_EXE_flameshot"))
            .env("XDG_CONFIG_HOME", temp.path())
            .env(
                "DBUS_SESSION_BUS_ADDRESS",
                "unix:path=/nonexistent/flameshot-test-bus",
            )
            .arg(flag)
            .output()
            .unwrap();
        assert!(output.status.success(), "{flag}: {:?}", output.stderr);
        assert_eq!(
            String::from_utf8(output.stdout).unwrap(),
            format!("flameshot {}\n", env!("CARGO_PKG_VERSION"))
        );
        assert!(output.stderr.is_empty());
    }
    assert_eq!(std::fs::read_dir(temp.path()).unwrap().count(), 0);
}

#[test]
fn documented_commands_and_long_options_are_present_in_the_manual() {
    let manual = include_str!("../../../data/man/man1/flameshot.1").replace("\\-", "-");
    for command in [
        None,
        Some("gui"),
        Some("screen"),
        Some("full"),
        Some("config"),
        Some("launcher"),
    ] {
        let mut process = Command::new(env!("CARGO_BIN_EXE_flameshot"));
        if let Some(command) = command {
            assert!(
                manual.contains(&format!(".B {command}\n")),
                "Missing command {command}"
            );
            process.arg(command);
        }
        let output = process.arg("--help").output().unwrap();
        assert!(output.status.success());
        let help = String::from_utf8(output.stdout).unwrap();
        assert!(!help.contains("internal-about"));
        assert!(!help.contains("internal-open-save-path"));
        for option in help
            .split_whitespace()
            .filter(|word| word.starts_with("--"))
        {
            assert!(
                manual.contains(option),
                "Missing manual option {option} for {command:?}"
            );
        }
    }
}

#[test]
fn unavailable_session_bus_has_legacy_status_without_output_or_config_writes() {
    let temp = tempfile::tempdir().unwrap();
    let missing = format!("unix:path={}/missing-bus", temp.path().display());
    for address in [missing.as_str(), "not-a-valid-dbus-address"] {
        for args in [vec![], vec!["full", "--raw"]] {
            let output = Command::new(env!("CARGO_BIN_EXE_flameshot"))
                .env("XDG_CONFIG_HOME", temp.path())
                .env("DBUS_SESSION_BUS_ADDRESS", address)
                .env_remove("DISPLAY")
                .env_remove("WAYLAND_DISPLAY")
                .args(&args)
                .output()
                .unwrap();
            assert_eq!(
                output.status.code(),
                Some(3),
                "{args:?}: {:?}",
                output.stderr
            );
            assert!(output.stdout.is_empty());
            assert!(String::from_utf8_lossy(&output.stderr).contains("session D-Bus"));
        }
    }
    assert_eq!(std::fs::read_dir(temp.path()).unwrap().count(), 0);
}

#[test]
fn configuration_failure_is_not_a_bus_connection_failure() {
    let temp = tempfile::tempdir().unwrap();
    let directory = temp.path().join("flameshot");
    std::fs::create_dir(&directory).unwrap();
    let path = directory.join("flameshot.ini");
    let content = "[General]\nundoLimit=1000\n";
    std::fs::write(&path, content).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_flameshot"))
        .env("XDG_CONFIG_HOME", temp.path())
        .env("DBUS_SESSION_BUS_ADDRESS", "not-a-valid-dbus-address")
        .args(["config", "--check"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stderr).contains("undoLimit"));
    assert_eq!(std::fs::read_to_string(path).unwrap(), content);
}

#[test]
fn vanished_internal_owner_exits_before_starting_capture() {
    let temp = tempfile::tempdir().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_flameshot"))
        .env("XDG_CONFIG_HOME", temp.path())
        .env("DBUS_SESSION_BUS_ADDRESS", "not-a-valid-dbus-address")
        .args(["--internal-parent", "0", "full", "--raw"])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(output.stdout.is_empty() && output.stderr.is_empty());
    assert_eq!(std::fs::read_dir(temp.path()).unwrap().count(), 0);
    let help = Command::new(env!("CARGO_BIN_EXE_flameshot"))
        .arg("--help")
        .output()
        .unwrap();
    assert!(!String::from_utf8_lossy(&help.stdout).contains("internal-parent"));
}
