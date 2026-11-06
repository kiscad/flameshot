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

pub fn check(source: &str) -> Vec<String> {
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
            errors.push(format!("Malformed configuration at line {}", index + 1));
        }
    }
    let mut shortcuts = BTreeMap::new();
    for ((section, key), ()) in keys {
        if section == "Shortcuts" {
            if !crate::shortcuts::DEFAULTS
                .iter()
                .any(|(name, _)| *name == key)
            {
                errors.push(format!("Unrecognized shortcut name: '{key}'"));
            } else {
                match ini.string(section, key) {
                    Ok(Some(value)) => {
                        shortcuts.insert(key.to_owned(), value);
                    }
                    Err(error) => errors.push(format!("Bad value in 'Shortcuts/{key}': {error}")),
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
            Some("integer >= 1")
        } else if contains(NONNEGATIVE, key) {
            Some("integer >= 0")
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
            errors.push(format!("Unrecognized setting: '{key}'"));
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
            errors.push(format!("Bad value in '{key}'. Expected: {expected}"));
        }
    }
    if let Err(error) = crate::shortcuts::validate_overrides(&shortcuts) {
        errors.push(error);
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
