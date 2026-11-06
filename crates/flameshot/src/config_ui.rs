// SPDX-License-Identifier: GPL-3.0-or-later
use crate::settings::Settings;
use eframe::egui;
pub fn show() -> anyhow::Result<()> {
    let settings = Settings::load()?;
    let path = settings
        .save_path
        .as_ref()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_default();
    eframe::run_native(
        "Flameshot Configuration",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default().with_inner_size([520.0, 420.0]),
            ..Default::default()
        },
        Box::new(move |cc| {
            crate::i18n::install_fonts(&cc.egui_ctx);
            Ok(Box::new(Config {
                settings,
                path,
                status: String::new(),
            }))
        }),
    )
    .map_err(|e| anyhow::anyhow!("Configuration window: {e}"))
}
struct Config {
    settings: Settings,
    path: String,
    status: String,
}
impl eframe::App for Config {
    fn ui(&mut self, ui: &mut egui::Ui, _: &mut eframe::Frame) {
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
        }
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.heading(crate::i18n::tr("ConfigWindow", "Configuration"));
            egui::ComboBox::from_label("Language")
                .selected_text(&self.settings.language)
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.settings.language, "auto".into(), "Auto");
                    for language in flameshot_core::i18n::languages() {
                        ui.selectable_value(&mut self.settings.language, language.into(), language);
                    }
                });
            ui.label("Save directory");
            ui.text_edit_singleline(&mut self.path);
            ui.label("Filename pattern");
            ui.text_edit_singleline(&mut self.settings.filename_pattern);
            ui.horizontal(|ui| {
                ui.label("Format");
                for value in ["png", "jpg"] {
                    ui.selectable_value(
                        &mut self.settings.extension,
                        value.into(),
                        value.to_uppercase(),
                    );
                }
            });
            ui.add(
                egui::Slider::new(&mut self.settings.jpeg_quality, 0..=100).text("JPEG quality"),
            );
            ui.checkbox(&mut self.settings.disabled_tray, "Disable tray icon");
            ui.checkbox(&mut self.settings.notifications, "Desktop notifications");
            ui.checkbox(&mut self.settings.show_help, "Show capture help");
            ui.checkbox(&mut self.settings.startup_launch, "Launch at startup");
            ui.checkbox(&mut self.settings.magnifier, "Show magnifier");
            ui.checkbox(&mut self.settings.square_magnifier, "Square magnifier");
            ui.checkbox(&mut self.settings.counter_outline, "Counter outline");
            if ui.button("Save settings").clicked() {
                let values = [
                    ("showMagnifier", self.settings.magnifier.to_string()),
                    (
                        "squareMagnifier",
                        self.settings.square_magnifier.to_string(),
                    ),
                    (
                        "drawCircleCounterOutline",
                        self.settings.counter_outline.to_string(),
                    ),
                    ("uiLanguage", self.settings.language.clone()),
                    ("savePath", self.path.clone()),
                    ("filenamePattern", self.settings.filename_pattern.clone()),
                    ("saveAsFileExtension", self.settings.extension.clone()),
                    ("jpegQuality", self.settings.jpeg_quality.to_string()),
                    ("disabledTrayIcon", self.settings.disabled_tray.to_string()),
                    (
                        "showDesktopNotification",
                        self.settings.notifications.to_string(),
                    ),
                    ("showHelp", self.settings.show_help.to_string()),
                ];
                self.status = match Settings::update(&values)
                    .and_then(|()| Settings::set_startup(self.settings.startup_launch))
                {
                    Ok(()) => "Settings saved".into(),
                    Err(e) => e.to_string(),
                };
            }
            ui.label(&self.status);
        });
    }
}
