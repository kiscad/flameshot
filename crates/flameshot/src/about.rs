// SPDX-License-Identifier: GPL-3.0-or-later
use crate::i18n::tr;
use eframe::egui;

fn release_value(source: &str, key: &str) -> Option<String> {
    let value = source.lines().rev().find_map(|line| {
        let (name, value) = line.split_once('=')?;
        (name.trim() == key).then(|| value.trim())
    })?;
    if let Some(value) = value.strip_prefix('\'') {
        return value.strip_suffix('\'').map(str::to_owned);
    }
    if let Some(value) = value.strip_prefix('"') {
        let value = value.strip_suffix('"')?;
        let mut result = String::new();
        let mut chars = value.chars().peekable();
        while let Some(c) = chars.next() {
            if c == '\\'
                && chars
                    .peek()
                    .is_some_and(|c| matches!(c, '\\' | '"' | '$' | '`'))
            {
                result.push(chars.next().unwrap());
            } else {
                result.push(c);
            }
        }
        return Some(result);
    }
    Some(value.to_owned())
}
fn version_info() -> String {
    format!(
        "Flameshot {} ({})\nCompiled with {}",
        env!("CARGO_PKG_VERSION"),
        env!("FLAMESHOT_REVISION"),
        env!("FLAMESHOT_COMPILER")
    )
}
fn os_info() -> String {
    let system = rustix::system::uname();
    let release = std::fs::read_to_string("/etc/os-release")
        .or_else(|_| std::fs::read_to_string("/usr/lib/os-release"))
        .unwrap_or_default();
    format!(
        "{}: {}\n{}: {}",
        system.sysname().to_string_lossy().to_ascii_lowercase(),
        system.release().to_string_lossy(),
        release_value(&release, "ID").unwrap_or_else(|| "unknown".into()),
        release_value(&release, "VERSION_ID").unwrap_or_else(|| "unknown".into())
    )
}
pub fn show() -> anyhow::Result<()> {
    let version = version_info();
    let system = os_info();
    let viewport = crate::app_ui::viewport().with_inner_size([520.0, 310.0]);
    let icon = viewport.icon.as_ref().expect("application icon").clone();
    let title = format!("Flameshot {}", tr("InfoWindow", "About"));
    eframe::run_native(
        &title,
        eframe::NativeOptions {
            viewport,
            ..Default::default()
        },
        Box::new(move |cc| {
            crate::i18n::install_fonts(&cc.egui_ctx);
            let texture = cc.egui_ctx.load_texture(
                "about-logo",
                egui::ColorImage::from_rgba_unmultiplied(
                    [icon.width as usize, icon.height as usize],
                    &icon.rgba,
                ),
                egui::TextureOptions::LINEAR,
            );
            Ok(Box::new(About {
                version,
                system,
                icon: texture,
                job: None,
                error: String::new(),
                focus_copy: true,
            }))
        }),
    )
    .map_err(|error| anyhow::anyhow!("About window: {error}"))
}
struct About {
    version: String,
    system: String,
    icon: egui::TextureHandle,
    job: Option<std::sync::mpsc::Receiver<Result<(), String>>>,
    error: String,
    focus_copy: bool,
}
impl eframe::App for About {
    fn ui(&mut self, ui: &mut egui::Ui, _: &mut eframe::Frame) {
        if ui.input(|input| input.key_pressed(egui::Key::Escape)) {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
        }
        if let Some(job) = &self.job
            && let Ok(result) = job.try_recv()
        {
            self.job = None;
            self.error = result.err().unwrap_or_default();
            self.focus_copy = true;
        }
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.image((self.icon.id(), egui::vec2(72.0, 72.0)));
                ui.vertical(|ui| {
                    ui.heading("Flameshot");
                    ui.label(tr("InfoWindow", "License"));
                    ui.label("GPLv3+");
                });
            });
            ui.separator();
            ui.strong(tr("InfoWindow", "Version"));
            ui.add(egui::Label::new(&self.version).selectable(true));
            ui.strong(tr("InfoWindow", "OS Info"));
            ui.add(egui::Label::new(&self.system).selectable(true));
            let copy = ui.add_enabled(
                self.job.is_none(),
                egui::Button::new(tr("InfoWindow", "Copy Info")),
            );
            if self.focus_copy {
                copy.request_focus();
                self.focus_copy = false;
            }
            if copy.clicked() {
                let text = format!("{}\n{}", self.version, self.system);
                let (sender, receiver) = std::sync::mpsc::channel();
                self.job = Some(receiver);
                let ctx = ui.ctx().clone();
                tokio::spawn(async move {
                    let result = crate::daemon::copy_text(&text)
                        .await
                        .map_err(|e| e.to_string());
                    let _ = sender.send(result);
                    ctx.request_repaint();
                });
            }
            if self.job.is_some() {
                ui.spinner();
                ui.ctx()
                    .request_repaint_after(std::time::Duration::from_millis(100));
            }
            if !self.error.is_empty() {
                ui.colored_label(egui::Color32::RED, &self.error);
            }
        });
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn system_release_values_are_data_not_shell_expressions() {
        let source = "# comment\nID=old\nID=example\nVERSION_ID=\"24.04\"\nPRETTY_NAME='$(do-not-run)'\nESCAPED=\"a\\\"b\\\\c\\$d\"\n";
        assert_eq!(release_value(source, "ID").as_deref(), Some("example"));
        assert_eq!(
            release_value(source, "VERSION_ID").as_deref(),
            Some("24.04")
        );
        assert_eq!(
            release_value(source, "PRETTY_NAME").as_deref(),
            Some("$(do-not-run)")
        );
        assert_eq!(
            release_value(source, "ESCAPED").as_deref(),
            Some("a\"b\\c$d")
        );
        assert_eq!(release_value(source, "MISSING"), None);
        assert!(version_info().starts_with(&format!("Flameshot {} (", env!("CARGO_PKG_VERSION"))));
        assert!(version_info().contains("rustc"));
        assert!(!os_info().is_empty());
    }
}
