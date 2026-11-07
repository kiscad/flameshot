// SPDX-License-Identifier: GPL-3.0-or-later
use crate::i18n::tr;
use eframe::egui;
use flameshot_platform::updates::{self, Update};
pub fn show() -> anyhow::Result<()> {
    let (sender, receiver) = std::sync::mpsc::channel();
    tokio::spawn(async move {
        let result = updates::check(env!("CARGO_PKG_VERSION"), "")
            .await
            .map_err(|e| e.to_string());
        let _ = sender.send(result);
    });
    eframe::run_native(
        tr("RustUpdates", "Flameshot Updates"),
        eframe::NativeOptions {
            viewport: crate::app_ui::viewport().with_inner_size([440.0, 180.0]),
            ..Default::default()
        },
        Box::new(move |cc| {
            crate::i18n::install_fonts(&cc.egui_ctx);
            Ok(Box::new(Updates {
                links: Default::default(),
                job: Some(receiver),
                update: None,
                message: tr("RustUpdates", "Checking for updates…").into(),
            }))
        }),
    )
    .map_err(|e| anyhow::anyhow!("Updates window: {e}"))
}
struct Updates {
    links: crate::links::LinkOpener,
    job: Option<std::sync::mpsc::Receiver<Result<Option<Update>, String>>>,
    update: Option<Update>,
    message: String,
}
impl Updates {
    fn poll(&mut self) {
        let Some(job) = &self.job else {
            return;
        };
        let result = match job.try_recv() {
            Ok(result) => result,
            Err(std::sync::mpsc::TryRecvError::Empty) => return,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => Err("Update worker stopped".into()),
        };
        self.job = None;
        match result {
            Ok(Some(update)) => {
                self.message =
                    tr("RustUpdates", "Version %1 is available").replace("%1", &update.version);
                self.update = Some(update);
            }
            Ok(None) => self.message = tr("RustUpdates", "You have the latest version").into(),
            Err(error) => {
                self.message = tr("RustUpdates", "Update check failed: %1").replace("%1", &error)
            }
        }
    }
}
impl eframe::App for Updates {
    fn ui(&mut self, ui: &mut egui::Ui, _: &mut eframe::Frame) {
        self.poll();
        egui::ScrollArea::vertical()
            .max_height((ui.available_height() - 32.0).max(0.0))
            .show(ui, |ui| {
                ui.heading(tr("RustUpdates", "Flameshot Updates"));
                ui.label(&self.message);
                if self.job.is_some() {
                    ui.spinner();
                    ui.ctx()
                        .request_repaint_after(std::time::Duration::from_millis(100));
                }
                if let Some(update) = &self.update {
                    self.links
                        .link(ui, tr("RustUpdates", "Open release page"), &update.url);
                    if ui
                        .button(tr("RustUpdates", "Ignore this version"))
                        .clicked()
                    {
                        match crate::settings::Settings::update(&[(
                            "ignoreUpdateToVersion",
                            update.version.clone(),
                        )]) {
                            Ok(()) => self.message = tr("RustUpdates", "Version ignored").into(),
                            Err(e) => self.message = e.to_string(),
                        }
                    }
                }
                self.links.status(ui);
            });
        if ui.button(tr("UtilityPanel", "Close")).clicked()
            || ui.input(|i| i.key_pressed(egui::Key::Escape))
        {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn worker_completion_and_disconnect_leave_the_pending_state() {
        let (sender, receiver) = std::sync::mpsc::channel();
        let mut window = Updates {
            links: Default::default(),
            job: Some(receiver),
            update: None,
            message: "pending".into(),
        };
        window.poll();
        assert!(window.job.is_some());
        assert_eq!(window.message, "pending");
        drop(sender);
        window.poll();
        assert!(window.job.is_none());
        assert!(window.message.contains("Update worker stopped"));
        for available in [
            None,
            Some(Update {
                version: "99.0.0".into(),
                url: "https://github.com/flameshot-org/flameshot/releases/tag/v99.0.0".into(),
            }),
        ] {
            let (sender, receiver) = std::sync::mpsc::channel();
            window.job = Some(receiver);
            sender.send(Ok(available.clone())).unwrap();
            drop(sender); // Buffered completion must win over disconnect.
            window.poll();
            assert!(window.job.is_none());
            assert_eq!(window.update, available);
        }
    }
}
