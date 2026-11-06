// SPDX-License-Identifier: GPL-3.0-or-later
use eframe::egui;
use flameshot_core::geometry::PixelRect;
use std::time::{Duration, Instant};
pub struct GeometryOverlay {
    last: Option<PixelRect>,
    changed: Instant,
}
impl Default for GeometryOverlay {
    fn default() -> Self {
        Self {
            last: None,
            changed: Instant::now(),
        }
    }
}
impl GeometryOverlay {
    fn update(&mut self, selection: Option<PixelRect>, now: Instant) {
        if self.last != selection {
            self.last = selection;
            self.changed = now;
        }
    }
    fn visible(&self, mode: u8, timeout: Duration, now: Instant) -> bool {
        mode != 0
            && self.last.is_some()
            && (timeout.is_zero() || now.saturating_duration_since(self.changed) < timeout)
    }
    pub fn show(
        &mut self,
        ui: &egui::Ui,
        selection: Option<PixelRect>,
        bounds: Option<egui::Rect>,
        mode: u8,
        hide_ms: u64,
    ) {
        let now = Instant::now();
        self.update(selection, now);
        let timeout = Duration::from_millis(hide_ms);
        if !self.visible(mode, timeout, now) {
            return;
        }
        let (Some(region), Some(bounds)) = (selection, bounds) else {
            return;
        };
        let text = format!(
            "{}x{}+{}+{}",
            region.width, region.height, region.x, region.y
        );
        let galley =
            ui.painter()
                .layout_no_wrap(text, egui::FontId::monospace(14.0), egui::Color32::WHITE);
        let size = galley.size() + egui::vec2(12.0, 8.0);
        let point = match mode {
            1 => bounds.left_top(),
            2 => bounds.right_top() - egui::vec2(size.x, 0.0),
            3 => bounds.left_bottom() - egui::vec2(0.0, size.y),
            4 => bounds.right_bottom() - size,
            _ => bounds.center() - size / 2.0,
        };
        let screen = ui.ctx().content_rect();
        let point = egui::pos2(
            point
                .x
                .clamp(screen.left(), (screen.right() - size.x).max(screen.left())),
            point
                .y
                .clamp(screen.top(), (screen.bottom() - size.y).max(screen.top())),
        );
        let rect = egui::Rect::from_min_size(point, size);
        ui.painter()
            .rect_filled(rect, 3.0, egui::Color32::from_black_alpha(210));
        ui.painter()
            .galley(point + egui::vec2(6.0, 4.0), galley, egui::Color32::WHITE);
        if !timeout.is_zero() {
            ui.ctx().request_repaint_after(
                timeout.saturating_sub(now.saturating_duration_since(self.changed)),
            );
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn selection_changes_restart_timeout_but_repaints_do_not() {
        let mut overlay = GeometryOverlay::default();
        let now = Instant::now();
        let region = PixelRect {
            x: 10,
            y: 20,
            width: 30,
            height: 40,
        };
        overlay.update(Some(region), now);
        assert!(overlay.visible(4, Duration::from_secs(1), now));
        overlay.update(Some(region), now + Duration::from_millis(900));
        assert!(!overlay.visible(4, Duration::from_secs(1), now + Duration::from_secs(1)));
        assert!(overlay.visible(4, Duration::ZERO, now + Duration::from_secs(100)));
        assert!(!overlay.visible(0, Duration::ZERO, now));
        overlay.update(
            Some(PixelRect { x: 11, ..region }),
            now + Duration::from_secs(2),
        );
        assert!(overlay.visible(5, Duration::from_secs(1), now + Duration::from_secs(2)));
        overlay.update(None, now);
        assert!(!overlay.visible(4, Duration::ZERO, now));
    }
}
