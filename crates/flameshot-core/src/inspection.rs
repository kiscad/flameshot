// SPDX-License-Identifier: GPL-3.0-or-later
use crate::geometry::PixelRect;
use image::RgbaImage;
pub struct Sample {
    pub rgba: [u8; 4],
    pub neighborhood: PixelRect,
    pub center: [u32; 2],
}
pub fn sample(image: &RgbaImage, x: u32, y: u32, radius: u32) -> Option<Sample> {
    if x >= image.width() || y >= image.height() {
        return None;
    }
    let radius = radius.min(128);
    let left = x.saturating_sub(radius);
    let top = y.saturating_sub(radius);
    let right = x
        .saturating_add(radius)
        .saturating_add(1)
        .min(image.width());
    let bottom = y
        .saturating_add(radius)
        .saturating_add(1)
        .min(image.height());
    Some(Sample {
        rgba: image.get_pixel(x, y).0,
        neighborhood: PixelRect {
            x: left,
            y: top,
            width: right - left,
            height: bottom - top,
        },
        center: [x - left, y - top],
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn samples_exact_rgba_and_clips_at_corners() {
        let image = RgbaImage::from_fn(10, 8, |x, y| image::Rgba([x as u8, y as u8, 90, 128]));
        let s = sample(&image, 9, 7, 2).unwrap();
        assert_eq!(s.rgba, [9, 7, 90, 128]);
        assert_eq!(s.center, [2, 2]);
        assert_eq!(
            s.neighborhood,
            PixelRect {
                x: 7,
                y: 5,
                width: 3,
                height: 3
            }
        );
        assert!(sample(&image, 10, 7, 2).is_none());
        assert_eq!(
            sample(&image, 0, 0, u32::MAX).unwrap().neighborhood.width,
            10
        );
    }
}
