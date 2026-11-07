// SPDX-License-Identifier: GPL-3.0-or-later
use crate::i18n::tr;
use eframe::egui;
use flameshot_platform::applications::{self, Application};
use std::{io::Write, path::PathBuf, sync::mpsc, time::Duration};
pub struct OpenWith {
    completion_lock: std::sync::Arc<std::sync::Mutex<()>>,
    image: image::RgbaImage,
    apps: Vec<Application>,
    portal: bool,
    start_portal: bool,
    listing: Option<mpsc::Receiver<Vec<Application>>>,
    job: Option<mpsc::Receiver<Result<bool, String>>>,
    file: Option<PathBuf>,
    launched: bool,
    uncertain_launch: bool,
    keep_open: bool,
    query: String,
    error: String,
}
fn finish_launch(
    sender: mpsc::Sender<Result<bool, String>>,
    result: Result<bool, String>,
    file: &std::path::Path,
    previously_launched: bool,
    completion_lock: &std::sync::Mutex<()>,
) {
    let _guard = completion_lock.lock().unwrap_or_else(|e| e.into_inner());
    if let Err(result) = sender.send(result)
        && !previously_launched
        && !matches!(result.0, Ok(true))
    {
        let _ = std::fs::remove_file(file);
    }
}
enum Launch {
    Native(Application),
    Portal,
}
impl OpenWith {
    pub fn new(image: image::RgbaImage, keep_open: bool) -> Self {
        let portal = crate::app_ui::restricted_bus_names();
        let listing = if portal {
            None
        } else {
            let (sender, receiver) = mpsc::channel();
            std::thread::spawn(move || {
                let _ = sender.send(applications::list());
            });
            Some(receiver)
        };
        Self {
            completion_lock: Default::default(),
            image,
            apps: Vec::new(),
            listing,
            portal,
            start_portal: portal,
            job: None,
            file: None,
            launched: false,
            uncertain_launch: false,
            keep_open,
            query: String::new(),
            error: String::new(),
        }
    }
    pub fn was_launched(&self) -> bool {
        self.launched
    }
    fn poll_launch(&mut self) -> Option<Result<bool, String>> {
        let job = self.job.as_ref()?;
        let result = match job.try_recv() {
            Ok(result) => result,
            Err(mpsc::TryRecvError::Empty) => return None,
            Err(mpsc::TryRecvError::Disconnected) => {
                // The external program may have opened the image before its
                // worker stopped. Do not convert an unknown outcome to failure.
                self.uncertain_launch = true;
                Err("Application launcher worker stopped".into())
            }
        };
        self.job = None;
        Some(result)
    }
    pub fn show(&mut self, ctx: &egui::Context) -> bool {
        if let Some(listing) = &self.listing {
            match listing.try_recv() {
                Ok(apps) => {
                    self.apps = apps;
                    self.listing = None;
                }
                Err(mpsc::TryRecvError::Empty) => {}
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.listing = None;
                    self.error = "Application listing worker stopped".into();
                }
            }
        }
        if let Some(result) = self.poll_launch() {
            match result {
                Ok(false) => return false,
                Ok(true) => {
                    self.launched = true;
                    if !self.keep_open {
                        return false;
                    }
                }
                Err(error) => self.error = error,
            }
        }
        let mut keep = true;
        let mut launch = self.start_portal.then_some(Launch::Portal);
        self.start_portal = false;
        egui::Window::new(tr("AppLauncherWidget", "Open With"))
            .collapsible(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .default_width(450.0)
            .show(ctx, |ui| {
                if self.portal {
                    if ui
                        .add_enabled(
                            self.job.is_none(),
                            egui::Button::new(tr(
                                "AppLauncher",
                                "Choose an app to open the capture",
                            )),
                        )
                        .clicked()
                    {
                        launch = Some(Launch::Portal);
                    }
                } else {
                    ui.label(tr("AppLauncher", "Choose an app to open the capture"));
                    ui.text_edit_singleline(&mut self.query);
                    egui::ScrollArea::vertical()
                        .max_height(300.0)
                        .show(ui, |ui| {
                            let query = self.query.to_lowercase();
                            for app in &self.apps {
                                if app.name.to_lowercase().contains(&query)
                                    && ui
                                        .add_enabled(
                                            self.job.is_none(),
                                            egui::Button::new(&app.name),
                                        )
                                        .clicked()
                                {
                                    launch = Some(Launch::Native(app.clone()));
                                }
                            }
                        });
                }
                if self.listing.is_some() || self.job.is_some() {
                    ui.spinner();
                    ctx.request_repaint_after(Duration::from_millis(100));
                }
                if !self.portal && self.apps.is_empty() && self.listing.is_none() {
                    ui.label("No applications found");
                }
                if ui
                    .checkbox(
                        &mut self.keep_open,
                        tr("AppLauncherWidget", "Keep open after selection"),
                    )
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
        if keep && let Some(launch) = launch {
            let file = match &self.file {
                Some(path) => Ok(path.clone()),
                None => self.write_image(),
            };
            match file {
                Ok(file) => {
                    self.file = Some(file.clone());
                    let (sender, receiver) = mpsc::channel();
                    self.job = Some(receiver);
                    self.error.clear();
                    ctx.request_repaint();
                    let ctx = ctx.clone();
                    let previously_launched = self.launched || self.uncertain_launch;
                    let completion_lock = self.completion_lock.clone();
                    match launch {
                        Launch::Native(app) => {
                            std::thread::spawn(move || {
                                let result = applications::launch(&app, &file)
                                    .map(|()| true)
                                    .map_err(|e| e.to_string());
                                finish_launch(
                                    sender,
                                    result,
                                    &file,
                                    previously_launched,
                                    &completion_lock,
                                );
                                ctx.request_repaint();
                            });
                        }
                        Launch::Portal => {
                            tokio::spawn(async move {
                                let result = async {
                                    let descriptor = std::fs::OpenOptions::new()
                                        .read(true)
                                        .write(true)
                                        .open(&file)?;
                                    let connection = crate::daemon::session_connection().await?;
                                    flameshot_platform::open_portal::open_file(
                                        &connection,
                                        "",
                                        &descriptor,
                                        true,
                                        true,
                                        Duration::from_secs(120),
                                    )
                                    .await
                                }
                                .await;
                                finish_launch(
                                    sender,
                                    result.map_err(|e: anyhow::Error| e.to_string()),
                                    &file,
                                    previously_launched,
                                    &completion_lock,
                                );
                                ctx.request_repaint();
                            });
                        }
                    }
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
        let _guard = self
            .completion_lock
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let unaccepted = match self.job.take() {
            None => true,
            Some(job) => matches!(job.try_recv(), Ok(Ok(false) | Err(_))),
        };
        if !self.launched
            && !self.uncertain_launch
            && unaccepted
            && let Some(path) = &self.file
        {
            let _ = std::fs::remove_file(path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn dialog(path: &std::path::Path, receiver: mpsc::Receiver<Result<bool, String>>) -> OpenWith {
        OpenWith {
            completion_lock: Default::default(),
            image: image::RgbaImage::new(1, 1),
            apps: Vec::new(),
            portal: true,
            start_portal: false,
            listing: None,
            job: Some(receiver),
            file: Some(path.to_owned()),
            launched: false,
            uncertain_launch: false,
            keep_open: false,
            query: String::new(),
            error: String::new(),
        }
    }
    #[test]
    fn stopped_launch_worker_releases_controls_without_discarding_possible_consumer_data() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("image.png");
        std::fs::write(&path, b"image").unwrap();
        let (sender, receiver) = mpsc::channel();
        let mut window = dialog(&path, receiver);
        assert!(window.poll_launch().is_none());
        drop(sender);
        assert!(window.poll_launch().unwrap().is_err());
        assert!(window.job.is_none());
        assert!(!window.was_launched());
        assert!(window.uncertain_launch);
        // A later failed retry cannot revoke the first unknown consumer's file.
        let (sender, receiver) = mpsc::channel();
        window.job = Some(receiver);
        sender.send(Err("retry failed".into())).unwrap();
        assert!(window.poll_launch().unwrap().is_err());
        drop(window);
        assert_eq!(std::fs::read(path).unwrap(), b"image");
    }
    #[test]
    fn late_completion_cleans_only_images_never_accepted_by_an_application() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("image.png");
        for previously_launched in [false, true] {
            for result in [Ok(true), Ok(false), Err("launch failed".into())] {
                std::fs::write(&path, b"image").unwrap();
                let keep = previously_launched || matches!(result, Ok(true));
                let (sender, receiver) = mpsc::channel();
                drop(receiver);
                finish_launch(
                    sender,
                    result,
                    &path,
                    previously_launched,
                    &std::sync::Mutex::new(()),
                );
                assert_eq!(path.exists(), keep);
            }
        }
    }
    #[test]
    fn closing_with_a_buffered_result_preserves_success_and_cleans_failure() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("image.png");
        for accepted in [false, true] {
            std::fs::write(&path, b"image").unwrap();
            let (sender, receiver) = mpsc::channel();
            sender.send(Ok(accepted)).unwrap();
            drop(dialog(&path, receiver));
            assert_eq!(path.exists(), accepted);
        }
    }
}
