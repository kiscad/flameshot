// SPDX-License-Identifier: GPL-3.0-or-later
use crate::editor::Point;
use cosmic_text::{Attrs, Buffer, Color, Family, FontSystem, Metrics, Shaping, SwashCache, Wrap};
use image::{Pixel, Rgba, RgbaImage};

pub(crate) struct TextRenderer {
    fonts: FontSystem,
    cache: SwashCache,
}
impl TextRenderer {
    pub fn new() -> Self {
        Self {
            fonts: FontSystem::new(),
            cache: SwashCache::new(),
        }
    }
    pub fn number(
        &mut self,
        image: &mut RgbaImage,
        center: Point,
        number: u16,
        radius: f32,
        color: [u8; 4],
    ) {
        let text = number.to_string();
        let size = radius.min(radius * 1.4 / (text.len() as f32 * 0.7));
        let dimension = (radius * 4.0).ceil() as u32;
        let mut glyphs = RgbaImage::new(dimension, dimension);
        let mut buffer = Buffer::new(&mut self.fonts, Metrics::new(size, size * 1.25));
        let mut buffer = buffer.borrow_with(&mut self.fonts);
        buffer.set_size(None, Some(dimension as f32));
        buffer.set_text(
            &text,
            &Attrs::new()
                .family(Family::SansSerif)
                .weight(cosmic_text::Weight::BOLD),
            Shaping::Advanced,
            None,
        );
        buffer.draw(
            &mut self.cache,
            Color::rgba(color[0], color[1], color[2], 255),
            |x, y, w, h, c| {
                let (r, g, b, a) = c.as_rgba_tuple();
                for py in y.max(0) as u32
                    ..(i64::from(y) + i64::from(h)).clamp(0, i64::from(dimension)) as u32
                {
                    for px in x.max(0) as u32
                        ..(i64::from(x) + i64::from(w)).clamp(0, i64::from(dimension)) as u32
                    {
                        glyphs.get_pixel_mut(px, py).blend(&Rgba([r, g, b, a]));
                    }
                }
            },
        );
        let mut left = dimension;
        let mut top = dimension;
        let mut right = 0;
        let mut bottom = 0;
        for (x, y, p) in glyphs.enumerate_pixels() {
            if p[3] > 0 {
                left = left.min(x);
                top = top.min(y);
                right = right.max(x + 1);
                bottom = bottom.max(y + 1);
            }
        }
        if right <= left || bottom <= top {
            return;
        }
        let x = center.x.round() as i64 - i64::from(right - left) / 2;
        let y = center.y.round() as i64 - i64::from(bottom - top) / 2;
        let cropped =
            image::imageops::crop_imm(&glyphs, left, top, right - left, bottom - top).to_image();
        image::imageops::overlay(image, &cropped, x, y);
    }
    pub fn draw(
        &mut self,
        image: &mut RgbaImage,
        position: Point,
        text: &str,
        size: f32,
        family: &str,
        color: [u8; 4],
    ) {
        let mut buffer = Buffer::new(&mut self.fonts, Metrics::new(size, size * 1.25));
        let mut buffer = buffer.borrow_with(&mut self.fonts);
        buffer.set_size(None, Some(image.height() as f32 + position.y.abs()));
        buffer.set_wrap(Wrap::None);
        let family = if family.is_empty() {
            Family::SansSerif
        } else {
            Family::Name(family)
        };
        buffer.set_text(text, &Attrs::new().family(family), Shaping::Advanced, None);
        let opacity = color[3];
        buffer.draw(
            &mut self.cache,
            Color::rgba(color[0], color[1], color[2], 255),
            |x, y, w, h, color| {
                let (r, g, b, a) = color.as_rgba_tuple();
                let a = ((u16::from(a) * u16::from(opacity) + 127) / 255) as u8;
                let start_x = i64::from(x) + position.x.round() as i64;
                let start_y = i64::from(y) + position.y.round() as i64;
                for py in start_y.max(0)..(start_y + i64::from(h)).min(i64::from(image.height())) {
                    for px in start_x.max(0)..(start_x + i64::from(w)).min(i64::from(image.width()))
                    {
                        image
                            .get_pixel_mut(px as u32, py as u32)
                            .blend(&Rgba([r, g, b, a]));
                    }
                }
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        editor::{Annotation, Document, Point},
        geometry::PixelRect,
    };
    use image::RgbaImage;
    #[test]
    fn multiline_text_is_rasterized_exported_and_undoable() {
        let base = RgbaImage::new(300, 140);
        let mut doc = Document::new(base.clone(), 10).unwrap();
        doc.add(Annotation::Text {
            position: Point { x: 5.0, y: 5.0 },
            text: "Hello Rust\nSecond line".into(),
            font_size: 24.0,
            family: String::new(),
            color: [255, 0, 0, 128],
        })
        .unwrap();
        let rendered = doc.render().unwrap();
        assert!(
            rendered
                .enumerate_pixels()
                .any(|(_, y, p)| y < 30 && p[3] > 0),
            "System sans-serif font required"
        );
        assert!(
            rendered
                .enumerate_pixels()
                .any(|(_, y, p)| y > 35 && p[3] > 0)
        );
        assert!(rendered.pixels().all(|p| p[3] <= 128));
        assert_eq!(
            doc.export(PixelRect {
                x: 0,
                y: 0,
                width: 300,
                height: 140
            })
            .unwrap(),
            rendered
        );
        assert!(doc.undo());
        assert_eq!(doc.render().unwrap(), base);
        assert!(doc.redo());
        assert_eq!(doc.render().unwrap(), rendered);
    }
}
