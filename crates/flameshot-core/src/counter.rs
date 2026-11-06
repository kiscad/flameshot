// SPDX-License-Identifier: GPL-3.0-or-later
use crate::{
    editor::{EditError, Point},
    text::TextRenderer,
};
use image::{Pixel, Rgba, RgbaImage};
use tiny_skia::{FillRule, Paint, PathBuilder, Pixmap, Stroke, Transform};
pub(crate) fn draw(
    image: &mut RgbaImage,
    center: Point,
    tip: Point,
    radius: f32,
    color: [u8; 4],
    outline: bool,
) -> Result<[u8; 4], EditError> {
    let mut layer = Pixmap::new(image.width(), image.height()).ok_or(EditError::InvalidImage)?;
    let mut paint = Paint::default();
    paint.set_color_rgba8(color[0], color[1], color[2], color[3]);
    paint.anti_alias = true;
    let dx = tip.x - center.x;
    let dy = tip.y - center.y;
    let length = dx.hypot(dy);
    if length > radius {
        let nx = -dy / length * radius;
        let ny = dx / length * radius;
        let mut path = PathBuilder::new();
        path.move_to(center.x + nx, center.y + ny);
        path.line_to(tip.x, tip.y);
        path.line_to(center.x - nx, center.y - ny);
        path.close();
        if let Some(path) = path.finish() {
            layer.fill_path(
                &path,
                &paint,
                FillRule::Winding,
                Transform::identity(),
                None,
            );
        }
    }
    let luminance =
        (u32::from(color[0]) * 299 + u32::from(color[1]) * 587 + u32::from(color[2]) * 114) / 1000;
    let contrast = if luminance < 128 {
        [255, 255, 255, 255]
    } else {
        [0, 0, 0, 255]
    };
    if outline {
        let mut path = PathBuilder::new();
        path.push_circle(center.x, center.y, radius + 2.0);
        if let Some(path) = path.finish() {
            let mut border = Paint::default();
            border.set_color_rgba8(255 - contrast[0], 255 - contrast[1], 255 - contrast[2], 255);
            layer.fill_path(
                &path,
                &border,
                FillRule::Winding,
                Transform::identity(),
                None,
            );
            border.set_color_rgba8(contrast[0], contrast[1], contrast[2], 255);
            layer.stroke_path(
                &path,
                &border,
                &Stroke {
                    width: 1.0,
                    ..Default::default()
                },
                Transform::identity(),
                None,
            );
        }
    }
    let mut path = PathBuilder::new();
    path.push_circle(center.x, center.y, radius);
    if let Some(path) = path.finish() {
        layer.fill_path(
            &path,
            &paint,
            FillRule::Winding,
            Transform::identity(),
            None,
        );
    }
    for (pixel, source) in image.pixels_mut().zip(layer.pixels()) {
        let source = source.demultiply();
        pixel.blend(&Rgba([
            source.red(),
            source.green(),
            source.blue(),
            source.alpha(),
        ]));
    }
    Ok(contrast)
}
pub(crate) fn label(
    renderer: &mut TextRenderer,
    image: &mut RgbaImage,
    center: Point,
    number: u16,
    radius: f32,
    color: [u8; 4],
) {
    renderer.number(image, center, number, radius, color);
}
