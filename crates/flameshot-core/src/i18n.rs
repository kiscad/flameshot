// SPDX-License-Identifier: GPL-3.0-or-later
struct Catalog {
    language: &'static str,
    messages: &'static [(&'static str, &'static str, &'static str)],
}
include!(concat!(env!("OUT_DIR"), "/catalogs.rs"));
pub fn languages() -> impl Iterator<Item = &'static str> {
    CATALOGS.iter().map(|c| c.language)
}
pub struct Translator {
    catalog: Option<&'static Catalog>,
}
impl Translator {
    pub fn new(locale: &str) -> Self {
        let locale = locale
            .split(['.', '@'])
            .next()
            .unwrap_or(locale)
            .replace('-', "_")
            .to_ascii_lowercase();
        let language = locale.split('_').next().unwrap_or(&locale);
        let catalog = CATALOGS
            .iter()
            .find(|c| c.language.eq_ignore_ascii_case(&locale))
            .or_else(|| CATALOGS.iter().find(|c| c.language == language))
            .or_else(|| {
                CATALOGS
                    .iter()
                    .find(|c| c.language.split('_').next() == Some(language))
            });
        Self { catalog }
    }
    pub fn translate<'a>(&self, context: &str, source: &'a str) -> &'a str {
        self.catalog
            .and_then(|c| {
                c.messages
                    .binary_search_by(|(ctx, key, _)| (*ctx, *key).cmp(&(context, source)))
                    .ok()
                    .map(|i| c.messages[i].2)
            })
            .unwrap_or(source)
    }
}
/// Convert Qt ampersand mnemonics to the underscore notation used by D-Bus menus.
pub fn dbus_menu_label(value: &str) -> String {
    let mut output = String::new();
    let mut chars = value.chars().peekable();
    while let Some(character) = chars.next() {
        match character {
            '&' if chars.peek() == Some(&'&') => {
                output.push('&');
                chars.next();
            }
            '&' => output.push('_'),
            '_' => output.push_str("__"),
            _ => output.push(character),
        }
    }
    output
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn menu_labels_preserve_translation_and_literal_ampersands() {
        let chinese = Translator::new("zh_CN");
        assert_eq!(
            dbus_menu_label(chinese.translate("TrayIcon", "&Take Screenshot")),
            "进行截图(_T)"
        );
        assert_eq!(
            dbus_menu_label(chinese.translate("TrayIcon", "&Quit")),
            "退出(_Q)"
        );
        assert_eq!(
            dbus_menu_label(Translator::new("unknown").translate("TrayIcon", "&Quit")),
            "_Quit"
        );
        assert_eq!(dbus_menu_label("Rock && Roll (&R)"), "Rock & Roll (_R)");
        assert_eq!(dbus_menu_label("A (&&) B"), "A (&) B");
        assert_eq!(dbus_menu_label("&Édition"), "_Édition");
        assert_eq!(dbus_menu_label("file_name"), "file__name");
    }
    #[test]
    fn catalogs_are_built_without_qt_and_contexts_remain_distinct() {
        assert_eq!(languages().count(), 49);
        let tr = Translator::new("zh-CN.UTF-8");
        assert_eq!(tr.translate("CopyTool", "Copy"), "复制");
        assert_eq!(tr.translate("ConfigWindow", "Configuration"), "配置");
        assert_eq!(tr.translate("VisualsEditor", "Configuration"), "设置");
        assert_eq!(tr.translate("Missing", "Copy"), "Copy");
    }
    #[test]
    fn launcher_and_update_catalogs_keep_placeholders_and_source_fallback() {
        for locale in ["zh_CN", "zh_TW", "zh_HK"] {
            let tr = Translator::new(locale);
            for (context, source) in [
                ("CaptureLauncher", "Capture in %1…"),
                ("CaptureLauncher", "Capture process exited with %1"),
                ("RustUpdates", "Version %1 is available"),
                ("RustUpdates", "Update check failed: %1"),
            ] {
                let text = tr.translate(context, source);
                assert_ne!(text, source, "{locale}: {source}");
                assert_eq!(text.matches("%1").count(), 1);
            }
        }
        assert_eq!(
            Translator::new("zh_CN").translate("CaptureLauncher", "Flameshot Launcher"),
            "Flameshot 启动器"
        );
        assert_eq!(
            Translator::new("zh_TW").translate("CaptureLauncher", "Flameshot Launcher"),
            "Flameshot 啟動器"
        );
        assert_eq!(
            Translator::new("unknown").translate("RustUpdates", "Open release page"),
            "Open release page"
        );
    }
    #[test]
    fn chinese_configuration_catalogs_cover_labels_and_error_placeholder() {
        for locale in ["zh_CN", "zh_TW", "zh_HK"] {
            let catalog = CATALOGS.iter().find(|c| c.language == locale).unwrap();
            let messages: Vec<_> = catalog
                .messages
                .iter()
                .filter(|(context, _, _)| *context == "RustConfig")
                .collect();
            assert_eq!(messages.len(), 79, "{locale}");
            for (_, source, translated) in messages {
                assert_ne!(source, translated, "{locale}: {source}");
                assert_eq!(
                    source.matches("%1").count(),
                    translated.matches("%1").count()
                );
            }
        }
        assert_eq!(
            Translator::new("unknown").translate("RustConfig", "Save settings"),
            "Save settings"
        );
    }
    #[test]
    fn unknown_locale_and_unfinished_entries_fall_back_to_source() {
        assert_eq!(
            Translator::new("unknown").translate("CopyTool", "Copy"),
            "Copy"
        );
        assert_eq!(Translator::new("en").translate("CopyTool", "Copy"), "Copy");
        assert_eq!(
            Translator::new("zh_ZZ").translate("CopyTool", "Copy"),
            "复制"
        );
        assert_eq!(
            Translator::new("ZH-zz.UTF-8").translate("CopyTool", "Copy"),
            "复制"
        );
    }
}
