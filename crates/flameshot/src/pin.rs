// SPDX-License-Identifier: GPL-3.0-or-later
use eframe::egui;
use flameshot_core::qt_wire::Pin;
pub fn show(pin: Pin) -> anyhow::Result<()> {
    let size = [
        pin.image.width() as f32 / pin.scale as f32,
        pin.image.height() as f32 / pin.scale as f32,
    ];
    let pixels = egui::ColorImage::from_rgba_unmultiplied(
        [pin.image.width() as usize, pin.image.height() as usize],
        pin.image.as_raw(),
    );
    eframe::run_native(
        "flameshot-pin",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_decorations(false)
                .with_inner_size(size)
                .with_position([pin.geometry.x as f32, pin.geometry.y as f32])
                .with_always_on_top(),
            ..Default::default()
        },
        Box::new(move |cc| {
            Ok(Box::new(Pinned {
                texture: cc
                    .egui_ctx
                    .load_texture("pin", pixels, egui::TextureOptions::LINEAR),
            }))
        }),
    )
    .map_err(|e| anyhow::anyhow!("Cannot display pin: {e}"))
}
struct Pinned {
    texture: egui::TextureHandle,
}
impl eframe::App for Pinned {
    fn ui(&mut self, ui: &mut egui::Ui, _: &mut eframe::Frame) {
        if ui.input(|i| {
            i.key_pressed(egui::Key::Escape) || (i.modifiers.command && i.key_pressed(egui::Key::Q))
        }) {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
        }
        let rect = ui.ctx().content_rect();
        ui.painter().image(
            self.texture.id(),
            rect,
            egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
            egui::Color32::WHITE,
        );
        if ui
            .interact(rect, egui::Id::unique("pin-drag"), egui::Sense::drag())
            .drag_started()
        {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
        }
    }
}
