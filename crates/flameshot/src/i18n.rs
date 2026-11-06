// SPDX-License-Identifier: GPL-3.0-or-later
use flameshot_core::i18n::Translator;
use std::sync::OnceLock;
pub fn tr<'a>(context: &str, source: &'a str) -> &'a str {
    static TRANSLATOR: OnceLock<Translator> = OnceLock::new();
    TRANSLATOR
        .get_or_init(|| {
            let language = crate::settings::Settings::load()
                .map(|s| s.language)
                .unwrap_or_else(|_| "auto".into());
            let locale = if language == "auto" {
                ["LC_ALL", "LC_MESSAGES", "LANG"]
                    .iter()
                    .find_map(|key| std::env::var(key).ok().filter(|v| !v.is_empty()))
                    .unwrap_or_else(|| "en".into())
            } else {
                language
            };
            Translator::new(&locale)
        })
        .translate(context, source)
}

/// Supplement bundled Latin fonts using installed script-specific fonts.
pub fn install_fonts(ctx: &eframe::egui::Context) {
    use eframe::egui::{FontData, FontDefinitions, FontFamily};
    let mut database = fontdb::Database::new();
    database.load_system_fonts();
    let mut fonts = FontDefinitions::default();
    for family in [
        "Noto Sans CJK SC",
        "WenQuanYi Zen Hei",
        "Noto Sans Arabic",
        "Noto Sans Hebrew",
        "Noto Sans Devanagari",
        "Noto Sans Thai",
        "Noto Sans Tamil",
    ] {
        if let Some(id) = database.query(&fontdb::Query {
            families: &[fontdb::Family::Name(family)],
            ..Default::default()
        }) && let Some(data) = database.with_face_data(id, |bytes, index| {
            let mut data = FontData::from_owned(bytes.to_vec());
            data.index = index;
            data
        }) {
            fonts
                .font_data
                .insert(family.into(), std::sync::Arc::new(data));
            fonts
                .families
                .entry(FontFamily::Proportional)
                .or_default()
                .push(family.into());
        }
    }
    ctx.set_fonts(fonts);
}
