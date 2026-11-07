// SPDX-License-Identifier: GPL-3.0-or-later
use std::sync::Arc;
pub struct Tray {
    pub quit: Arc<tokio::sync::Notify>,
    pub activity: Arc<crate::daemon::Activity>,
}
impl Tray {
    fn launch(&self, command: &str) {
        let _ = self.activity.launch(command);
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
        self.launch("gui");
    }
    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        use ksni::menu::StandardItem;
        vec![
            StandardItem {
                label: crate::i18n::dbus_menu_label("TrayIcon", "&Take Screenshot"),
                activate: Box::new(|tray: &mut Self| tray.launch("gui")),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: crate::i18n::dbus_menu_label("TrayIcon", "&Open Launcher"),
                activate: Box::new(|tray: &mut Self| tray.launch("launcher")),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: crate::i18n::dbus_menu_label("TrayIcon", "&Configuration"),
                activate: Box::new(|tray: &mut Self| tray.launch("config")),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: crate::i18n::dbus_menu_label("TrayIcon", "&About"),
                activate: Box::new(|tray: &mut Self| tray.launch("internal-about")),
                ..Default::default()
            }
            .into(),
            #[cfg(feature = "imgur")]
            StandardItem {
                label: crate::i18n::dbus_menu_label("TrayIcon", "&Latest Uploads"),
                activate: Box::new(|tray: &mut Self| tray.launch("history")),
                ..Default::default()
            }
            .into(),
            #[cfg(feature = "update-check")]
            StandardItem {
                label: crate::i18n::dbus_menu_label("TrayIcon", "Check for updates"),
                activate: Box::new(|tray: &mut Self| tray.launch("updates")),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: crate::i18n::dbus_menu_label("TrayIcon", "&Quit"),
                activate: Box::new(|tray: &mut Self| tray.quit.notify_one()),
                ..Default::default()
            }
            .into(),
        ]
    }
}
