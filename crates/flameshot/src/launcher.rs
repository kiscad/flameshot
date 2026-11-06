// SPDX-License-Identifier: GPL-3.0-or-later
use anyhow::{Result, ensure};
use eframe::egui;
use flameshot_core::region::Region;
use std::{
    ffi::OsString,
    path::PathBuf,
    process::{Command, Stdio},
    sync::mpsc,
    time::{Duration, Instant},
};
#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Region,
    Full,
}
struct Request {
    mode: Mode,
    screen: Option<usize>,
    region: Option<Region>,
    clipboard: bool,
    path: Option<PathBuf>,
}
impl Request {
    fn arguments(&self) -> Result<Vec<OsString>> {
        let mut args = Vec::<OsString>::new();
        if let Some(index) = self.screen {
            args.extend(["screen".into(), "--number".into(), index.to_string().into()]);
            if self.mode == Mode::Region {
                args.push("--edit".into());
            }
        } else {
            args.push(
                if self.mode == Mode::Region {
                    "gui"
                } else {
                    "full"
                }
                .into(),
            );
        }
        if self.mode == Mode::Region
            && let Some(region) = self.region
        {
            ensure!(
                region.width > 0 && region.height > 0,
                "Region dimensions must be positive"
            );
            args.extend([
                "--region".into(),
                format!(
                    "{}x{}+{}+{}",
                    region.width, region.height, region.x, region.y
                )
                .into(),
            ]);
        }
        if self.clipboard {
            args.push("--clipboard".into());
        }
        if let Some(path) = &self.path {
            crate::output::validate_destination(path)?;
            args.extend(["--path".into(), path.as_os_str().to_owned()]);
        }
        // Give the window system time to hide the launcher before capture.
        args.extend(["--delay".into(), "600".into()]);
        Ok(args)
    }
}
pub fn show() -> Result<()> {
    let screens = flameshot_platform::screens::list(crate::is_wayland());
    let (screens, error) = match screens {
        Ok(screens) => (screens, String::new()),
        Err(e) => (vec![], e.to_string()),
    };
    let last = crate::region_cache::load().ok().flatten();
    let launcher = Launcher {
        selected: if screens.is_empty() { None } else { Some(0) },
        screens,
        mode: Mode::Region,
        delay: 0,
        clipboard: false,
        path: String::new(),
        use_region: last.is_some(),
        region: last.unwrap_or(Region {
            x: 0,
            y: 0,
            width: 100,
            height: 100,
        }),
        deadline: None,
        job: None,
        error,
    };
    eframe::run_native(
        "Flameshot Launcher",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default().with_inner_size([500.0, 400.0]),
            ..Default::default()
        },
        Box::new(move |cc| {
            crate::i18n::install_fonts(&cc.egui_ctx);
            Ok(Box::new(launcher))
        }),
    )
    .map_err(|e| anyhow::anyhow!("Launcher: {e}"))
}
struct Launcher {
    screens: Vec<flameshot_platform::screens::Screen>,
    selected: Option<usize>,
    mode: Mode,
    delay: u32,
    clipboard: bool,
    path: String,
    use_region: bool,
    region: Region,
    deadline: Option<Instant>,
    job: Option<mpsc::Receiver<Result<std::process::ExitStatus, String>>>,
    error: String,
}
impl Launcher {
    fn launch(&mut self, ctx: &egui::Context) {
        let request = Request {
            mode: self.mode,
            screen: self.selected,
            region: self.use_region.then_some(self.region),
            clipboard: self.clipboard,
            path: (!self.path.is_empty()).then(|| PathBuf::from(&self.path)),
        };
        let args = match request.arguments() {
            Ok(args) => args,
            Err(e) => {
                self.error = e.to_string();
                return;
            }
        };
        let executable = match std::env::current_exe() {
            Ok(exe) => exe,
            Err(e) => {
                self.error = e.to_string();
                return;
            }
        };
        match Command::new(executable)
            .args(args)
            .stdout(Stdio::null())
            .spawn()
        {
            Ok(mut child) => {
                let (sender, receiver) = mpsc::channel();
                self.job = Some(receiver);
                ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
                let ctx = ctx.clone();
                std::thread::spawn(move || {
                    let _ = sender.send(child.wait().map_err(|e| e.to_string()));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                    ctx.request_repaint();
                });
            }
            Err(error) => self.error = error.to_string(),
        }
    }
}
impl eframe::App for Launcher {
    fn ui(&mut self, ui: &mut egui::Ui, _: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if let Some(job) = &self.job
            && let Ok(result) = job.try_recv()
        {
            self.job = None;
            self.error = match result {
                Ok(status) if status.success() || status.code() == Some(2) => String::new(),
                Ok(status) => format!("Capture process exited with {status}"),
                Err(error) => error,
            };
            if let Ok(Some(region)) = crate::region_cache::load() {
                self.region = region;
            }
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) && self.deadline.take().is_none() {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        if let Some(deadline) = self.deadline {
            let remaining = deadline.saturating_duration_since(Instant::now());
            ui.heading(format!(
                "Capture in {}…",
                remaining.as_secs() + u64::from(remaining.subsec_nanos() > 0)
            ));
            if ui.button("Cancel").clicked() {
                self.deadline = None;
            } else if remaining.is_zero() {
                self.deadline = None;
                self.launch(&ctx);
            }
            ctx.request_repaint_after(Duration::from_millis(100));
            return;
        }
        if self.job.is_some() {
            ctx.request_repaint_after(Duration::from_millis(100));
            return;
        }
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.heading("Capture Launcher");
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.mode, Mode::Region, "Rectangular Region");
                ui.selectable_value(&mut self.mode, Mode::Full, "Full Screen");
            });
            egui::ComboBox::from_label("Screen")
                .selected_text(
                    self.selected
                        .and_then(|i| self.screens.get(i))
                        .map(|s| s.name.as_str())
                        .unwrap_or("All screens"),
                )
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.selected, None, "All screens");
                    for (i, screen) in self.screens.iter().enumerate() {
                        ui.selectable_value(
                            &mut self.selected,
                            Some(i),
                            format!(
                                "{}: {} ({}×{})",
                                i + 1,
                                screen.name,
                                screen.bounds.width,
                                screen.bounds.height
                            ),
                        );
                    }
                });
            ui.add(egui::Slider::new(&mut self.delay, 0..=60).text("Delay (seconds)"));
            if self.mode == Mode::Region {
                ui.checkbox(&mut self.use_region, "Initial region");
                if self.use_region {
                    ui.horizontal(|ui| {
                        ui.add(egui::DragValue::new(&mut self.region.x).prefix("X "));
                        ui.add(egui::DragValue::new(&mut self.region.y).prefix("Y "));
                        ui.add(
                            egui::DragValue::new(&mut self.region.width)
                                .range(1..=100000)
                                .prefix("W "),
                        );
                        ui.add(
                            egui::DragValue::new(&mut self.region.height)
                                .range(1..=100000)
                                .prefix("H "),
                        );
                    });
                }
            }
            ui.checkbox(&mut self.clipboard, "Copy to clipboard");
            ui.label("Save path (optional)");
            ui.text_edit_singleline(&mut self.path);
            if !self.error.is_empty() {
                ui.colored_label(egui::Color32::RED, &self.error);
            }
            if ui.button("Launch").clicked()
                || (!ctx.egui_wants_keyboard_input()
                    && ctx.input(|i| i.key_pressed(egui::Key::Enter)))
            {
                self.error.clear();
                self.deadline = Some(Instant::now() + Duration::from_secs(u64::from(self.delay)));
                ctx.request_repaint();
            }
        });
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn launcher_builds_argument_vectors_without_shell_interpretation() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("space $(literal).png");
        let request = Request {
            mode: Mode::Region,
            screen: Some(2),
            region: Some(Region {
                x: -10,
                y: 20,
                width: 30,
                height: 40,
            }),
            clipboard: true,
            path: Some(path.clone()),
        };
        let args = request.arguments().unwrap();
        assert_eq!(
            &args[..3],
            &[OsString::from("screen"), "--number".into(), "2".into()]
        );
        assert!(args.contains(&"--edit".into()));
        assert!(args.contains(&"30x40+-10+20".into()));
        assert!(args.contains(&path.into_os_string()));
        assert_eq!(args.last().unwrap(), "600");
    }
}
