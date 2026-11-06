// SPDX-License-Identifier: GPL-3.0-or-later
use anyhow::{Context, Result, bail, ensure};
use image::{ImageFormat, RgbaImage};
use std::{
    fs::OpenOptions,
    io::{Cursor, Write},
    path::{Path, PathBuf},
};

pub fn validate_destination(path: &Path) -> Result<()> {
    ensure!(!path.as_os_str().is_empty(), "Empty output path");
    if path.is_dir() {
        return Ok(());
    }
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    ensure!(
        parent.is_dir(),
        "Output parent directory does not exist: {}",
        parent.display()
    );
    format_for(path)?;
    Ok(())
}
fn format_for(path: &Path) -> Result<ImageFormat> {
    match path
        .extension()
        .and_then(|s| s.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        None | Some("png") => Ok(ImageFormat::Png),
        Some("jpg" | "jpeg") => Ok(ImageFormat::Jpeg),
        _ => bail!("Unsupported image extension: {}", path.display()),
    }
}
pub fn encode(image: &RgbaImage, format: ImageFormat) -> Result<Vec<u8>> {
    encode_quality(image, format, 75)
}
fn encode_quality(image: &RgbaImage, format: ImageFormat, quality: u8) -> Result<Vec<u8>> {
    let mut output = Cursor::new(Vec::new());
    if format == ImageFormat::Jpeg {
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut output, quality)
            .encode_image(&image::DynamicImage::ImageRgba8(image.clone()).to_rgb8())?;
    } else {
        image.write_to(&mut output, format)?;
    }
    Ok(output.into_inner())
}
#[cfg(test)]
pub fn save(image: &RgbaImage, path: &Path) -> Result<PathBuf> {
    save_with_settings(image, path, &crate::settings::Settings::default())
}
pub fn save_with_settings(
    image: &RgbaImage,
    path: &Path,
    settings: &crate::settings::Settings,
) -> Result<PathBuf> {
    validate_destination(path)?;
    let mut destination = if path.is_dir() {
        path.join(format!(
            "{}.{}",
            chrono::Local::now().format(&settings.filename_pattern),
            settings.extension
        ))
    } else {
        path.to_owned()
    };
    if destination.extension().is_none() {
        destination.set_extension(&settings.extension);
    }
    let data = encode_quality(image, format_for(&destination)?, settings.jpeg_quality)?;
    let stem = destination
        .file_stem()
        .context("Missing filename")?
        .to_os_string();
    let extension = destination
        .extension()
        .context("Missing extension")?
        .to_os_string();
    for index in 0_u64.. {
        let candidate = if index == 0 {
            destination.clone()
        } else {
            let mut name = stem.clone();
            name.push(format!("_{index}."));
            name.push(&extension);
            destination.with_file_name(name)
        };
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidate)
        {
            Ok(mut file) => {
                if let Err(error) = file.write_all(&data).and_then(|()| file.sync_all()) {
                    drop(file);
                    let _ = std::fs::remove_file(&candidate);
                    return Err(error.into());
                }
                return Ok(candidate);
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error.into()),
        }
    }
    bail!("No available output filename")
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn saves_correct_formats_and_preserves_existing_file() {
        let temp = tempfile::tempdir().unwrap();
        let image = RgbaImage::from_pixel(3, 2, image::Rgba([255, 20, 30, 255]));
        let first = save(&image, &temp.path().join("截图")).unwrap();
        let bytes = std::fs::read(&first).unwrap();
        assert_eq!(image::load_from_memory(&bytes).unwrap().to_rgba8(), image);
        let second = save(&image, &temp.path().join("截图")).unwrap();
        assert_eq!(second.file_name().unwrap(), "截图_1.png");
        assert_eq!(std::fs::read(first).unwrap(), bytes);
        let jpg = save(&image, &temp.path().join("image.JPG")).unwrap();
        assert_eq!(
            image::guess_format(&std::fs::read(jpg).unwrap()).unwrap(),
            ImageFormat::Jpeg
        );
    }
    #[test]
    fn invalid_destination_does_not_create_directories() {
        let temp = tempfile::tempdir().unwrap();
        assert!(validate_destination(&temp.path().join("missing/image.png")).is_err());
        assert!(!temp.path().join("missing").exists());
        assert!(validate_destination(&temp.path().join("image.unknown")).is_err());
    }
}
