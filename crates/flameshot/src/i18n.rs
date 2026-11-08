// SPDX-License-Identifier: GPL-3.0-or-later
use flameshot_core::i18n::Translator;
use std::sync::OnceLock;
fn configured_language(source: &str) -> String {
    flameshot_core::config::IniDocument::parse(source.to_owned())
        .string("General", "uiLanguage")
        .ok()
        .flatten()
        .unwrap_or_else(|| "auto".into())
}
struct Localization {
    language: String,
    translator: Translator,
}
fn localization() -> &'static Localization {
    static LOCALIZATION: OnceLock<Localization> = OnceLock::new();
    LOCALIZATION.get_or_init(|| {
        // Widget-frame tests use source labels; they must not read the developer's preferences.
        // Production locale selection is covered by the parser and native locale fixtures.
        #[cfg(test)]
        let language = "en".to_owned();
        #[cfg(not(test))]
        let language = {
            // An unrelated malformed setting must not change the language of its repair UI.
            let source = dirs::config_dir()
                .and_then(|directory| {
                    std::fs::read_to_string(directory.join("flameshot/flameshot.ini")).ok()
                })
                .unwrap_or_default();
            configured_language(&source)
        };
        let locale = if language == "auto" {
            ["LC_ALL", "LC_MESSAGES", "LANG"]
                .iter()
                .find_map(|key| std::env::var(key).ok().filter(|value| !value.is_empty()))
                .unwrap_or_else(|| "en".into())
        } else {
            language.clone()
        };
        Localization {
            language,
            translator: Translator::new(&locale),
        }
    })
}
pub(crate) fn session_language() -> &'static str {
    &localization().language
}
pub fn tr<'a>(context: &str, source: &'a str) -> &'a str {
    localization().translator.translate(context, source)
}

pub fn dbus_menu_label(context: &str, source: &str) -> String {
    flameshot_core::i18n::dbus_menu_label(tr(context, source))
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn invalid_preferences_do_not_replace_a_valid_configured_language() {
        for locale in ["zh_CN", "zh_TW"] {
            let source = format!("[General]\ndrawThickness=bad\nuiLanguage=\"{locale}\"\n");
            assert!(!crate::config_check::check(&source).is_empty());
            let language = configured_language(&source);
            assert_eq!(language, locale);
            assert_ne!(
                Translator::new(&language)
                    .translate("ConfigResolver", "Resolve configuration errors"),
                "Resolve configuration errors"
            );
        }
        assert_eq!(
            configured_language("[General]\nuiLanguage=@Variant(opaque)\n"),
            "auto"
        );
        assert_eq!(
            configured_language("[General]\ndrawThickness=bad\n"),
            "auto"
        );
        assert_eq!(
            configured_language("[General]\nuiLanguage=en\n[General]\nuiLanguage=zh_TW\n"),
            "zh_TW"
        );
    }
}
