// SPDX-License-Identifier: GPL-3.0-or-later
use anyhow::{Result, ensure};
use flameshot_platform::imgur::UploadedImage;
use std::{
    io::{Cursor, Write},
    path::{Path, PathBuf},
};
#[derive(Clone)]
pub struct Entry {
    pub path: PathBuf,
    pub link: String,
    pub deletehash: Option<String>,
}
pub fn directory() -> Result<PathBuf> {
    Ok(dirs::cache_dir()
        .ok_or_else(|| anyhow::anyhow!("No cache directory"))?
        .join("flameshot/history"))
}
fn token(text: &str) -> bool {
    !text.is_empty() && text.bytes().all(|b| b.is_ascii_alphanumeric())
}
pub fn list(directory: &Path) -> Result<Vec<Entry>> {
    if !directory.exists() {
        return Ok(vec![]);
    }
    let mut records = Vec::new();
    for file in std::fs::read_dir(directory)? {
        let file = file?;
        if !file.file_type()?.is_file() {
            continue;
        }
        let name = file.file_name();
        let name = name.to_string_lossy();
        let Some(value) = name.strip_prefix("imgur-") else {
            continue;
        };
        let (hash, image) = value
            .split_once('-')
            .map(|(hash, image)| (Some(hash), image))
            .unwrap_or((None, value));
        let Some((id, extension)) = image.rsplit_once('.') else {
            continue;
        };
        if !token(id)
            || !matches!(
                extension.to_ascii_lowercase().as_str(),
                "png" | "jpg" | "jpeg"
            )
            || hash.is_some_and(|h| !token(h))
        {
            continue;
        }
        records.push((
            file.metadata()?.modified()?,
            Entry {
                path: file.path(),
                link: format!("https://i.imgur.com/{image}"),
                deletehash: hash.map(str::to_owned),
            },
        ));
    }
    records.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.path.cmp(&b.1.path)));
    Ok(records.into_iter().map(|(_, record)| record).collect())
}
pub fn save(
    directory: &Path,
    uploaded: &UploadedImage,
    image: &image::RgbaImage,
    limit: usize,
) -> Result<()> {
    ensure!(
        token(&uploaded.id) && token(&uploaded.deletehash),
        "Invalid upload history identifiers"
    );
    std::fs::create_dir_all(directory)?;
    let thumbnail = flameshot_core::preview::thumbnail(image, 250, 100);
    let mut data = Cursor::new(Vec::new());
    thumbnail.write_to(&mut data, image::ImageFormat::Png)?;
    let mut file = tempfile::NamedTempFile::new_in(directory)?;
    file.write_all(data.get_ref())?;
    file.as_file().sync_all()?;
    file.persist(directory.join(format!("imgur-{}-{}.png", uploaded.deletehash, uploaded.id)))
        .map_err(|e| e.error)?;
    for entry in list(directory)?.into_iter().skip(limit) {
        std::fs::remove_file(entry.path)?;
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn legacy_history_and_thumbnail_retention_are_compatible() {
        let temp = tempfile::tempdir().unwrap();
        let image = image::RgbaImage::new(1000, 500);
        let uploaded = UploadedImage {
            id: "abc".into(),
            link: "https://i.imgur.com/abc.png".into(),
            deletehash: "secret".into(),
        };
        save(temp.path(), &uploaded, &image, 25).unwrap();
        let entries = list(temp.path()).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].deletehash.as_deref(), Some("secret"));
        assert_eq!(entries[0].link, uploaded.link);
        let thumbnail = image::open(&entries[0].path).unwrap();
        assert_eq!((thumbnail.width(), thumbnail.height()), (200, 100));
        std::fs::write(temp.path().join("unrelated.png"), "keep").unwrap();
        save(temp.path(), &uploaded, &image, 0).unwrap();
        assert!(list(temp.path()).unwrap().is_empty());
        assert!(temp.path().join("unrelated.png").exists());
    }
}
