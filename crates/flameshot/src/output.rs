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
pub const EXTENSIONS: &[&str] = &[
    "png", "jpg", "jpeg", "bmp", "ico", "cur", "pbm", "pgm", "ppm", "xbm", "xpm", "tif", "tiff",
    "webp", "gif",
];
enum Format {
    Raster(ImageFormat),
    Pbm,
    Pgm,
    Ppm,
    Xbm,
    Xpm,
    Cursor,
}
fn format_for(path: &Path) -> Result<Format> {
    Ok(
        match path
            .extension()
            .and_then(|s| s.to_str())
            .map(str::to_ascii_lowercase)
            .as_deref()
        {
            None | Some("png") => Format::Raster(ImageFormat::Png),
            Some("jpg" | "jpeg") => Format::Raster(ImageFormat::Jpeg),
            Some("bmp") => Format::Raster(ImageFormat::Bmp),
            Some("ico") => Format::Raster(ImageFormat::Ico),
            Some("cur") => Format::Cursor,
            Some("pbm") => Format::Pbm,
            Some("pgm") => Format::Pgm,
            Some("ppm") => Format::Ppm,
            Some("xbm") => Format::Xbm,
            Some("xpm") => Format::Xpm,
            Some("tif" | "tiff") => Format::Raster(ImageFormat::Tiff),
            Some("webp") => Format::Raster(ImageFormat::WebP),
            Some("gif") => Format::Raster(ImageFormat::Gif),
            _ => bail!("Unsupported image extension: {}", path.display()),
        },
    )
}
fn gray(p: &image::Rgba<u8>) -> u8 {
    ((u32::from(p[0]) * 299 + u32::from(p[1]) * 587 + u32::from(p[2]) * 114) / 1000) as u8
}
fn encode_file(image: &RgbaImage, format: Format, quality: u8) -> Result<Vec<u8>> {
    ensure!(
        image.width() > 0 && image.height() > 0,
        "Cannot encode an empty image"
    );
    match format {
        Format::Raster(format) => encode_quality(image, format, quality),
        Format::Cursor => {
            let mut bytes = encode_quality(image, ImageFormat::Ico, quality)?;
            bytes[2..4].copy_from_slice(&2_u16.to_le_bytes());
            bytes[10..14].fill(0);
            Ok(bytes)
        }
        Format::Ppm | Format::Pgm => {
            let color = matches!(format, Format::Ppm);
            let mut bytes = format!(
                "{}\n{} {}\n255\n",
                if color { "P6" } else { "P5" },
                image.width(),
                image.height()
            )
            .into_bytes();
            for p in image.pixels() {
                if color {
                    bytes.extend_from_slice(&p.0[..3]);
                } else {
                    bytes.push(gray(p));
                }
            }
            Ok(bytes)
        }
        Format::Pbm | Format::Xbm => {
            let xbm = matches!(format, Format::Xbm);
            let mut packed = Vec::new();
            for y in 0..image.height() {
                for block in 0..image.width().div_ceil(8) {
                    let mut byte = 0;
                    for bit in 0..8 {
                        let x = block * 8 + bit;
                        if x < image.width() && gray(image.get_pixel(x, y)) < 128 {
                            byte |= 1 << if xbm { bit } else { 7 - bit };
                        }
                    }
                    packed.push(byte);
                }
            }
            if xbm {
                let mut text = format!(
                    "#define capture_width {}\n#define capture_height {}\nstatic unsigned char capture_bits[] = {{\n",
                    image.width(),
                    image.height()
                );
                for byte in packed {
                    text.push_str(&format!("0x{byte:02x},"));
                }
                text.push_str("\n};\n");
                Ok(text.into_bytes())
            } else {
                let mut bytes = format!("P4\n{} {}\n", image.width(), image.height()).into_bytes();
                bytes.extend(packed);
                Ok(bytes)
            }
        }
        Format::Xpm => {
            let mut palette = std::collections::HashMap::new();
            let mut colors = Vec::new();
            let mut pixels = Vec::new();
            for p in image.pixels() {
                let key = if p[3] < 128 {
                    [0, 0, 0, 0]
                } else {
                    [p[0], p[1], p[2], 255]
                };
                let index = *palette.entry(key).or_insert_with(|| {
                    colors.push(key);
                    colors.len() - 1
                });
                pixels.push(index);
            }
            let mut cpp = 1;
            let mut capacity = 64;
            while capacity < colors.len() {
                cpp += 1;
                capacity *= 64;
            }
            let code = |mut index: usize| {
                let alphabet = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789_.";
                let mut bytes = vec![b'A'; cpp];
                for byte in bytes.iter_mut().rev() {
                    *byte = alphabet[index % 64];
                    index /= 64;
                }
                String::from_utf8(bytes).unwrap()
            };
            let mut text = format!(
                "/* XPM */\nstatic const char *capture_xpm[] = {{\n\"{} {} {} {}\",\n",
                image.width(),
                image.height(),
                colors.len(),
                cpp
            );
            for (index, color) in colors.iter().enumerate() {
                let value = if color[3] == 0 {
                    "None".into()
                } else {
                    format!("#{:02X}{:02X}{:02X}", color[0], color[1], color[2])
                };
                text.push_str(&format!("\"{} c {value}\",\n", code(index)));
            }
            for row in pixels.chunks(image.width() as usize) {
                text.push('"');
                for index in row {
                    text.push_str(&code(*index));
                }
                text.push_str("\",\n");
            }
            text.push_str("};\n");
            Ok(text.into_bytes())
        }
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
    let data = encode_file(image, format_for(&destination)?, settings.jpeg_quality)?;
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
    fn supported_formats_encode_and_raster_codecs_decode() {
        let image = RgbaImage::from_fn(16, 12, |x, y| {
            image::Rgba([(x * 15) as u8, (y * 20) as u8, 100, 255])
        });
        let temp = tempfile::tempdir().unwrap();
        for extension in EXTENSIONS {
            let path = temp.path().join(format!("capture.{extension}"));
            let data = encode_file(&image, format_for(&path).unwrap(), 75).unwrap();
            if !matches!(*extension, "cur" | "xbm" | "xpm") {
                let decoded = image::load_from_memory(&data).unwrap();
                assert_eq!((decoded.width(), decoded.height()), (16, 12), "{extension}");
                if matches!(
                    *extension,
                    "png" | "bmp" | "ico" | "ppm" | "tif" | "tiff" | "webp"
                ) {
                    assert_eq!(decoded.to_rgba8(), image, "{extension}");
                }
            }
            if let Some(directory) = std::env::var_os("FLAMESHOT_FORMAT_ARTIFACT_DIR") {
                std::fs::create_dir_all(&directory).unwrap();
                std::fs::write(
                    PathBuf::from(directory).join(format!("capture.{extension}")),
                    data,
                )
                .unwrap();
            }
        }
        assert!(encode_file(&RgbaImage::new(0, 0), Format::Xpm, 75).is_err());
    }
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
