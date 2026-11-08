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
        egui::ScrollArea::vertical()
            .id_salt("configuration-repair-view")
            .show(ui, |ui| self.show_content(ui))
            .inner
    }
    fn show_content(&mut self, ui: &mut egui::Ui) -> bool {
        let tr = |text| crate::i18n::tr("RustConfig", text);
        if ui.input(|input| input.key_pressed(egui::Key::Escape)) {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
            return false;
        }
        ui.heading(crate::i18n::tr(
            "ConfigResolver",
            "Resolve configuration errors",
        ));
        ui.label(self.path.display().to_string());
        let frame = ui.ctx().cumulative_frame_nr();
        let problems = crate::config_check::diagnose(&self.draft);
        let mut repairs = Vec::new();
        egui::ScrollArea::vertical()
            .id_salt("configuration-repair-errors")
            .max_height(120.0)
            .show(ui, |ui| {
                for (index, problem) in problems.iter().enumerate() {
                    ui.push_id(index, |ui| {
                        ui.horizontal(|ui| {
                            if let Some(repair) = &problem.repair {
                                let label = if repair.reset { "Reset" } else { "Remove" };
                                if ui
                                    .button(crate::i18n::tr("ConfigResolver", label))
                                    .clicked()
                                    && self.handled_frame != Some(frame)
                                {
                                    repairs.push(repair.clone());
                                }
                            }
                            ui.add(egui::Label::new(problem.display()).wrap());
                        });
                    });
                }
            });
        if problems.iter().any(|problem| problem.repair.is_some())
            && ui
                .button(crate::i18n::tr("ConfigResolver", "Resolve all"))
                .clicked()
            && self.handled_frame != Some(frame)
        {
            repairs = problems
                .into_iter()
                .filter_map(|problem| problem.repair)
                .collect();
        }
        if !repairs.is_empty() {
            self.handled_frame = Some(frame);
            let mut ini = flameshot_core::config::IniDocument::parse(self.draft.clone());
            for repair in repairs {
                if let Err(error) = ini.remove(&repair.section, &repair.key) {
                    self.status = error.to_string();
                    return false;
                }
            }
            self.draft = ini.source().to_owned();
            self.status.clear();
        }
        egui::ScrollArea::both()
            .id_salt("configuration-repair-source")
            .max_height(220.0)
            .show(ui, |ui| {
                let response = ui.add(
                    egui::TextEdit::multiline(&mut self.draft)
                        .id(egui::Id::unique("configuration-repair-text"))
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
        let save_key = ui.input(|input| input.modifiers.command && input.key_pressed(egui::Key::S));
        let reload_key = ui.input(|input| input.key_pressed(egui::Key::F5));
        ui.horizontal(|ui| {
            if (ui.button(tr("Save")).on_hover_text("Ctrl+S").clicked() || save_key)
                && self.handled_frame != Some(frame)
            {
                self.handled_frame = Some(frame);
                let problems = crate::config_check::diagnose(&self.draft);
                if problems.is_empty() {
                    match Settings::replace_source_checked(&self.path, &self.baseline, &self.draft)
                    {
                        Ok(()) => done = true,
                        Err(error) => self.status = error.to_string(),
                    }
                } else {
                    self.status = problems
                        .iter()
                        .map(crate::config_check::Problem::display)
                        .collect::<Vec<_>>()
                        .join("\n");
                }
            }
            if (ui
                .button(tr("Discard edits and reload"))
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
        ui.label(crate::i18n::tr("RustConfig", &self.status));
        done
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn typing_keeps_focus_when_repair_buttons_disappear_and_return() {
        let root = tempfile::tempdir().unwrap();
        let mut recovery = Recovery {
            path: root.path().join("unused.ini"),
            baseline: String::new(),
            draft: "[General]\nshowHelp=bad\n".into(),
            status: String::new(),
            handled_frame: None,
            focus_editor: true,
        };
        let ctx = egui::Context::default();
        let mut render = |ui: &mut egui::Ui| {
            recovery.show(ui);
        };
        crate::ui_test::ui_frame(&ctx, vec![], &mut render);
        crate::ui_test::ui_frame(
            &ctx,
            vec![
                egui::Event::Key {
                    key: egui::Key::A,
                    physical_key: Some(egui::Key::A),
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers {
                        ctrl: true,
                        command: true,
                        ..Default::default()
                    },
                },
                egui::Event::Text("; ".into()),
            ],
            &mut render,
        );
        crate::ui_test::ui_frame(
            &ctx,
            vec![egui::Event::Text("keep\n[General]\nshowHelp=bad\n".into())],
            &mut render,
        );
        crate::ui_test::ui_frame(&ctx, vec![egui::Event::Text("; tail".into())], &mut render);
        assert_eq!(recovery.draft, "; keep\n[General]\nshowHelp=bad\n; tail");
    }
    #[test]
    fn resolve_all_keeps_manual_errors_and_does_not_publish_an_invalid_draft() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("flameshot.ini");
        let original = "[General]\nshowHelp=bad\nunknown=x\n[Shortcuts]\nTYPE_COPY=Ctrl+NoSuchKey\n[External]\nunknown=keep\nmalformed line\n";
        std::fs::write(&path, original).unwrap();
        let mut recovery = Recovery {
            path: path.clone(),
            baseline: original.into(),
            draft: original.into(),
            status: String::new(),
            handled_frame: None,
            focus_editor: true,
        };
        let ctx = egui::Context::default();
        for label in ["Resolve all", "Save"] {
            crate::ui_test::click_label(&ctx, label, |ui| {
                assert!(!recovery.show(ui));
            });
        }
        assert_eq!(
            recovery.draft,
            "[General]\n[Shortcuts]\n[External]\nunknown=keep\nmalformed line\n"
        );
        assert!(recovery.status.contains("Malformed"));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), original);
    }
    #[test]
    fn reset_and_remove_only_change_the_draft_until_save() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("flameshot.ini");
        let original = "; keep\r\n[General]\r\nshowHelp=old\r\nshowHelp=bad\r\nunknown=value\r\n[External]\r\nunknown=keep\r\n";
        std::fs::write(&path, original).unwrap();
        let mut recovery = Recovery {
            path: path.clone(),
            baseline: original.into(),
            draft: original.into(),
            status: String::new(),
            handled_frame: None,
            focus_editor: true,
        };
        let ctx = egui::Context::default();
        let click = |label, recovery: &mut Recovery| {
            crate::ui_test::click_label(&ctx, label, |ui| {
                recovery.show(ui);
                if ui.ctx().current_pass_index() == 0 {
                    ui.ctx()
                        .request_discard("exercise draft repair actions across layout passes");
                }
            });
        };
        click("Reset", &mut recovery);
        assert!(!recovery.draft.contains("showHelp="));
        assert!(recovery.draft.contains("unknown=value"));
        click("Remove", &mut recovery);
        assert_eq!(
            recovery.draft,
            "; keep\r\n[General]\r\n[External]\r\nunknown=keep\r\n"
        );
        assert_eq!(std::fs::read_to_string(&path).unwrap(), original);
        click("Save", &mut recovery);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), recovery.draft);
    }
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
        click("Discard edits and reload", &mut recovery);
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
