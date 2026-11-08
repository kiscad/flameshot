// SPDX-License-Identifier: GPL-3.0-or-later
use crate::i18n::tr;
use eframe::egui;
use flameshot_platform::imgur::{Imgur, UploadedImage};
use std::{sync::mpsc, time::Duration};
#[derive(Default)]
struct LinkClipboard {
    job: Option<mpsc::Receiver<Result<(), String>>>,
}
impl LinkClipboard {
    fn copy(&mut self, ctx: &egui::Context, text: &str) {
        let (sender, receiver) = mpsc::channel();
        self.job = Some(receiver);
        let text = text.to_owned();
        let ctx = ctx.clone();
        tokio::spawn(async move {
            let _ = sender.send(
                crate::daemon::copy_text(&text)
                    .await
                    .map_err(|e| e.to_string()),
            );
            ctx.request_repaint();
        });
    }
    fn poll(&mut self, ctx: &egui::Context) -> Option<String> {
        let result = match self.job.as_ref()?.try_recv() {
            Ok(result) => result,
            Err(mpsc::TryRecvError::Empty) => {
                ctx.request_repaint_after(Duration::from_millis(100));
                return None;
            }
            Err(mpsc::TryRecvError::Disconnected) => Err("Clipboard worker stopped".into()),
        };
        self.job = None;
        Some(match result {
            Ok(()) => tr("ImgUploaderBase", "URL copied to clipboard.").into(),
            Err(error) => error,
        })
    }
}
pub struct UploadDialog {
    links: crate::links::LinkOpener,
    clipboard: LinkClipboard,
    clipboard_message: String,
    image: image::RgbaImage,
    texture: Option<egui::TextureHandle>,
    job: Option<mpsc::Receiver<Result<(UploadedImage, String), String>>>,
    uploaded: Option<UploadedImage>,
    message: String,
    auto_start_pending: bool,
    #[cfg(test)]
    test_begin: Option<(crate::settings::Settings, Imgur, std::path::PathBuf)>,
    copy_after_upload: bool,
    #[cfg(test)]
    test_upload_rect: Option<egui::Rect>,
}
impl UploadDialog {
    pub fn new(image: image::RgbaImage) -> Self {
        let settings = crate::settings::Settings::load();
        let mut dialog = Self::with_settings(image, settings.as_ref().ok());
        if let Err(error) = settings {
            dialog.message = error.to_string();
        }
        dialog
    }
    fn with_settings(
        image: image::RgbaImage,
        settings: Option<&crate::settings::Settings>,
    ) -> Self {
        Self {
            links: Default::default(),
            clipboard: LinkClipboard::default(),
            clipboard_message: String::new(),
            image,
            texture: None,
            job: None,
            uploaded: None,
            message: String::new(),
            auto_start_pending: settings.is_some_and(|s| s.upload_without_confirmation),
            #[cfg(test)]
            test_begin: None,
            #[cfg(test)]
            test_upload_rect: None,
            copy_after_upload: settings.is_some_and(|s| s.copy_url_after_upload),
        }
    }
    fn take_auto_start(&mut self) -> bool {
        std::mem::take(&mut self.auto_start_pending)
    }
    fn begin(&mut self, ctx: &egui::Context) {
        #[cfg(test)]
        {
            if let Some((settings, api, history)) = self.test_begin.take() {
                self.start(ctx, settings, api, Ok(history));
            } else {
                self.message = "Test upload client not configured".into();
            }
        }
        #[cfg(not(test))]
        {
            let setup = (|| {
                let settings = crate::settings::Settings::load()?;
                let api = Imgur::new(&settings.upload_client_id)?;
                Ok::<_, anyhow::Error>((settings, api))
            })();
            match setup {
                Ok((settings, api)) => {
                    self.start(ctx, settings, api, crate::upload_history::directory())
                }
                Err(error) => self.message = error.to_string(),
            }
        }
    }
    fn start(
        &mut self,
        ctx: &egui::Context,
        settings: crate::settings::Settings,
        api: Imgur,
        history: anyhow::Result<std::path::PathBuf>,
    ) {
        if self.job.is_some() || self.uploaded.is_some() {
            return;
        }
        self.auto_start_pending = false;
        self.copy_after_upload = settings.copy_url_after_upload;
        let image = self.image.clone();
        let ctx = ctx.clone();
        let (sender, receiver) = mpsc::channel();
        self.job = Some(receiver);
        self.message.clear();
        self.clipboard_message.clear();
        tokio::spawn(async move {
            let result = async {
                let png = crate::output::encode(&image, image::ImageFormat::Png)?;
                let uploaded = api
                    .upload(
                        png,
                        &chrono::Local::now()
                            .format(&settings.filename_pattern)
                            .to_string(),
                    )
                    .await?;
                let warning = match history.and_then(|path| {
                    crate::upload_history::save(
                        &path,
                        &uploaded,
                        &image,
                        settings.upload_history_max,
                    )
                }) {
                    Ok(()) => String::new(),
                    Err(error) => format!("Uploaded, but history could not be saved: {error}"),
                };
                Ok::<_, anyhow::Error>((uploaded, warning))
            }
            .await
            .map_err(|error| error.to_string());
            let _ = sender.send(result);
            ctx.request_repaint();
        });
    }
    fn poll_upload(&mut self) -> Option<String> {
        let result = match self.job.as_ref()?.try_recv() {
            Ok(result) => result,
            Err(mpsc::TryRecvError::Empty) => return None,
            Err(mpsc::TryRecvError::Disconnected) => Err("Upload worker stopped".into()),
        };
        self.job = None;
        match result {
            Ok((image, message)) => {
                let copy = self.copy_after_upload.then(|| image.link.clone());
                self.uploaded = Some(image);
                self.message = message;
                copy
            }
            Err(error) => {
                self.message = error;
                None
            }
        }
    }
    pub fn show(&mut self, ctx: &egui::Context) -> bool {
        if self.take_auto_start() {
            self.begin(ctx);
        }
        if let Some(message) = self.clipboard.poll(ctx) {
            self.clipboard_message = message;
        }
        if let Some(link) = self.poll_upload() {
            self.clipboard.copy(ctx, &link);
        }
        let mut keep = true;
        egui::Window::new(format!("{} — Imgur", tr("ImgUploaderBase", "Upload image")))
            .collapsible(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .show(ctx, |ui| {
                if self.texture.is_none() {
                    let thumbnail = flameshot_core::preview::thumbnail(&self.image, 400, 250);
                    self.texture = Some(ctx.load_texture(
                        "upload-preview",
                        egui::ColorImage::from_rgba_unmultiplied(
                            [thumbnail.width() as usize, thumbnail.height() as usize],
                            thumbnail.as_raw(),
                        ),
                        Default::default(),
                    ));
                }
                ui.image(self.texture.as_ref().unwrap());
                if let Some(uploaded) = &self.uploaded {
                    self.links.link(ui, &uploaded.link, &uploaded.link);
                    if ui.button(tr("ImgUploaderBase", "Copy URL")).clicked() {
                        self.clipboard.copy(ctx, &uploaded.link);
                    }
                } else if self.job.is_some() {
                    ui.spinner();
                    ui.label(tr("ImgUploaderBase", "Uploading Image"));
                    ctx.request_repaint_after(Duration::from_millis(100));
                } else {
                    ui.label(tr("ImgUploadDialog", "Do you want to upload this capture?"));
                    let upload = ui.button(tr("ImgUploaderBase", "Upload image"));
                    #[cfg(test)]
                    {
                        self.test_upload_rect = Some(upload.rect);
                    }
                    if upload.clicked() {
                        self.begin(ctx);
                    }
                }
                self.links.status(ui);
                if !self.message.is_empty() {
                    ui.label(&self.message);
                }
                if !self.clipboard_message.is_empty() {
                    ui.label(&self.clipboard_message);
                }
                if self.job.is_none()
                    && self.clipboard.job.is_none()
                    && (ui.button(tr("UtilityPanel", "Close")).clicked()
                        || ctx.input(|i| i.key_pressed(egui::Key::Escape)))
                {
                    keep = false;
                }
            });
        keep
    }
}
pub fn history() -> anyhow::Result<()> {
    let confirmation_required = crate::settings::Settings::load()?.history_confirmation_to_delete;
    let entries = crate::upload_history::list(&crate::upload_history::directory()?)?;
    eframe::run_native(
        &format!("Flameshot {}", tr("UploadHistory", "Upload History")),
        eframe::NativeOptions {
            viewport: crate::app_ui::viewport(),
            ..Default::default()
        },
        Box::new(move |cc| {
            crate::i18n::install_fonts(&cc.egui_ctx);
            Ok(Box::new(History {
                confirmation_required,
                #[cfg(test)]
                test_delete: None,
                textures: Default::default(),
                links: Default::default(),
                clipboard: LinkClipboard::default(),
                entries,
                confirm: None,
                job: None,
                remote_deleted: Default::default(),
                message: String::new(),
            }))
        }),
    )
    .map_err(|e| anyhow::anyhow!("Upload history: {e}"))
}
enum DeleteOutcome {
    Complete(usize),
    Failed {
        index: usize,
        remote_deleted: bool,
        message: String,
    },
}
struct History {
    links: crate::links::LinkOpener,
    confirmation_required: bool,
    #[cfg(test)]
    test_delete: Option<Imgur>,
    textures: std::collections::HashMap<std::path::PathBuf, Option<egui::TextureHandle>>,
    clipboard: LinkClipboard,
    entries: Vec<crate::upload_history::Entry>,
    confirm: Option<usize>,
    job: Option<mpsc::Receiver<DeleteOutcome>>,
    remote_deleted: std::collections::HashSet<std::path::PathBuf>,
    message: String,
}
impl History {
    fn poll_delete(&mut self) {
        let Some(job) = &self.job else {
            return;
        };
        let result = match job.try_recv() {
            Ok(result) => result,
            Err(mpsc::TryRecvError::Empty) => return,
            Err(mpsc::TryRecvError::Disconnected) => {
                self.job = None;
                self.message = "Deletion worker stopped".into();
                return;
            }
        };
        self.job = None;
        match result {
            DeleteOutcome::Complete(index) => {
                self.remote_deleted.remove(&self.entries[index].path);
                self.textures.remove(&self.entries[index].path);
                self.entries.remove(index);
                self.confirm = None;
            }
            DeleteOutcome::Failed {
                index,
                remote_deleted,
                message,
            } => {
                if remote_deleted {
                    self.remote_deleted.insert(self.entries[index].path.clone());
                    self.confirm = None;
                    self.message = tr(
                        "UploadHistory",
                        "Remote image deleted; local history cleanup failed: %1",
                    )
                    .replace("%1", &message);
                } else {
                    self.message = message;
                }
            }
        }
    }
    fn request_delete(&mut self, index: usize) -> bool {
        if self.confirmation_required && !self.remote_deleted.contains(&self.entries[index].path) {
            self.confirm = Some(index);
            false
        } else {
            self.confirm = None;
            true
        }
    }
    fn start_delete(&mut self, ctx: &egui::Context, index: usize) {
        if self.remote_deleted.contains(&self.entries[index].path) {
            self.delete_with_client(ctx, index, None);
            return;
        }
        #[cfg(test)]
        {
            if let Some(api) = self.test_delete.take() {
                self.begin_delete(ctx, index, api);
            } else {
                self.message = "Test deletion client not configured".into();
            }
        }
        #[cfg(not(test))]
        {
            match crate::settings::Settings::load()
                .and_then(|settings| Imgur::new(&settings.upload_client_id))
            {
                Ok(api) => self.begin_delete(ctx, index, api),
                Err(error) => self.message = error.to_string(),
            }
        }
    }
    fn begin_delete(&mut self, ctx: &egui::Context, index: usize, api: Imgur) {
        self.delete_with_client(ctx, index, Some(api));
    }
    fn delete_with_client(&mut self, ctx: &egui::Context, index: usize, api: Option<Imgur>) {
        if self.job.is_some() {
            return;
        }
        self.message.clear();
        let entry = self.entries[index].clone();
        let mut remote_deleted = self.remote_deleted.contains(&entry.path);
        let (sender, receiver) = mpsc::channel();
        self.job = Some(receiver);
        let ctx = ctx.clone();
        tokio::spawn(async move {
            let result = async {
                if !remote_deleted {
                    let api = api
                        .as_ref()
                        .ok_or_else(|| anyhow::anyhow!("Missing deletion client"))?;
                    api.delete(entry.deletehash.as_deref().unwrap()).await?;
                    remote_deleted = true;
                }
                match std::fs::remove_file(&entry.path) {
                    Ok(()) => {}
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                    Err(error) => return Err(error.into()),
                }
                Ok::<_, anyhow::Error>(())
            }
            .await;
            let outcome = match result {
                Ok(()) => DeleteOutcome::Complete(index),
                Err(error) => DeleteOutcome::Failed {
                    index,
                    remote_deleted,
                    message: error.to_string(),
                },
            };
            let _ = sender.send(outcome);
            ctx.request_repaint();
        });
    }
}
impl History {
    fn show(&mut self, ui: &mut egui::Ui) {
        self.poll_delete();
        if let Some(message) = self.clipboard.poll(ui.ctx()) {
            self.message = message;
        }
        ui.heading(tr("UploadHistory", "Upload History"));
        if self.entries.is_empty() {
            ui.label(tr("UploadHistory", "Screenshots history is empty"));
        }
        let mut requested_delete = None;
        egui::ScrollArea::vertical().show(ui, |ui| {
            for (index, entry) in self.entries.iter().enumerate() {
                ui.horizontal(|ui| {
                    let texture = self.textures.entry(entry.path.clone()).or_insert_with(|| {
                        let reader = image::ImageReader::open(&entry.path)
                            .ok()?
                            .with_guessed_format()
                            .ok()?;
                        let image = reader.decode().ok()?.thumbnail(250, 100).to_rgba8();
                        Some(ui.ctx().load_texture(
                            entry.path.to_string_lossy(),
                            egui::ColorImage::from_rgba_unmultiplied(
                                [image.width() as usize, image.height() as usize],
                                image.as_raw(),
                            ),
                            Default::default(),
                        ))
                    });
                    if let Some(texture) = texture {
                        ui.add(egui::Image::new(&*texture).max_size(egui::vec2(125.0, 50.0)));
                    }
                    self.links.link(ui, &entry.link, &entry.link);
                    if ui.button(tr("ImgUploaderBase", "Copy URL")).clicked() {
                        self.clipboard.copy(ui.ctx(), &entry.link);
                    }
                    if entry.deletehash.is_some()
                        && ui
                            .add_enabled(
                                self.job.is_none(),
                                egui::Button::new(if self.remote_deleted.contains(&entry.path) {
                                    tr("UploadHistory", "Retry local cleanup")
                                } else {
                                    tr("ImgUploaderBase", "Delete image")
                                }),
                            )
                            .clicked()
                    {
                        requested_delete = Some(index);
                    }
                });
            }
        });
        if let Some(index) = requested_delete
            && self.request_delete(index)
        {
            self.start_delete(ui.ctx(), index);
        }
        if let Some(index) = self.confirm {
            ui.label(tr(
                "UploadLineItem",
                "Are you sure you want to delete a screenshot from the latest uploads and server?",
            ));
            if self.job.is_none() {
                ui.horizontal(|ui| {
                    if ui
                        .button(tr("UploadLineItem", "Confirm to delete"))
                        .clicked()
                    {
                        self.start_delete(ui.ctx(), index);
                    }
                    if ui.button(tr("ColorGrabWidget", "Cancel")).clicked() {
                        self.confirm = None;
                    }
                });
            }
        }
        if self.job.is_some() {
            ui.spinner();
            ui.ctx().request_repaint_after(Duration::from_millis(100));
        }
        self.links.status(ui);
        if !self.message.is_empty() {
            ui.label(&self.message);
        }
        // Escape dismisses an outstanding confirmation before closing history.
        if self.job.is_none()
            && self.clipboard.job.is_none()
            && ui.input(|i| i.key_pressed(egui::Key::Escape))
            && self.confirm.take().is_none()
        {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
}

impl eframe::App for History {
    fn ui(&mut self, ui: &mut egui::Ui, _: &mut eframe::Frame) {
        self.show(ui);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui_test::{click_label, ui_frame};
    use std::io::{Read, Write};
    #[test]
    #[ignore = "requires the private native upload-window fixture"]
    #[cfg(target_os = "linux")]
    fn native_upload_window_confirms_retries_and_saves_history() {
        use winit::platform::x11::EventLoopBuilderExtX11;
        assert_eq!(
            std::env::var("FLAMESHOT_TEST_NATIVE_UPLOAD").as_deref(),
            Ok("1")
        );
        assert_eq!(
            std::env::var("FLAMESHOT_TEST_PRIVATE_BUS").as_deref(),
            Ok("1")
        );
        let state =
            std::path::PathBuf::from(std::env::var_os("FLAMESHOT_TEST_NATIVE_STATE").unwrap());
        let history = tempfile::tempdir().unwrap();
        let settings = crate::settings::Settings {
            copy_url_after_upload: false,
            upload_without_confirmation: false,
            ..Default::default()
        };
        let pixels = image::RgbaImage::from_pixel(64, 48, image::Rgba([32, 64, 96, 255]));
        let (first, failed) = mock_server(false);
        let (second, uploaded) = mock_server(true);
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let _runtime = runtime.enter();
        struct Native {
            dialog: UploadDialog,
            retry: Option<(crate::settings::Settings, Imgur, std::path::PathBuf)>,
            history: std::path::PathBuf,
            state: std::path::PathBuf,
            phase: &'static str,
            complete: std::sync::Arc<std::sync::atomic::AtomicBool>,
        }
        impl eframe::App for Native {
            fn ui(&mut self, ui: &mut egui::Ui, _: &mut eframe::Frame) {
                let keep = self.dialog.show(ui.ctx());
                if self.phase == "confirm"
                    && self.dialog.job.is_none()
                    && self.dialog.message.is_empty()
                {
                    assert!(
                        self.dialog.test_begin.is_some(),
                        "Request began without confirmation"
                    );
                    assert!(
                        crate::upload_history::list(&self.history)
                            .unwrap()
                            .is_empty()
                    );
                }
                if self.phase == "confirm"
                    && self.dialog.job.is_none()
                    && !self.dialog.message.is_empty()
                {
                    assert!(self.dialog.uploaded.is_none());
                    assert!(
                        crate::upload_history::list(&self.history)
                            .unwrap()
                            .is_empty()
                    );
                    self.dialog.test_begin = self.retry.take();
                    self.phase = "failed";
                }
                if self.phase == "failed" && self.dialog.uploaded.is_some() {
                    assert!(self.dialog.message.is_empty());
                    assert_eq!(crate::upload_history::list(&self.history).unwrap().len(), 1);
                    self.phase = "uploaded";
                    self.complete
                        .store(true, std::sync::atomic::Ordering::SeqCst);
                }
                let point = self
                    .dialog
                    .test_upload_rect
                    .map(|rect| rect.center() * ui.ctx().pixels_per_point())
                    .unwrap_or_default();
                let temporary = self.state.with_extension("tmp");
                std::fs::write(
                    &temporary,
                    format!("{}\n{} {}\n", self.phase, point.x, point.y),
                )
                .unwrap();
                std::fs::rename(temporary, &self.state).unwrap();
                if !keep {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
        }
        let complete = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let completed = complete.clone();
        let directory = history.path().to_owned();
        let image = pixels.clone();
        eframe::run_native(
            "Flameshot Upload Fixture",
            eframe::NativeOptions {
                viewport: crate::app_ui::viewport().with_inner_size([640.0, 480.0]),
                event_loop_builder: Some(Box::new(|builder| {
                    builder.with_any_thread(true);
                })),
                ..Default::default()
            },
            Box::new(move |cc| {
                crate::i18n::install_fonts(&cc.egui_ctx);
                let mut dialog = UploadDialog::with_settings(image, Some(&settings));
                dialog.test_begin = Some((settings.clone(), first, directory.clone()));
                Ok(Box::new(Native {
                    dialog,
                    retry: Some((settings, second, directory.clone())),
                    history: directory,
                    state,
                    phase: "confirm",
                    complete: completed,
                }))
            }),
        )
        .unwrap();
        assert!(complete.load(std::sync::atomic::Ordering::SeqCst));
        for request in [failed.join().unwrap(), uploaded.join().unwrap()] {
            assert!(request.starts_with(b"POST /3/image "));
            let png = request
                .windows(8)
                .position(|bytes| bytes == b"\x89PNG\r\n\x1a\n")
                .unwrap();
            assert_eq!(
                image::load_from_memory(&request[png..]).unwrap().to_rgba8(),
                pixels
            );
        }
        let entries = crate::upload_history::list(history.path()).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].link, "https://i.imgur.com/abc123.png");
        assert_eq!(entries[0].deletehash.as_deref(), Some("secret123"));
        let thumbnail = image::open(&entries[0].path).unwrap().to_rgba8();
        assert_eq!(thumbnail.dimensions(), (133, 100));
        assert!(thumbnail.pixels().all(|pixel| pixel.0 == [32, 64, 96, 255]));
    }
    fn mock_server(success: bool) -> (Imgur, std::thread::JoinHandle<Vec<u8>>) {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let endpoint = format!("http://{}/3/image", listener.local_addr().unwrap());
        let task = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut bytes = Vec::new();
            let mut buffer = [0; 4096];
            loop {
                let count = stream.read(&mut buffer).unwrap();
                assert_ne!(count, 0);
                bytes.extend_from_slice(&buffer[..count]);
                if let Some(header_end) = bytes.windows(4).position(|part| part == b"\r\n\r\n") {
                    let header = String::from_utf8_lossy(&bytes[..header_end]);
                    let length = header
                        .lines()
                        .find_map(|line| {
                            line.to_ascii_lowercase()
                                .strip_prefix("content-length:")
                                .map(|value| value.trim().parse::<usize>().unwrap())
                        })
                        .unwrap_or(0);
                    if bytes.len() >= header_end + 4 + length {
                        break;
                    }
                }
            }
            let body = if success && bytes.starts_with(b"DELETE") {
                r#"{"success":true,"data":true}"#
            } else if success {
                r#"{"success":true,"data":{"id":"abc123","link":"https://i.imgur.com/abc123.png","deletehash":"secret123"}}"#
            } else {
                r#"{"success":false}"#
            };
            write!(stream,"HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n{}",body.len(),body).unwrap();
            bytes
        });
        (
            Imgur::local_test_server("test-client", &endpoint).unwrap(),
            task,
        )
    }
    async fn finish(dialog: &mut UploadDialog) -> Option<String> {
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        let mut copied = None;
        while dialog.job.is_some() {
            copied = dialog.poll_upload().or(copied);
            assert!(
                std::time::Instant::now() < deadline,
                "Upload test timed out"
            );
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        copied
    }
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn upload_buttons_confirm_retry_and_close_against_loopback() {
        let ctx = egui::Context::default();
        let root = tempfile::tempdir().unwrap();
        let settings = crate::settings::Settings {
            copy_url_after_upload: false,
            upload_without_confirmation: false,
            ..Default::default()
        };
        let image = image::RgbaImage::from_pixel(1024, 768, image::Rgba([32, 64, 96, 255]));
        let mut cancelled = UploadDialog::with_settings(image.clone(), Some(&settings));
        let mut keep = true;
        click_label(&ctx, tr("UtilityPanel", "Close"), |ui| {
            keep = cancelled.show(ui.ctx())
        });
        assert!(!keep && cancelled.job.is_none() && cancelled.message.is_empty());
        let mut dialog = UploadDialog::with_settings(image.clone(), Some(&settings));
        let (api, failed) = mock_server(false);
        dialog.test_begin = Some((settings.clone(), api, root.path().to_owned()));
        ui_frame(&ctx, vec![], &mut |ui| {
            dialog.show(ui.ctx());
        });
        assert!(dialog.job.is_none() && dialog.test_begin.is_some());
        click_label(&ctx, tr("ImgUploaderBase", "Upload image"), |ui| {
            dialog.show(ui.ctx());
        });
        assert!(
            dialog.job.is_some(),
            "Upload button did not start a request"
        );
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while dialog.job.is_some() {
            assert!(std::time::Instant::now() < deadline);
            ui_frame(&ctx, vec![], &mut |ui| {
                dialog.show(ui.ctx());
            });
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        assert!(dialog.uploaded.is_none() && !dialog.message.is_empty());
        assert!(failed.join().unwrap().starts_with(b"POST /3/image "));
        assert!(crate::upload_history::list(root.path()).unwrap().is_empty());
        let (api, success) = mock_server(true);
        dialog.test_begin = Some((settings, api, root.path().to_owned()));
        click_label(&ctx, tr("ImgUploaderBase", "Upload image"), |ui| {
            dialog.show(ui.ctx());
        });
        assert!(
            dialog.job.is_some(),
            "Upload button did not start a request"
        );
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while dialog.job.is_some() {
            assert!(std::time::Instant::now() < deadline);
            ui_frame(&ctx, vec![], &mut |ui| {
                dialog.show(ui.ctx());
            });
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        assert!(dialog.uploaded.is_some() && dialog.message.is_empty());
        let request = success.join().unwrap();
        let png = request
            .windows(8)
            .position(|bytes| bytes == b"\x89PNG\r\n\x1a\n")
            .unwrap();
        assert_eq!(
            image::load_from_memory(&request[png..]).unwrap().to_rgba8(),
            image
        );
        assert_eq!(crate::upload_history::list(root.path()).unwrap().len(), 1);
        click_label(&ctx, tr("UtilityPanel", "Close"), |ui| {
            keep = dialog.show(ui.ctx())
        });
        assert!(!keep);
    }
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn history_buttons_confirm_cancel_retry_and_remove_only_after_success() {
        let ctx = egui::Context::default();
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("image.png");
        image::RgbaImage::from_pixel(4, 3, image::Rgba([32, 64, 96, 255]))
            .save(&path)
            .unwrap();
        let original = std::fs::read(&path).unwrap();
        let mut history = History {
            links: Default::default(),
            confirmation_required: true,
            test_delete: None,
            textures: Default::default(),
            clipboard: Default::default(),
            entries: vec![crate::upload_history::Entry {
                path: path.clone(),
                link: "https://i.imgur.com/abc123.png".into(),
                deletehash: Some("secret123".into()),
            }],
            confirm: None,
            job: None,
            remote_deleted: Default::default(),
            message: String::new(),
        };
        click_label(&ctx, tr("ImgUploaderBase", "Delete image"), |ui| {
            history.show(ui)
        });
        assert_eq!(history.confirm, Some(0));
        assert!(history.job.is_none() && history.message.is_empty());
        click_label(&ctx, tr("ColorGrabWidget", "Cancel"), |ui| history.show(ui));
        assert!(history.confirm.is_none());
        assert_eq!(std::fs::read(&path).unwrap(), original);
        click_label(&ctx, tr("ImgUploaderBase", "Delete image"), |ui| {
            history.show(ui)
        });
        let output = ui_frame(
            &ctx,
            vec![egui::Event::Key {
                key: egui::Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
            &mut |ui| history.show(ui),
        );
        assert!(history.confirm.is_none() && history.job.is_none());
        assert!(output.viewport_output.values().all(|viewport| {
            !viewport
                .commands
                .iter()
                .any(|command| matches!(command, egui::ViewportCommand::Close))
        }));
        ui_frame(
            &ctx,
            vec![egui::Event::Key {
                key: egui::Key::Escape,
                physical_key: None,
                pressed: false,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
            &mut |ui| history.show(ui),
        );
        assert_eq!(std::fs::read(&path).unwrap(), original);
        for success in [false, true] {
            let (api, server) = mock_server(success);
            history.test_delete = Some(api);
            if history.confirm.is_none() {
                click_label(&ctx, tr("ImgUploaderBase", "Delete image"), |ui| {
                    history.show(ui)
                });
            }
            assert!(history.job.is_none() && history.test_delete.is_some());
            click_label(&ctx, tr("UploadLineItem", "Confirm to delete"), |ui| {
                history.show(ui)
            });
            assert!(
                history.job.is_some(),
                "Confirmation button did not start deletion"
            );
            let deadline = std::time::Instant::now() + Duration::from_secs(5);
            while history.job.is_some() {
                assert!(std::time::Instant::now() < deadline);
                ui_frame(&ctx, vec![], &mut |ui| history.show(ui));
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
            assert!(
                server
                    .join()
                    .unwrap()
                    .starts_with(b"DELETE /3/image/secret123 ")
            );
            assert_eq!(history.entries.is_empty(), success);
            assert_eq!(path.exists(), !success);
            if !success {
                assert_eq!(std::fs::read(&path).unwrap(), original);
                assert!(!history.message.is_empty());
            }
        }
    }
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn confirmed_remote_deletion_accepts_a_concurrently_removed_thumbnail() {
        let ctx = egui::Context::default();
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("image.png");
        image::RgbaImage::new(4, 3).save(&path).unwrap();
        let mut history = History {
            links: Default::default(),
            confirmation_required: true,
            test_delete: None,
            textures: Default::default(),
            clipboard: Default::default(),
            entries: vec![crate::upload_history::Entry {
                path: path.clone(),
                link: "https://i.imgur.com/abc123.png".into(),
                deletehash: Some("secret123".into()),
            }],
            confirm: None,
            job: None,
            remote_deleted: Default::default(),
            message: String::new(),
        };
        click_label(&ctx, tr("ImgUploaderBase", "Delete image"), |ui| {
            history.show(ui)
        });
        std::fs::remove_file(&path).unwrap(); // Retention cleanup in another process.
        let (api, server) = mock_server(true);
        history.test_delete = Some(api);
        click_label(&ctx, tr("UploadLineItem", "Confirm to delete"), |ui| {
            history.show(ui)
        });
        assert!(history.job.is_some());
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while history.job.is_some() {
            assert!(std::time::Instant::now() < deadline);
            ui_frame(&ctx, vec![], &mut |ui| history.show(ui));
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        server.join().unwrap();
        assert!(
            history.entries.is_empty(),
            "Remote deletion succeeded but stale history remained: {}",
            history.message
        );
        assert!(history.message.is_empty());
        // Other filesystem errors must still be reported, not silently ignored.
        std::fs::create_dir(&path).unwrap();
        history.entries.push(crate::upload_history::Entry {
            path: path.clone(),
            link: "https://i.imgur.com/abc123.png".into(),
            deletehash: Some("secret123".into()),
        });
        let (api, server) = mock_server(true);
        history.test_delete = Some(api);
        click_label(&ctx, tr("ImgUploaderBase", "Delete image"), |ui| {
            history.show(ui)
        });
        click_label(&ctx, tr("UploadLineItem", "Confirm to delete"), |ui| {
            history.show(ui)
        });
        assert!(history.job.is_some());
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while history.job.is_some() {
            assert!(std::time::Instant::now() < deadline);
            ui_frame(&ctx, vec![], &mut |ui| history.show(ui));
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        server.join().unwrap();
        assert!(path.is_dir() && history.entries.len() == 1 && !history.message.is_empty());
        assert!(history.remote_deleted.contains(&path));
        assert!(history.test_delete.is_none());
        for repaired in [false, true] {
            if repaired {
                std::fs::remove_dir(&path).unwrap();
            }
            // No injected API remains. A remote retry would fail the test guard.
            click_label(&ctx, tr("UploadHistory", "Retry local cleanup"), |ui| {
                history.show(ui)
            });
            assert!(history.job.is_some() && history.confirm.is_none());
            let deadline = std::time::Instant::now() + Duration::from_secs(5);
            while history.job.is_some() {
                assert!(std::time::Instant::now() < deadline);
                ui_frame(&ctx, vec![], &mut |ui| history.show(ui));
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
            assert_eq!(history.entries.is_empty(), repaired);
            assert_eq!(history.remote_deleted.contains(&path), !repaired);
        }
    }
    #[test]
    fn stopped_clipboard_worker_releases_pending_state() {
        let (sender, receiver) = mpsc::channel();
        drop(sender);
        let mut clipboard = LinkClipboard {
            job: Some(receiver),
        };
        assert_eq!(
            clipboard.poll(&egui::Context::default()).as_deref(),
            Some("Clipboard worker stopped")
        );
        assert!(clipboard.job.is_none());
        for endpoint in [
            "https://api.imgur.com/3/image",
            "http://192.0.2.1:1234/3/image",
            "http://user@127.0.0.1:1234/3/image",
        ] {
            assert!(Imgur::local_test_server("test", endpoint).is_err());
        }
    }
    #[test]
    fn default_confirmation_window_does_not_start_a_request() {
        let settings = crate::settings::Settings::default();
        let mut dialog = UploadDialog::with_settings(image::RgbaImage::new(4, 3), Some(&settings));
        let ctx = egui::Context::default();
        let mut output = ctx.run_ui(Default::default(), |ui| {
            assert!(dialog.show(ui.ctx()));
        });
        output.textures_delta.clear();
        assert!(dialog.job.is_none());
        assert!(dialog.uploaded.is_none());
    }
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn automatic_upload_runs_once_and_auto_copy_follows_preference() {
        for copy in [false, true] {
            let settings = crate::settings::Settings {
                upload_without_confirmation: true,
                copy_url_after_upload: copy,
                ..Default::default()
            };
            let mut dialog = UploadDialog::with_settings(
                image::RgbaImage::from_pixel(4, 3, image::Rgba([1, 2, 3, 255])),
                Some(&settings),
            );
            assert!(dialog.take_auto_start());
            assert!(!dialog.take_auto_start());
            let temp = tempfile::tempdir().unwrap();
            let history = temp.path().join("history");
            let (api, server) = mock_server(true);
            dialog.start(
                &egui::Context::default(),
                settings,
                api,
                Ok(history.clone()),
            );
            assert_eq!(
                finish(&mut dialog).await,
                copy.then(|| "https://i.imgur.com/abc123.png".into())
            );
            assert!(dialog.uploaded.is_some());
            assert!(!dialog.take_auto_start());
            assert!(dialog.poll_upload().is_none());
            let request = server.join().unwrap();
            assert!(request.starts_with(b"POST /3/image HTTP/1.1"));
            assert!(request.windows(8).any(|part| part == b"\x89PNG\r\n\x1a\n"));
            assert_eq!(crate::upload_history::list(&history).unwrap().len(), 1);
        }
    }
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn confirmation_failure_retry_and_history_warning_remain_distinct() {
        let settings = crate::settings::Settings::default();
        let mut dialog = UploadDialog::with_settings(image::RgbaImage::new(4, 3), Some(&settings));
        assert!(!dialog.take_auto_start());
        assert!(dialog.job.is_none());
        let (api, server) = mock_server(false);
        dialog.start(
            &egui::Context::default(),
            settings.clone(),
            api,
            Err(anyhow::anyhow!("missing history")),
        );
        assert!(finish(&mut dialog).await.is_none());
        server.join().unwrap();
        assert!(dialog.uploaded.is_none());
        assert!(!dialog.message.is_empty());
        assert!(!dialog.take_auto_start());
        let (api, server) = mock_server(true);
        dialog.start(
            &egui::Context::default(),
            settings,
            api,
            Err(anyhow::anyhow!("missing history")),
        );
        assert!(finish(&mut dialog).await.is_some());
        server.join().unwrap();
        assert!(dialog.message.contains("history could not be saved"));
        assert!(dialog.uploaded.is_some());
    }
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn deletion_preference_and_failures_preserve_history_until_remote_success() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("image.png");
        std::fs::write(&path, b"synthetic").unwrap();
        let mut history = History {
            links: Default::default(),
            confirmation_required: true,
            test_delete: None,
            textures: Default::default(),
            clipboard: Default::default(),
            entries: vec![crate::upload_history::Entry {
                path: path.clone(),
                link: "https://i.imgur.com/abc123.png".into(),
                deletehash: Some("secret123".into()),
            }],
            confirm: None,
            job: None,
            remote_deleted: Default::default(),
            message: String::new(),
        };
        assert!(!history.request_delete(0));
        assert_eq!(history.confirm, Some(0));
        assert!(history.job.is_none());
        history.confirmation_required = false;
        assert!(history.request_delete(0));
        assert!(history.confirm.is_none());
        for success in [false, true] {
            let (api, task) = mock_server(success);
            history.begin_delete(&egui::Context::default(), 0, api);
            let deadline = std::time::Instant::now() + Duration::from_secs(5);
            while history.job.is_some() {
                history.poll_delete();
                assert!(std::time::Instant::now() < deadline);
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
            assert!(
                task.join()
                    .unwrap()
                    .starts_with(b"DELETE /3/image/secret123 HTTP/1.1")
            );
            assert_eq!(path.exists(), !success);
            assert_eq!(history.entries.is_empty(), success);
        }
    }
}
