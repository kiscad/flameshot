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
    run_launcher(
        Command::new("gio")
            .arg("launch")
            .arg(&app.desktop_file)
            .arg(file),
    )
}
/// Force terminal execution through GIO, retaining its field-code expansion.
/// The original desktop entry is never changed. D-Bus activation is disabled in
/// this temporary entry because it would bypass the requested terminal.
pub fn launch_in_terminal(app: &Application, file: &Path) -> Result<()> {
    let entry = terminal_entry(&app.desktop_file)?;
    run_launcher(
        Command::new("gio")
            .arg("launch")
            .arg(entry.path())
            .arg(file),
    )
}
fn terminal_entry(path: &Path) -> Result<tempfile::NamedTempFile> {
    use std::io::Write;
    let path = std::path::absolute(path)?;
    let source = std::fs::read_to_string(&path).context("Cannot read application desktop entry")?;
    let original = path
        .to_str()
        .context("Application desktop path is not UTF-8")?;
    // Quote an Exec argument, then apply the outer Desktop Entry string escaping.
    let mut argument = String::from("\"");
    for ch in original.chars() {
        match ch {
            '%' => argument.push_str("%%"),
            '"' | '`' | '$' | '\\' => {
                argument.push('\\');
                argument.push(ch);
            }
            _ => argument.push(ch),
        }
    }
    argument.push('"');
    let argument = argument
        .replace('\\', "\\\\")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t");
    let mut output = String::new();
    let mut main = false;
    let mut found = false;
    let mut has_exec = false;
    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            main = trimmed == "[Desktop Entry]";
            if main {
                ensure!(!found, "Duplicate Desktop Entry group");
                found = true;
                output.push_str("[Desktop Entry]\nTerminal=true\nDBusActivatable=false\n");
                continue;
            }
        }
        if main && let Some((key, value)) = line.split_once('=') {
            match key.trim() {
                "Terminal" | "DBusActivatable" => continue,
                "Exec" => {
                    ensure!(!has_exec, "Duplicate application Exec key");
                    ensure!(
                        !value.trim().is_empty(),
                        "Application Exec command is empty"
                    );
                    has_exec = true;
                    output.push_str("Exec=");
                    let mut chars = value.chars();
                    while let Some(ch) = chars.next() {
                        if ch == '%' {
                            match chars.next() {
                                Some('k') => output.push_str(&argument),
                                Some(code) => {
                                    output.push('%');
                                    output.push(code);
                                }
                                None => output.push('%'),
                            }
                        } else {
                            output.push(ch);
                        }
                    }
                    output.push('\n');
                    continue;
                }
                _ => {}
            }
        }
        output.push_str(line);
        output.push('\n');
    }
    ensure!(
        found && has_exec,
        "Application has no Exec command for terminal launch"
    );
    let mut entry = tempfile::Builder::new()
        .prefix("flameshot-terminal-")
        .suffix(".desktop")
        .tempfile()?;
    entry.write_all(output.as_bytes())?;
    entry.flush()?;
    Ok(entry)
}
/// Open a local directory with the desktop's configured file manager. Resolve
/// relative paths before dispatch so names cannot be interpreted as URI schemes
/// or command options. The external file manager is not owned by Flameshot.
pub fn open_directory(directory: &Path) -> Result<()> {
    let directory = std::path::absolute(directory)?;
    ensure!(
        directory.is_dir(),
        "Save directory does not exist: {}",
        directory.display()
    );
    run_launcher(Command::new("gio").arg("open").arg(directory))
}
fn run_launcher(command: &mut Command) -> Result<()> {
    let mut child = command
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
    fn forced_terminal_preserves_field_expansion_and_original_entry() {
        use std::os::unix::fs::PermissionsExt;
        let root = tempfile::tempdir().unwrap();
        let bin = root.path().join("bin");
        std::fs::create_dir(&bin).unwrap();
        let terminal = bin.join("gnome-terminal");
        std::fs::write(&terminal, "#!/usr/bin/python3\nimport os,sys\nwith open(os.environ['FLAMESHOT_TERMINAL_RECEIPT'], 'wb') as f:\n f.write(b'\\0'.join(os.fsencode(v) for v in sys.argv[1:]))\n").unwrap();
        std::fs::set_permissions(&terminal, std::fs::Permissions::from_mode(0o755)).unwrap();
        let receipt = root.path().join("receipt");
        let desktop = root.path().join("entry %k $ ` \\\".desktop");
        let original = "[Desktop Entry]\nType=Application\nName=Test Viewer\nIcon=test-icon\nExec=/bin/echo %f %k %c %i %%k\nTerminal=false\nDBusActivatable=true\n\n[Desktop Action Example]\nExec=unused %k\nTerminal=false\n";
        std::fs::write(&desktop, original).unwrap();
        let image = root.path().join("image $(literal) space.png");
        std::fs::write(&image, b"synthetic").unwrap();
        let staged = terminal_entry(&desktop).unwrap();
        let text = std::fs::read_to_string(staged.path()).unwrap();
        assert!(text.contains("[Desktop Action Example]\nExec=unused %k\nTerminal=false"));
        run_launcher(
            Command::new("/usr/bin/gio")
                .arg("launch")
                .arg(staged.path())
                .arg(&image)
                .env("PATH", &bin)
                .env("FLAMESHOT_TERMINAL_RECEIPT", &receipt),
        )
        .unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while !receipt.exists() {
            assert!(
                std::time::Instant::now() < deadline,
                "fake terminal was not started"
            );
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        let bytes = std::fs::read(receipt).unwrap();
        let args: Vec<_> = bytes
            .split(|byte| *byte == 0)
            .map(|value| String::from_utf8(value.to_vec()).unwrap())
            .collect();
        let command = args.iter().position(|arg| arg == "/bin/echo").unwrap();
        assert_eq!(
            &args[command + 1..],
            &[
                image.to_str().unwrap(),
                desktop.to_str().unwrap(),
                "Test Viewer",
                "--icon",
                "test-icon",
                "%k"
            ]
        );
        assert_eq!(std::fs::read_to_string(&desktop).unwrap(), original);
        // An empty PATH must report a failed launch, not silently bypass Terminal.
        std::fs::remove_file(terminal).unwrap();
        assert!(
            run_launcher(
                Command::new("/usr/bin/gio")
                    .arg("launch")
                    .arg(staged.path())
                    .arg(&image)
                    .env("PATH", &bin)
            )
            .is_err()
        );
    }
    #[test]
    fn terminal_override_requires_an_exec_command() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("test.desktop");
        for source in [
            "[Desktop Entry]\nType=Application\nDBusActivatable=true\n",
            "[Desktop Entry]\nExec=\n",
            "[Other]\nExec=test\n",
        ] {
            std::fs::write(&path, source).unwrap();
            assert!(terminal_entry(&path).is_err());
            assert_eq!(std::fs::read_to_string(&path).unwrap(), source);
        }
    }
    #[test]
    fn gio_launch_keeps_spaced_paths_and_literal_metacharacters_intact() {
        let root = tempfile::tempdir().unwrap();
        let receipt = root.path().join("received.txt");
        let script = root.path().join("receiver script.py");
        std::fs::write(&script,format!("import pathlib,sys\npathlib.Path({:?}).write_text(str(len(sys.argv)-1)+'\\n'+sys.argv[1])\n",receipt.to_str().unwrap())).unwrap();
        let desktop = root.path().join("receiver.desktop");
        std::fs::write(&desktop,format!("[Desktop Entry]\nType=Application\nName=Test\nExec=python3 \"{}\" %f\nTerminal=false\n",script.display())).unwrap();
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
