// SPDX-License-Identifier: GPL-3.0-or-later
use eframe::egui;
use std::sync::{Arc, OnceLock};
pub fn flatpak() -> bool {
    std::env::var_os("FLATPAK_ID").is_some() || std::path::Path::new("/.flatpak-info").exists()
}
fn application_id(wayland: bool, sandboxed: bool) -> &'static str {
    if wayland || sandboxed {
        "org.flameshot.Flameshot"
    } else {
        "flameshot"
    }
}
pub fn viewport() -> egui::ViewportBuilder {
    static ICON: OnceLock<Arc<egui::IconData>> = OnceLock::new();
    let icon = ICON.get_or_init(|| {
        let image = image::load_from_memory(include_bytes!(
            "../../../data/img/app/org.flameshot.Flameshot.png"
        ))
        .expect("embedded app icon")
        .to_rgba8();
        Arc::new(egui::IconData {
            width: image.width(),
            height: image.height(),
            rgba: image.into_raw(),
        })
    });
    egui::ViewportBuilder::default()
        .with_app_id(application_id(crate::is_wayland(), flatpak()))
        .with_icon(icon.clone())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn identity_preserves_x11_class_and_matches_wayland_desktop_id() {
        assert_eq!(application_id(false, false), "flameshot");
        assert_eq!(application_id(true, false), "org.flameshot.Flameshot");
        assert_eq!(application_id(false, true), "org.flameshot.Flameshot");
        let viewport = viewport();
        let icon = viewport.icon.unwrap();
        assert_eq!(icon.rgba.len(), (icon.width * icon.height * 4) as usize);
    }
}
