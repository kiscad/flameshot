// SPDX-License-Identifier: GPL-3.0-or-later
use crate::settings::Settings;
use eframe::egui;
fn tr(source: &str) -> &str {
    crate::i18n::tr("RustConfig", source)
}
fn shortcut_label(name: &str) -> &str {
    match name {
        "TYPE_PENCIL" => tr("Pencil"),
        "TYPE_DRAWER" => tr("Line"),
        "TYPE_ARROW" => tr("Arrow"),
        "TYPE_SELECTION" => tr("Outline rectangle"),
        "TYPE_RECTANGLE" => tr("Filled rectangle"),
        "TYPE_CIRCLE" => tr("Ellipse"),
        "TYPE_MARKER" => tr("Marker"),
        "TYPE_MOVESELECTION" => tr("Move selection"),
        "TYPE_UNDO" => tr("Undo"),
        "TYPE_COPY" => tr("Copy"),
        "TYPE_SAVE" => tr("Save"),
        "TYPE_ACCEPT" => tr("Accept"),
        "TYPE_IMAGEUPLOADER" => tr("Upload"),
        "TYPE_OPEN_APP" => tr("Open with"),
        "TYPE_PIXELATE" => tr("Pixelate"),
        "TYPE_INVERT" => tr("Invert"),
        "TYPE_REDO" => tr("Redo"),
        "TYPE_TEXT" => tr("Text"),
        "TYPE_PIN" => tr("Pin"),
        "TYPE_SIZEINCREASE" => tr("Increase size"),
        "TYPE_SIZEDECREASE" => tr("Decrease size"),
        "TYPE_CIRCLECOUNT" => tr("Circle counter"),
        "TYPE_TOGGLE_PANEL" => crate::i18n::tr("ShortcutsWidget", "Toggle side panel"),
        "TYPE_GRAB_COLOR" => crate::i18n::tr("ShortcutsWidget", "Grab a color from the screen"),
        "TYPE_RESIZE_LEFT" => crate::i18n::tr("ShortcutsWidget", "Resize selection left 1px"),
        "TYPE_RESIZE_RIGHT" => crate::i18n::tr("ShortcutsWidget", "Resize selection right 1px"),
        "TYPE_RESIZE_UP" => crate::i18n::tr("ShortcutsWidget", "Resize selection up 1px"),
        "TYPE_RESIZE_DOWN" => crate::i18n::tr("ShortcutsWidget", "Resize selection down 1px"),
        "TYPE_SYM_RESIZE_LEFT" => {
            crate::i18n::tr("ShortcutsWidget", "Symmetrically decrease width by 2px")
        }
        "TYPE_SYM_RESIZE_RIGHT" => {
            crate::i18n::tr("ShortcutsWidget", "Symmetrically increase width by 2px")
        }
        "TYPE_SYM_RESIZE_UP" => {
            crate::i18n::tr("ShortcutsWidget", "Symmetrically increase height by 2px")
        }
        "TYPE_SYM_RESIZE_DOWN" => {
            crate::i18n::tr("ShortcutsWidget", "Symmetrically decrease height by 2px")
        }
        "TYPE_SELECT_ALL" => crate::i18n::tr("ShortcutsWidget", "Select entire screen"),
        "TYPE_MOVE_LEFT" => crate::i18n::tr("ShortcutsWidget", "Move selection left 1px"),
        "TYPE_MOVE_RIGHT" => crate::i18n::tr("ShortcutsWidget", "Move selection right 1px"),
        "TYPE_MOVE_UP" => crate::i18n::tr("ShortcutsWidget", "Move selection up 1px"),
        "TYPE_MOVE_DOWN" => crate::i18n::tr("ShortcutsWidget", "Move selection down 1px"),
        "TYPE_COMMIT_CURRENT_TOOL" => {
            crate::i18n::tr("ShortcutsWidget", "Commit text in text area")
        }
        "TYPE_DELETE_CURRENT_TOOL" => {
            crate::i18n::tr("ShortcutsWidget", "Delete selected drawn object")
        }
        "TYPE_CANCEL" => crate::i18n::tr("ShortcutsWidget", "Cancel current selection"),
        "TYPE_CLOSE_ACTIVE_PIN_WINDOW" => {
            crate::i18n::tr("ShortcutsWidget", "Close Active Pinned Window")
        }
        "TYPE_EXIT" => crate::i18n::tr("QObject", "Quit capture"),
        _ => name,
    }
}
pub fn show() -> anyhow::Result<()> {
    let recovery = crate::config_recovery::Recovery::load()?;
    let settings = if recovery.is_some() {
        Settings::default()
    } else {
        Settings::load()?
    };
    let path = settings
        .save_path
        .as_ref()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_default();
    eframe::run_native(
        tr("Flameshot Configuration"),
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
                recovery,
                recovered_frame: None,
                initial_language: settings.language.clone(),
                buttons_reset: false,
                baseline: Default::default(),
                shortcut_baseline: Default::default(),
                startup_baseline: false,
                palette,
                settings,
                path,
                status: String::new(),
                last_refresh: std::time::Instant::now(),
                reload_error: None,
            };
            config.remember();
            Ok(Box::new(config))
        }),
    )
    .map_err(|e| anyhow::anyhow!("Configuration window: {e}"))
}
#[derive(Clone)]
struct Config {
    recovery: Option<crate::config_recovery::Recovery>,
    recovered_frame: Option<u64>,
    settings: Settings,
    initial_language: String,
    path: String,
    palette: String,
    buttons_reset: bool,
    baseline: std::collections::BTreeMap<&'static str, String>,
    shortcut_baseline: std::collections::BTreeMap<String, String>,
    startup_baseline: bool,
    status: String,
    last_refresh: std::time::Instant,
    reload_error: Option<String>,
}
impl Config {
    fn validate_external_changes(
        &self,
        current: &Settings,
        proposed: &Settings,
    ) -> anyhow::Result<()> {
        let mut latest = self.clone();
        latest.reload_saved(current.clone());
        let mut desired = self.clone();
        desired.reload_saved(proposed.clone());
        let general_conflict = self.changes().iter().any(|(key, _)| {
            latest.baseline.get(key) != self.baseline.get(key)
                && latest.baseline.get(key) != desired.baseline.get(key)
        });
        let shortcut_conflict = self.shortcut_changes().keys().any(|key| {
            let value = |map: &std::collections::BTreeMap<String, String>| {
                map.get(key).cloned().unwrap_or_default()
            };
            !crate::shortcuts::equivalent(
                &value(&latest.shortcut_baseline),
                &value(&self.shortcut_baseline),
            ) && !crate::shortcuts::equivalent(
                &value(&latest.shortcut_baseline),
                &value(&desired.shortcut_baseline),
            )
        });
        anyhow::ensure!(
            !general_conflict && !shortcut_conflict,
            "{}",
            tr("Settings changed outside this window. Reload saved settings before saving.")
        );
        Ok(())
    }
    fn is_dirty(&self) -> bool {
        !self.changes().is_empty()
            || !self.shortcut_changes().is_empty()
            || self.settings.startup_launch != self.startup_baseline
    }
    fn refresh_if_clean(&mut self, load: impl FnOnce() -> anyhow::Result<Settings>) {
        if self.is_dirty() {
            return;
        }
        match load() {
            Ok(settings) => {
                self.reload_saved(settings);
                self.reload_error = None;
            }
            Err(error) => self.reload_error = Some(error.to_string()),
        }
    }
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
        if self.recovered_frame == Some(ui.ctx().cumulative_frame_nr()) {
            return;
        }
        if let Some(recovery) = &mut self.recovery {
            if recovery.show(ui) {
                match Settings::load() {
                    Ok(settings) => {
                        self.recovered_frame = Some(ui.ctx().cumulative_frame_nr());
                        self.recovery = None;
                        self.reload_saved(settings);
                    }
                    Err(error) => recovery.status = error.to_string(),
                }
            }
            return;
        }
        let refresh_interval = std::time::Duration::from_secs(1);
        if self.last_refresh.elapsed() >= refresh_interval {
            if !self.is_dirty()
                && let Some(recovery) = crate::config_recovery::Recovery::load().ok().flatten()
            {
                self.recovery = Some(recovery);
                return;
            }
            self.refresh_if_clean(Settings::load);
            self.last_refresh = std::time::Instant::now();
        }
        ui.ctx().request_repaint_after(refresh_interval);
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
        }
        egui::ScrollArea::vertical().show(ui, |ui| {
            #[cfg(feature = "update-check")]
            ui.checkbox(&mut self.settings.check_updates, tr("Check for updates"));
            ui.heading(crate::i18n::tr("ConfigWindow", "Configuration"));
            egui::ComboBox::from_label(tr("Language"))
                .selected_text(&self.settings.language)
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.settings.language, "auto".into(), tr("Auto"));
                    for language in flameshot_core::i18n::languages() {
                        ui.selectable_value(&mut self.settings.language, language.into(), language);
                    }
                });
            ui.label(tr("Save directory"));
            ui.text_edit_singleline(&mut self.path);
            ui.checkbox(
                &mut self.settings.save_path_fixed,
                tr("Use fixed save directory"),
            );
            ui.label(tr("Filename pattern"));
            ui.text_edit_singleline(&mut self.settings.filename_pattern);
            egui::ComboBox::from_label(tr("Format"))
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
                egui::Slider::new(&mut self.settings.jpeg_quality, 0..=100)
                    .text(tr("JPEG quality")),
            );
            ui.checkbox(&mut self.settings.disabled_tray, tr("Disable tray icon"));
            ui.checkbox(
                &mut self.settings.notifications,
                tr("Desktop notifications"),
            );
            ui.checkbox(&mut self.settings.show_help, tr("Show capture help"));
            ui.checkbox(&mut self.settings.startup_launch, tr("Launch at startup"));
            ui.checkbox(
                &mut self.settings.antialias_pin_zoom,
                tr("Smooth pinned image zoom"),
            );
            ui.checkbox(
                &mut self.settings.allow_multiple_gui,
                tr("Allow multiple capture editors"),
            );
            ui.add(
                egui::DragValue::new(&mut self.settings.undo_limit)
                    .range(0..=999)
                    .prefix(tr("Undo limit (0 = unlimited): ")),
            );
            ui.horizontal(|ui| {
                ui.label(tr("Main UI color"));
                ui.color_edit_button_srgba_unmultiplied(&mut self.settings.ui_color);
                ui.label(tr("Contrast UI color"));
                ui.color_edit_button_srgba_unmultiplied(&mut self.settings.contrast_ui_color);
            });
            crate::theme::Theme::from_settings(&self.settings).apply(ui.ctx());
            ui.label(tr("Palette (3–17 colors or picker, separated by commas)"));
            ui.text_edit_singleline(&mut self.palette);
            ui.horizontal(|ui| {
                for (label, large) in [
                    ("Small default palette", false),
                    ("Large default palette", true),
                ] {
                    if ui.button(tr(label)).clicked() {
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
                tr("Capture monitor under pointer (X11)"),
            );
            ui.checkbox(
                &mut self.settings.use_x11_legacy_screenshot,
                tr("Use direct X11 capture instead of the portal"),
            );
            ui.checkbox(
                &mut self.settings.insecure_pixelate,
                tr("Insecure pixelation (retains image information)"),
            );
            #[cfg(feature = "imgur")]
            {
                ui.checkbox(
                    &mut self.settings.upload_without_confirmation,
                    tr("Upload without confirmation"),
                );
                ui.checkbox(
                    &mut self.settings.copy_url_after_upload,
                    tr("Copy URL after upload"),
                );
                ui.checkbox(
                    &mut self.settings.history_confirmation_to_delete,
                    tr("Confirm history image deletion"),
                );
            }
            ui.checkbox(
                &mut self.settings.abort_notifications,
                tr("Notify when screenshot is aborted"),
            );
            ui.checkbox(
                &mut self.settings.startup_message,
                tr("Show startup tray message"),
            );
            ui.checkbox(
                &mut self.settings.show_quit_prompt,
                tr("Ask before quitting capture"),
            );
            ui.checkbox(
                &mut self.settings.auto_close_idle,
                tr("Close daemon after clipboard and windows are released"),
            );
            ui.checkbox(
                &mut self.settings.jpeg_clipboard,
                tr("Use JPEG for clipboard images"),
            );
            ui.checkbox(
                &mut self.settings.save_after_copy,
                tr("Save image after copy"),
            );
            ui.checkbox(
                &mut self.settings.copy_path_after_save,
                tr("Copy file path after save"),
            );
            ui.checkbox(
                &mut self.settings.copy_on_double_click,
                tr("Copy on double-click inside selection"),
            );
            ui.checkbox(
                &mut self.settings.use_last_region,
                tr("Use last region for GUI mode"),
            );
            ui.add(
                egui::Slider::new(&mut self.settings.contrast_opacity, 0..=255)
                    .text(tr("Outside selection opacity")),
            );
            ui.checkbox(
                &mut self.settings.show_side_panel_button,
                tr("Show objects panel button"),
            );
            const GEOMETRY: [&str; 6] = [
                "None",
                "Top left",
                "Top right",
                "Bottom left",
                "Bottom right",
                "Center",
            ];
            egui::ComboBox::from_label(tr("Selection geometry"))
                .selected_text(tr(GEOMETRY[self.settings.selection_geometry as usize]))
                .show_ui(ui, |ui| {
                    for (index, label) in GEOMETRY.iter().enumerate() {
                        ui.selectable_value(
                            &mut self.settings.selection_geometry,
                            index as u8,
                            tr(label),
                        );
                    }
                });
            ui.add(
                egui::DragValue::new(&mut self.settings.selection_geometry_timeout)
                    .range(0..=i32::MAX as u64)
                    .prefix(tr("Geometry timeout (ms, 0 = always): ")),
            );
            ui.checkbox(&mut self.settings.magnifier, tr("Show magnifier"));
            ui.checkbox(&mut self.settings.square_magnifier, tr("Square magnifier"));
            ui.checkbox(&mut self.settings.counter_outline, tr("Counter outline"));
            ui.collapsing(tr("Toolbar buttons"), |ui| {
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
                    if ui.checkbox(&mut enabled, tr(label)).changed() {
                        if enabled {
                            selected.push(*id);
                        } else {
                            selected.retain(|value| value != id);
                        }
                        self.settings.buttons = Some(crate::buttons::normalize(selected.clone()));
                    }
                }
                if ui.button(tr("Restore default toolbar")).clicked() {
                    self.settings.buttons = Some(crate::buttons::defaults());
                    self.buttons_reset = true;
                }
            });
            ui.collapsing(tr("Shortcuts"), |ui| {
                for (name, default) in crate::shortcuts::DEFAULTS {
                    let mut value = self
                        .settings
                        .shortcuts
                        .get(*name)
                        .cloned()
                        .unwrap_or_else(|| (*default).into());
                    ui.horizontal(|ui| {
                        ui.label(shortcut_label(name));
                        if ui.text_edit_singleline(&mut value).changed() {
                            self.settings.shortcuts.insert((*name).into(), value);
                        }
                    });
                }
            });
            let (save_requested, reload_requested) = ui
                .horizontal(|ui| {
                    (
                        ui.button(tr("Save settings")).clicked(),
                        ui.button(tr("Discard edits and reload")).clicked(),
                    )
                })
                .inner;
            if reload_requested {
                match Settings::load() {
                    Ok(settings) => {
                        self.reload_saved(settings);
                        self.reload_error = None;
                        self.status.clear();
                    }
                    Err(error) => self.reload_error = Some(error.to_string()),
                }
            } else if save_requested {
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
                            Settings::update_form(&values, &shortcuts, |current, proposed| {
                                self.validate_external_changes(current, proposed)
                            })
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
                            tr("No changes to save")
                        } else {
                            tr("Settings saved")
                        };
                        match Settings::load() {
                            Ok(settings) => {
                                self.reload_saved(settings);
                                message.into()
                            }
                            Err(error) => format!(
                                "{message}; {}",
                                tr("Cannot reload settings: %1").replace("%1", &error.to_string())
                            ),
                        }
                    }
                    Err(e) => e.to_string(),
                };
            }
            ui.label(&self.status);
            if let Some(error) = &self.reload_error {
                ui.label(tr("Cannot reload settings: %1").replace("%1", error));
            }
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
    #[test]
    fn every_configurable_shortcut_has_a_readable_label() {
        for (name, _) in crate::shortcuts::DEFAULTS {
            assert_ne!(shortcut_label(name), *name);
        }
    }
    fn form() -> Config {
        let mut form = Config {
            recovery: None,
            recovered_frame: None,
            initial_language: Settings::default().language,
            settings: Settings::default(),
            path: String::new(),
            palette: "red, blue, picker".into(),
            buttons_reset: false,
            baseline: Default::default(),
            shortcut_baseline: Default::default(),
            startup_baseline: false,
            status: String::new(),
            last_refresh: std::time::Instant::now(),
            reload_error: None,
        };
        form.remember();
        form
    }
    #[test]
    fn form_conflicts_preserve_drafts_and_allow_unrelated_or_matching_updates() {
        let mut form = form();
        form.settings.filename_pattern = "draft".into();
        let mut current = Settings {
            filename_pattern: "external".into(),
            ..Settings::default()
        };
        let mut proposed = current.clone();
        proposed.filename_pattern = "draft".into();
        assert!(form.validate_external_changes(&current, &proposed).is_err());
        assert_eq!(form.settings.filename_pattern, "draft");
        current.filename_pattern = Settings::default().filename_pattern;
        current.extension = "jpg".into();
        assert!(form.validate_external_changes(&current, &proposed).is_ok());
        current.filename_pattern = "draft".into();
        assert!(form.validate_external_changes(&current, &proposed).is_ok());
        form.settings
            .shortcuts
            .insert("TYPE_ACCEPT".into(), "F9".into());
        current.shortcuts.insert("TYPE_ACCEPT".into(), "F10".into());
        proposed.shortcuts.insert("TYPE_ACCEPT".into(), "F9".into());
        assert!(form.validate_external_changes(&current, &proposed).is_err());
        current.shortcuts.insert("TYPE_ACCEPT".into(), "F9".into());
        assert!(form.validate_external_changes(&current, &proposed).is_ok());
        form.settings
            .shortcuts
            .insert("TYPE_ACCEPT".into(), "Ctrl+F9".into());
        current
            .shortcuts
            .insert("TYPE_ACCEPT".into(), "Control+F9".into());
        proposed
            .shortcuts
            .insert("TYPE_ACCEPT".into(), "Ctrl+F9".into());
        assert!(form.validate_external_changes(&current, &proposed).is_ok());
    }
    #[test]
    fn background_refresh_preserves_every_kind_of_unsaved_draft() {
        let mut forms = [form(), form(), form(), form()];
        forms[0].path = "/tmp/draft".into();
        forms[1]
            .settings
            .shortcuts
            .insert("TYPE_ACCEPT".into(), "F9".into());
        forms[2].settings.startup_launch = !forms[2].startup_baseline;
        forms[3].buttons_reset = true;
        for mut form in forms {
            assert!(form.is_dirty());
            let before = form.values();
            let shortcuts = form.settings.shortcuts.clone();
            let startup = form.settings.startup_launch;
            form.refresh_if_clean(|| panic!("must not load over an unsaved draft"));
            assert_eq!(form.values(), before);
            assert_eq!(form.settings.shortcuts, shortcuts);
            assert_eq!(form.settings.startup_launch, startup);
        }
    }
    #[test]
    fn background_refresh_recovers_after_invalid_external_configuration() {
        let mut form = form();
        let before = form.values();
        form.refresh_if_clean(|| anyhow::bail!("invalid external settings"));
        assert_eq!(form.values(), before);
        assert_eq!(
            form.reload_error.as_deref(),
            Some("invalid external settings")
        );
        assert!(!form.is_dirty());
        form.refresh_if_clean(|| {
            Ok(Settings {
                filename_pattern: "external".into(),
                language: "zh_CN".into(),
                ..Settings::default()
            })
        });
        assert_eq!(form.settings.filename_pattern, "external");
        assert!(form.reload_error.is_none());
        assert!(form.language_restart_needed());
        assert!(!form.is_dirty());
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
