// SPDX-License-Identifier: GPL-3.0-or-later
//! Aspect-preserving previews with pixel-rounded dimensions inside a bounding box.
pub fn thumbnail(image: &image::RgbaImage, width: u32, height: u32) -> image::RgbaImage {
    if image.width() == 0 || image.height() == 0 || width == 0 || height == 0 {
        return image::RgbaImage::new(0, 0);
    }
    let source_width = u64::from(image.width());
    let source_height = u64::from(image.height());
    let (width, height) = if source_width * u64::from(height) > source_height * u64::from(width) {
        let scaled = (source_height * u64::from(width) + source_width / 2) / source_width;
        (width, scaled.clamp(1, u64::from(height)) as u32)
    } else {
        let scaled = (source_width * u64::from(height) + source_height / 2) / source_height;
        (scaled.clamp(1, u64::from(width)) as u32, height)
    };
    image::imageops::resize(image, width, height, image::imageops::FilterType::Triangle)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn previews_fit_without_stretching_and_keep_small_or_thin_images_valid() {
        for (source, expected) in [
            ((1000, 500), (200, 100)),
            ((100, 2000), (5, 100)),
            ((64, 48), (133, 100)),
            ((10000, 1), (250, 1)),
        ] {
            let image =
                image::RgbaImage::from_pixel(source.0, source.1, image::Rgba([32, 64, 96, 255]));
            let result = thumbnail(&image, 250, 100);
            assert_eq!(result.dimensions(), expected);
            assert!(result.pixels().all(|pixel| pixel.0 == [32, 64, 96, 255]));
        }
        assert_eq!(
            thumbnail(&image::RgbaImage::new(0, 10), 250, 100).dimensions(),
            (0, 0)
        );
        assert_eq!(
            thumbnail(&image::RgbaImage::new(10, 10), 0, 100).dimensions(),
            (0, 0)
        );
    }
}
