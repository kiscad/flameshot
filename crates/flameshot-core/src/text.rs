// SPDX-License-Identifier: GPL-3.0-or-later
use crate::editor::{Point, TextAlignment, TextStyle};
use cosmic_text::{Attrs, Buffer, Color, Family, FontSystem, Metrics, Shaping, SwashCache, Wrap};
use image::{Pixel, Rgba, RgbaImage};

pub(crate) struct TextAppearance<'a> {
    pub size: f32,
    pub family: &'a str,
    pub color: [u8; 4],
    pub format: TextStyle,
}
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
        appearance: TextAppearance<'_>,
    ) {
        let TextAppearance {
            size,
            family,
            color,
            format,
        } = appearance;
        let mut buffer = Buffer::new(&mut self.fonts, Metrics::new(size, size * 1.25));
        let mut buffer = buffer.borrow_with(&mut self.fonts);
        buffer.set_size(None, Some(image.height() as f32 + position.y.abs()));
        buffer.set_wrap(Wrap::None);
        let family = if family.is_empty() {
            Family::SansSerif
        } else {
            Family::Name(family)
        };
        let mut attrs = Attrs::new()
            .family(family)
            .weight(if format.bold {
                cosmic_text::Weight::BOLD
            } else {
                cosmic_text::Weight::NORMAL
            })
            .style(if format.italic {
                cosmic_text::Style::Italic
            } else {
                cosmic_text::Style::Normal
            });
        if format.underline {
            attrs = attrs.underline(cosmic_text::UnderlineStyle::Single);
        }
        if format.strikeout {
            attrs = attrs.strikethrough();
        }
        buffer.set_text(
            text,
            &attrs,
            Shaping::Advanced,
            Some(cosmic_text::Align::Left),
        );
        if format.alignment != TextAlignment::Left {
            // Align shorter lines inside the annotation's natural widest line,
            // rather than against the capture canvas or a clipped viewport.
            buffer.set_size(None, None);
            let width = buffer
                .layout_runs()
                .map(|run| run.line_w)
                .fold(1.0_f32, f32::max);
            buffer.set_size(Some(width), Some(image.height() as f32 + position.y.abs()));
            let align = match format.alignment {
                TextAlignment::Left => cosmic_text::Align::Left,
                TextAlignment::Center => cosmic_text::Align::Center,
                TextAlignment::Right => cosmic_text::Align::Right,
            };
            buffer.set_text(text, &attrs, Shaping::Advanced, Some(align));
        }
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
    fn styled(format: crate::editor::TextStyle, text: &str) -> Document {
        let mut doc = Document::new(RgbaImage::new(360, 140), 10).unwrap();
        doc.add(Annotation::Text {
            position: Point { x: 10.0, y: 5.0 },
            text: text.into(),
            font_size: 32.0,
            family: "DejaVu Sans".into(),
            color: [255, 0, 0, 255],
            format,
        })
        .unwrap();
        doc
    }
    #[test]
    fn formatting_changes_pixels_and_alignment_uses_annotation_width() {
        use crate::editor::{TextAlignment, TextStyle};
        let normal = styled(TextStyle::default(), "MMMM\nI").render().unwrap();
        for format in [
            TextStyle {
                bold: true,
                ..Default::default()
            },
            TextStyle {
                italic: true,
                ..Default::default()
            },
            TextStyle {
                underline: true,
                ..Default::default()
            },
            TextStyle {
                strikeout: true,
                ..Default::default()
            },
        ] {
            assert_ne!(
                styled(format, "MMMM\nI").render().unwrap(),
                normal,
                "{format:?}"
            );
        }
        let mut starts = Vec::new();
        for alignment in [
            TextAlignment::Left,
            TextAlignment::Center,
            TextAlignment::Right,
        ] {
            let pixels = styled(
                TextStyle {
                    alignment,
                    ..Default::default()
                },
                "MMMM\nI",
            )
            .render()
            .unwrap();
            starts.push(
                pixels
                    .enumerate_pixels()
                    .filter(|(_, y, p)| *y >= 45 && p[3] > 0)
                    .map(|(x, _, _)| x)
                    .min()
                    .unwrap(),
            );
            assert_eq!(
                image::imageops::crop_imm(&pixels, 0, 0, 360, 40).to_image(),
                image::imageops::crop_imm(&normal, 0, 0, 360, 40).to_image()
            );
        }
        assert!(starts[0] < starts[1] && starts[1] < starts[2], "{starts:?}");
    }
    #[test]
    fn decorated_preview_export_picking_and_undo_share_pixels() {
        use crate::editor::TextStyle;
        let mut doc = styled(TextStyle::default(), "M     M");
        let plain = doc.render().unwrap();
        let mut annotation = doc.annotations()[0].clone();
        if let Annotation::Text { format, .. } = &mut annotation {
            format.underline = true;
            format.strikeout = true;
        }
        let preview = doc.preview_replacement(0, Some(&annotation)).unwrap();
        doc.replace(0, annotation).unwrap();
        assert_eq!(doc.render().unwrap(), preview);
        assert_eq!(
            doc.export(PixelRect {
                x: 0,
                y: 0,
                width: 360,
                height: 140
            })
            .unwrap(),
            preview
        );
        let (x, y, _) = preview
            .enumerate_pixels()
            .find(|(x, y, p)| {
                (55..75).contains(x) && *y < 45 && p[3] > 0 && plain.get_pixel(*x, *y)[3] == 0
            })
            .unwrap();
        assert_eq!(
            doc.hit_test(Point {
                x: x as f32,
                y: y as f32
            }),
            Some(0)
        );
        assert!(doc.undo());
        assert_eq!(doc.render().unwrap(), plain);
        assert!(doc.redo());
        assert_eq!(doc.render().unwrap(), preview);
    }
    #[test]
    fn multiline_text_is_rasterized_exported_and_undoable() {
        let base = RgbaImage::new(300, 140);
        let mut doc = Document::new(base.clone(), 10).unwrap();
        doc.add(Annotation::Text {
            position: Point { x: 5.0, y: 5.0 },
            text: "Hello Rust\nSecond line".into(),
            font_size: 24.0,
            family: String::new(),
            format: Default::default(),
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
