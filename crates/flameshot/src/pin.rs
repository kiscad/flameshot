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
// Raw wheel and pinch events remain present when egui repeats a layout pass.
fn zoom_from_input(zoom: f32, frame: &mut Option<u64>, ctx: &egui::Context) -> Option<f32> {
    let current = ctx.cumulative_frame_nr();
    if *frame == Some(current) {
        return None;
    }
    *frame = Some(current);
    let (scroll, pinch) = ctx.input(|input| {
        let scroll: f32 = input
            .events
            .iter()
            .filter_map(|event| {
                if let egui::Event::MouseWheel { delta, .. } = event {
                    Some(delta.y)
                } else {
                    None
                }
            })
            .sum();
        (scroll, input.zoom_delta())
    });
    let factor = if pinch != 1.0 {
        pinch
    } else if scroll > 0.0 {
        1.1
    } else if scroll < 0.0 {
        1.0 / 1.1
    } else {
        return None;
    };
    Some((zoom * factor).clamp(0.05, 8.0))
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
                copy_job: None,
                copy_error: None,
                close_after_copy: false,
                shortcut_frame: None,
                zoom_frame: None,
                dismissed_dialog_frame: None,
                save_dialog: None,
                close_after_save: false,
                copy_path: None,
                save_error: false,
            }))
        }),
    )
    .map_err(|e| anyhow::anyhow!("Cannot display pin: {e}"))
}
struct PinSave {
    dialog: crate::save_dialog::SaveDialog,
    image: image::RgbaImage,
    settings: crate::settings::Settings,
}
struct Pinned {
    shortcuts: crate::shortcuts::Shortcuts,
    texture: egui::TextureHandle,
    image: image::RgbaImage,
    scale: f64,
    zoom: f32,
    opacity: f32,
    texture_options: egui::TextureOptions,
    copy_job: Option<std::sync::mpsc::Receiver<Result<(), String>>>,
    copy_error: Option<String>,
    close_after_copy: bool,
    shortcut_frame: Option<u64>,
    zoom_frame: Option<u64>,
    dismissed_dialog_frame: Option<u64>,
    save_dialog: Option<PinSave>,
    close_after_save: bool,
    copy_path: Option<std::path::PathBuf>,
    save_error: bool,
}
impl Pinned {
    fn copy(&mut self, ctx: &egui::Context) {
        if self.copy_job.is_some() {
            return;
        }
        self.copy_error = None;
        self.copy_path = None;
        self.save_error = false;
        let (sender, receiver) = std::sync::mpsc::channel();
        self.copy_job = Some(receiver);
        let image = self.image.clone();
        let ctx = ctx.clone();
        tokio::spawn(async move {
            let result = crate::daemon::copy(&image)
                .await
                .map_err(|error| error.to_string());
            let _ = sender.send(result);
            ctx.request_repaint();
        });
    }
    fn request_close(&mut self, ctx: &egui::Context) {
        if self.save_dialog.is_some() {
            self.close_after_save = true;
            return;
        }
        if self.copy_job.is_some() {
            self.close_after_copy = true;
        } else {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
    fn poll_copy(&mut self, ctx: &egui::Context) {
        let Some(job) = &self.copy_job else {
            return;
        };
        let Some(result) = crate::jobs::poll(job, "Clipboard worker stopped") else {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
            return;
        };
        self.copy_job = None;
        match result {
            Ok(()) if self.close_after_copy => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
            Ok(()) => {}
            Err(error) => self.copy_error = Some(error),
        }
        self.close_after_copy = false;
    }
    fn start_save(&mut self, ctx: &egui::Context) {
        self.copy_error = None;
        let settings = match crate::settings::Settings::load() {
            Ok(settings) => settings,
            Err(error) => {
                self.save_error = true;
                self.copy_error = Some(error.to_string());
                return;
            }
        };
        let directory = settings.save_directory();
        let mut error = String::new();
        if settings.save_path_fixed {
            match crate::output::save_with_settings(&self.image, &directory, &settings) {
                Ok(path) => {
                    self.saved(ctx, path, &settings);
                    return;
                }
                Err(failure) => error = failure.to_string(),
            }
        }
        let path = directory.join(format!(
            "{}.{}",
            chrono::Local::now().format(&settings.filename_pattern),
            settings.extension
        ));
        self.save_dialog = Some(PinSave {
            dialog: crate::save_dialog::SaveDialog::new(path, error),
            image: self.image.clone(),
            settings,
        });
        ctx.send_viewport_cmd(egui::ViewportCommand::WindowLevel(
            egui::WindowLevel::Normal,
        ));
        ctx.request_repaint();
    }
    fn saved(
        &mut self,
        ctx: &egui::Context,
        path: std::path::PathBuf,
        settings: &crate::settings::Settings,
    ) {
        let path = std::path::absolute(&path).unwrap_or(path);
        eprintln!("flameshot: Capture saved as {}", path.display());
        crate::save_dialog::remember_directory(&path);
        self.copy_path = settings
            .copy_path_after_save
            .then(|| std::fs::canonicalize(&path).unwrap_or_else(|_| path.clone()));
        self.save_error = false;
        if !settings.notifications && self.copy_path.is_none() {
            return;
        }
        let (sender, receiver) = std::sync::mpsc::channel();
        self.copy_job = Some(receiver);
        let copy_path = self.copy_path.clone();
        let notify = settings.notifications;
        let ctx = ctx.clone();
        tokio::spawn(async move {
            if notify && let Ok(connection) = crate::daemon::session_connection().await {
                let body = format!("{}{}", tr("QObject", "Capture saved as "), path.display());
                let _ =
                    flameshot_platform::desktop::notify_no_reply(&connection, "Flameshot", &body)
                        .await;
            }
            let result = if let Some(path) = copy_path {
                crate::daemon::copy_text(&path.to_string_lossy())
                    .await
                    .map_err(|error| error.to_string())
            } else {
                Ok(())
            };
            let _ = sender.send(result);
            ctx.request_repaint();
        });
    }
    fn retry_copy_path(&mut self, ctx: &egui::Context, path: std::path::PathBuf) {
        self.copy_error = None;
        let (sender, receiver) = std::sync::mpsc::channel();
        self.copy_job = Some(receiver);
        let ctx = ctx.clone();
        tokio::spawn(async move {
            let result = crate::daemon::copy_text(&path.to_string_lossy())
                .await
                .map_err(|error| error.to_string());
            let _ = sender.send(result);
            ctx.request_repaint();
        });
    }
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
    fn raw_input_hook(&mut self, ctx: &egui::Context, input: &mut egui::RawInput) {
        if (self.copy_job.is_some() || self.save_dialog.is_some())
            && input.viewport_id == egui::ViewportId::ROOT
            && let Some(viewport) = input.viewports.get_mut(&input.viewport_id)
            && viewport.close_requested()
        {
            viewport
                .events
                .retain(|event| !matches!(event, egui::ViewportEvent::Close));
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            if self.save_dialog.is_some() {
                self.close_after_save = true;
            } else {
                self.close_after_copy = true;
            }
        }
    }
    fn clear_color(&self, _: &egui::Visuals) -> [f32; 4] {
        [0.0; 4]
    }
    fn ui(&mut self, ui: &mut egui::Ui, _: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.poll_copy(&ctx);
        if self.save_dialog.is_some() || self.copy_error.is_some() {
            self.zoom_frame = Some(ctx.cumulative_frame_nr());
        }
        if let Some(mut save) = self.save_dialog.take() {
            let mut completed = None;
            ctx.show_viewport_immediate(
                egui::ViewportId::from_hash_of("pin-save-dialog"),
                crate::app_ui::viewport()
                    .with_title(tr("RustEditor", "Save Screenshot"))
                    .with_inner_size([620.0, 520.0])
                    .with_min_inner_size([420.0, 320.0]),
                |ui, _| {
                    if completed.is_some() {
                        return;
                    }
                    let result = if ui.input(|input| input.viewport().close_requested()) {
                        save.dialog.cancel();
                        crate::save_dialog::SaveOutcome::Cancelled
                    } else {
                        save.dialog.show(ui, |path, extension| {
                            save.settings = crate::settings::Settings::load()?;
                            let settings = &save.settings;
                            if let Some(extension) = extension {
                                crate::output::save_selected(&save.image, path, extension, settings)
                            } else {
                                crate::output::save_with_settings(&save.image, path, settings)
                            }
                        })
                    };
                    if !matches!(result, crate::save_dialog::SaveOutcome::Pending) {
                        ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                        completed = Some(result);
                    }
                },
            );
            match completed {
                None => self.save_dialog = Some(save),
                Some(result) => {
                    ctx.send_viewport_cmd(egui::ViewportCommand::WindowLevel(
                        egui::WindowLevel::AlwaysOnTop,
                    ));
                    ctx.send_viewport_cmd(egui::ViewportCommand::MousePassthrough(false));
                    self.dismissed_dialog_frame = Some(ctx.cumulative_frame_nr());
                    self.shortcuts.reset();
                    if let crate::save_dialog::SaveOutcome::Saved(path) = result {
                        self.saved(&ctx, path, &save.settings);
                    }
                    if self.close_after_save {
                        self.close_after_save = false;
                        self.request_close(&ctx);
                    }
                    ctx.request_repaint();
                }
            }
            return;
        }
        let actions = if self.shortcut_frame == Some(ctx.cumulative_frame_nr()) {
            Vec::new()
        } else {
            self.shortcut_frame = Some(ctx.cumulative_frame_nr());
            self.shortcuts.actions(&ctx)
        };
        if self.copy_error.is_some() {
            self.shortcuts.reset_sequences();
        }
        if self.copy_error.is_none()
            && self.dismissed_dialog_frame != Some(ctx.cumulative_frame_nr())
            && (ui.input(|i| i.key_pressed(egui::Key::Escape))
                || actions
                    .iter()
                    .any(|name| name == "TYPE_CLOSE_ACTIVE_PIN_WINDOW"))
        {
            self.request_close(&ctx);
        }
        if self.copy_error.is_none() {
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
            if let Some(zoom) = zoom_from_input(self.zoom, &mut self.zoom_frame, &ctx) {
                self.zoom = zoom;
                self.resize(&ctx);
            }
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
        if let Some(error) = self.copy_error.clone() {
            let mut retry = false;
            let mut dismiss = ctx.input(|input| input.key_pressed(egui::Key::Escape));
            let title = if self.save_error {
                tr("QObject", "Save Error")
            } else {
                tr("QObject", "Error while saving to clipboard")
            };
            egui::Window::new(title)
                .collapsible(false)
                .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
                .show(&ctx, |ui| {
                    ui.label(error);
                    let retry_label = if self.save_error {
                        tr("PinWidget", "Save to file")
                    } else {
                        tr("PinWidget", "Copy to clipboard")
                    };
                    retry = ui.button(retry_label).clicked();
                    dismiss |= ui.button(tr("PinWidget", "Close")).clicked();
                });
            if retry || dismiss {
                self.dismissed_dialog_frame = Some(ctx.cumulative_frame_nr());
            }
            if retry {
                if self.save_error {
                    self.start_save(&ctx);
                } else if let Some(path) = self.copy_path.clone() {
                    self.retry_copy_path(&ctx, path);
                } else {
                    self.copy(&ctx);
                }
            } else if dismiss {
                self.copy_error = None;
            }
            return;
        }
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
            if ui
                .add_enabled(
                    self.copy_job.is_none(),
                    egui::Button::new(tr("PinWidget", "Copy to clipboard")),
                )
                .clicked()
            {
                self.copy(&ctx);
                ui.close();
            }
            if ui
                .add_enabled(
                    self.copy_job.is_none(),
                    egui::Button::new(tr("PinWidget", "Save to file")),
                )
                .clicked()
            {
                self.start_save(&ctx);
                ui.close();
            }
            ui.separator();
            if ui.button(tr("PinWidget", "Rotate Right")).clicked() {
                self.rotate(true, &ctx);
                ui.close();
            }
            if ui.button(tr("PinWidget", "Rotate Left")).clicked() {
                self.rotate(false, &ctx);
                ui.close();
            }
            if ui.button(tr("PinWidget", "Original size")).clicked() {
                self.zoom = 1.0;
                self.resize(&ctx);
                ui.close();
            }
            if ui.button(tr("PinWidget", "Increase Opacity")).clicked() {
                self.opacity = (self.opacity + 0.1).min(1.0);
                ui.close();
            }
            if ui.button(tr("PinWidget", "Decrease Opacity")).clicked() {
                self.opacity = (self.opacity - 0.1).max(0.0);
                ui.close();
            }
            ui.add(
                egui::Slider::new(&mut self.opacity, 0.0..=1.0).text(tr("PinWidget", "Opacity")),
            );
            if ui.button(tr("PinWidget", "Close")).clicked() {
                self.request_close(&ctx);
            }
        });
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn zoom_events_apply_once_per_frame_and_accept_next_frame() {
        let ctx = egui::Context::default();
        let mut zoom = 1.0;
        let mut frame = None;
        let wheel = |y| egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            phase: egui::TouchPhase::Move,
            source: egui::MouseWheelSource::Unknown,
            delta: egui::vec2(0.0, y),
            modifiers: egui::Modifiers::NONE,
        };
        for (event, expected) in [
            (wheel(1.0), 1.1),
            (wheel(1.0), 1.21),
            (wheel(-1.0), 1.1),
            (egui::Event::Zoom(2.0), 2.2),
            (egui::Event::Zoom(10.0), 8.0),
            (egui::Event::Zoom(0.001), 0.05),
        ] {
            let mut passes = 0;
            let mut changes = 0;
            crate::ui_test::ui_frame(&ctx, vec![event], &mut |ui| {
                passes += 1;
                if let Some(value) = zoom_from_input(zoom, &mut frame, ui.ctx()) {
                    zoom = value;
                    changes += 1;
                }
                if ui.ctx().current_pass_index() == 0 {
                    ui.ctx()
                        .request_discard("exercise pin zoom across layout passes");
                }
            });
            assert_eq!(passes, 2);
            assert_eq!(changes, 1);
            assert!((zoom - expected).abs() < 0.00001, "{zoom} != {expected}");
        }
    }
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
