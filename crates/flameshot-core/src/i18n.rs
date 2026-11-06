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
            .replace('-', "_");
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
#[cfg(test)]
mod tests {
    use super::*;
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
    }
}
