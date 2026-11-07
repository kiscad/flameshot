// SPDX-License-Identifier: GPL-3.0-or-later
use eframe::egui::{self, Color32, Pos2, Rect};
use image::RgbaImage;

const ZOOM: f32 = 15.0;

pub struct PrecisionPick {
    pixels: RgbaImage,
    bounds: Rect,
}
impl PrecisionPick {
    pub fn new(image: &RgbaImage, x: u32, y: u32, pointer: Pos2) -> Option<Self> {
        let sample = flameshot_core::inspection::sample(image, x, y, 5)?;
        let r = sample.neighborhood;
        let pixels = image::imageops::crop_imm(image, r.x, r.y, r.width, r.height).to_image();
        // Keep the pressed pixel under the cursor, even when the source crop
        // is clipped at an image edge. Each source pixel occupies one cell.
        let min = pointer
            - egui::vec2(
                (sample.center[0] as f32 + 0.5) * ZOOM,
                (sample.center[1] as f32 + 0.5) * ZOOM,
            );
        Some(Self {
            pixels,
            bounds: Rect::from_min_size(
                min,
                egui::vec2(r.width as f32 * ZOOM, r.height as f32 * ZOOM),
            ),
        })
    }
    fn cell(&self, pointer: Pos2) -> Option<(u32, u32)> {
        if pointer.x < self.bounds.min.x || pointer.y < self.bounds.min.y {
            return None;
        }
        let x = ((pointer.x - self.bounds.min.x) / ZOOM).floor() as u32;
        let y = ((pointer.y - self.bounds.min.y) / ZOOM).floor() as u32;
        (x < self.pixels.width() && y < self.pixels.height()).then_some((x, y))
    }
    pub fn sample(&self, pointer: Pos2) -> Option<[u8; 4]> {
        self.cell(pointer)
            .map(|(x, y)| self.pixels.get_pixel(x, y).0)
    }
    pub fn paint(&self, ctx: &egui::Context, pointer: Pos2) {
        let painter = ctx.layer_painter(egui::LayerId::new(
            egui::Order::Tooltip,
            egui::Id::unique("precision-color-pick"),
        ));
        let cell_rect = |x: u32, y: u32| {
            Rect::from_min_size(
                self.bounds.min + egui::vec2(x as f32 * ZOOM, y as f32 * ZOOM),
                egui::vec2(ZOOM, ZOOM),
            )
        };
        let mut mesh = egui::Mesh::default();
        for (x, y, pixel) in self.pixels.enumerate_pixels() {
            mesh.add_colored_rect(
                cell_rect(x, y),
                Color32::from_rgba_unmultiplied(pixel[0], pixel[1], pixel[2], pixel[3]),
            );
        }
        painter.add(egui::Shape::mesh(mesh));
        painter.rect_stroke(
            self.bounds,
            0.0,
            egui::Stroke::new(1.0, Color32::WHITE),
            egui::StrokeKind::Inside,
        );
        if let Some((x, y)) = self.cell(pointer) {
            painter.rect_stroke(
                cell_rect(x, y),
                0.0,
                egui::Stroke::new(1.0, Color32::RED),
                egui::StrokeKind::Inside,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn frozen_pixels_map_to_individual_cells_and_preserve_edge_anchor() {
        let mut image = RgbaImage::from_fn(20, 20, |x, y| image::Rgba([x as u8, y as u8, 0, 255]));
        let pointer = egui::pos2(50., 50.);
        let pick = PrecisionPick::new(&image, 10, 10, pointer).unwrap();
        image.put_pixel(11, 10, image::Rgba([255; 4]));
        assert_eq!(pick.sample(pointer), Some([10, 10, 0, 255]));
        assert_eq!(
            pick.sample(pointer + egui::vec2(15., 0.)),
            Some([11, 10, 0, 255])
        );
        assert_eq!(pick.sample(pick.bounds.max), None);
        assert_eq!(pick.sample(pick.bounds.min - egui::vec2(0.1, 0.)), None);
        let edge = PrecisionPick::new(&image, 0, 0, pointer).unwrap();
        assert_eq!(edge.sample(pointer), Some([0, 0, 0, 255]));
        assert_eq!(
            edge.sample(pointer + egui::vec2(15., 15.)),
            Some([1, 1, 0, 255])
        );
        assert!(PrecisionPick::new(&image, 20, 0, pointer).is_none());
    }
}
