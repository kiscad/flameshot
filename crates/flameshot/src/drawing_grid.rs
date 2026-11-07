// SPDX-License-Identifier: GPL-3.0-or-later
use crate::i18n::tr;
use eframe::egui::{self, Color32, Pos2, Rect};

#[derive(Clone, Copy)]
pub struct DrawingGrid {
    enabled: bool,
    spacing: u32,
}
impl Default for DrawingGrid {
    fn default() -> Self {
        Self {
            enabled: false,
            spacing: 10,
        }
    }
}
impl DrawingGrid {
    pub fn controls(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.checkbox(&mut self.enabled, tr("SidePanelWidget", "Display grid"));
            ui.add_enabled(
                self.enabled,
                egui::DragValue::new(&mut self.spacing)
                    .range(5..=50)
                    .speed(5),
            );
        });
    }
    pub fn snap(self, point: Pos2, origin: Pos2) -> Pos2 {
        if !self.enabled {
            return point;
        }
        let step = self.spacing as f32;
        egui::pos2(
            ((point.x + origin.x) / step).round() * step - origin.x,
            ((point.y + origin.y) / step).round() * step - origin.y,
        )
    }
    pub fn paint(self, ui: &egui::Ui, bounds: Rect, origin: Pos2, color: Color32) {
        if !self.enabled || !bounds.is_positive() {
            return;
        }
        let step = self.spacing as f32;
        let first = egui::pos2(
            ((bounds.min.x + origin.x) / step).ceil() * step - origin.x,
            ((bounds.min.y + origin.y) / step).ceil() * step - origin.y,
        );
        let mut mesh = egui::Mesh::default();
        let color = Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 100);
        let mut y = first.y;
        while y < bounds.max.y {
            let mut x = first.x;
            while x < bounds.max.x {
                mesh.add_colored_rect(
                    Rect::from_center_size(egui::pos2(x, y), egui::vec2(2.0, 2.0)),
                    color,
                );
                x += step;
            }
            y += step;
        }
        ui.painter()
            .with_clip_rect(bounds)
            .add(egui::Shape::mesh(mesh));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn snapping_uses_global_logical_coordinates_including_negative_origins() {
        let grid = DrawingGrid {
            enabled: true,
            spacing: 10,
        };
        assert_eq!(
            grid.snap(egui::pos2(13., 27.), Pos2::ZERO),
            egui::pos2(10., 30.)
        );
        assert_eq!(
            grid.snap(egui::pos2(13., 27.), egui::pos2(-853., -17.)),
            egui::pos2(13., 27.)
        );
        assert_eq!(
            grid.snap(egui::pos2(8., 22.), egui::pos2(-853., -17.)),
            egui::pos2(3., 27.)
        );
        assert_eq!(
            DrawingGrid::default().snap(egui::pos2(13., 27.), Pos2::ZERO),
            egui::pos2(13., 27.)
        );
    }
}
