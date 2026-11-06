// SPDX-License-Identifier: GPL-3.0-or-later
use anyhow::{Context, Result};
use std::{io::Write, path::Path, time::Duration};
/// Keep the historical filename so upgrading does not create duplicate startup entries.
pub fn autostart_enabled(config_home: &Path) -> bool {
    config_home.join("autostart/Flameshot.desktop").is_file()
}
pub fn set_autostart(config_home: &Path, enabled: bool) -> Result<()> {
    let path = config_home.join("autostart/Flameshot.desktop");
    if !enabled {
        return match std::fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.into()),
        };
    }
    let parent = path.parent().context("Invalid startup path")?;
    std::fs::create_dir_all(parent)?;
    let mut file = tempfile::NamedTempFile::new_in(parent)?;
    file.write_all(b"[Desktop Entry]\nName=Flameshot\nIcon=org.flameshot.Flameshot\nExec=flameshot\nTerminal=false\nType=Application\nX-GNOME-Autostart-enabled=true\n")?;
    file.as_file().sync_all()?;
    file.persist(path).map_err(|e| e.error)?;
    Ok(())
}
pub async fn notify(connection: &zbus::Connection, title: &str, body: &str) -> Result<u32> {
    tokio::time::timeout(Duration::from_secs(2), async {
        let proxy = zbus::Proxy::new(
            connection,
            "org.freedesktop.Notifications",
            "/org/freedesktop/Notifications",
            "org.freedesktop.Notifications",
        )
        .await?;
        let hints = std::collections::HashMap::from([(
            "desktop-entry",
            zbus::zvariant::Value::from("org.flameshot.Flameshot"),
        )]);
        proxy
            .call::<_, _, u32>(
                "Notify",
                &(
                    "Flameshot",
                    0_u32,
                    "org.flameshot.Flameshot",
                    title,
                    body,
                    Vec::<String>::new(),
                    hints,
                    -1_i32,
                ),
            )
            .await
    })
    .await
    .context("Notification service timed out")?
    .map_err(Into::into)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn autostart_updates_only_the_legacy_flameshot_entry() {
        let dir = tempfile::tempdir().unwrap();
        assert!(!autostart_enabled(dir.path()));
        set_autostart(dir.path(), false).unwrap();
        set_autostart(dir.path(), true).unwrap();
        assert!(autostart_enabled(dir.path()));
        let file = dir.path().join("autostart/Flameshot.desktop");
        let data = std::fs::read_to_string(&file).unwrap();
        assert!(data.contains("\nExec=flameshot\n"));
        std::fs::write(dir.path().join("autostart/Other.desktop"), "unchanged").unwrap();
        set_autostart(dir.path(), true).unwrap();
        assert_eq!(std::fs::read_to_string(&file).unwrap(), data);
        set_autostart(dir.path(), false).unwrap();
        assert!(!autostart_enabled(dir.path()));
        assert_eq!(
            std::fs::read_to_string(dir.path().join("autostart/Other.desktop")).unwrap(),
            "unchanged"
        );
    }
}
