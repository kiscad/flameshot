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
            crate::theme::Theme::from_settings(&settings).apply(&cc.egui_ctx);
            let palette = settings
                .palette
                .iter()
                .map(|color| {
                    color
                        .map(flameshot_core::color::encode_palette)
                        .unwrap_or_else(|| "picker".into())
                })
                .collect::<Vec<_>>()
                .join(", ");
            Ok(Box::new(Config {
                buttons_changed: false,
                palette,
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
    palette: String,
    buttons_changed: bool,
    status: String,
}
impl eframe::App for Config {
    fn ui(&mut self, ui: &mut egui::Ui, _: &mut eframe::Frame) {
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
        }
        egui::ScrollArea::vertical().show(ui, |ui| {
            #[cfg(feature = "update-check")]
            ui.checkbox(&mut self.settings.check_updates, "Check for updates");
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
            ui.checkbox(
                &mut self.settings.save_path_fixed,
                "Use fixed save directory",
            );
            ui.label("Filename pattern");
            ui.text_edit_singleline(&mut self.settings.filename_pattern);
            egui::ComboBox::from_label("Format")
                .selected_text(&self.settings.extension)
                .show_ui(ui, |ui| {
                    for &value in crate::output::EXTENSIONS {
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
            ui.checkbox(
                &mut self.settings.antialias_pin_zoom,
                "Smooth pinned image zoom",
            );
            ui.checkbox(
                &mut self.settings.allow_multiple_gui,
                "Allow multiple capture editors",
            );
            ui.add(
                egui::DragValue::new(&mut self.settings.undo_limit)
                    .range(0..=999)
                    .prefix("Undo limit (0 = unlimited): "),
            );
            ui.horizontal(|ui| {
                ui.label("Main UI color");
                ui.color_edit_button_srgba_unmultiplied(&mut self.settings.ui_color);
                ui.label("Contrast UI color");
                ui.color_edit_button_srgba_unmultiplied(&mut self.settings.contrast_ui_color);
            });
            crate::theme::Theme::from_settings(&self.settings).apply(ui.ctx());
            ui.label("Palette (3–17 colors or picker, separated by commas)");
            ui.text_edit_singleline(&mut self.palette);
            ui.horizontal(|ui| {
                for (label, large) in [
                    ("Small default palette", false),
                    ("Large default palette", true),
                ] {
                    if ui.button(label).clicked() {
                        self.settings.large_palette = large;
                        self.palette = crate::settings::default_palette(large)
                            .iter()
                            .map(|color| {
                                color
                                    .map(flameshot_core::color::encode_palette)
                                    .unwrap_or_else(|| "picker".into())
                            })
                            .collect::<Vec<_>>()
                            .join(", ");
                    }
                }
            });
            ui.checkbox(
                &mut self.settings.capture_active_monitor,
                "Capture monitor under pointer (X11)",
            );
            ui.checkbox(
                &mut self.settings.use_x11_legacy_screenshot,
                "Use direct X11 capture instead of the portal",
            );
            ui.checkbox(
                &mut self.settings.insecure_pixelate,
                "Insecure pixelation (retains image information)",
            );
            #[cfg(feature = "imgur")]
            {
                ui.checkbox(
                    &mut self.settings.upload_without_confirmation,
                    "Upload without confirmation",
                );
                ui.checkbox(
                    &mut self.settings.copy_url_after_upload,
                    "Copy URL after upload",
                );
                ui.checkbox(
                    &mut self.settings.history_confirmation_to_delete,
                    "Confirm history image deletion",
                );
            }
            ui.checkbox(
                &mut self.settings.abort_notifications,
                "Notify when screenshot is aborted",
            );
            ui.checkbox(
                &mut self.settings.startup_message,
                "Show startup tray message",
            );
            ui.checkbox(
                &mut self.settings.show_quit_prompt,
                "Ask before quitting capture",
            );
            ui.checkbox(
                &mut self.settings.auto_close_idle,
                "Close daemon after clipboard and windows are released",
            );
            ui.checkbox(
                &mut self.settings.jpeg_clipboard,
                "Use JPEG for clipboard images",
            );
            ui.checkbox(&mut self.settings.save_after_copy, "Save image after copy");
            ui.checkbox(
                &mut self.settings.copy_path_after_save,
                "Copy file path after save",
            );
            ui.checkbox(
                &mut self.settings.copy_on_double_click,
                "Copy on double-click inside selection",
            );
            ui.checkbox(
                &mut self.settings.use_last_region,
                "Use last region for GUI mode",
            );
            ui.add(
                egui::Slider::new(&mut self.settings.contrast_opacity, 0..=255)
                    .text("Outside selection opacity"),
            );
            ui.checkbox(
                &mut self.settings.show_side_panel_button,
                "Show objects panel button",
            );
            const GEOMETRY: [&str; 6] = [
                "None",
                "Top left",
                "Top right",
                "Bottom left",
                "Bottom right",
                "Center",
            ];
            egui::ComboBox::from_label("Selection geometry")
                .selected_text(GEOMETRY[self.settings.selection_geometry as usize])
                .show_ui(ui, |ui| {
                    for (index, label) in GEOMETRY.iter().enumerate() {
                        ui.selectable_value(
                            &mut self.settings.selection_geometry,
                            index as u8,
                            *label,
                        );
                    }
                });
            ui.add(
                egui::DragValue::new(&mut self.settings.selection_geometry_timeout)
                    .range(0..=i32::MAX as u64)
                    .prefix("Geometry timeout (ms, 0 = always): "),
            );
            ui.checkbox(&mut self.settings.magnifier, "Show magnifier");
            ui.checkbox(&mut self.settings.square_magnifier, "Square magnifier");
            ui.checkbox(&mut self.settings.counter_outline, "Counter outline");
            ui.collapsing("Toolbar buttons", |ui| {
                let mut selected = self
                    .settings
                    .buttons
                    .clone()
                    .unwrap_or_else(crate::buttons::defaults);
                for (id, label) in crate::buttons::CATALOG {
                    if !crate::buttons::available(*id) {
                        continue;
                    }
                    let mut enabled = selected.contains(id);
                    if ui.checkbox(&mut enabled, *label).changed() {
                        if enabled {
                            selected.push(*id);
                        } else {
                            selected.retain(|value| value != id);
                        }
                        self.settings.buttons = Some(crate::buttons::normalize(selected.clone()));
                        self.buttons_changed = true;
                    }
                }
                if ui.button("Restore default toolbar").clicked() {
                    self.settings.buttons = Some(crate::buttons::defaults());
                    self.buttons_changed = true;
                }
            });
            ui.collapsing("Shortcuts", |ui| {
                for (name, default) in crate::shortcuts::DEFAULTS {
                    let mut value = self
                        .settings
                        .shortcuts
                        .get(*name)
                        .cloned()
                        .unwrap_or_else(|| (*default).into());
                    ui.horizontal(|ui| {
                        ui.label(name.trim_start_matches("TYPE_"));
                        if ui.text_edit_singleline(&mut value).changed() {
                            self.settings.shortcuts.insert((*name).into(), value);
                        }
                    });
                }
            });
            if ui.button("Save settings").clicked() {
                let mut values = vec![
                    (
                        "autoCloseIdleDaemon",
                        self.settings.auto_close_idle.to_string(),
                    ),
                    (
                        "useJpgForClipboard",
                        self.settings.jpeg_clipboard.to_string(),
                    ),
                    ("showQuitPrompt", self.settings.show_quit_prompt.to_string()),
                    (
                        "showAbortNotification",
                        self.settings.abort_notifications.to_string(),
                    ),
                    (
                        "showStartupLaunchMessage",
                        self.settings.startup_message.to_string(),
                    ),
                    #[cfg(feature = "imgur")]
                    (
                        "uploadWithoutConfirmation",
                        self.settings.upload_without_confirmation.to_string(),
                    ),
                    #[cfg(feature = "imgur")]
                    (
                        "copyURLAfterUpload",
                        self.settings.copy_url_after_upload.to_string(),
                    ),
                    #[cfg(feature = "imgur")]
                    (
                        "historyConfirmationToDelete",
                        self.settings.history_confirmation_to_delete.to_string(),
                    ),
                    (
                        "insecurePixelate",
                        self.settings.insecure_pixelate.to_string(),
                    ),
                    (
                        "showSidePanelButton",
                        self.settings.show_side_panel_button.to_string(),
                    ),
                    (
                        "showSelectionGeometry",
                        self.settings.selection_geometry.to_string(),
                    ),
                    (
                        "showSelectionGeometryHideTime",
                        self.settings.selection_geometry_timeout.to_string(),
                    ),
                    (
                        "captureActiveMonitor",
                        self.settings.capture_active_monitor.to_string(),
                    ),
                    (
                        "useX11LegacyScreenshot",
                        self.settings.use_x11_legacy_screenshot.to_string(),
                    ),
                    ("userColors", self.palette.clone()),
                    (
                        "predefinedColorPaletteLarge",
                        self.settings.large_palette.to_string(),
                    ),
                    (
                        "uiColor",
                        flameshot_core::color::encode(self.settings.ui_color),
                    ),
                    (
                        "contrastUiColor",
                        flameshot_core::color::encode(self.settings.contrast_ui_color),
                    ),
                    (
                        "copyOnDoubleClick",
                        self.settings.copy_on_double_click.to_string(),
                    ),
                    ("saveLastRegion", self.settings.use_last_region.to_string()),
                    (
                        "contrastOpacity",
                        self.settings.contrast_opacity.to_string(),
                    ),
                    ("saveAfterCopy", self.settings.save_after_copy.to_string()),
                    (
                        "copyPathAfterSave",
                        self.settings.copy_path_after_save.to_string(),
                    ),
                    ("undoLimit", self.settings.undo_limit.to_string()),
                    (
                        "allowMultipleGuiInstances",
                        self.settings.allow_multiple_gui.to_string(),
                    ),
                    (
                        "antialiasingPinZoom",
                        self.settings.antialias_pin_zoom.to_string(),
                    ),
                    ("savePathFixed", self.settings.save_path_fixed.to_string()),
                    #[cfg(feature = "update-check")]
                    ("checkForUpdates", self.settings.check_updates.to_string()),
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
                if self.buttons_changed {
                    values.push((
                        "buttons",
                        self.settings
                            .buttons
                            .as_deref()
                            .unwrap_or(&[])
                            .iter()
                            .map(i32::to_string)
                            .collect::<Vec<_>>()
                            .join(","),
                    ));
                }
                self.status = match crate::shortcuts::validate_overrides(&self.settings.shortcuts)
                    .map_err(anyhow::Error::msg)
                    .and_then(|()| Settings::update(&values))
                    .and_then(|()| Settings::set_startup(self.settings.startup_launch))
                    .and_then(|()| Settings::update_shortcuts(&self.settings.shortcuts))
                {
                    Ok(()) => "Settings saved".into(),
                    Err(e) => e.to_string(),
                };
            }
            ui.label(&self.status);
        });
    }
}
