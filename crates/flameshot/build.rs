// SPDX-License-Identifier: GPL-3.0-or-later
use std::{
    path::Path,
    process::{Command, Stdio},
};
fn output(root: &Path, args: &[&str]) -> Option<String> {
    let result = Command::new("git")
        .current_dir(root)
        .args(args)
        .stderr(Stdio::null())
        .output()
        .ok()?;
    result
        .status
        .success()
        .then(|| String::from_utf8_lossy(&result.stdout).trim().to_owned())
}
fn main() {
    println!("cargo:rerun-if-env-changed=GIT_HASH");
    println!("cargo:rerun-if-env-changed=RUSTC");
    let manifest = std::env::var_os("CARGO_MANIFEST_DIR").unwrap();
    let root = Path::new(&manifest).join("../..").canonicalize().unwrap();
    let archive_revision = root.join(".git-version");
    if archive_revision.exists() {
        println!("cargo:rerun-if-changed={}", archive_revision.display());
    }
    let revision = std::env::var("GIT_HASH")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .or_else(|| std::fs::read_to_string(root.join(".git-version")).ok())
        .or_else(|| {
            let top = output(&root, &["rev-parse", "--show-toplevel"])?;
            if Path::new(&top).canonicalize().ok()? != root {
                return None;
            }
            if let Some(directory) = output(&root, &["rev-parse", "--git-common-dir"]) {
                println!("cargo:rerun-if-changed={}", root.join(directory).display());
            }
            if let Some(directory) = output(&root, &["rev-parse", "--absolute-git-dir"]) {
                println!("cargo:rerun-if-changed={directory}");
            }
            output(&root, &["rev-parse", "--short", "HEAD"])
        })
        .unwrap_or_else(|| "-".into());
    let compiler = Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()))
        .arg("--version")
        .output()
        .ok()
        .filter(|r| r.status.success())
        .map(|r| String::from_utf8_lossy(&r.stdout).trim().to_owned())
        .unwrap_or_else(|| "Rust".into());
    println!(
        "cargo:rustc-env=FLAMESHOT_REVISION={}",
        revision.lines().next().unwrap_or("-").trim()
    );
    println!(
        "cargo:rustc-env=FLAMESHOT_COMPILER={}",
        compiler.lines().next().unwrap_or("Rust").trim()
    );
}
