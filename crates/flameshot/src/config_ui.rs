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
            viewport: crate::app_ui::viewport().with_inner_size([520.0, 420.0]),
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
            let mut config = Config {
                initial_language: settings.language.clone(),
                buttons_reset: false,
                baseline: Default::default(),
                shortcut_baseline: Default::default(),
                startup_baseline: false,
                palette,
                settings,
                path,
                status: String::new(),
            };
            config.remember();
            Ok(Box::new(config))
        }),
    )
    .map_err(|e| anyhow::anyhow!("Configuration window: {e}"))
}
struct Config {
    settings: Settings,
    initial_language: String,
    path: String,
    palette: String,
    buttons_reset: bool,
    baseline: std::collections::BTreeMap<&'static str, String>,
    shortcut_baseline: std::collections::BTreeMap<String, String>,
    startup_baseline: bool,
    status: String,
}
impl Config {
    fn language_restart_needed(&self) -> bool {
        self.baseline
            .get("uiLanguage")
            .is_some_and(|language| language != &self.initial_language)
    }
    fn values(&self) -> Vec<(&'static str, String)> {
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
        {
            values.push((
                "buttons",
                self.settings
                    .buttons
                    .as_deref()
                    .unwrap_or(&crate::buttons::defaults())
                    .iter()
                    .map(i32::to_string)
                    .collect::<Vec<_>>()
                    .join(","),
            ));
        }
        values
    }
    fn reload_saved(&mut self, settings: Settings) {
        self.path = settings
            .save_path
            .as_ref()
            .map(|path| path.to_string_lossy().into_owned())
            .unwrap_or_default();
        self.palette = settings
            .palette
            .iter()
            .map(|color| {
                color
                    .map(flameshot_core::color::encode_palette)
                    .unwrap_or_else(|| "picker".into())
            })
            .collect::<Vec<_>>()
            .join(", ");
        self.settings = settings;
        self.remember();
    }
    fn remember(&mut self) {
        self.remember_file();
        self.startup_baseline = self.settings.startup_launch;
    }
    fn remember_file(&mut self) {
        self.buttons_reset = false;
        self.baseline = self.values().into_iter().collect();
        self.shortcut_baseline = crate::shortcuts::DEFAULTS
            .iter()
            .map(|(key, default)| {
                (
                    (*key).to_owned(),
                    self.settings
                        .shortcuts
                        .get(*key)
                        .cloned()
                        .unwrap_or_else(|| (*default).into()),
                )
            })
            .collect();
    }
    fn shortcut_changes(&self) -> std::collections::BTreeMap<String, String> {
        self.settings
            .shortcuts
            .iter()
            .filter(|(key, value)| self.shortcut_baseline.get(*key) != Some(*value))
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect()
    }
    fn changes(&self) -> Vec<(&'static str, String)> {
        self.values()
            .into_iter()
            .filter(|(key, value)| {
                (*key == "buttons" && self.buttons_reset) || self.baseline.get(key) != Some(value)
            })
            .collect()
    }
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
                    }
                }
                if ui.button("Restore default toolbar").clicked() {
                    self.settings.buttons = Some(crate::buttons::defaults());
                    self.buttons_reset = true;
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
            let save_requested = ui.button("Save settings").clicked();
            if save_requested {
                let values = self.changes();
                let shortcuts = self.shortcut_changes();
                let unchanged = values.is_empty()
                    && shortcuts.is_empty()
                    && self.settings.startup_launch == self.startup_baseline;
                self.status = match crate::shortcuts::validate_overrides(&shortcuts)
                    .map_err(anyhow::Error::msg)
                    .and_then(|()| {
                        if values.is_empty() && shortcuts.is_empty() {
                            Ok(())
                        } else {
                            Settings::update_form(&values, &shortcuts)
                        }
                    })
                    .map(|()| self.remember_file())
                    .and_then(|()| {
                        if self.settings.startup_launch == self.startup_baseline {
                            Ok(())
                        } else {
                            Settings::set_startup(self.settings.startup_launch)
                        }
                    }) {
                    Ok(()) => {
                        self.remember();
                        let message = if unchanged {
                            "No changes to save"
                        } else {
                            "Settings saved"
                        };
                        match Settings::load() {
                            Ok(settings) => {
                                self.reload_saved(settings);
                                message.into()
                            }
                            Err(error) => format!("{message}; cannot reload settings: {error}"),
                        }
                    }
                    Err(e) => e.to_string(),
                };
            }
            ui.label(&self.status);
            if self.language_restart_needed() {
                let notice = ui.label(crate::i18n::tr(
                    "VisualsEditor",
                    "Flameshot must be restarted to apply these changes!",
                ));
                if save_requested {
                    notice.scroll_to_me(Some(egui::Align::BOTTOM));
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn form() -> Config {
        let mut form = Config {
            initial_language: Settings::default().language,
            settings: Settings::default(),
            path: String::new(),
            palette: "red, blue, picker".into(),
            buttons_reset: false,
            baseline: Default::default(),
            shortcut_baseline: Default::default(),
            startup_baseline: false,
            status: String::new(),
        };
        form.remember();
        form
    }
    #[test]
    fn saved_reload_updates_external_fields_and_keeps_original_language() {
        let mut form = form();
        let initial = form.initial_language.clone();
        let mut latest = Settings {
            filename_pattern: "external-pattern".into(),
            language: "zh_CN".into(),
            save_path: Some("/tmp/new folder".into()),
            startup_launch: true,
            ..Settings::default()
        };
        latest.shortcuts.insert("TYPE_ACCEPT".into(), "F9".into());
        form.reload_saved(latest);
        assert_eq!(form.settings.filename_pattern, "external-pattern");
        assert_eq!(form.path, "/tmp/new folder");
        assert_eq!(form.initial_language, initial);
        assert!(form.language_restart_needed());
        assert!(form.startup_baseline);
        assert!(form.changes().is_empty());
        assert!(form.shortcut_changes().is_empty());
    }
    #[test]
    fn restart_notice_tracks_committed_language_not_unsaved_edits() {
        let mut form = form();
        assert!(!form.language_restart_needed());
        form.settings.language = "zh_CN".into();
        assert!(!form.language_restart_needed());
        form.remember_file(); // Also covers a committed INI followed by startup failure.
        assert!(form.language_restart_needed());
        form.remember(); // A subsequent unchanged save must not hide the notice.
        assert!(form.language_restart_needed());
        form.settings.language = form.initial_language.clone();
        assert!(form.language_restart_needed());
        form.remember_file();
        assert!(!form.language_restart_needed());
        let translated = flameshot_core::i18n::Translator::new("zh_CN").translate(
            "VisualsEditor",
            "Flameshot must be restarted to apply these changes!",
        );
        assert_ne!(
            translated,
            "Flameshot must be restarted to apply these changes!"
        );
    }
    #[test]
    fn unchanged_fields_do_not_overwrite_newer_external_values() {
        let mut form = form();
        assert!(form.changes().is_empty());
        form.settings.show_help = !form.settings.show_help;
        let changes = form.changes();
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].0, "showHelp");
        let mut latest = flameshot_core::config::IniDocument::parse("[General]\nfilenamePattern=external\nshowDesktopNotification=false\n[Other]\nprivate=keep\n".into());
        for (key, value) in changes {
            latest.set_string("General", key, &value).unwrap();
        }
        assert_eq!(
            latest
                .string("General", "filenamePattern")
                .unwrap()
                .as_deref(),
            Some("external")
        );
        assert_eq!(
            latest
                .string("General", "showDesktopNotification")
                .unwrap()
                .as_deref(),
            Some("false")
        );
        assert_eq!(
            latest.string("Other", "private").unwrap().as_deref(),
            Some("keep")
        );
        form.remember();
        assert!(form.changes().is_empty());
    }
    #[test]
    fn file_commit_keeps_startup_pending_and_tracks_later_reverts() {
        let mut form = form();
        let original_help = form.settings.show_help;
        let original_startup = form.startup_baseline;
        form.settings.show_help = !original_help;
        form.settings.startup_launch = !original_startup;
        form.settings
            .shortcuts
            .insert("TYPE_COPY".into(), "F9".into());
        form.remember_file();
        assert!(form.changes().is_empty() && form.shortcut_changes().is_empty());
        assert_eq!(form.startup_baseline, original_startup);
        assert_ne!(form.settings.startup_launch, form.startup_baseline);
        form.settings.show_help = original_help;
        assert_eq!(form.changes()[0].0, "showHelp");
    }
    #[test]
    fn toolbar_and_shortcuts_compare_effective_defaults() {
        let mut form = form();
        form.settings.buttons = Some(crate::buttons::defaults());
        assert!(form.changes().is_empty());
        form.buttons_reset = true;
        assert_eq!(form.changes()[0].0, "buttons");
        form.remember();
        assert!(form.changes().is_empty());
        form.settings
            .shortcuts
            .insert("TYPE_COPY".into(), "Ctrl+C".into());
        assert!(form.shortcut_changes().is_empty());
        form.settings
            .shortcuts
            .insert("TYPE_COPY".into(), "F9".into());
        assert_eq!(form.shortcut_changes().len(), 1);
        form.remember();
        assert!(form.shortcut_changes().is_empty());
        form.settings
            .shortcuts
            .insert("TYPE_COPY".into(), String::new());
        assert_eq!(
            form.shortcut_changes().get("TYPE_COPY"),
            Some(&String::new())
        );
    }
}
