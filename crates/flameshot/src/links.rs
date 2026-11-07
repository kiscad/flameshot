// SPDX-License-Identifier: GPL-3.0-or-later
use eframe::egui;
use std::{sync::mpsc, time::Duration};
#[derive(Default)]
pub struct LinkOpener {
    job: Option<mpsc::Receiver<Result<bool, String>>>,
    error: String,
}
impl LinkOpener {
    pub fn link(&mut self, ui: &mut egui::Ui, label: &str, url: &str) {
        if !crate::app_ui::restricted_bus_names() {
            ui.hyperlink_to(label, url);
            return;
        }
        let response = ui
            .add_enabled(self.job.is_none(), egui::Link::new(label))
            .on_hover_text(url);
        response.context_menu(|ui| {
            if ui
                .button(crate::i18n::tr("ImgUploaderBase", "Copy URL"))
                .clicked()
            {
                ui.ctx().copy_text(url.to_owned());
                ui.close();
            }
        });
        if response.clicked() {
            let (sender, receiver) = mpsc::channel();
            self.job = Some(receiver);
            self.error.clear();
            let url = url.to_owned();
            let ctx = ui.ctx().clone();
            tokio::spawn(async move {
                let result = async {
                    let connection = crate::daemon::session_connection().await?;
                    flameshot_platform::open_portal::open_web_uri(
                        &connection,
                        "",
                        &url,
                        Duration::from_secs(120),
                    )
                    .await
                }
                .await;
                let _ = sender.send(result.map_err(|e: anyhow::Error| e.to_string()));
                ctx.request_repaint();
            });
        }
    }
    pub fn status(&mut self, ui: &mut egui::Ui) {
        if let Some(job) = &self.job {
            let result = match job.try_recv() {
                Ok(result) => Some(result),
                Err(mpsc::TryRecvError::Empty) => None,
                Err(mpsc::TryRecvError::Disconnected) => Some(Err("Link opener stopped".into())),
            };
            if let Some(result) = result {
                self.job = None;
                self.error = result.err().unwrap_or_default();
            }
        }
        if self.job.is_some() {
            ui.spinner();
            ui.ctx().request_repaint_after(Duration::from_millis(100));
        }
        if !self.error.is_empty() {
            ui.colored_label(egui::Color32::RED, &self.error);
        }
    }
}
