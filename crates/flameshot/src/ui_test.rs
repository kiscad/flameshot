// SPDX-License-Identifier: GPL-3.0-or-later
//! Widget-frame input helpers; compiled only for tests.
use eframe::egui;
pub(crate) fn ui_frame(
    ctx: &egui::Context,
    events: Vec<egui::Event>,
    render: &mut impl FnMut(&mut egui::Ui),
) -> egui::FullOutput {
    let mut output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(700.0, 500.0),
            )),
            events,
            ..Default::default()
        },
        render,
    );
    output.textures_delta.clear();
    output
}
fn label_position(shape: &egui::Shape, label: &str) -> Option<egui::Pos2> {
    match shape {
        egui::Shape::Text(text) if text.galley.job.text == label => {
            Some(text.pos + text.galley.rect.center().to_vec2())
        }
        egui::Shape::Vec(shapes) => shapes.iter().find_map(|shape| label_position(shape, label)),
        _ => None,
    }
}
pub(crate) fn click_label(
    ctx: &egui::Context,
    label: &str,
    mut render: impl FnMut(&mut egui::Ui),
) -> egui::FullOutput {
    ui_frame(ctx, vec![], &mut render);
    let mut output = ui_frame(ctx, vec![], &mut render);
    let point = output
        .shapes
        .iter()
        .find_map(|shape| label_position(&shape.shape, label))
        .unwrap_or_else(|| panic!("Missing UI label: {label}"));
    for pressed in [true, false] {
        output = ui_frame(
            ctx,
            vec![
                egui::Event::PointerMoved(point),
                egui::Event::PointerButton {
                    pos: point,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
            &mut render,
        );
    }
    output
}
