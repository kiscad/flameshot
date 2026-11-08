// SPDX-License-Identifier: GPL-3.0-or-later
use crate::i18n::tr;
use anyhow::Result;
use eframe::egui::{self, Color32};
use std::path::{Path, PathBuf};

pub(crate) fn remember_directory(path: &Path) {
    let path = std::path::absolute(path).unwrap_or_else(|_| path.to_owned());
    if let Some(parent) = path.parent()
        && let Err(error) = crate::settings::Settings::update(&[(
            "savePath",
            parent.to_string_lossy().into_owned(),
        )])
    {
        eprintln!("flameshot: Cannot remember save directory: {error}");
    }
}

pub(crate) enum SaveOutcome {
    Pending,
    Cancelled,
    Saved(PathBuf),
}
struct FileEntry {
    name: String,
    path: std::path::PathBuf,
    directory: bool,
}
type FolderResult = Result<Vec<FileEntry>, String>;
type PortalSaveResult = Result<Option<flameshot_platform::file_chooser::SaveSelection>, String>;
pub(crate) struct SaveDialog {
    portal: bool,
    portal_start: bool,
    portal_task: Option<std::sync::mpsc::Receiver<PortalSaveResult>>,
    portal_cancel: Option<tokio::sync::oneshot::Sender<()>>,
    cancelled: bool,
    focus: bool,
    listed_folder: Option<std::path::PathBuf>,
    pending_folder: Option<std::path::PathBuf>,
    listing_task: Option<std::sync::mpsc::Receiver<FolderResult>>,
    entries: Vec<FileEntry>,
    folder_error: String,
    path: String,
    extension: String,
    error: String,
    replace: Option<(PathBuf, u64)>,
    replace_focus: bool,
    dismissed_replace_frame: Option<u64>,
}
impl SaveDialog {
    fn refresh_folder(&mut self, folder: &std::path::Path, ctx: &egui::Context) {
        if let Some(receiver) = &self.listing_task
            && let Some(result) = crate::jobs::poll(receiver, "Folder listing worker stopped")
        {
            self.listed_folder = self.pending_folder.take();
            self.listing_task = None;
            match result {
                Ok(entries) => {
                    self.entries = entries;
                    self.folder_error.clear();
                }
                Err(error) => {
                    self.entries.clear();
                    self.folder_error = error;
                }
            }
        }
        if self.listing_task.is_none() && self.listed_folder.as_deref() != Some(folder) {
            self.entries.clear();
            self.folder_error.clear();
            let folder = folder.to_owned();
            self.pending_folder = Some(folder.clone());
            let (sender, receiver) = std::sync::mpsc::channel();
            self.listing_task = Some(receiver);
            std::thread::spawn(move || {
                let result = std::fs::read_dir(&folder)
                    .map(|entries| {
                        let mut entries: Vec<_> = entries
                            .filter_map(Result::ok)
                            .map(|entry| {
                                let path = entry.path();
                                let directory = path.is_dir();
                                FileEntry {
                                    name: entry.file_name().to_string_lossy().into_owned(),
                                    path,
                                    directory,
                                }
                            })
                            .collect();
                        entries
                            .sort_by(|a, b| (!a.directory, &a.name).cmp(&(!b.directory, &b.name)));
                        entries
                    })
                    .map_err(|e| e.to_string());
                let _ = sender.send(result);
            });
        }
        if self.listing_task.is_some() {
            ctx.request_repaint_after(std::time::Duration::from_millis(50));
        }
    }
}
impl SaveDialog {
    pub fn new(path: PathBuf, error: String) -> Self {
        let extension = path
            .extension()
            .and_then(|value| value.to_str())
            .map(str::to_ascii_lowercase)
            .unwrap_or_else(|| {
                crate::settings::Settings::load()
                    .map(|settings| settings.extension)
                    .unwrap_or_else(|_| "png".into())
            });
        Self {
            portal: crate::app_ui::restricted_bus_names(),
            portal_start: crate::app_ui::restricted_bus_names(),
            portal_task: None,
            portal_cancel: None,
            cancelled: false,
            focus: true,
            listed_folder: None,
            pending_folder: None,
            listing_task: None,
            entries: Vec::new(),
            folder_error: String::new(),
            path: path.to_string_lossy().into_owned(),
            extension,
            error,
            replace: None,
            replace_focus: false,
            dismissed_replace_frame: None,
        }
    }
    pub fn cancel(&mut self) {
        self.cancelled = true;
        if let Some(cancel) = self.portal_cancel.take() {
            let _ = cancel.send(());
        }
        self.portal_task = None;
    }
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        mut write_file: impl FnMut(&Path, Option<&str>) -> Result<PathBuf>,
    ) -> SaveOutcome {
        let ctx = ui.ctx().clone();
        if self.cancelled {
            return SaveOutcome::Cancelled;
        }
        if self.dismissed_replace_frame == Some(ctx.cumulative_frame_nr()) {
            return SaveOutcome::Pending;
        }
        if let Some((path, started)) = self.replace.clone() {
            let mut replace = false;
            let mut decline = ctx.input(|input| input.key_pressed(egui::Key::Escape));
            egui::Window::new(tr("RustEditor", "Replace existing file?"))
                .collapsible(false)
                .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
                .show(&ctx, |ui| {
                    if started == ctx.cumulative_frame_nr() {
                        ui.disable();
                    }
                    ui.label(path.to_string_lossy());
                    ui.horizontal(|ui| {
                        replace = ui.button(tr("RustEditor", "Yes")).clicked();
                        let no = ui.button(tr("RustEditor", "No"));
                        if self.replace_focus && started != ctx.cumulative_frame_nr() {
                            no.request_focus();
                            self.replace_focus = false;
                        }
                        decline |= no.clicked();
                    });
                });
            if decline {
                self.replace = None;
                self.focus = true;
                self.dismissed_replace_frame = Some(ctx.cumulative_frame_nr());
            }
            if replace {
                self.replace = None;
                let extension = path
                    .extension()
                    .and_then(|value| value.to_str())
                    .unwrap_or("png");
                match write_file(&path, Some(extension)) {
                    Ok(saved) => return SaveOutcome::Saved(saved),
                    Err(error) => {
                        self.error = error.to_string();
                        self.focus = true;
                    }
                }
            }
            return SaveOutcome::Pending;
        }
        if self.portal {
            let mut retry = self.portal_start;
            self.portal_start = false;
            let mut cancel = ctx.input(|input| input.key_pressed(egui::Key::Escape));
            egui::Window::new(tr("SaveTool", "Save"))
                .collapsible(false)
                .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
                .show(&ctx, |ui| {
                    if self.portal_task.is_some() {
                        ui.spinner();
                        ctx.request_repaint_after(std::time::Duration::from_millis(100));
                    } else {
                        if !self.error.is_empty() {
                            ui.colored_label(Color32::RED, &self.error);
                        }
                        retry |= ui.button(tr("SaveTool", "Save")).clicked()
                            || ctx.input(|i| i.key_pressed(egui::Key::Enter));
                    }
                    cancel |= ui.button(tr("ColorGrabWidget", "Cancel")).clicked();
                });
            if cancel {
                self.cancel();
                return SaveOutcome::Cancelled;
            }
            if let Some(task) = &self.portal_task
                && let Some(result) = crate::jobs::poll(task, "File chooser worker stopped")
            {
                self.portal_task = None;
                self.portal_cancel = None;
                match result {
                    Ok(None) => return SaveOutcome::Cancelled,
                    Ok(Some(selection)) => {
                        self.path = selection.path.to_string_lossy().into_owned();
                        match write_file(&selection.path, Some(&selection.extension)) {
                            Ok(saved) => return SaveOutcome::Saved(saved),
                            Err(error) => self.error = error.to_string(),
                        }
                    }
                    Err(error) => self.error = error,
                }
            }
            if retry && self.portal_task.is_none() {
                let (sender, receiver) = std::sync::mpsc::channel();
                self.portal_task = Some(receiver);
                let (cancel, cancelled) = tokio::sync::oneshot::channel();
                self.portal_cancel = Some(cancel);
                self.error.clear();
                let path = std::path::PathBuf::from(&self.path);
                ctx.request_repaint();
                let ctx = ctx.clone();
                tokio::spawn(async move {
                    let result = async {
                        let connection = crate::daemon::session_connection().await?;
                        flameshot_platform::file_chooser::save_file_cancellable(
                            &connection,
                            "",
                            tr("SaveTool", "Save"),
                            &path,
                            crate::output::EXTENSIONS,
                            std::time::Duration::from_secs(120),
                            async move {
                                let _ = cancelled.await;
                            },
                        )
                        .await
                    }
                    .await;
                    let _ = sender.send(result.map_err(|e: anyhow::Error| e.to_string()));
                    ctx.request_repaint();
                });
            }
            return SaveOutcome::Pending;
        }

        let popup_open = egui::Popup::is_any_open(&ctx);
        let mut cancel = !popup_open && ctx.input(|i| i.key_pressed(egui::Key::Escape));
        let mut save = !popup_open && ctx.input(|i| i.key_pressed(egui::Key::Enter));
        egui::Window::new(tr("RustEditor", "Save Screenshot"))
            .collapsible(false)
            .resizable(true)
            .default_width(550.0)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .show(&ctx, |ui| {
                if let Some(extension) = Path::new(&self.path)
                    .extension()
                    .and_then(|value| value.to_str())
                {
                    self.extension = extension.to_ascii_lowercase();
                }
                let original = self.extension.clone();
                let mut extension = original.clone();
                let format = egui::ComboBox::from_label(tr("RustEditor", "Image format"))
                    .selected_text(&extension)
                    .show_ui(ui, |ui| {
                        for &value in crate::output::EXTENSIONS {
                            ui.selectable_value(&mut extension, value.into(), value.to_uppercase());
                        }
                    });
                if format.response.has_focus() || egui::Popup::is_any_open(&ctx) {
                    save = false;
                }
                self.extension = extension.clone();
                if extension != original {
                    let mut path = std::path::PathBuf::from(&self.path);
                    if path.is_dir() {
                        path = path.join("capture");
                    }
                    path.set_extension(extension);
                    self.path = path.to_string_lossy().into_owned();
                }
                ui.label(tr("RustEditor", "Destination file"));
                let id = egui::Id::unique("save-destination");
                let response = ui.add(
                    egui::TextEdit::singleline(&mut self.path)
                        .id(id)
                        .desired_width(f32::INFINITY),
                );
                if self.focus {
                    response.request_focus();
                    self.focus = false;
                }
                let path = std::path::PathBuf::from(&self.path);
                let folder = if path.is_dir() {
                    path.clone()
                } else {
                    path.parent()
                        .filter(|parent| !parent.as_os_str().is_empty())
                        .unwrap_or(std::path::Path::new("."))
                        .to_owned()
                };
                let folder = std::path::absolute(&folder).unwrap_or(folder);
                if ui.button(tr("RustEditor", "Parent folder")).clicked()
                    && let Some(parent) = folder.parent()
                {
                    self.path = parent.to_string_lossy().into_owned();
                }
                self.refresh_folder(&folder, &ctx);
                if !self.folder_error.is_empty() {
                    ui.colored_label(Color32::RED, &self.folder_error);
                }
                let row_height =
                    ui.text_style_height(&egui::TextStyle::Body) + ui.spacing().item_spacing.y;
                egui::ScrollArea::vertical().max_height(250.0).show_rows(
                    ui,
                    row_height,
                    self.entries.len(),
                    |ui, range| {
                        for index in range {
                            let entry = &self.entries[index];
                            let label = if entry.directory {
                                format!("{}/", entry.name)
                            } else {
                                entry.name.clone()
                            };
                            if ui.selectable_label(false, label).clicked() {
                                self.path = entry.path.to_string_lossy().into_owned();
                            }
                        }
                    },
                );
                if !self.error.is_empty() {
                    ui.colored_label(Color32::RED, &self.error);
                }
                ui.horizontal(|ui| {
                    save |= ui.button(tr("SaveTool", "Save")).clicked();
                    cancel |= ui.button(tr("ColorGrabWidget", "Cancel")).clicked();
                });
            });
        if cancel {
            return SaveOutcome::Cancelled;
        }
        if save {
            let mut path = PathBuf::from(&self.path);
            if !path.is_dir() && path.extension().is_none() {
                path.set_extension(&self.extension);
            }
            let path = std::path::absolute(&path).unwrap_or(path);
            if path.is_file() {
                self.replace = Some((path, ctx.cumulative_frame_nr()));
                self.replace_focus = true;
                ctx.request_repaint();
                return SaveOutcome::Pending;
            }
            match write_file(&path, None) {
                Ok(saved) => return SaveOutcome::Saved(saved),
                Err(error) => {
                    self.error = error.to_string();
                    self.focus = true;
                }
            }
        }
        SaveOutcome::Pending
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cancelling_a_queued_portal_result_never_calls_the_writer() {
        let ctx = egui::Context::default();
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("selected.png");
        let mut dialog = SaveDialog::new(path.clone(), String::new());
        dialog.portal = true;
        dialog.portal_start = false;
        let (sender, receiver) = std::sync::mpsc::channel();
        sender
            .send(Ok(Some(flameshot_platform::file_chooser::SaveSelection {
                path,
                extension: "png".into(),
            })))
            .unwrap();
        dialog.portal_task = Some(receiver);
        let (cancel, mut cancelled) = tokio::sync::oneshot::channel();
        dialog.portal_cancel = Some(cancel);
        let mut closed = false;
        crate::ui_test::ui_frame(
            &ctx,
            vec![egui::Event::Key {
                key: egui::Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
            &mut |ui| {
                let outcome = dialog.show(ui, |_, _| {
                    panic!("Cancelled portal result reached the writer")
                });
                closed |= matches!(outcome, SaveOutcome::Cancelled);
            },
        );
        assert!(closed);
        assert_eq!(cancelled.try_recv(), Ok(()));
        assert!(dialog.portal_task.is_none());
    }
    #[test]
    fn cancel_click_beats_a_result_arriving_on_the_release_frame() {
        let ctx = egui::Context::default();
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("selected.png");
        let mut dialog = SaveDialog::new(path.clone(), String::new());
        dialog.portal = true;
        dialog.portal_start = false;
        let (sender, receiver) = std::sync::mpsc::channel();
        dialog.portal_task = Some(receiver);
        let mut late_result = Some(flameshot_platform::file_chooser::SaveSelection {
            path,
            extension: "png".into(),
        });
        let mut cancelled = false;
        crate::ui_test::click_label(&ctx, tr("ColorGrabWidget", "Cancel"), |ui| {
            if ui.input(|input| input.pointer.primary_released())
                && let Some(result) = late_result.take()
            {
                sender.send(Ok(Some(result))).unwrap();
            }
            let outcome = dialog.show(ui, |_, _| {
                panic!("Cancel click lost to a queued save result")
            });
            cancelled |= matches!(outcome, SaveOutcome::Cancelled);
        });
        assert!(late_result.is_none());
        assert!(cancelled);
    }
    #[test]
    fn dropping_a_dialog_signals_cancellation_to_its_worker() {
        let mut dialog = SaveDialog::new(PathBuf::from("unused.png"), String::new());
        let (cancel, mut cancelled) = tokio::sync::oneshot::channel();
        dialog.portal_cancel = Some(cancel);
        drop(dialog);
        assert!(matches!(
            cancelled.try_recv(),
            Err(tokio::sync::oneshot::error::TryRecvError::Closed)
        ));
    }
    #[test]
    fn format_popup_keys_do_not_save_and_suffixless_renames_keep_selected_format() {
        use crate::ui_test::{click_label, ui_frame};
        let root = tempfile::tempdir().unwrap();
        let mut dialog = SaveDialog::new(root.path().join("original.png"), String::new());
        // Never contact a desktop portal from a widget-frame test.
        dialog.portal = false;
        dialog.portal_start = false;
        let writes = std::cell::RefCell::new(Vec::new());
        let cancelled = std::cell::Cell::new(false);
        let ctx = egui::Context::default();
        let mut render = |ui: &mut egui::Ui| {
            let keyboard = ui.input(|input| {
                input.key_pressed(egui::Key::Escape) || input.key_pressed(egui::Key::Enter)
            });
            let outcome = dialog.show(ui, |path, _| {
                writes.borrow_mut().push(path.to_owned());
                Ok(path.to_owned())
            });
            if matches!(outcome, SaveOutcome::Cancelled) {
                cancelled.set(true);
            }
            if keyboard && ui.ctx().current_pass_index() == 0 {
                ui.ctx().request_discard(
                    "exercise format-popup keyboard handling across layout passes",
                );
            }
        };
        click_label(&ctx, "png", &mut render);
        assert!(egui::Popup::is_any_open(&ctx));
        let key = |key, pressed, modifiers| egui::Event::Key {
            key,
            physical_key: None,
            pressed,
            repeat: false,
            modifiers,
        };
        ui_frame(
            &ctx,
            vec![
                key(egui::Key::Escape, true, egui::Modifiers::NONE),
                key(egui::Key::Escape, false, egui::Modifiers::NONE),
            ],
            &mut render,
        );
        assert!(
            !cancelled.get(),
            "Closing the format popup cancelled the save dialog"
        );
        click_label(&ctx, "png", &mut render);
        ui_frame(
            &ctx,
            vec![
                key(egui::Key::Enter, true, egui::Modifiers::NONE),
                key(egui::Key::Enter, false, egui::Modifiers::NONE),
            ],
            &mut render,
        );
        assert!(
            writes.borrow().is_empty(),
            "Choosing a format submitted the save dialog"
        );
        egui::Popup::close_all(&ctx);
        click_label(&ctx, "png", &mut render);
        click_label(&ctx, "BMP", &mut render);
        ctx.memory_mut(|memory| memory.request_focus(egui::Id::unique("save-destination")));
        let modifiers = egui::Modifiers {
            ctrl: true,
            command: true,
            ..Default::default()
        };
        ui_frame(
            &ctx,
            vec![
                key(egui::Key::A, true, modifiers),
                key(egui::Key::A, false, modifiers),
                egui::Event::Text(root.path().join("renamed").to_string_lossy().into_owned()),
            ],
            &mut render,
        );
        click_label(&ctx, tr("SaveTool", "Save"), &mut render);
        assert_eq!(*writes.borrow(), [root.path().join("renamed.bmp")]);
    }
}
