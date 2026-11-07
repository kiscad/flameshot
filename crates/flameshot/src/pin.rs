// SPDX-License-Identifier: GPL-3.0-or-later
use crate::i18n::tr;
use eframe::egui;
use flameshot_core::qt_wire::Pin;
fn scaled_size(width: u32, height: u32, scale: f64, zoom: f32) -> anyhow::Result<egui::Vec2> {
    let width = width as f32 / scale as f32;
    let height = height as f32 / scale as f32;
    anyhow::ensure!(
        width.is_finite() && height.is_finite() && width > 0.0 && height > 0.0,
        "Invalid pin dimensions"
    );
    let factor = zoom.clamp(0.05, 8.0).min(16384.0 / width.max(height));
    Ok(egui::vec2(width * factor, height * factor))
}
pub fn show(pin: Pin) -> anyhow::Result<()> {
    let settings = crate::settings::Settings::load()?;
    let shortcuts = crate::shortcuts::Shortcuts::for_pin(&settings.shortcuts);
    let size = scaled_size(pin.image.width(), pin.image.height(), pin.scale, 1.0)?;
    let texture_options = if settings.antialias_pin_zoom {
        egui::TextureOptions::LINEAR
    } else {
        egui::TextureOptions::NEAREST
    };
    let pixels = egui::ColorImage::from_rgba_unmultiplied(
        [pin.image.width() as usize, pin.image.height() as usize],
        pin.image.as_raw(),
    );
    eframe::run_native(
        "flameshot-pin",
        eframe::NativeOptions {
            viewport: crate::app_ui::viewport()
                .with_decorations(false)
                .with_resizable(false)
                .with_transparent(true)
                .with_inner_size(size)
                .with_position([pin.geometry.x as f32, pin.geometry.y as f32])
                .with_always_on_top(),
            ..Default::default()
        },
        Box::new(move |cc| {
            crate::i18n::install_fonts(&cc.egui_ctx);
            crate::theme::Theme::from_settings(&settings).apply(&cc.egui_ctx);
            Ok(Box::new(Pinned {
                shortcuts,
                texture: cc.egui_ctx.load_texture("pin", pixels, texture_options),
                image: pin.image,
                scale: pin.scale,
                zoom: 1.0,
                opacity: 1.0,
                texture_options,
            }))
        }),
    )
    .map_err(|e| anyhow::anyhow!("Cannot display pin: {e}"))
}
struct Pinned {
    shortcuts: crate::shortcuts::Shortcuts,
    texture: egui::TextureHandle,
    image: image::RgbaImage,
    scale: f64,
    zoom: f32,
    opacity: f32,
    texture_options: egui::TextureOptions,
}
impl Pinned {
    fn resize(&self, ctx: &egui::Context) {
        if let Ok(size) = scaled_size(
            self.image.width(),
            self.image.height(),
            self.scale,
            self.zoom,
        ) {
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(size));
        }
    }
    fn rotate(&mut self, right: bool, ctx: &egui::Context) {
        self.image = if right {
            image::imageops::rotate90(&self.image)
        } else {
            image::imageops::rotate270(&self.image)
        };
        self.texture.set(
            egui::ColorImage::from_rgba_unmultiplied(
                [self.image.width() as usize, self.image.height() as usize],
                self.image.as_raw(),
            ),
            self.texture_options,
        );
        self.resize(ctx);
    }
}
impl eframe::App for Pinned {
    fn clear_color(&self, _: &egui::Visuals) -> [f32; 4] {
        [0.0; 4]
    }
    fn ui(&mut self, ui: &mut egui::Ui, _: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if ui.input(|i| i.key_pressed(egui::Key::Escape))
            || self
                .shortcuts
                .actions(&ctx)
                .iter()
                .any(|name| name == "TYPE_CLOSE_ACTIVE_PIN_WINDOW")
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        for (index, key) in [
            egui::Key::Num0,
            egui::Key::Num1,
            egui::Key::Num2,
            egui::Key::Num3,
            egui::Key::Num4,
            egui::Key::Num5,
            egui::Key::Num6,
            egui::Key::Num7,
            egui::Key::Num8,
            egui::Key::Num9,
        ]
        .into_iter()
        .enumerate()
        {
            if ui.input(|i| i.key_pressed(key)) {
                self.opacity = if index == 0 { 1.0 } else { index as f32 / 10.0 };
            }
        }
        let scroll: f32 = ui.input(|i| {
            i.events
                .iter()
                .filter_map(|event| {
                    if let egui::Event::MouseWheel { delta, .. } = event {
                        Some(delta.y)
                    } else {
                        None
                    }
                })
                .sum()
        });
        let pinch = ui.input(|i| i.zoom_delta());
        if scroll != 0.0 || pinch != 1.0 {
            self.zoom = (self.zoom
                * if pinch != 1.0 {
                    pinch
                } else if scroll > 0.0 {
                    1.1
                } else {
                    1.0 / 1.1
                })
            .clamp(0.05, 8.0);
            self.resize(&ctx);
        }
        let available = ctx.content_rect();
        let aspect = self.image.width() as f32 / self.image.height() as f32;
        let width = available.width().min(available.height() * aspect);
        let rect =
            egui::Rect::from_center_size(available.center(), egui::vec2(width, width / aspect));
        ui.painter().image(
            self.texture.id(),
            rect,
            egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
            egui::Color32::from_white_alpha((self.opacity * 255.0).round() as u8),
        );
        let response = ui.interact(
            available,
            egui::Id::unique("pin-drag"),
            egui::Sense::click_and_drag(),
        );
        if response.drag_started() {
            ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
        }
        if response.double_clicked() {
            self.zoom = 1.0;
            self.resize(&ctx);
        }
        response.context_menu(|ui| {
            if ui.button(tr("PinWidget", "Rotate Left")).clicked() {
                self.rotate(false, &ctx);
                ui.close();
            }
            if ui.button(tr("PinWidget", "Rotate Right")).clicked() {
                self.rotate(true, &ctx);
                ui.close();
            }
            if ui.button(tr("PinWidget", "Original size")).clicked() {
                self.zoom = 1.0;
                self.resize(&ctx);
                ui.close();
            }
            ui.add(
                egui::Slider::new(&mut self.opacity, 0.0..=1.0).text(tr("PinWidget", "Opacity")),
            );
            if ui.button(tr("PinWidget", "Close")).clicked() {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        });
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn zoom_preserves_aspect_ratio_and_device_scale() {
        let size = scaled_size(600, 300, 1.5, 2.0).unwrap();
        assert_eq!(size, egui::vec2(800.0, 400.0));
        let size = scaled_size(10000, 5000, 1.0, 8.0).unwrap();
        assert_eq!(size, egui::vec2(16384.0, 8192.0));
        assert!(scaled_size(100, 100, 1e-300, 1.0).is_err());
        assert!(scaled_size(0, 100, 1.0, 1.0).is_err());
    }
}
