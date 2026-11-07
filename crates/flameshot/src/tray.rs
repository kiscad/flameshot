// SPDX-License-Identifier: GPL-3.0-or-later
use std::sync::Arc;
pub struct Tray {
    pub quit: Arc<tokio::sync::Notify>,
    pub activity: Arc<crate::daemon::Activity>,
    screens: Vec<flameshot_platform::screens::Screen>,
}
impl Tray {
    pub fn new(quit: Arc<tokio::sync::Notify>, activity: Arc<crate::daemon::Activity>) -> Self {
        Self {
            quit,
            activity,
            screens: flameshot_platform::screens::list(crate::is_wayland()).unwrap_or_default(),
        }
    }
    fn screen_menu(&self) -> Option<ksni::MenuItem<Self>> {
        if self.screens.len() <= 1 {
            return None;
        }
        Some(
            ksni::menu::SubMenu {
                label: crate::i18n::dbus_menu_label("TrayIcon", "Select Screen"),
                submenu: self
                    .screens
                    .iter()
                    .enumerate()
                    .map(|(index, screen)| {
                        ksni::menu::StandardItem {
                            label: crate::i18n::dbus_menu_label(
                                "TrayIcon",
                                "Monitor %1: %2 (%3x%4)",
                            )
                            .replace("%1", &(index + 1).to_string())
                            .replace("%3", &screen.bounds.width.to_string())
                            .replace("%4", &screen.bounds.height.to_string())
                            .replace("%2", &screen.name.replace('_', "__")),
                            activate: Box::new(move |tray: &mut Self| {
                                // Match the baseline's menu-hide and capture delay.
                                let _ = tray.activity.launch_args(&[
                                    "gui",
                                    "--region",
                                    &format!("screen{index}"),
                                    "--delay",
                                    "500",
                                ]);
                            }),
                            ..Default::default()
                        }
                        .into()
                    })
                    .collect(),
                ..Default::default()
            }
            .into(),
        )
    }

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
    fn menu_about_to_show(&mut self) {
        self.screens = flameshot_platform::screens::list(crate::is_wayland()).unwrap_or_default();
    }
    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        use ksni::menu::StandardItem;
        let mut menu = vec![
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
            StandardItem {
                label: crate::i18n::dbus_menu_label("TrayIcon", "&Open Save Path"),
                activate: Box::new(|tray: &mut Self| tray.launch("internal-open-save-path")),
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
        ];
        if let Some(screens) = self.screen_menu() {
            menu.insert(2, screens);
        }
        menu
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn screen_menu_requires_multiple_outputs_and_preserves_literal_names() {
        let mut tray = Tray {
            quit: Arc::default(),
            activity: Arc::default(),
            screens: Vec::new(),
        };
        assert!(tray.screen_menu().is_none());
        let screen = flameshot_platform::screens::Screen {
            name: "DP_1 & %3".into(),
            bounds: flameshot_core::geometry::DesktopRect {
                x: -640.0,
                y: 0.0,
                width: 640.0,
                height: 480.0,
            },
        };
        tray.screens.push(screen.clone());
        assert!(tray.screen_menu().is_none());
        tray.screens.push(screen);
        let Some(ksni::MenuItem::SubMenu(menu)) = tray.screen_menu() else {
            panic!("Missing submenu");
        };
        assert_eq!(menu.submenu.len(), 2);
        let ksni::MenuItem::Standard(item) = &menu.submenu[1] else {
            panic!("Missing monitor action");
        };
        assert!(item.label.contains("DP__1 & %3"), "{}", item.label);
        assert!(item.label.contains("640x480"), "{}", item.label);
    }
}
