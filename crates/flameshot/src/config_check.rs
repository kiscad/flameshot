// SPDX-License-Identifier: GPL-3.0-or-later
//! Read-only validation of the Linux INI schema, including settings not yet exposed in the UI.
use flameshot_core::config::IniDocument;
use std::collections::BTreeMap;

const BOOLEANS: &str = "showHelp showSidePanelButton showDesktopNotification showAbortNotification disabledTrayIcon historyConfirmationToDelete checkForUpdates allowMultipleGuiInstances showMagnifier squareMagnifier autoCloseIdleDaemon startupLaunch showStartupLaunchMessage showQuitPrompt copyURLAfterUpload copyPathAfterSave antialiasingPinZoom useJpgForClipboard uploadWithoutConfirmation saveAfterCopy savePathFixed saveLastRegion drawCircleCounterOutline keepOpenAppLauncher predefinedColorPaletteLarge copyOnDoubleClick reverseArrow insecurePixelate captureActiveMonitor useX11LegacyScreenshot";
const STRINGS: &str = "uiLanguage ignoreUpdateToVersion fontFamily uploadClientSecret";
const POSITIVE: &str = "drawThickness drawFontSize drawCircleCounterSize drawPixelateSize drawRectangleSize drawMarkerSize";
const NONNEGATIVE: &str = "uploadHistoryMax showSelectionGeometryHideTime";
fn contains(words: &str, value: &str) -> bool {
    words.split_whitespace().any(|word| word == value)
}

#[derive(Clone, Debug)]
pub(crate) struct Repair {
    pub section: String,
    pub key: String,
    pub reset: bool,
}
#[derive(Debug)]
pub(crate) struct Problem {
    pub message: String,
    pub repair: Option<Repair>,
    presentation: Option<(&'static str, &'static str, Vec<String>)>,
}
impl Problem {
    fn manual(message: String) -> Self {
        Self {
            message,
            repair: None,
            presentation: None,
        }
    }
    fn setting(message: String, section: &str, key: &str, reset: bool) -> Self {
        let problem = Self {
            message,
            repair: (!key.is_empty()).then(|| Repair {
                section: section.into(),
                key: key.into(),
                reset,
            }),
            presentation: None,
        };
        if reset {
            problem
        } else if section == "Shortcuts" {
            problem.localized(
                "ConfigHandler",
                "Unrecognized shortcut name: '%1'.\n",
                vec![key.into()],
            )
        } else {
            problem.localized(
                "ConfigHandler",
                "Unrecognized setting: '%1'\n",
                vec![key.into()],
            )
        }
    }
    fn localized(
        mut self,
        context: &'static str,
        source: &'static str,
        arguments: Vec<String>,
    ) -> Self {
        self.presentation = Some((context, source, arguments));
        self
    }
    pub(crate) fn display(&self) -> String {
        self.display_with(|context, source| crate::i18n::tr(context, source).to_owned())
    }
    fn display_with(&self, translate: impl Fn(&str, &str) -> String) -> String {
        let Some((context, source, arguments)) = &self.presentation else {
            return self.message.clone();
        };
        let template = translate(context, source);
        substitute(template.strip_suffix('\n').unwrap_or(&template), arguments)
    }
}
// Replace placeholders in one pass; percent signs in setting names/details are data.
fn substitute(template: &str, arguments: &[String]) -> String {
    let mut output = String::new();
    let mut rest = template;
    while let Some(position) = rest.find('%') {
        output.push_str(&rest[..position]);
        rest = &rest[position + 1..];
        let length = rest.bytes().take_while(u8::is_ascii_digit).count();
        if let Some(value) = rest[..length]
            .parse::<usize>()
            .ok()
            .and_then(|index| index.checked_sub(1))
            .and_then(|index| arguments.get(index))
        {
            output.push_str(value);
            rest = &rest[length..];
        } else {
            output.push('%');
        }
    }
    output.push_str(rest);
    output
}
pub fn check(source: &str) -> Vec<String> {
    diagnose(source)
        .into_iter()
        .map(|problem| problem.message)
        .collect()
}
pub(crate) fn diagnose(source: &str) -> Vec<Problem> {
    let ini = IniDocument::parse(source.into());
    let mut keys = BTreeMap::new();
    let mut section = "General";
    let mut errors = Vec::new();
    for (index, line) in source.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with([';', '#']) {
            continue;
        }
        if let Some(name) = line.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
            section = name;
        } else if let Some((key, _)) = line.split_once('=') {
            keys.insert((section, key.trim()), ());
        } else {
            errors.push(
                Problem::manual(format!("Malformed configuration at line {}", index + 1))
                    .localized(
                        "RustConfig",
                        "Malformed configuration at line %1",
                        vec![(index + 1).to_string()],
                    ),
            );
        }
    }
    let mut shortcuts = BTreeMap::new();
    for ((section, key), ()) in keys {
        if section == "Shortcuts" {
            if !crate::shortcuts::DEFAULTS
                .iter()
                .any(|(name, _)| *name == key)
            {
                errors.push(Problem::setting(
                    format!("Unrecognized shortcut name: '{key}'"),
                    section,
                    key,
                    false,
                ));
            } else {
                match ini.string(section, key) {
                    Ok(Some(value)) => {
                        if let Err(error) = crate::shortcuts::validate(&value) {
                            errors.push(
                                Problem::setting(
                                    format!("Bad value in 'Shortcuts/{key}': {error}"),
                                    section,
                                    key,
                                    true,
                                )
                                .localized(
                                    "RustConfig",
                                    "Invalid shortcut %1: %2",
                                    vec![key.into(), error],
                                ),
                            );
                        } else {
                            shortcuts.insert(key.to_owned(), value);
                        }
                    }
                    Err(error) => errors.push(
                        Problem::setting(
                            format!("Bad value in 'Shortcuts/{key}': {error}"),
                            section,
                            key,
                            true,
                        )
                        .localized(
                            "RustConfig",
                            "Invalid shortcut %1: %2",
                            vec![key.into(), error.to_string()],
                        ),
                    ),
                    _ => {}
                }
            }
            continue;
        }
        // QSettings permits application-specific groups; Qt checks only General/Shortcuts.
        if section != "General" {
            continue;
        }
        let expected = if contains(BOOLEANS, key) {
            Some("true or false")
        } else if contains(STRINGS, key) {
            Some("string")
        } else if contains(POSITIVE, key) {
            Some("integer between 1 and 2147483647")
        } else if contains(NONNEGATIVE, key) {
            Some("integer between 0 and 2147483647")
        } else {
            match key {
                "undoLimit" => Some("integer between 0 and 999"),
                "contrastOpacity" => Some("integer between 0 and 255"),
                "jpegQuality" => Some("integer between 0 and 100"),
                "showSelectionGeometry" => Some("integer between 0 and 5"),
                "arrowStyle" => Some("integer between 0 and 1"),
                "uiColor" | "contrastUiColor" | "drawColor" => {
                    Some("color name, #RRGGBB or #RRGGBBAA")
                }
                "savePath" => Some("existing directory"),
                "filenamePattern" => Some(
                    "nonempty filename pattern with valid date directives and no path separators",
                ),
                "saveAsFileExtension" => Some("supported image extension"),
                "buttons" => Some("Qt integer list of tool IDs"),
                "userColors" => Some("3 to 17 colors or picker"),
                _ => None,
            }
        };
        let Some(expected) = expected else {
            errors.push(Problem::setting(
                format!("Unrecognized setting: '{key}'"),
                section,
                key,
                false,
            ));
            continue;
        };
        let valid = if key == "buttons" {
            ini.integer_list(section, key)
                .ok()
                .flatten()
                .is_some_and(|values| values.iter().all(|v| crate::buttons::valid(*v)))
        } else if key == "userColors" {
            ini.string_list(section, key)
                .ok()
                .flatten()
                .is_some_and(|values| {
                    (3..=17).contains(&values.len())
                        && values.iter().all(|v| v == "picker" || valid_color(v, true))
                })
        } else {
            ini.string(section, key)
                .ok()
                .flatten()
                .is_some_and(|value| {
                    if contains(BOOLEANS, key) {
                        return matches!(value.as_str(), "true" | "false");
                    }
                    if contains(STRINGS, key) {
                        return true;
                    }
                    if contains(POSITIVE, key) {
                        return value.parse::<i32>().is_ok_and(|v| v >= 1);
                    }
                    if contains(NONNEGATIVE, key) {
                        return value.parse::<i32>().is_ok_and(|v| v >= 0);
                    }
                    match key {
                        "undoLimit"
                        | "contrastOpacity"
                        | "jpegQuality"
                        | "showSelectionGeometry"
                        | "arrowStyle" => {
                            let max = match key {
                                "undoLimit" => 999,
                                "contrastOpacity" => 255,
                                "jpegQuality" => 100,
                                "showSelectionGeometry" => 5,
                                _ => 1,
                            };
                            value.parse::<i32>().is_ok_and(|v| (0..=max).contains(&v))
                        }
                        "uiColor" | "contrastUiColor" | "drawColor" => valid_color(&value, false),
                        "savePath" => std::path::Path::new(&value).is_dir(),
                        "filenamePattern" => {
                            !value.is_empty()
                                && !value.contains(['/', '\\'])
                                && !chrono::format::StrftimeItems::new(&value)
                                    .any(|item| matches!(item, chrono::format::Item::Error))
                        }
                        "saveAsFileExtension" => crate::output::EXTENSIONS
                            .contains(&value.trim_start_matches('.').to_ascii_lowercase().as_str()),
                        _ => false,
                    }
                })
        };
        if !valid {
            errors.push(
                Problem::setting(
                    format!("Bad value in '{key}'. Expected: {expected}"),
                    section,
                    key,
                    true,
                )
                .localized(
                    "ConfigHandler",
                    "Bad value in '%1'. Expected: %2\n",
                    vec![key.into(), expected.into()],
                ),
            );
        }
    }
    if let Err(error) = crate::shortcuts::validate_overrides(&shortcuts) {
        errors.push(Problem::manual(error.clone()).localized(
            "RustConfig",
            "Shortcut conflict: %1",
            vec![error],
        ));
    }
    errors
}

fn valid_color(value: &str, palette: bool) -> bool {
    if palette {
        flameshot_core::color::parse_palette(value).is_some()
    } else {
        flameshot_core::color::parse(value).is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn repaired_greek_and_tamil_templates_keep_setting_names() {
        let problems =
            diagnose("[General]\ncustom%2=value\ndrawThickness=bad\n[Shortcuts]\nUNKNOWN=F8\n");
        for locale in ["el", "ta"] {
            let translator = flameshot_core::i18n::Translator::new(locale);
            let messages: Vec<_> = problems
                .iter()
                .map(|problem| {
                    problem.display_with(|context, source| {
                        translator.translate(context, source).to_owned()
                    })
                })
                .collect();
            assert!(
                messages.iter().all(|message| !message.is_ascii()),
                "{locale}: {messages:?}"
            );
            let text = messages.join("\n");
            for value in ["custom%2", "drawThickness", "2147483647", "UNKNOWN"] {
                assert!(text.contains(value), "{locale}: {text}");
            }
        }
    }
    #[test]
    fn diagnostics_translate_explanations_and_preserve_literal_details() {
        let source = "[General]\ndrawThickness=bad\ncustom%2=value\n[Shortcuts]\nUNKNOWN=F8\nTYPE_COPY=Ctrl+NoSuchKey\nTYPE_SAVE=F9\nTYPE_EXIT=F9\nmalformed line\n";
        let problems = diagnose(source);
        assert_eq!(problems.len(), 6);
        for locale in ["zh_CN", "zh_TW", "zh_HK"] {
            let translator = flameshot_core::i18n::Translator::new(locale);
            let messages: Vec<_> = problems
                .iter()
                .map(|problem| {
                    problem.display_with(|context, source| {
                        translator.translate(context, source).to_owned()
                    })
                })
                .collect();
            assert!(
                messages.iter().all(|message| !message.is_ascii()),
                "{locale}: {messages:?}"
            );
            let joined = messages.join("\n");
            for detail in [
                "drawThickness",
                "2147483647",
                "custom%2",
                "UNKNOWN",
                "TYPE_COPY",
                "NoSuchKey",
                "TYPE_EXIT",
                "F9",
                "9",
            ] {
                assert!(
                    joined.contains(detail),
                    "{locale}: missing {detail}: {joined}"
                );
            }
        }
        assert!(
            check(source)
                .iter()
                .any(|message| message.starts_with("Bad value in 'drawThickness'"))
        );
        assert_eq!(
            substitute(
                "%1 / %2 / %10 / %0 / %",
                &["设置%2".into(), "detail%1".into()]
            ),
            "设置%2 / detail%1 / %10 / %0 / %"
        );
    }
    #[test]
    fn repair_actions_identify_only_invalid_or_unrecognized_keys() {
        let source = "[General]\nshowHelp=bad\nunknown=x\n[Shortcuts]\nTYPE_COPY=Ctrl+NoSuchKey\nUNKNOWN=F1\nTYPE_SAVE=F9\nTYPE_EXIT=F9\n[External]\nunknown=keep\nmalformed line\n";
        let problems = diagnose(source);
        let actions: Vec<_> = problems
            .iter()
            .filter_map(|p| p.repair.as_ref())
            .map(|r| (r.section.as_str(), r.key.as_str(), r.reset))
            .collect();
        assert_eq!(
            actions,
            vec![
                ("General", "showHelp", true),
                ("General", "unknown", false),
                ("Shortcuts", "TYPE_COPY", true),
                ("Shortcuts", "UNKNOWN", false)
            ]
        );
        assert!(
            problems
                .iter()
                .any(|p| p.repair.is_none() && p.message.contains("conflicts"))
        );
        assert!(
            problems
                .iter()
                .any(|p| p.repair.is_none() && p.message.contains("Malformed"))
        );
    }
    #[test]
    fn validates_the_legacy_schema_and_collects_all_errors() {
        assert!(check("[General]\narrowStyle=1\nreverseArrow=true\nuserColors=red, \"#abcdef\", picker\nuiColor=#11223344\nundoLimit=0\n[OtherApplication]\nprivate=value\n").is_empty());
        let errors = check(
            "[General]\narrowStyle=2\nreverseArrow=perhaps\nunknown=1\nuserColors=red,blue\nuiColor=#123\nundoLimit=1000\n[Shortcuts]\nUNKNOWN=F8\nTYPE_COPY=F9\nTYPE_SAVE=F9\n",
        );
        for key in [
            "arrowStyle",
            "reverseArrow",
            "unknown",
            "userColors",
            "uiColor",
            "undoLimit",
            "UNKNOWN",
            "conflicts",
        ] {
            assert!(errors.iter().any(|e| e.contains(key)), "{key}: {errors:?}");
        }
    }
    #[test]
    fn validates_serialized_buttons_and_path() {
        let mut ini = IniDocument::parse("[General]\n".into());
        ini.set_integer_list("General", "buttons", &[0, 2, 23])
            .unwrap();
        assert!(check(ini.source()).is_empty());
        ini.set_integer_list("General", "buttons", &[7, 24])
            .unwrap();
        assert!(check(ini.source()).iter().any(|e| e.contains("buttons")));
        let directory = tempfile::tempdir().unwrap();
        ini.set_integer_list("General", "buttons", &[]).unwrap();
        ini.set_string("General", "savePath", directory.path().to_str().unwrap())
            .unwrap();
        assert!(check(ini.source()).is_empty());
        drop(directory);
        assert!(check(ini.source()).iter().any(|e| e.contains("savePath")));
    }
}
