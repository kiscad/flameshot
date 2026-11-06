// SPDX-License-Identifier: GPL-3.0-or-later
use anyhow::Result;
use flameshot_core::region::Region;
use std::io::Write;
fn path() -> Option<std::path::PathBuf> {
    dirs::cache_dir().map(|p| p.join("flameshot/flameshot/region.txt"))
}
pub fn load() -> Result<Option<Region>> {
    let Some(path) = path() else {
        return Ok(None);
    };
    match std::fs::read(path) {
        Ok(data) => Ok(Some(Region::decode(&data)?)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}
pub fn save(region: Region) -> Result<()> {
    let Some(path) = path() else {
        return Ok(());
    };
    let parent = path.parent().unwrap();
    std::fs::create_dir_all(parent)?;
    let mut file = tempfile::NamedTempFile::new_in(parent)?;
    file.write_all(&region.encode()?)?;
    file.as_file().sync_all()?;
    file.persist(path).map_err(|e| e.error)?;
    Ok(())
}
