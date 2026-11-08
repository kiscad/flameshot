// SPDX-License-Identifier: GPL-3.0-or-later
use crate::i18n::tr;
use eframe::egui;
use flameshot_platform::updates::{self, Update};
pub fn show() -> anyhow::Result<()> {
    eframe::run_native(
        tr("RustUpdates", "Flameshot Updates"),
        eframe::NativeOptions {
            viewport: crate::app_ui::viewport().with_inner_size([440.0, 180.0]),
            ..Default::default()
        },
        Box::new(move |cc| {
            crate::i18n::install_fonts(&cc.egui_ctx);
            let mut window = Updates {
                links: Default::default(),
                job: None,
                #[cfg(test)]
                test_endpoint: None,
                #[cfg(test)]
                test_settings_path: None,
                #[cfg(test)]
                test_controls: Default::default(),
                update: None,
                message: tr("RustUpdates", "Checking for updates…").into(),
            };
            window.begin(&cc.egui_ctx);
            Ok(Box::new(window))
        }),
    )
    .map_err(|e| anyhow::anyhow!("Updates window: {e}"))
}
struct Updates {
    #[cfg(test)]
    test_endpoint: Option<String>,
    #[cfg(test)]
    test_settings_path: Option<std::path::PathBuf>,
    #[cfg(test)]
    test_controls: std::collections::HashMap<&'static str, egui::Rect>,
    links: crate::links::LinkOpener,
    job: Option<std::sync::mpsc::Receiver<Result<Option<Update>, String>>>,
    update: Option<Update>,
    message: String,
}
impl Updates {
    fn begin(&mut self, ctx: &egui::Context) {
        if self.job.is_some() {
            return;
        }
        self.update = None;
        self.message = tr("RustUpdates", "Checking for updates…").into();
        let (sender, receiver) = std::sync::mpsc::channel();
        self.job = Some(receiver);
        let ctx = ctx.clone();
        #[cfg(test)]
        let endpoint = self.test_endpoint.take();
        tokio::spawn(async move {
            #[cfg(test)]
            let result = match endpoint {
                Some(endpoint) => {
                    updates::check_local_test_server(&endpoint, env!("CARGO_PKG_VERSION"), "").await
                }
                None => Err(anyhow::anyhow!("Test update endpoint not configured")),
            };
            #[cfg(not(test))]
            let result = updates::check(env!("CARGO_PKG_VERSION"), "").await;
            let _ = sender.send(result.map_err(|error| error.to_string()));
            ctx.request_repaint();
        });
    }
    fn ignore_current(&mut self) {
        let Some(update) = &self.update else {
            return;
        };
        let values = [("ignoreUpdateToVersion", update.version.clone())];
        #[cfg(test)]
        let result = self
            .test_settings_path
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Test settings path not configured"))
            .and_then(|path| crate::settings::Settings::update_at(path, &values));
        #[cfg(not(test))]
        let result = crate::settings::Settings::update(&values);
        self.message = match result {
            Ok(()) => tr("RustUpdates", "Version ignored").into(),
            Err(error) => error.to_string(),
        };
    }
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
            Ok(None) => {
                self.update = None;
                self.message = tr("RustUpdates", "You have the latest version").into();
            }
            Err(error) => {
                self.update = None;
                self.message = tr("RustUpdates", "Update check failed: %1").replace("%1", &error)
            }
        }
    }
}
impl Updates {
    fn ui_contents(&mut self, ui: &mut egui::Ui) {
        self.poll();
        #[cfg(test)]
        self.test_controls.clear();
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
                    let ignore = ui.button(tr("RustUpdates", "Ignore this version"));
                    #[cfg(test)]
                    self.test_controls.insert("ignore", ignore.rect);
                    if ignore.clicked() {
                        self.ignore_current();
                    }
                }
                self.links.status(ui);
                let check = ui.add_enabled(
                    self.job.is_none(),
                    egui::Button::new(tr("RustConfig", "Check for updates")),
                );
                #[cfg(test)]
                self.test_controls.insert("check", check.rect);
                if check.clicked() {
                    self.begin(ui.ctx());
                }
            });
        if ui.button(tr("UtilityPanel", "Close")).clicked()
            || ui.input(|i| i.key_pressed(egui::Key::Escape))
        {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
}

impl eframe::App for Updates {
    fn ui(&mut self, ui: &mut egui::Ui, _: &mut eframe::Frame) {
        self.ui_contents(ui);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui_test::{click_label, ui_frame};
    #[test]
    #[ignore = "requires the private native update-window fixture"]
    #[cfg(target_os = "linux")]
    fn native_update_window_uses_only_loopback_and_persists_ignore() {
        use winit::platform::x11::EventLoopBuilderExtX11;
        assert_eq!(
            std::env::var("FLAMESHOT_TEST_NATIVE_UPDATES").as_deref(),
            Ok("1")
        );
        assert_eq!(
            std::env::var("FLAMESHOT_TEST_PRIVATE_BUS").as_deref(),
            Ok("1")
        );
        let state =
            std::path::PathBuf::from(std::env::var_os("FLAMESHOT_TEST_NATIVE_STATE").unwrap());
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("flameshot.ini");
        std::fs::write(
            &path,
            "[General]\ncheckForUpdates=false\n[External]\nprivate=keep\n",
        )
        .unwrap();
        let (failure, release_failure, failure_server) = server("invalid JSON");
        let (first, release_first, first_server) = server(
            r#"{"tag_name":"v99.0.0","html_url":"https://github.com/flameshot-org/flameshot/releases/tag/v99.0.0"}"#,
        );
        let (second, release_second, second_server) = server(
            r#"{"tag_name":"v1.0.0","html_url":"https://github.com/flameshot-org/flameshot/releases/tag/v1.0.0"}"#,
        );
        release_failure.send(()).unwrap();
        release_first.send(()).unwrap();
        release_second.send(()).unwrap();
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let _runtime = runtime.enter();
        struct Native {
            window: Updates,
            state: std::path::PathBuf,
            retry: Option<String>,
            second: Option<String>,
            phase: &'static str,
            verified: std::sync::Arc<std::sync::atomic::AtomicBool>,
        }
        impl eframe::App for Native {
            fn ui(&mut self, ui: &mut egui::Ui, _: &mut eframe::Frame) {
                self.window.ui_contents(ui);
                if self.phase == "waiting"
                    && self.window.job.is_none()
                    && self.window.message.contains("Invalid release response")
                {
                    assert!(self.window.update.is_none());
                    self.window.test_endpoint = self.retry.take();
                    self.phase = "failed";
                }
                if self.phase == "failed" && self.window.update.is_some() {
                    self.phase = "available";
                }
                if self.phase == "available" && self.window.message == "Version ignored" {
                    self.window.test_endpoint = self.second.take();
                    self.phase = "ignored";
                }
                if self.phase == "ignored"
                    && self.window.job.is_none()
                    && self.window.message == "You have the latest version"
                {
                    assert!(self.window.update.is_none());
                    self.phase = "rechecked";
                    self.verified
                        .store(true, std::sync::atomic::Ordering::SeqCst);
                }
                let point = |name| {
                    self.window
                        .test_controls
                        .get(name)
                        .map(|rect| rect.center() * ui.ctx().pixels_per_point())
                        .unwrap_or_default()
                };
                let ignore = point("ignore");
                let check = point("check");
                let value = format!(
                    "{}\n{} {}\n{} {}\n",
                    self.phase, ignore.x, ignore.y, check.x, check.y
                );
                let temporary = self.state.with_extension("tmp");
                std::fs::write(&temporary, value).unwrap();
                std::fs::rename(temporary, &self.state).unwrap();
            }
        }
        let verified = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let finished = verified.clone();
        let settings_path = path.clone();
        eframe::run_native(
            "Flameshot Updates",
            eframe::NativeOptions {
                viewport: crate::app_ui::viewport().with_inner_size([440.0, 180.0]),
                event_loop_builder: Some(Box::new(|builder| {
                    builder.with_any_thread(true);
                })),
                ..Default::default()
            },
            Box::new(move |cc| {
                crate::i18n::install_fonts(&cc.egui_ctx);
                let mut window = Updates {
                    links: Default::default(),
                    job: None,
                    update: None,
                    message: String::new(),
                    test_endpoint: Some(failure),
                    test_settings_path: Some(settings_path),
                    test_controls: Default::default(),
                };
                window.begin(&cc.egui_ctx);
                Ok(Box::new(Native {
                    window,
                    state,
                    retry: Some(first),
                    second: Some(second),
                    phase: "waiting",
                    verified: finished,
                }))
            }),
        )
        .unwrap();
        assert!(verified.load(std::sync::atomic::Ordering::SeqCst));
        assert!(failure_server.join().unwrap().starts_with(b"GET /latest "));
        assert!(first_server.join().unwrap().starts_with(b"GET /latest "));
        assert!(second_server.join().unwrap().starts_with(b"GET /latest "));
        let saved = std::fs::read_to_string(path).unwrap();
        let ini = flameshot_core::config::IniDocument::parse(saved.clone());
        assert_eq!(
            ini.string("General", "ignoreUpdateToVersion")
                .unwrap()
                .as_deref(),
            Some("99.0.0")
        );
        assert!(saved.contains("[External]\nprivate=keep\n"));
    }
    fn server(
        body: &str,
    ) -> (
        String,
        std::sync::mpsc::Sender<()>,
        std::thread::JoinHandle<Vec<u8>>,
    ) {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let endpoint = format!("http://{}/latest", listener.local_addr().unwrap());
        let body = body.to_owned();
        let (release, gate) = std::sync::mpsc::channel();
        let task = std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            socket
                .set_read_timeout(Some(std::time::Duration::from_secs(5)))
                .unwrap();
            let mut request = Vec::new();
            let mut chunk = [0; 1024];
            while !request.windows(4).any(|bytes| bytes == b"\r\n\r\n") {
                let count = socket.read(&mut chunk).unwrap();
                assert_ne!(count, 0);
                request.extend_from_slice(&chunk[..count]);
            }
            gate.recv_timeout(std::time::Duration::from_secs(5))
                .unwrap();
            write!(socket,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",body.len(),body).unwrap();
            request
        });
        (endpoint, release, task)
    }
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn update_controls_retry_ignore_and_clear_stale_release() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("flameshot.ini");
        std::fs::write(&path, "[General]\nprivateValue=keep\n").unwrap();
        let ctx = egui::Context::default();
        let mut window = Updates {
            links: Default::default(),
            job: None,
            update: None,
            message: String::new(),
            test_endpoint: None,
            test_settings_path: Some(path.clone()),
            test_controls: Default::default(),
        };
        for (body, available) in [
            ("invalid JSON", false),
            (
                r#"{"tag_name":"v99.0.0","html_url":"https://github.com/flameshot-org/flameshot/releases/tag/v99.0.0"}"#,
                true,
            ),
            (
                r#"{"tag_name":"v1.0.0","html_url":"https://github.com/flameshot-org/flameshot/releases/tag/v1.0.0"}"#,
                false,
            ),
        ] {
            let (endpoint, release, task) = server(body);
            window.test_endpoint = Some(endpoint);
            click_label(&ctx, tr("RustConfig", "Check for updates"), |ui| {
                window.ui_contents(ui)
            });
            assert!(window.job.is_some() && window.update.is_none());
            click_label(&ctx, tr("RustConfig", "Check for updates"), |ui| {
                window.ui_contents(ui)
            });
            assert!(
                window.job.is_some(),
                "Pending check must keep its disabled button"
            );
            release.send(()).unwrap();
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
            while window.job.is_some() {
                assert!(std::time::Instant::now() < deadline);
                ui_frame(&ctx, vec![], &mut |ui| window.ui_contents(ui));
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
            assert!(task.join().unwrap().starts_with(b"GET /latest "));
            assert_eq!(window.update.is_some(), available);
            assert!(!window.message.is_empty());
            if body == "invalid JSON" {
                assert!(window.message.contains("Invalid release response"));
            }
            if available {
                let blocked = root.path().join("directory-instead-of-settings");
                std::fs::create_dir(&blocked).unwrap();
                window.test_settings_path = Some(blocked);
                click_label(&ctx, tr("RustUpdates", "Ignore this version"), |ui| {
                    window.ui_contents(ui)
                });
                assert_ne!(window.message, tr("RustUpdates", "Version ignored"));
                assert_eq!(
                    std::fs::read_to_string(&path).unwrap(),
                    "[General]\nprivateValue=keep\n"
                );
                window.test_settings_path = Some(path.clone());
                click_label(&ctx, tr("RustUpdates", "Ignore this version"), |ui| {
                    window.ui_contents(ui)
                });
                let source = std::fs::read_to_string(&path).unwrap();
                let ini = flameshot_core::config::IniDocument::parse(source);
                assert_eq!(
                    ini.string("General", "ignoreUpdateToVersion")
                        .unwrap()
                        .as_deref(),
                    Some("99.0.0")
                );
                assert_eq!(
                    ini.string("General", "privateValue").unwrap().as_deref(),
                    Some("keep")
                );
                assert_eq!(window.message, tr("RustUpdates", "Version ignored"));
            }
        }
        assert_eq!(
            window.message,
            tr("RustUpdates", "You have the latest version")
        );
        let output = click_label(&ctx, tr("UtilityPanel", "Close"), |ui| {
            window.ui_contents(ui)
        });
        assert!(output.viewport_output.values().any(|viewport| {
            viewport
                .commands
                .iter()
                .any(|command| matches!(command, egui::ViewportCommand::Close))
        }));
    }
    #[test]
    fn worker_completion_and_disconnect_leave_the_pending_state() {
        let (sender, receiver) = std::sync::mpsc::channel();
        let mut window = Updates {
            test_endpoint: None,
            test_settings_path: None,
            test_controls: Default::default(),
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
