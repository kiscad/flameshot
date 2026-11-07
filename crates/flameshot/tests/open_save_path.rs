// SPDX-License-Identifier: GPL-3.0-or-later
use std::{path::Path, process::Command, time::Duration};

fn run(root: &Path) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_flameshot"))
        .arg("internal-open-save-path")
        .current_dir(root)
        .env("XDG_CONFIG_HOME", root)
        .env("XDG_DATA_HOME", root.join("data"))
        .env("XDG_DATA_DIRS", root.join("empty"))
        .env(
            "DBUS_SESSION_BUS_ADDRESS",
            "unix:path=/nonexistent/flameshot-test-bus",
        )
        .env("GIO_USE_VFS", "local")
        .env_remove("DISPLAY")
        .env_remove("WAYLAND_DISPLAY")
        .output()
        .unwrap()
}

#[test]
fn configured_directory_uses_default_handler_and_preserves_literal_paths() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let applications = root.join("data/applications");
    std::fs::create_dir_all(&applications).unwrap();
    std::fs::create_dir(root.join("flameshot")).unwrap();
    std::fs::create_dir(root.join("empty")).unwrap();
    let receipt = root.join("received");
    let receiver = root.join("receiver.py");
    std::fs::write(&receiver, format!(
        "import pathlib,sys\np=pathlib.Path({:?})\np.with_suffix('.tmp').write_text(str(len(sys.argv)-1)+'\\n'+sys.argv[1])\np.with_suffix('.tmp').replace(p)\n", receipt.to_str().unwrap()
    )).unwrap();
    std::fs::write(applications.join("flameshot-test-folder.desktop"), format!(
        "[Desktop Entry]\nType=Application\nName=Isolated folder receiver\nExec=python3 \"{}\" %f\nMimeType=inode/directory;\nTerminal=false\n", receiver.display()
    )).unwrap();
    std::fs::write(
        root.join("mimeapps.list"),
        "[Default Applications]\ninode/directory=flameshot-test-folder.desktop;\n",
    )
    .unwrap();
    let config = root.join("flameshot/flameshot.ini");
    std::fs::write(&config, "[General]\ncheckForUpdates=false\n").unwrap();
    let output = run(root);
    assert!(output.status.success(), "{:?}", output.stderr);
    assert!(
        !receipt.exists(),
        "Unset savePath unexpectedly opened a directory"
    );
    for name in ["截图 space $(literal); & folder", "-relative:folder"] {
        let directory = root.join(name);
        std::fs::create_dir(&directory).unwrap();
        // Relative settings exercise resolution against the process directory.
        std::fs::write(
            &config,
            format!("[General]\ncheckForUpdates=false\nsavePath={name}\n"),
        )
        .unwrap();
        let output = run(root);
        assert!(output.status.success(), "{:?}", output.stderr);
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while !receipt.exists() {
            assert!(
                std::time::Instant::now() < deadline,
                "Default handler did not receive directory"
            );
            std::thread::sleep(Duration::from_millis(20));
        }
        assert_eq!(
            std::fs::read_to_string(&receipt).unwrap(),
            format!("1\n{}", directory.display())
        );
        std::fs::remove_file(&receipt).unwrap();
    }
    std::fs::write(&config, "[General]\nsavePath=missing-directory\n").unwrap();
    let output = run(root);
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stderr).contains("Save directory does not exist"));
    assert!(!receipt.exists());
}
