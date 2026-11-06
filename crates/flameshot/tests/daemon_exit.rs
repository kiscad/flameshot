// SPDX-License-Identifier: GPL-3.0-or-later
use std::{
    io::{BufRead, BufReader},
    process::{Child, Command, Stdio},
    time::Duration,
};
struct ChildGuard(Child);
impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[tokio::test]
#[ignore = "requires private D-Bus and Unix signals"]
async fn signals_and_bus_loss_return_legacy_statuses() {
    assert_eq!(
        std::env::var("FLAMESHOT_TEST_PRIVATE_BUS").as_deref(),
        Ok("1")
    );
    for (signal, expected) in [(Some("-INT"), 130), (Some("-TERM"), 143), (None, 3)] {
        let mut bus = ChildGuard(
            Command::new("dbus-daemon")
                .args(["--session", "--nofork", "--print-address=1"])
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .spawn()
                .unwrap(),
        );
        let mut address = String::new();
        BufReader::new(bus.0.stdout.take().unwrap())
            .read_line(&mut address)
            .unwrap();
        let directory = tempfile::tempdir().unwrap();
        std::fs::create_dir(directory.path().join("flameshot")).unwrap();
        std::fs::write(directory.path().join("flameshot/flameshot.ini"),
            "[General]\ncheckForUpdates=false\ndisabledTrayIcon=true\nshowDesktopNotification=false\nshowStartupLaunchMessage=false\n").unwrap();
        let mut app = ChildGuard(
            Command::new(env!("CARGO_BIN_EXE_flameshot"))
                .env("DBUS_SESSION_BUS_ADDRESS", address.trim())
                .env("XDG_CONFIG_HOME", directory.path())
                .stdout(Stdio::null())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap(),
        );
        let connection = zbus::connection::Builder::address(address.trim())
            .unwrap()
            .build()
            .await
            .unwrap();
        let proxy = zbus::fdo::DBusProxy::new(&connection).await.unwrap();
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                assert!(
                    app.0.try_wait().unwrap().is_none(),
                    "Daemon stopped before readiness"
                );
                if proxy
                    .name_has_owner(
                        zbus::names::BusName::try_from("org.flameshot.Flameshot").unwrap(),
                    )
                    .await
                    .unwrap()
                {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await
        .unwrap();
        if let Some(signal) = signal {
            assert!(
                Command::new("kill")
                    .args([signal, &app.0.id().to_string()])
                    .status()
                    .unwrap()
                    .success()
            );
        } else {
            bus.0.kill().unwrap();
            bus.0.wait().unwrap();
        }
        let status = tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                if let Some(status) = app.0.try_wait().unwrap() {
                    break status;
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await
        .unwrap();
        assert_eq!(status.code(), Some(expected), "signal={signal:?}");
    }
}
