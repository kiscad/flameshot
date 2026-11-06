// SPDX-License-Identifier: GPL-3.0-or-later
use std::{
    process::{Command, Stdio},
    sync::Arc,
};
pub struct Tray {
    pub quit: Arc<tokio::sync::Notify>,
}
fn launch(command: &str) {
    if let Ok(executable) = std::env::current_exe()
        && let Ok(mut child) = Command::new(executable)
            .arg(command)
            .stdout(Stdio::null())
            .spawn()
    {
        std::thread::spawn(move || {
            let _ = child.wait();
        });
    }
}
impl ksni::Tray for Tray {
    fn id(&self) -> String {
        "flameshot".into()
    }
    fn title(&self) -> String {
        "Flameshot".into()
    }
    fn icon_name(&self) -> String {
        "org.flameshot.Flameshot".into()
    }
    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        static ICON: std::sync::OnceLock<Vec<u8>> = std::sync::OnceLock::new();
        let bytes = ICON.get_or_init(|| {
            let image = image::load_from_memory(include_bytes!(
                "../../../data/img/app/org.flameshot.Flameshot.png"
            ))
            .expect("embedded Flameshot icon")
            .resize_exact(32, 32, image::imageops::FilterType::Lanczos3)
            .to_rgba8();
            image
                .pixels()
                .flat_map(|p| [p[3], p[0], p[1], p[2]])
                .collect()
        });
        vec![ksni::Icon {
            width: 32,
            height: 32,
            data: bytes.clone(),
        }]
    }
    fn activate(&mut self, _x: i32, _y: i32) {
        launch("gui");
    }
    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        use ksni::menu::StandardItem;
        vec![
            StandardItem {
                label: "Take Screenshot".into(),
                activate: Box::new(|_| launch("gui")),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Configuration".into(),
                activate: Box::new(|_| launch("config")),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Quit".into(),
                activate: Box::new(|tray: &mut Self| tray.quit.notify_one()),
                ..Default::default()
            }
            .into(),
        ]
    }
}
