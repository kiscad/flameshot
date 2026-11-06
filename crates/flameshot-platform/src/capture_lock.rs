// SPDX-License-Identifier: GPL-3.0-or-later
//! Kernel-owned capture lock: a crash cannot leave a permanently locked editor.
use anyhow::Result;
#[cfg(unix)]
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::{
    fs::{File, OpenOptions, TryLockError},
    path::Path,
};
pub struct CaptureLock(File);
impl Drop for CaptureLock {
    fn drop(&mut self) {
        // Explicit unlock also releases transient inherited descriptors while
        // another thread is between fork and exec during application dispatch.
        let _ = self.0.unlock();
    }
}
pub fn acquire(path: &Path) -> Result<Option<CaptureLock>> {
    if let Some(parent) = path.parent() {
        let mut builder = std::fs::DirBuilder::new();
        builder.recursive(true);
        #[cfg(unix)]
        builder.mode(0o700);
        builder.create(parent)?;
    }
    let mut options = OpenOptions::new();
    options.read(true).write(true).create(true).truncate(false);
    #[cfg(unix)]
    options.mode(0o600);
    let file = options.open(path)?;
    match file.try_lock() {
        Ok(()) => Ok(Some(CaptureLock(file))),
        Err(TryLockError::WouldBlock) => Ok(None),
        Err(TryLockError::Error(error)) => Err(error.into()),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exclusive_lock_releases_when_handle_closes_and_stale_file_is_reusable() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("runtime/gui.lock");
        let first = acquire(&path).unwrap().unwrap();
        assert!(acquire(&path).unwrap().is_none());
        drop(first);
        assert!(path.exists());
        assert!(acquire(&path).unwrap().is_some());
    }
}
