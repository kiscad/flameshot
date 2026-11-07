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
/// Roll back the original entry object if the associated settings commit fails.
/// A hard-link snapshot preserves custom contents, permissions and symlinks on Linux.
pub fn set_autostart_with_commit(
    config_home: &Path,
    enabled: bool,
    commit: impl FnOnce() -> Result<()>,
) -> Result<()> {
    let path = config_home.join("autostart/Flameshot.desktop");
    std::fs::create_dir_all(path.parent().unwrap())?;
    let lock = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(path.with_file_name(".Flameshot.rust-lock"))?;
    lock.lock()?;
    let backup = match std::fs::symlink_metadata(&path) {
        Ok(_) => {
            let directory = tempfile::Builder::new()
                .prefix(".flameshot-startup-")
                .tempdir_in(path.parent().unwrap())?;
            std::fs::hard_link(&path, directory.path().join("entry"))?;
            Some(directory)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => return Err(error.into()),
    };
    set_autostart(config_home, enabled)?;
    if let Err(error) = commit() {
        let rollback = match backup {
            Some(directory) => match std::fs::rename(directory.path().join("entry"), &path) {
                Ok(()) => Ok(()),
                Err(error) => {
                    let retained = directory.keep();
                    Err(anyhow::Error::from(error)
                        .context(format!("Startup backup retained at {}", retained.display())))
                }
            },
            None => set_autostart(config_home, false),
        };
        if let Err(rollback_error) = rollback {
            return Err(error.context(format!(
                "Also failed to restore startup entry: {rollback_error:#}"
            )));
        }
        return Err(error);
    }
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
/// Enqueue a notification without waiting for the desktop service to handle it.
pub async fn notify_no_reply(connection: &zbus::Connection, title: &str, body: &str) -> Result<()> {
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
            .call_noreply(
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
    .context("Notification transport timed out")?
    .map_err(Into::into)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn failed_rollback_keeps_the_original_for_recovery() {
        let root = tempfile::tempdir().unwrap();
        let folder = root.path().join("autostart");
        std::fs::create_dir(&folder).unwrap();
        let path = folder.join("Flameshot.desktop");
        std::fs::write(&path, b"original").unwrap();
        let error = set_autostart_with_commit(root.path(), false, || {
            std::fs::create_dir(&path)?;
            anyhow::bail!("INI failed")
        })
        .unwrap_err();
        assert!(error.to_string().contains("Startup backup retained"));
        assert!(error.to_string().contains("restore startup entry"));
        let backup = std::fs::read_dir(&folder)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .find(|path| {
                path.file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with(".flameshot-startup-")
            })
            .unwrap();
        assert_eq!(std::fs::read(backup.join("entry")).unwrap(), b"original");
    }
    #[test]
    fn failed_commit_restores_custom_entry_permissions_and_symlinks() {
        use std::os::unix::fs::{PermissionsExt, symlink};
        let root = tempfile::tempdir().unwrap();
        let directory = root.path().join("autostart");
        std::fs::create_dir(&directory).unwrap();
        let path = directory.join("Flameshot.desktop");
        for enabled in [false, true] {
            std::fs::write(&path, b"custom desktop entry\n").unwrap();
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o640)).unwrap();
            assert!(
                set_autostart_with_commit(root.path(), enabled, || anyhow::bail!("INI failed"))
                    .is_err()
            );
            assert_eq!(std::fs::read(&path).unwrap(), b"custom desktop entry\n");
            assert_eq!(path.metadata().unwrap().permissions().mode() & 0o777, 0o640);
            std::fs::remove_file(&path).unwrap();
            symlink("custom-target", &path).unwrap();
            assert!(
                set_autostart_with_commit(root.path(), enabled, || anyhow::bail!("INI failed"))
                    .is_err()
            );
            assert_eq!(
                std::fs::read_link(&path).unwrap(),
                Path::new("custom-target")
            );
            std::fs::remove_file(&path).unwrap();
        }
        assert!(
            set_autostart_with_commit(root.path(), true, || anyhow::bail!("INI failed")).is_err()
        );
        assert!(!path.exists());
        set_autostart_with_commit(root.path(), true, || Ok(())).unwrap();
        assert!(path.is_file());
    }
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
