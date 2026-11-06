// SPDX-License-Identifier: GPL-3.0-or-later
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
        "Flameshot Updates",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default().with_inner_size([440.0, 180.0]),
            ..Default::default()
        },
        Box::new(move |_| {
            Ok(Box::new(Updates {
                job: Some(receiver),
                update: None,
                message: "Checking for updates…".into(),
            }))
        }),
    )
    .map_err(|e| anyhow::anyhow!("Updates window: {e}"))
}
struct Updates {
    job: Option<std::sync::mpsc::Receiver<Result<Option<Update>, String>>>,
    update: Option<Update>,
    message: String,
}
impl eframe::App for Updates {
    fn ui(&mut self, ui: &mut egui::Ui, _: &mut eframe::Frame) {
        if let Some(job) = &self.job
            && let Ok(result) = job.try_recv()
        {
            self.job = None;
            match result {
                Ok(Some(update)) => {
                    self.message = format!("Version {} is available", update.version);
                    self.update = Some(update);
                }
                Ok(None) => self.message = "You have the latest version".into(),
                Err(e) => self.message = format!("Update check failed: {e}"),
            }
        }
        ui.heading("Flameshot Updates");
        ui.label(&self.message);
        if self.job.is_some() {
            ui.spinner();
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(100));
        }
        if let Some(update) = &self.update {
            ui.hyperlink_to("Open release page", &update.url);
            if ui.button("Ignore this version").clicked() {
                match crate::settings::Settings::update(&[(
                    "ignoreUpdateToVersion",
                    update.version.clone(),
                )]) {
                    Ok(()) => self.message = "Version ignored".into(),
                    Err(e) => self.message = e.to_string(),
                }
            }
        }
        if ui.button("Close").clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
}
