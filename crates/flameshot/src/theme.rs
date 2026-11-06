// SPDX-License-Identifier: GPL-3.0-or-later
use eframe::egui::{self, Color32, Stroke};

#[derive(Clone, Copy)]
pub struct Theme {
    pub main: Color32,
    pub contrast: Color32,
}
impl Theme {
    pub fn from_settings(settings: &crate::settings::Settings) -> Self {
        let color =
            |rgba: [u8; 4]| Color32::from_rgba_unmultiplied(rgba[0], rgba[1], rgba[2], rgba[3]);
        Self {
            main: color(settings.ui_color),
            contrast: color(settings.contrast_ui_color),
        }
    }
    pub fn apply(self, ctx: &egui::Context) {
        ctx.all_styles_mut(|style| {
            style.visuals.selection.bg_fill = self.main;
            style.visuals.selection.stroke = Stroke::new(1.0, text_color(self.main));
            style.visuals.widgets.hovered.weak_bg_fill = self.main;
            style.visuals.widgets.hovered.bg_fill = self.main;
            style.visuals.widgets.hovered.fg_stroke = Stroke::new(1.0, text_color(self.main));
            style.visuals.widgets.active.weak_bg_fill = self.contrast;
            style.visuals.widgets.active.bg_fill = self.contrast;
            style.visuals.widgets.active.fg_stroke = Stroke::new(1.0, text_color(self.contrast));
        });
    }
}
fn text_color(background: Color32) -> Color32 {
    let [r, g, b, _] = background.to_srgba_unmultiplied();
    if u32::from(r) * 299 + u32::from(g) * 587 + u32::from(b) * 114 >= 128000 {
        Color32::BLACK
    } else {
        Color32::WHITE
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn custom_colors_reach_selection_and_active_widgets_with_readable_text() {
        let settings = crate::settings::Settings {
            ui_color: [255, 255, 0, 255],
            contrast_ui_color: [0, 0, 90, 255],
            ..Default::default()
        };
        let ctx = egui::Context::default();
        Theme::from_settings(&settings).apply(&ctx);
        let style = ctx.style_of(egui::Theme::Dark);
        assert_eq!(style.visuals.selection.bg_fill, Color32::YELLOW);
        assert_eq!(style.visuals.selection.stroke.color, Color32::BLACK);
        assert_eq!(
            style.visuals.widgets.active.bg_fill,
            Color32::from_rgb(0, 0, 90)
        );
        assert_eq!(style.visuals.widgets.active.fg_stroke.color, Color32::WHITE);
    }
}
