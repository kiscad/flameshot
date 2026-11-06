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
