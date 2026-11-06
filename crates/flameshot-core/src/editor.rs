// SPDX-License-Identifier: GPL-3.0-or-later
//! Raster editor shared by preview and export. Coordinates are image pixels.
use crate::geometry::PixelRect;
use image::{Rgba, RgbaImage};
use tiny_skia::{FillRule, LineCap, Paint, PathBuilder, Pixmap, Stroke, Transform};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Style {
    pub color: [u8; 4],
    pub width: f32,
}
#[derive(Debug, Clone, PartialEq)]
pub enum Annotation {
    Counter {
        center: Point,
        tip: Point,
        number: u16,
        radius: f32,
        color: [u8; 4],
        outline: bool,
    },
    Marker {
        start: Point,
        end: Point,
        style: Style,
    },
    Text {
        position: Point,
        text: String,
        font_size: f32,
        family: String,
        color: [u8; 4],
    },
    Pencil {
        points: Vec<Point>,
        style: Style,
    },
    Line {
        start: Point,
        end: Point,
        style: Style,
    },
    Arrow {
        start: Point,
        end: Point,
        style: Style,
    },
    Rectangle {
        start: Point,
        end: Point,
        style: Style,
        filled: bool,
    },
    Ellipse {
        start: Point,
        end: Point,
        style: Style,
        filled: bool,
    },
    Invert(PixelRect),
    /// Replacement depends only on pixels OUTSIDE the rectangle. A full-image
    /// rectangle uses a neutral fill; no original interior samples survive.
    Pixelate {
        region: PixelRect,
        block_size: u32,
    },
}
#[derive(Debug, thiserror::Error)]
pub enum EditError {
    #[error("Invalid annotation coordinates or stroke width")]
    InvalidAnnotation,
    #[error("Annotation index does not exist")]
    InvalidIndex,
    #[error("Image or selection is empty or too large")]
    InvalidImage,
}

#[derive(Clone)]
struct Change {
    before: Vec<Annotation>,
    after: Vec<Annotation>,
}
pub struct Document {
    base: RgbaImage,
    annotations: Vec<Annotation>,
    undo: Vec<Change>,
    redo: Vec<Change>,
    undo_limit: usize,
    text_renderer: std::cell::RefCell<Option<crate::text::TextRenderer>>,
}
impl Document {
    pub fn new(base: RgbaImage, undo_limit: usize) -> Result<Self, EditError> {
        if base.width() == 0 || base.height() == 0 || base.as_raw().len() > 1024 * 1024 * 1024 {
            return Err(EditError::InvalidImage);
        }
        Ok(Self {
            base,
            annotations: vec![],
            undo: vec![],
            redo: vec![],
            undo_limit,
            text_renderer: std::cell::RefCell::new(None),
        })
    }
    pub fn annotations(&self) -> &[Annotation] {
        &self.annotations
    }
    pub fn add(&mut self, annotation: Annotation) -> Result<(), EditError> {
        validate(&annotation)?;
        let mut next = self.annotations.clone();
        next.push(annotation);
        self.commit(next);
        Ok(())
    }
    pub fn replace(&mut self, index: usize, annotation: Annotation) -> Result<(), EditError> {
        validate(&annotation)?;
        let mut next = self.annotations.clone();
        *next.get_mut(index).ok_or(EditError::InvalidIndex)? = annotation;
        self.commit(next);
        Ok(())
    }
    pub fn remove(&mut self, index: usize) -> Result<(), EditError> {
        if index >= self.annotations.len() {
            return Err(EditError::InvalidIndex);
        }
        let mut next = self.annotations.clone();
        next.remove(index);
        self.commit(next);
        Ok(())
    }
    fn commit(&mut self, after: Vec<Annotation>) {
        let before = std::mem::replace(&mut self.annotations, after.clone());
        self.redo.clear();
        if self.undo_limit > 0 {
            self.undo.push(Change { before, after });
            if self.undo.len() > self.undo_limit {
                self.undo.remove(0);
            }
        }
    }
    pub fn next_counter(&self) -> u16 {
        self.annotations
            .iter()
            .filter_map(|a| {
                if let Annotation::Counter { number, .. } = a {
                    Some(*number)
                } else {
                    None
                }
            })
            .max()
            .unwrap_or(0)
            .saturating_add(1)
            .min(999)
    }
    pub fn hit_test(&self, point: Point) -> Option<usize> {
        self.annotations.iter().rposition(|a| {
            let (min, max) = a.bounds();
            point.x >= min.x && point.x <= max.x && point.y >= min.y && point.y <= max.y
        })
    }
    pub fn move_annotation(&mut self, index: usize, dx: f32, dy: f32) -> Result<(), EditError> {
        let mut annotation = self
            .annotations
            .get(index)
            .ok_or(EditError::InvalidIndex)?
            .clone();
        annotation.translate(dx, dy)?;
        self.replace(index, annotation)
    }
    pub fn undo(&mut self) -> bool {
        let Some(change) = self.undo.pop() else {
            return false;
        };
        self.annotations = change.before.clone();
        self.redo.push(change);
        true
    }
    pub fn redo(&mut self) -> bool {
        let Some(change) = self.redo.pop() else {
            return false;
        };
        self.annotations = change.after.clone();
        self.undo.push(change);
        true
    }
    pub fn render(&self) -> Result<RgbaImage, EditError> {
        self.preview(None)
    }
    /// Render a transient annotation without changing the document or history.
    pub fn preview(&self, pending: Option<&Annotation>) -> Result<RgbaImage, EditError> {
        if let Some(annotation) = pending {
            validate(annotation)?;
        }
        let mut image = self.base.clone();
        for annotation in self.annotations.iter().chain(pending) {
            match annotation {
                Annotation::Counter {
                    center,
                    tip,
                    number,
                    radius,
                    color,
                    outline,
                } => {
                    let contrast =
                        crate::counter::draw(&mut image, *center, *tip, *radius, *color, *outline)?;
                    let mut renderer = self.text_renderer.borrow_mut();
                    crate::counter::label(
                        renderer.get_or_insert_with(crate::text::TextRenderer::new),
                        &mut image,
                        *center,
                        *number,
                        *radius,
                        contrast,
                    );
                }
                Annotation::Text {
                    position,
                    text,
                    font_size,
                    family,
                    color,
                } => {
                    let mut renderer = self.text_renderer.borrow_mut();
                    renderer
                        .get_or_insert_with(crate::text::TextRenderer::new)
                        .draw(&mut image, *position, text, *font_size, family, *color);
                }
                Annotation::Invert(region) => {
                    if let Some(r) = clip(*region, &image) {
                        for y in r.y..r.y + r.height {
                            for x in r.x..r.x + r.width {
                                let p = image.get_pixel_mut(x, y);
                                for channel in &mut p.0[..3] {
                                    *channel = 255 - *channel;
                                }
                            }
                        }
                    }
                }
                Annotation::Pixelate { region, block_size } => {
                    pixelate(&mut image, *region, *block_size)
                }
                _ => draw(&mut image, annotation)?,
            }
        }
        Ok(image)
    }
    pub fn export(&self, selection: PixelRect) -> Result<RgbaImage, EditError> {
        let image = self.render()?;
        let r = clip(selection, &image).ok_or(EditError::InvalidImage)?;
        Ok(image::imageops::crop_imm(&image, r.x, r.y, r.width, r.height).to_image())
    }
}
fn clip(r: PixelRect, image: &RgbaImage) -> Option<PixelRect> {
    let end_x = r.x.saturating_add(r.width).min(image.width());
    let end_y = r.y.saturating_add(r.height).min(image.height());
    (end_x > r.x && end_y > r.y).then(|| PixelRect {
        x: r.x,
        y: r.y,
        width: end_x - r.x,
        height: end_y - r.y,
    })
}
fn validate(a: &Annotation) -> Result<(), EditError> {
    let valid_point =
        |p: &Point| p.x.is_finite() && p.y.is_finite() && p.x.abs() <= 1e7 && p.y.abs() <= 1e7;
    let valid_style = |s: &Style| s.width.is_finite() && s.width > 0.0 && s.width <= 10000.0;
    let valid = match a {
        Annotation::Counter {
            center,
            tip,
            number,
            radius,
            ..
        } => {
            valid_point(center)
                && valid_point(tip)
                && (1..=999).contains(number)
                && radius.is_finite()
                && *radius >= 1.0
                && *radius <= 1000.0
        }
        Annotation::Text {
            position,
            text,
            font_size,
            ..
        } => {
            valid_point(position)
                && !text.is_empty()
                && text.len() <= 1_000_000
                && font_size.is_finite()
                && *font_size > 0.0
                && *font_size <= 1000.0
        }
        Annotation::Pencil { points, style } => {
            points.len() >= 2 && points.iter().all(valid_point) && valid_style(style)
        }
        Annotation::Line { start, end, style }
        | Annotation::Marker { start, end, style }
        | Annotation::Arrow { start, end, style }
        | Annotation::Rectangle {
            start, end, style, ..
        }
        | Annotation::Ellipse {
            start, end, style, ..
        } => valid_point(start) && valid_point(end) && valid_style(style),
        Annotation::Invert(r) => r.width > 0 && r.height > 0,
        Annotation::Pixelate { region, block_size } => {
            region.width > 0 && region.height > 0 && *block_size > 0
        }
    };
    if valid {
        Ok(())
    } else {
        Err(EditError::InvalidAnnotation)
    }
}
fn draw(image: &mut RgbaImage, a: &Annotation) -> Result<(), EditError> {
    let mut builder = PathBuilder::new();
    let (style, filled) = match a {
        Annotation::Pencil { points, style } => {
            builder.move_to(points[0].x, points[0].y);
            for p in &points[1..] {
                builder.line_to(p.x, p.y);
            }
            (*style, false)
        }
        Annotation::Line { start, end, style }
        | Annotation::Marker { start, end, style }
        | Annotation::Arrow { start, end, style } => {
            builder.move_to(start.x, start.y);
            builder.line_to(end.x, end.y);
            if matches!(a, Annotation::Arrow { .. }) {
                let angle = (end.y - start.y).atan2(end.x - start.x);
                let length = (style.width * 4.0).max(10.0);
                for delta in [-0.5_f32, 0.5] {
                    builder.move_to(end.x, end.y);
                    builder.line_to(
                        end.x - length * (angle + delta).cos(),
                        end.y - length * (angle + delta).sin(),
                    );
                }
            }
            (*style, false)
        }
        Annotation::Rectangle {
            start,
            end,
            style,
            filled,
        }
        | Annotation::Ellipse {
            start,
            end,
            style,
            filled,
        } => {
            let Some(rect) = tiny_skia::Rect::from_ltrb(
                start.x.min(end.x),
                start.y.min(end.y),
                start.x.max(end.x),
                start.y.max(end.y),
            ) else {
                return Ok(());
            };
            if matches!(a, Annotation::Ellipse { .. }) {
                builder.push_oval(rect);
            } else {
                builder.push_rect(rect);
            }
            (*style, *filled)
        }
        _ => return Ok(()),
    };
    let Some(path) = builder.finish() else {
        return Ok(());
    };
    let mut layer = Pixmap::new(image.width(), image.height()).ok_or(EditError::InvalidImage)?;
    let mut paint = Paint::default();
    let marker = matches!(a, Annotation::Marker { .. });
    paint.set_color_rgba8(
        style.color[0],
        style.color[1],
        style.color[2],
        if marker {
            (u16::from(style.color[3]) * 35 / 100) as u8
        } else {
            style.color[3]
        },
    );
    paint.anti_alias = true;
    if filled {
        layer.fill_path(
            &path,
            &paint,
            FillRule::Winding,
            Transform::identity(),
            None,
        );
    } else {
        let stroke = Stroke {
            width: if marker {
                style.width + 14.0
            } else {
                style.width
            },
            line_cap: if marker {
                LineCap::Square
            } else {
                LineCap::Round
            },
            ..Default::default()
        };
        layer.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
    }
    for (destination, source) in image.pixels_mut().zip(layer.pixels()) {
        let alpha = u32::from(source.alpha());
        if alpha == 0 {
            continue;
        }
        let dst_alpha = u32::from(destination[3]);
        let out_alpha = alpha + (dst_alpha * (255 - alpha) + 127) / 255;
        for (channel, premultiplied) in [source.red(), source.green(), source.blue()]
            .into_iter()
            .enumerate()
        {
            let source = u32::from(premultiplied);
            let background = u32::from(destination[channel]);
            let source_term = if marker {
                source * (255 - dst_alpha) + (source * background * dst_alpha + 127) / 255
            } else {
                source * 255
            };
            let value = source_term + (background * dst_alpha * (255 - alpha) + 127) / 255;
            destination[channel] = ((value + out_alpha / 2) / out_alpha).min(255) as u8;
        }
        destination[3] = out_alpha as u8;
    }
    Ok(())
}
fn pixelate(image: &mut RgbaImage, region: PixelRect, block_size: u32) {
    let Some(r) = clip(region, image) else {
        return;
    };
    let mut fringe = Vec::new();
    for x in r.x..r.x + r.width {
        if r.y > 0 {
            fringe.push(*image.get_pixel(x, r.y - 1));
        }
        if r.y + r.height < image.height() {
            fringe.push(*image.get_pixel(x, r.y + r.height));
        }
    }
    for y in r.y..r.y + r.height {
        if r.x > 0 {
            fringe.push(*image.get_pixel(r.x - 1, y));
        }
        if r.x + r.width < image.width() {
            fringe.push(*image.get_pixel(r.x + r.width, y));
        }
    }
    if fringe.is_empty() {
        fringe.push(Rgba([128, 128, 128, 255]));
    }
    for y in r.y..r.y + r.height {
        for x in r.x..r.x + r.width {
            let hash = u64::from((x - r.x) / block_size).wrapping_mul(0x9e3779b9)
                ^ u64::from((y - r.y) / block_size).wrapping_mul(0x85ebca6b);
            let mut color = fringe[(hash % fringe.len() as u64) as usize];
            color[3] = 255;
            image.put_pixel(x, y, color);
        }
    }
}

impl Annotation {
    pub fn bounds(&self) -> (Point, Point) {
        let (points, padding) = match self {
            Self::Counter {
                center,
                tip,
                radius,
                ..
            } => {
                return (
                    Point {
                        x: (center.x - radius - 3.0).min(tip.x),
                        y: (center.y - radius - 3.0).min(tip.y),
                    },
                    Point {
                        x: (center.x + radius + 3.0).max(tip.x),
                        y: (center.y + radius + 3.0).max(tip.y),
                    },
                );
            }
            Self::Text {
                position,
                text,
                font_size,
                ..
            } => {
                let width = text
                    .lines()
                    .map(|line| line.chars().count())
                    .max()
                    .unwrap_or(0) as f32
                    * font_size;
                return (
                    *position,
                    Point {
                        x: position.x + width,
                        y: position.y + text.lines().count() as f32 * font_size * 1.25,
                    },
                );
            }
            Self::Pencil { points, style } => (points.clone(), style.width / 2.0 + 3.0),
            Self::Line { start, end, style }
            | Self::Rectangle {
                start, end, style, ..
            }
            | Self::Ellipse {
                start, end, style, ..
            } => (vec![*start, *end], style.width / 2.0 + 3.0),
            Self::Marker { start, end, style } => {
                (vec![*start, *end], (style.width + 14.0) / 2.0 + 3.0)
            }
            Self::Arrow { start, end, style } => {
                (vec![*start, *end], (style.width * 4.0).max(10.0))
            }
            Self::Invert(r) | Self::Pixelate { region: r, .. } => {
                return (
                    Point {
                        x: r.x as f32,
                        y: r.y as f32,
                    },
                    Point {
                        x: r.x.saturating_add(r.width) as f32,
                        y: r.y.saturating_add(r.height) as f32,
                    },
                );
            }
        };
        let mut min = Point {
            x: f32::INFINITY,
            y: f32::INFINITY,
        };
        let mut max = Point {
            x: f32::NEG_INFINITY,
            y: f32::NEG_INFINITY,
        };
        for p in points {
            min.x = min.x.min(p.x);
            min.y = min.y.min(p.y);
            max.x = max.x.max(p.x);
            max.y = max.y.max(p.y);
        }
        (
            Point {
                x: min.x - padding,
                y: min.y - padding,
            },
            Point {
                x: max.x + padding,
                y: max.y + padding,
            },
        )
    }
    fn translate(&mut self, dx: f32, dy: f32) -> Result<(), EditError> {
        if !dx.is_finite() || !dy.is_finite() {
            return Err(EditError::InvalidAnnotation);
        }
        let shift = |point: &mut Point| {
            point.x += dx;
            point.y += dy;
        };
        match self {
            Self::Counter { center, tip, .. } => {
                shift(center);
                shift(tip);
            }
            Self::Text { position, .. } => shift(position),
            Self::Pencil { points, .. } => {
                for point in points {
                    shift(point);
                }
            }
            Self::Line { start, end, .. }
            | Self::Marker { start, end, .. }
            | Self::Arrow { start, end, .. }
            | Self::Rectangle { start, end, .. }
            | Self::Ellipse { start, end, .. } => {
                shift(start);
                shift(end);
            }
            Self::Invert(r) | Self::Pixelate { region: r, .. } => {
                let x = f64::from(r.x) + f64::from(dx.round());
                let y = f64::from(r.y) + f64::from(dy.round());
                if x < 0.0
                    || y < 0.0
                    || x > f64::from(u32::MAX - r.width)
                    || y > f64::from(u32::MAX - r.height)
                {
                    return Err(EditError::InvalidAnnotation);
                }
                r.x = x as u32;
                r.y = y as u32;
            }
        }
        validate(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn base() -> RgbaImage {
        RgbaImage::from_pixel(20, 20, Rgba([255, 255, 255, 255]))
    }
    fn mark() -> Annotation {
        Annotation::Rectangle {
            start: Point { x: 4.0, y: 4.0 },
            end: Point { x: 12.0, y: 12.0 },
            style: Style {
                color: [255, 0, 0, 255],
                width: 2.0,
            },
            filled: true,
        }
    }
    #[test]
    fn counters_render_pointer_and_renumber_after_undo() {
        let mut doc = Document::new(RgbaImage::from_pixel(100, 100, Rgba([255; 4])), 10).unwrap();
        assert_eq!(doc.next_counter(), 1);
        let counter = Annotation::Counter {
            center: Point { x: 30.0, y: 30.0 },
            tip: Point { x: 80.0, y: 30.0 },
            number: 1,
            radius: 15.0,
            color: [255, 0, 0, 255],
            outline: true,
        };
        let preview = doc.preview(Some(&counter)).unwrap();
        doc.add(counter).unwrap();
        assert_eq!(doc.next_counter(), 2);
        let rendered = doc.render().unwrap();
        assert_eq!(preview, rendered);
        assert_eq!(*rendered.get_pixel(60, 30), Rgba([255, 0, 0, 255]));
        assert!(
            rendered
                .enumerate_pixels()
                .any(|(x, y, p)| (22..38).contains(&x)
                    && (20..40).contains(&y)
                    && p[1] > 100
                    && p[2] > 100)
        );
        doc.move_annotation(0, 0.0, 20.0).unwrap();
        assert_eq!(
            *doc.render().unwrap().get_pixel(60, 50),
            Rgba([255, 0, 0, 255])
        );
        assert!(doc.undo());
        assert_eq!(doc.render().unwrap(), rendered);
        assert!(doc.undo());
        assert_eq!(doc.next_counter(), 1);
        assert!(doc.redo());
        assert_eq!(doc.next_counter(), 2);
    }
    #[test]
    fn marker_uses_multiply_and_keeps_dark_pixels_dark() {
        let mut image = base();
        image.put_pixel(10, 10, Rgba([0, 0, 0, 255]));
        let mut doc = Document::new(image, 10).unwrap();
        doc.add(Annotation::Marker {
            start: Point { x: 2.0, y: 10.0 },
            end: Point { x: 18.0, y: 10.0 },
            style: Style {
                color: [255, 255, 0, 255],
                width: 3.0,
            },
        })
        .unwrap();
        let result = doc.render().unwrap();
        assert_eq!(*result.get_pixel(10, 10), Rgba([0, 0, 0, 255]));
        assert_eq!(result.get_pixel(5, 10)[0], 255);
        assert!(result.get_pixel(5, 10)[2] < 200);
    }
    #[test]
    fn moves_topmost_object_and_undo_restores_original_pixels() {
        let mut doc = Document::new(base(), 10).unwrap();
        doc.add(mark()).unwrap();
        doc.add(mark()).unwrap();
        assert_eq!(doc.hit_test(Point { x: 6.0, y: 6.0 }), Some(1));
        let before = doc.render().unwrap();
        doc.move_annotation(1, 6.0, 0.0).unwrap();
        assert_eq!(
            *doc.render().unwrap().get_pixel(15, 6),
            Rgba([255, 0, 0, 255])
        );
        assert!(doc.undo());
        assert_eq!(doc.render().unwrap(), before);
        assert!(doc.move_annotation(1, f32::NAN, 0.0).is_err());
        assert_eq!(doc.render().unwrap(), before);
    }
    #[test]
    fn transient_preview_matches_commit_without_touching_history() {
        let mut doc = Document::new(base(), 10).unwrap();
        let annotation = mark();
        let preview = doc.preview(Some(&annotation)).unwrap();
        assert!(doc.annotations().is_empty());
        assert!(!doc.undo());
        assert_eq!(doc.render().unwrap(), base());
        doc.add(annotation).unwrap();
        assert_eq!(doc.render().unwrap(), preview);
        assert!(doc.undo());
        assert_eq!(doc.render().unwrap(), base());
    }
    #[test]
    fn renders_and_exports_identical_pixels_without_mutating_base() {
        let mut doc = Document::new(base(), 100).unwrap();
        doc.add(mark()).unwrap();
        let rendered = doc.render().unwrap();
        assert_eq!(*rendered.get_pixel(6, 6), Rgba([255, 0, 0, 255]));
        assert_eq!(*rendered.get_pixel(0, 0), Rgba([255, 255, 255, 255]));
        let region = PixelRect {
            x: 3,
            y: 3,
            width: 12,
            height: 12,
        };
        assert_eq!(
            doc.export(region).unwrap(),
            image::imageops::crop_imm(&rendered, 3, 3, 12, 12).to_image()
        );
        assert!(doc.undo());
        assert_eq!(doc.render().unwrap(), base());
        assert!(doc.redo());
        assert_eq!(doc.render().unwrap(), rendered);
    }
    #[test]
    fn edit_after_undo_discards_redo_and_invalid_edits_do_not_change_history() {
        let mut doc = Document::new(base(), 2).unwrap();
        doc.add(mark()).unwrap();
        doc.remove(0).unwrap();
        assert!(doc.undo());
        assert_eq!(doc.annotations().len(), 1);
        assert!(doc.remove(10).is_err());
        doc.replace(
            0,
            Annotation::Invert(PixelRect {
                x: 0,
                y: 0,
                width: 1,
                height: 1,
            }),
        )
        .unwrap();
        assert!(!doc.redo());
        assert!(doc.undo());
        assert_eq!(doc.annotations(), &[mark()]);
    }
    #[test]
    fn redaction_is_independent_of_all_interior_pixels_including_edges() {
        for region in [
            PixelRect {
                x: 4,
                y: 4,
                width: 8,
                height: 8,
            },
            PixelRect {
                x: 0,
                y: 0,
                width: 10,
                height: 10,
            },
            PixelRect {
                x: 0,
                y: 0,
                width: 20,
                height: 20,
            },
        ] {
            let a = base();
            let mut b = a.clone();
            for y in region.y..region.y + region.height {
                for x in region.x..region.x + region.width {
                    b.put_pixel(x, y, Rgba([0, 0, 0, 0]));
                }
            }
            let mut da = Document::new(a, 100).unwrap();
            let mut db = Document::new(b, 100).unwrap();
            let annotation = Annotation::Pixelate {
                region,
                block_size: 3,
            };
            da.add(annotation.clone()).unwrap();
            db.add(annotation).unwrap();
            assert_eq!(da.render().unwrap(), db.render().unwrap());
        }
    }
    #[test]
    fn transparent_composition_and_inversion_preserve_alpha() {
        let mut doc = Document::new(RgbaImage::new(20, 20), 10).unwrap();
        let mut annotation = mark();
        if let Annotation::Rectangle { style, .. } = &mut annotation {
            style.color = [255, 0, 0, 128];
        }
        doc.add(annotation).unwrap();
        let rendered = doc.render().unwrap();
        assert_eq!(*rendered.get_pixel(6, 6), Rgba([255, 0, 0, 128]));
        doc.add(Annotation::Invert(PixelRect {
            x: 5,
            y: 5,
            width: 3,
            height: 3,
        }))
        .unwrap();
        assert_eq!(
            *doc.render().unwrap().get_pixel(6, 6),
            Rgba([0, 255, 255, 128])
        );
    }
    #[test]
    fn history_limit_and_invalid_strokes() {
        let mut doc = Document::new(base(), 1).unwrap();
        doc.add(mark()).unwrap();
        doc.add(mark()).unwrap();
        assert!(doc.undo());
        assert!(!doc.undo());
        assert!(
            doc.add(Annotation::Line {
                start: Point {
                    x: f32::NAN,
                    y: 0.0
                },
                end: Point { x: 1.0, y: 1.0 },
                style: Style {
                    color: [0; 4],
                    width: 1.0
                }
            })
            .is_err()
        );
        assert!(
            doc.add(Annotation::Pixelate {
                region: PixelRect {
                    x: 0,
                    y: 0,
                    width: 1,
                    height: 1
                },
                block_size: 0
            })
            .is_err()
        );
    }
}
