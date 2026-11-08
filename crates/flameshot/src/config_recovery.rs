// SPDX-License-Identifier: GPL-3.0-or-later
//! An explicit, lossless repair view for configurations that cannot open the normal form.
use crate::settings::Settings;
use anyhow::Context;
use eframe::egui;
use std::path::PathBuf;

#[derive(Clone)]
pub(crate) struct Recovery {
    path: PathBuf,
    baseline: String,
    draft: String,
    pub(crate) status: String,
    handled_frame: Option<u64>,
    focus_editor: bool,
}
impl Recovery {
    pub(crate) fn load() -> anyhow::Result<Option<Self>> {
        let path = dirs::config_dir()
            .context("Cannot locate configuration directory")?
            .join("flameshot/flameshot.ini");
        let source = match std::fs::read_to_string(&path) {
            Ok(source) => source,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error.into()),
        };
        if crate::config_check::check(&source).is_empty() {
            return Ok(None);
        }
        Ok(Some(Self {
            path,
            baseline: source.clone(),
            draft: source,
            status: String::new(),
            handled_frame: None,
            focus_editor: true,
        }))
    }
    pub(crate) fn show(&mut self, ui: &mut egui::Ui) -> bool {
        let tr = |text| crate::i18n::tr("RustConfig", text);
        if ui.input(|input| input.key_pressed(egui::Key::Escape)) {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
            return false;
        }
        ui.heading(crate::i18n::tr("ConfigWindow", "Configuration"));
        ui.label(crate::i18n::tr(
            "ConfigHandler",
            "The configuration contains an error. Open configuration to resolve.",
        ));
        ui.label(self.path.display().to_string());
        egui::ScrollArea::vertical()
            .id_salt("configuration-repair-errors")
            .max_height(90.0)
            .show(ui, |ui| {
                for error in crate::config_check::check(&self.draft) {
                    ui.label(error);
                }
            });
        egui::ScrollArea::both()
            .id_salt("configuration-repair-source")
            .max_height(220.0)
            .show(ui, |ui| {
                let response = ui.add(
                    egui::TextEdit::multiline(&mut self.draft)
                        .code_editor()
                        .desired_width(f32::INFINITY)
                        .desired_rows(10),
                );
                if self.focus_editor {
                    response.request_focus();
                    self.focus_editor = false;
                }
            });
        let mut done = false;
        let frame = ui.ctx().cumulative_frame_nr();
        let save_key = ui.input(|input| input.modifiers.command && input.key_pressed(egui::Key::S));
        let reload_key = ui.input(|input| input.key_pressed(egui::Key::F5));
        ui.horizontal(|ui| {
            if (ui.button(tr("Save")).on_hover_text("Ctrl+S").clicked() || save_key)
                && self.handled_frame != Some(frame)
            {
                self.handled_frame = Some(frame);
                match Settings::replace_source_checked(&self.path, &self.baseline, &self.draft) {
                    Ok(()) => done = true,
                    Err(error) => self.status = error.to_string(),
                }
            }
            if (ui
                .button(tr("Reload saved settings"))
                .on_hover_text("F5")
                .clicked()
                || reload_key)
                && self.handled_frame != Some(frame)
            {
                self.handled_frame = Some(frame);
                match std::fs::read_to_string(&self.path) {
                    Ok(source) => {
                        self.baseline = source.clone();
                        self.draft = source;
                        self.status.clear();
                        self.focus_editor = true;
                        done = crate::config_check::check(&self.draft).is_empty();
                    }
                    Err(error) => self.status = error.to_string(),
                }
            }
        });
        ui.label(&self.status);
        done
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn repair_buttons_validate_reload_and_save_once_across_layout_passes() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("flameshot.ini");
        let invalid = "[General]\ndrawThickness=bad\n";
        std::fs::write(&path, invalid).unwrap();
        let mut recovery = Recovery {
            path: path.clone(),
            baseline: invalid.into(),
            draft: invalid.into(),
            status: String::new(),
            handled_frame: None,
            focus_editor: true,
        };
        let ctx = egui::Context::default();
        let completed = std::cell::Cell::new(false);
        let click = |label: &str, recovery: &mut Recovery| {
            crate::ui_test::click_label(&ctx, label, |ui| {
                if recovery.show(ui) {
                    completed.set(true);
                }
                if ui.ctx().current_pass_index() == 0 {
                    ui.ctx()
                        .request_discard("exercise configuration repair across layout passes");
                }
            });
        };
        click("Save", &mut recovery);
        assert!(!completed.get());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), invalid);
        let external = format!("; external comment\n{invalid}");
        std::fs::write(&path, &external).unwrap();
        recovery.draft = invalid.replace("bad", "7");
        click("Save", &mut recovery);
        assert!(recovery.status.contains("changed outside"));
        click("Reload saved settings", &mut recovery);
        assert_eq!(recovery.draft, external);
        assert!(!completed.get());
        recovery.draft = external.replace("bad", "7");
        click("Save", &mut recovery);
        assert!(completed.get());
        assert!(recovery.status.is_empty(), "{}", recovery.status);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), recovery.draft);
    }
    #[test]
    fn repair_rejects_invalid_and_stale_edits_and_preserves_other_bytes() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("flameshot.ini");
        let original = "; keep\r\n[General]\r\ndrawThickness=bad\r\n[External]\r\nopaque=@Variant(untouched)\r\n";
        std::fs::write(&path, original).unwrap();
        let fixed = original.replace("drawThickness=bad", "drawThickness=7");
        assert!(Settings::replace_source_checked(&path, original, original).is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), original);
        let external = original.replace("; keep", "; externally edited");
        std::fs::write(&path, &external).unwrap();
        assert!(Settings::replace_source_checked(&path, original, &fixed).is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), external);
        let fixed = external.replace("drawThickness=bad", "drawThickness=7");
        Settings::replace_source_checked(&path, &external, &fixed).unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), fixed);
    }
}
