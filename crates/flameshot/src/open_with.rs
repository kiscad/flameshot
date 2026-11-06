// SPDX-License-Identifier: GPL-3.0-or-later
use eframe::egui;
use flameshot_platform::applications::{self, Application};
use std::{io::Write, path::PathBuf, sync::mpsc, time::Duration};
pub struct OpenWith {
    image: image::RgbaImage,
    apps: Vec<Application>,
    listing: Option<mpsc::Receiver<Vec<Application>>>,
    job: Option<mpsc::Receiver<Result<(), String>>>,
    file: Option<PathBuf>,
    launched: bool,
    keep_open: bool,
    query: String,
    error: String,
}
impl OpenWith {
    pub fn new(image: image::RgbaImage, keep_open: bool) -> Self {
        let (sender, receiver) = mpsc::channel();
        std::thread::spawn(move || {
            let _ = sender.send(applications::list());
        });
        Self {
            image,
            apps: Vec::new(),
            listing: Some(receiver),
            job: None,
            file: None,
            launched: false,
            keep_open,
            query: String::new(),
            error: String::new(),
        }
    }
    pub fn was_launched(&self) -> bool {
        self.launched
    }
    pub fn show(&mut self, ctx: &egui::Context) -> bool {
        if let Some(listing) = &self.listing
            && let Ok(apps) = listing.try_recv()
        {
            self.apps = apps;
            self.listing = None;
        }
        if let Some(job) = &self.job
            && let Ok(result) = job.try_recv()
        {
            self.job = None;
            match result {
                Ok(()) => {
                    self.launched = true;
                    if !self.keep_open {
                        return false;
                    }
                }
                Err(error) => self.error = error,
            }
        }
        let mut keep = true;
        let mut launch = None;
        egui::Window::new("Open With")
            .collapsible(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .default_width(450.0)
            .show(ctx, |ui| {
                ui.label("Choose an application for this screenshot");
                ui.text_edit_singleline(&mut self.query);
                egui::ScrollArea::vertical()
                    .max_height(300.0)
                    .show(ui, |ui| {
                        let query = self.query.to_lowercase();
                        for app in &self.apps {
                            if app.name.to_lowercase().contains(&query)
                                && ui
                                    .add_enabled(self.job.is_none(), egui::Button::new(&app.name))
                                    .clicked()
                            {
                                launch = Some(app.clone());
                            }
                        }
                    });
                if self.listing.is_some() || self.job.is_some() {
                    ui.spinner();
                    ctx.request_repaint_after(Duration::from_millis(100));
                }
                if self.apps.is_empty() && self.listing.is_none() {
                    ui.label("No applications found");
                }
                if ui
                    .checkbox(&mut self.keep_open, "Keep open after selection")
                    .changed()
                    && let Err(error) = crate::settings::Settings::update(&[(
                        "keepOpenAppLauncher",
                        self.keep_open.to_string(),
                    )])
                {
                    self.error = error.to_string();
                }
                if !self.error.is_empty() {
                    ui.colored_label(egui::Color32::RED, &self.error);
                }
                if self.job.is_none()
                    && (ui.button("Close").clicked()
                        || ctx.input(|i| i.key_pressed(egui::Key::Escape)))
                {
                    keep = false;
                }
            });
        if let Some(app) = launch {
            let file = match &self.file {
                Some(path) => Ok(path.clone()),
                None => self.write_image(),
            };
            match file {
                Ok(file) => {
                    self.file = Some(file.clone());
                    let (sender, receiver) = mpsc::channel();
                    self.job = Some(receiver);
                    std::thread::spawn(move || {
                        let _ = sender
                            .send(applications::launch(&app, &file).map_err(|e| e.to_string()));
                    });
                    ctx.request_repaint();
                }
                Err(error) => self.error = error.to_string(),
            }
        }
        keep
    }
    fn write_image(&self) -> anyhow::Result<PathBuf> {
        let mut file = tempfile::Builder::new()
            .prefix("flameshot-open-")
            .suffix(".png")
            .tempfile()?;
        file.write_all(&crate::output::encode(
            &self.image,
            image::ImageFormat::Png,
        )?)?;
        file.as_file().sync_all()?;
        let (_, path) = file.keep().map_err(|e| e.error)?;
        Ok(path)
    }
}
impl Drop for OpenWith {
    fn drop(&mut self) {
        if !self.launched
            && self.job.is_none()
            && let Some(path) = &self.file
        {
            let _ = std::fs::remove_file(path);
        }
    }
}
