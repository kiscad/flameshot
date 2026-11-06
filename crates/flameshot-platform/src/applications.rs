// SPDX-License-Identifier: GPL-3.0-or-later
use anyhow::{Context, Result, ensure};
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};
#[derive(Debug, Clone)]
pub struct Application {
    pub name: String,
    pub desktop_file: PathBuf,
    pub supports_png: bool,
}
pub fn list() -> Vec<Application> {
    from_paths(freedesktop_desktop_entry::default_paths().collect())
}
fn from_paths(paths: Vec<PathBuf>) -> Vec<Application> {
    let languages = freedesktop_desktop_entry::get_languages_from_env();
    let mut seen = HashSet::new();
    let mut result = Vec::new();
    for entry in freedesktop_desktop_entry::Iter::new(paths.into_iter()).entries(Some(&languages)) {
        if !seen.insert(entry.appid.clone())
            || entry.hidden()
            || entry.no_display()
            || entry.desktop_entry("Type") != Some("Application")
        {
            continue;
        }
        let Some(name) = entry.name(&languages).map(|n| n.into_owned()) else {
            continue;
        };
        result.push(Application {
            name,
            desktop_file: entry.path.clone(),
            supports_png: entry
                .mime_type()
                .is_some_and(|types| types.contains(&"image/png")),
        });
    }
    result.sort_by(|a, b| (!a.supports_png, &a.name).cmp(&(!b.supports_png, &b.name)));
    result
}
/// GIO performs Desktop Entry field-code expansion, D-Bus activation and terminal
/// dispatch using the desktop's standard application launcher. No shell is used.
pub fn launch(app: &Application, file: &Path) -> Result<()> {
    let mut child = Command::new("gio")
        .arg("launch")
        .arg(&app.desktop_file)
        .arg(file)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .context("Unable to run the system application launcher (gio)")?;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if std::time::Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            anyhow::bail!("Application launcher timed out");
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    };
    ensure!(
        status.success(),
        "The selected application could not be launched ({status})"
    );
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn gio_launch_keeps_spaced_paths_and_literal_metacharacters_intact() {
        let root = tempfile::tempdir().unwrap();
        let receipt = root.path().join("received.txt");
        let script = root.path().join("receiver script.py");
        std::fs::write(&script,format!("import pathlib,sys\npathlib.Path({:?}).write_text(str(len(sys.argv)-1)+'\\n'+sys.argv[1])\n",receipt.to_str().unwrap())).unwrap();
        let desktop = root.path().join("receiver.desktop");
        std::fs::write(&desktop,format!("[Desktop Entry]\nType=Application\nName=Test\nExec=/usr/bin/python3 \"{}\" %f\nTerminal=false\n",script.display())).unwrap();
        let image = root.path().join("space $(literal).png");
        std::fs::write(&image, b"synthetic").unwrap();
        launch(
            &Application {
                name: "Test".into(),
                desktop_file: desktop,
                supports_png: true,
            },
            &image,
        )
        .unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while !receipt.exists() {
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        assert_eq!(
            std::fs::read_to_string(receipt).unwrap(),
            format!("1\n{}", image.display())
        );
    }
    #[test]
    fn user_entries_override_system_entries_and_hidden_apps_stay_hidden() {
        let root = tempfile::tempdir().unwrap();
        let user = root.path().join("user");
        let system = root.path().join("system");
        std::fs::create_dir(&user).unwrap();
        std::fs::create_dir(&system).unwrap();
        std::fs::write(
            system.join("viewer.desktop"),
            "[Desktop Entry]\nType=Application\nName=System\nExec=viewer %f\n",
        )
        .unwrap();
        std::fs::write(user.join("viewer.desktop"),"[Desktop Entry]\nType=Application\nName=User Viewer\nExec=viewer %f\nMimeType=image/png;\n").unwrap();
        std::fs::write(
            user.join("hidden.desktop"),
            "[Desktop Entry]\nType=Application\nName=Hidden\nExec=viewer\nHidden=true\n",
        )
        .unwrap();
        let apps = from_paths(vec![user, system]);
        assert_eq!(apps.len(), 1);
        assert_eq!(apps[0].name, "User Viewer");
        assert!(apps[0].supports_png);
    }
}
