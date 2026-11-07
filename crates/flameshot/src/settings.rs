// SPDX-License-Identifier: GPL-3.0-or-later
use anyhow::{Context, Result, ensure};
use flameshot_core::config::IniDocument;
use std::path::PathBuf;
fn integer_setting(value: &str, key: &str, minimum: i32, maximum: i32) -> Result<i32> {
    let expected = || format!("{key} must be an integer between {minimum} and {maximum}");
    let parsed: i32 = value.parse().with_context(expected)?;
    ensure!((minimum..=maximum).contains(&parsed), "{}", expected());
    Ok(parsed)
}
#[derive(Debug, Clone)]
pub struct Settings {
    pub auto_close_idle: bool,
    pub jpeg_clipboard: bool,
    pub show_quit_prompt: bool,
    pub abort_notifications: bool,
    pub startup_message: bool,
    pub insecure_pixelate: bool,
    pub show_side_panel_button: bool,
    pub selection_geometry: u8,
    pub selection_geometry_timeout: u64,
    pub capture_active_monitor: bool,
    pub use_x11_legacy_screenshot: bool,
    pub palette: Vec<Option<[u8; 4]>>,
    pub large_palette: bool,
    pub ui_color: [u8; 4],
    pub contrast_ui_color: [u8; 4],
    pub copy_on_double_click: bool,
    pub use_last_region: bool,
    pub contrast_opacity: u8,
    pub draw_color: [u8; 4],
    pub font_family: String,
    pub save_after_copy: bool,
    pub copy_path_after_save: bool,
    pub undo_limit: usize,
    pub marker_size: f32,
    pub pixelate_size: f32,
    pub counter_size: f32,
    pub arrow_style: u8,
    pub reverse_arrow: bool,
    pub rectangle_radius: f32,
    pub allow_multiple_gui: bool,
    pub antialias_pin_zoom: bool,
    pub keep_open_app_launcher: bool,
    pub shortcuts: std::collections::BTreeMap<String, String>,
    pub buttons: Option<Vec<i32>>,
    #[cfg(feature = "update-check")]
    pub check_updates: bool,
    #[cfg(feature = "update-check")]
    pub ignored_update: String,
    #[cfg(feature = "imgur")]
    pub upload_client_id: String,
    #[cfg(feature = "imgur")]
    pub upload_without_confirmation: bool,
    #[cfg(feature = "imgur")]
    pub copy_url_after_upload: bool,
    #[cfg(feature = "imgur")]
    pub history_confirmation_to_delete: bool,
    #[cfg(feature = "imgur")]
    pub upload_history_max: usize,
    pub startup_launch: bool,
    pub language: String,
    pub save_path: Option<PathBuf>,
    pub save_path_fixed: bool,
    pub filename_pattern: String,
    pub extension: String,
    pub jpeg_quality: u8,
    pub disabled_tray: bool,
    pub notifications: bool,
    pub show_help: bool,
    pub magnifier: bool,
    pub square_magnifier: bool,
    pub counter_outline: bool,
    pub draw_thickness: f32,
    pub font_size: f32,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            auto_close_idle: false,
            jpeg_clipboard: false,
            show_quit_prompt: false,
            abort_notifications: true,
            startup_message: true,
            insecure_pixelate: false,
            show_side_panel_button: true,
            selection_geometry: 4,
            selection_geometry_timeout: 3000,
            capture_active_monitor: false,
            use_x11_legacy_screenshot: false,
            palette: default_palette(false),
            large_palette: false,
            ui_color: [116, 0, 150, 255],
            contrast_ui_color: [39, 0, 50, 255],
            copy_on_double_click: false,
            use_last_region: false,
            contrast_opacity: 190,
            draw_color: [255, 0, 0, 255],
            font_family: String::new(),
            save_after_copy: false,
            copy_path_after_save: false,
            undo_limit: 100,
            marker_size: 5.0,
            pixelate_size: 2.0,
            counter_size: 1.0,
            arrow_style: 0,
            reverse_arrow: false,
            rectangle_radius: 1.0,
            allow_multiple_gui: false,
            antialias_pin_zoom: true,
            keep_open_app_launcher: false,
            shortcuts: Default::default(),
            buttons: None,
            #[cfg(feature = "update-check")]
            check_updates: true,
            #[cfg(feature = "update-check")]
            ignored_update: String::new(),
            #[cfg(feature = "imgur")]
            upload_client_id: "313baf0c7b4d3ff".into(),
            #[cfg(feature = "imgur")]
            upload_without_confirmation: false,
            #[cfg(feature = "imgur")]
            copy_url_after_upload: true,
            #[cfg(feature = "imgur")]
            history_confirmation_to_delete: true,
            #[cfg(feature = "imgur")]
            upload_history_max: 25,
            startup_launch: false,
            language: "auto".into(),
            save_path: None,
            save_path_fixed: false,
            filename_pattern: "%F_%H-%M".into(),
            extension: "png".into(),
            jpeg_quality: 75,
            disabled_tray: false,
            notifications: true,
            show_help: true,
            magnifier: false,
            square_magnifier: false,
            counter_outline: true,
            draw_thickness: 3.0,
            font_size: 8.0,
        }
    }
}
impl Settings {
    pub fn save_directory(&self) -> PathBuf {
        self.save_path.clone().unwrap_or_else(|| {
            dirs::picture_dir()
                .filter(|path| path.is_dir())
                .or_else(|| dirs::home_dir().filter(|path| path.is_dir()))
                .unwrap_or_else(std::env::temp_dir)
        })
    }
    pub fn load() -> Result<Self> {
        let Some(directory) = dirs::config_dir() else {
            return Ok(Self::default());
        };
        let path = directory.join("flameshot/flameshot.ini");
        let source = match std::fs::read_to_string(&path) {
            Ok(source) => source,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
            Err(e) => return Err(e).context("Cannot read Flameshot configuration"),
        };
        let mut settings = Self::parse(&source)?;
        settings.startup_launch = flameshot_platform::desktop::autostart_enabled(&directory);
        Ok(settings)
    }
    pub fn set_startup(enabled: bool) -> Result<()> {
        let directory = dirs::config_dir().context("Cannot locate startup directory")?;
        let previous = flameshot_platform::desktop::autostart_enabled(&directory);
        flameshot_platform::desktop::set_autostart(&directory, enabled)?;
        if let Err(error) = Self::update(&[("startupLaunch", enabled.to_string())]) {
            let _ = flameshot_platform::desktop::set_autostart(&directory, previous);
            return Err(error);
        }
        Ok(())
    }
    pub fn update(values: &[(&str, String)]) -> Result<()> {
        let path = dirs::config_dir()
            .context("Cannot locate configuration directory")?
            .join("flameshot/flameshot.ini");
        Self::update_at(&path, values)
    }
    pub fn update_form(
        values: &[(&str, String)],
        shortcuts: &std::collections::BTreeMap<String, String>,
    ) -> Result<()> {
        crate::shortcuts::validate_overrides(shortcuts).map_err(anyhow::Error::msg)?;
        let path = dirs::config_dir()
            .context("Cannot locate configuration directory")?
            .join("flameshot/flameshot.ini");
        let shortcuts: Vec<_> = shortcuts
            .iter()
            .map(|(key, value)| (key.as_str(), value.clone()))
            .collect();
        Self::update_sections_at(&path, &[("General", values), ("Shortcuts", &shortcuts)])
    }
    fn update_at(path: &std::path::Path, values: &[(&str, String)]) -> Result<()> {
        Self::update_section_at(path, "General", values)
    }
    fn update_section_at(
        path: &std::path::Path,
        section: &str,
        values: &[(&str, String)],
    ) -> Result<()> {
        Self::update_sections_at(path, &[(section, values)])
    }
    fn update_sections_at(
        path: &std::path::Path,
        sections: &[(&str, &[(&str, String)])],
    ) -> Result<()> {
        use std::io::Write;
        let parent = path.parent().context("Invalid configuration path")?;
        std::fs::create_dir_all(parent)?;
        let mut lock_name = path
            .file_name()
            .context("Missing configuration filename")?
            .to_os_string();
        lock_name.push(".rust-lock");
        // Keep this separate from the atomically replaced INI inode. Never
        // unlink it: queued writers must continue locking the same file.
        let mut options = std::fs::OpenOptions::new();
        options.read(true).write(true).create(true).truncate(false);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let lock = options
            .open(path.with_file_name(lock_name))
            .context("Cannot open configuration lock")?;
        lock.lock().context("Cannot lock configuration")?;
        let source = match std::fs::read_to_string(path) {
            Ok(source) => source,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
            Err(e) => return Err(e.into()),
        };
        let mut ini = IniDocument::parse(source);
        for &(section, values) in sections {
            for (key, value) in values {
                if section == "General" && *key == "buttons" {
                    let buttons = if value.is_empty() {
                        Vec::new()
                    } else {
                        value
                            .split(',')
                            .map(|v| v.trim().parse::<i32>().context("Invalid toolbar button ID"))
                            .collect::<Result<Vec<_>>>()?
                    };
                    ensure!(
                        buttons.iter().all(|id| crate::buttons::valid(*id)),
                        "Unknown toolbar button ID"
                    );
                    ini.set_integer_list(section, key, &crate::buttons::normalize(buttons))?;
                } else if section == "General" && *key == "userColors" {
                    ini.set_string_list(
                        section,
                        key,
                        &value
                            .split(',')
                            .map(|v| v.trim().to_owned())
                            .collect::<Vec<_>>(),
                    )?;
                } else {
                    ini.set_string(section, key, value)?;
                }
            }
        }
        let merged = Self::parse(ini.source())?;
        if sections
            .iter()
            .any(|(section, values)| *section == "Shortcuts" && !values.is_empty())
        {
            crate::shortcuts::validate_overrides(&merged.shortcuts).map_err(anyhow::Error::msg)?;
        }
        let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
        if let Ok(metadata) = std::fs::metadata(path) {
            temporary
                .as_file()
                .set_permissions(metadata.permissions())?;
        }
        temporary.write_all(ini.source().as_bytes())?;
        temporary.as_file().sync_all()?;
        temporary.persist(path).map_err(|e| e.error)?;
        Ok(())
    }
    fn parse(source: &str) -> Result<Self> {
        let ini = IniDocument::parse(source.into());
        let mut settings = Self {
            buttons: ini
                .integer_list("General", "buttons")
                .unwrap_or(None)
                .map(crate::buttons::normalize),
            ..Self::default()
        };
        for (key, destination) in [
            ("uiColor", &mut settings.ui_color),
            ("contrastUiColor", &mut settings.contrast_ui_color),
        ] {
            if let Some(value) = ini.string("General", key)? {
                *destination = flameshot_core::color::parse(&value)
                    .with_context(|| format!("Invalid {key}"))?;
            }
        }
        if let Some(value) = ini.string("General", "drawColor")? {
            settings.draw_color =
                flameshot_core::color::parse(&value).context("Invalid drawColor")?;
        }
        if let Some(value) = ini.string("General", "fontFamily")? {
            settings.font_family = value;
        }
        if let Some(value) = ini.string("General", "uiLanguage")? {
            settings.language = value;
        }
        if let Some(value) = ini.string("General", "savePath")?
            && !value.is_empty()
        {
            settings.save_path = Some(value.into());
        }
        if let Some(value) = ini.string("General", "filenamePattern")? {
            ensure!(
                !value.is_empty() && !value.contains(['/', '\\']),
                "Invalid filename pattern"
            );
            ensure!(
                !chrono::format::StrftimeItems::new(&value)
                    .any(|item| matches!(item, chrono::format::Item::Error)),
                "Invalid filename format directive"
            );
            settings.filename_pattern = value;
        }
        if let Some(value) = ini.string("General", "saveAsFileExtension")? {
            let value = value.trim_start_matches('.').to_ascii_lowercase();
            ensure!(
                crate::output::EXTENSIONS.contains(&value.as_str()),
                "Unsupported configured image extension: {value}"
            );
            settings.extension = value;
        }
        if let Some(value) = ini.string("General", "showSelectionGeometry")? {
            settings.selection_geometry =
                integer_setting(&value, "showSelectionGeometry", 0, 5)? as u8;
        }
        if let Some(value) = ini.string("General", "showSelectionGeometryHideTime")? {
            settings.selection_geometry_timeout =
                integer_setting(&value, "showSelectionGeometryHideTime", 0, i32::MAX)? as u64;
        }
        if let Some(value) = ini.string("General", "contrastOpacity")? {
            settings.contrast_opacity = integer_setting(&value, "contrastOpacity", 0, 255)? as u8;
        }
        if let Some(value) = ini.string("General", "jpegQuality")? {
            settings.jpeg_quality = integer_setting(&value, "jpegQuality", 0, 100)? as u8;
        }
        for (key, destination) in [
            ("disabledTrayIcon", &mut settings.disabled_tray),
            ("showDesktopNotification", &mut settings.notifications),
            ("showHelp", &mut settings.show_help),
            ("autoCloseIdleDaemon", &mut settings.auto_close_idle),
            ("useJpgForClipboard", &mut settings.jpeg_clipboard),
            ("showQuitPrompt", &mut settings.show_quit_prompt),
            ("showAbortNotification", &mut settings.abort_notifications),
            ("showStartupLaunchMessage", &mut settings.startup_message),
            ("insecurePixelate", &mut settings.insecure_pixelate),
            ("showSidePanelButton", &mut settings.show_side_panel_button),
            ("captureActiveMonitor", &mut settings.capture_active_monitor),
            (
                "useX11LegacyScreenshot",
                &mut settings.use_x11_legacy_screenshot,
            ),
            ("predefinedColorPaletteLarge", &mut settings.large_palette),
            ("copyOnDoubleClick", &mut settings.copy_on_double_click),
            ("saveLastRegion", &mut settings.use_last_region),
            ("reverseArrow", &mut settings.reverse_arrow),
            ("saveAfterCopy", &mut settings.save_after_copy),
            ("copyPathAfterSave", &mut settings.copy_path_after_save),
            (
                "allowMultipleGuiInstances",
                &mut settings.allow_multiple_gui,
            ),
            ("antialiasingPinZoom", &mut settings.antialias_pin_zoom),
            ("keepOpenAppLauncher", &mut settings.keep_open_app_launcher),
            ("savePathFixed", &mut settings.save_path_fixed),
            ("showMagnifier", &mut settings.magnifier),
            ("squareMagnifier", &mut settings.square_magnifier),
            ("drawCircleCounterOutline", &mut settings.counter_outline),
        ] {
            if let Some(value) = ini.string("General", key)? {
                *destination = value
                    .parse()
                    .with_context(|| format!("Invalid boolean setting {key}"))?;
            }
        }
        for (key, destination) in [
            ("drawThickness", &mut settings.draw_thickness),
            ("drawMarkerSize", &mut settings.marker_size),
            ("drawPixelateSize", &mut settings.pixelate_size),
            ("drawCircleCounterSize", &mut settings.counter_size),
            ("drawFontSize", &mut settings.font_size),
        ] {
            if let Some(value) = ini.string("General", key)? {
                *destination = integer_setting(&value, key, 1, i32::MAX)? as f32;
            }
        }
        #[cfg(feature = "imgur")]
        {
            for (key, destination) in [
                (
                    "uploadWithoutConfirmation",
                    &mut settings.upload_without_confirmation,
                ),
                ("copyURLAfterUpload", &mut settings.copy_url_after_upload),
                (
                    "historyConfirmationToDelete",
                    &mut settings.history_confirmation_to_delete,
                ),
            ] {
                if let Some(value) = ini.string("General", key)? {
                    *destination = value.parse().with_context(|| format!("Invalid {key}"))?;
                }
            }
            if let Some(value) = ini.string("General", "uploadClientSecret")? {
                settings.upload_client_id = value;
            }
            if let Some(value) = ini.string("General", "uploadHistoryMax")? {
                settings.upload_history_max =
                    integer_setting(&value, "uploadHistoryMax", 0, i32::MAX)? as usize;
            }
        }
        #[cfg(feature = "update-check")]
        {
            if let Some(value) = ini.string("General", "checkForUpdates")? {
                settings.check_updates =
                    value.parse().context("Invalid checkForUpdates boolean")?;
            }
            if let Some(value) = ini.string("General", "ignoreUpdateToVersion")? {
                settings.ignored_update = value;
            }
        }
        for (name, _) in crate::shortcuts::DEFAULTS {
            if let Some(value) = ini.string("Shortcuts", name)? {
                settings.shortcuts.insert((*name).into(), value);
            }
        }
        if let Some(value) = ini.string("General", "undoLimit")? {
            settings.undo_limit = integer_setting(&value, "undoLimit", 0, 999)? as usize;
        }
        if let Some(value) = ini.string("General", "arrowStyle")? {
            settings.arrow_style = integer_setting(&value, "arrowStyle", 0, 1)? as u8;
        }
        if let Some(value) = ini.string("General", "drawRectangleSize")? {
            let radius = integer_setting(&value, "drawRectangleSize", 1, i32::MAX)?;
            settings.rectangle_radius = (radius as f32).min(10000.0);
        }
        settings.palette = default_palette(settings.large_palette);
        if let Some(values) = ini.string_list("General", "userColors")? {
            ensure!(
                (3..=17).contains(&values.len()),
                "userColors requires 3 to 17 entries"
            );
            settings.palette = values
                .iter()
                .map(|value| {
                    if value == "picker" {
                        Ok(None)
                    } else {
                        flameshot_core::color::parse_palette(value)
                            .map(Some)
                            .with_context(|| format!("Invalid palette color: {value}"))
                    }
                })
                .collect::<Result<_>>()?;
        }
        Ok(settings)
    }
}
pub fn default_palette(large: bool) -> Vec<Option<[u8; 4]>> {
    let colors: &[u32] = if large {
        &[
            0xffffff, 0xff0000, 0x00ff00, 0x0000ff, 0x000000, 0x800000, 0x008000, 0x000080,
            0x808080, 0x00ffff, 0xff00ff, 0xffff00, 0xc0c0c0, 0x008080, 0x800080, 0x808000,
        ]
    } else {
        &[
            0x800000, 0xff0000, 0xffff00, 0x00ff00, 0x008000, 0x00ffff, 0x0000ff, 0xff00ff,
            0x800080,
        ]
    };
    std::iter::once(None)
        .chain(
            colors
                .iter()
                .map(|value| Some([(value >> 16) as u8, (value >> 8) as u8, *value as u8, 255])),
        )
        .collect()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn form_updates_preserve_opaque_future_fields_and_shortcuts() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("flameshot.ini");
        let original = "[General]\r\nfilenamePattern=before\r\nfutureValue=@Variant(\\x00opaque)\r\n[Shortcuts]\r\nTYPE_COPY=F9\r\nFUTURE_ACTION=Ctrl+UnknownKey\r\n[Vendor]\r\ncustom=keep this\r\n";
        std::fs::write(&path, original).unwrap();
        let loaded = Settings::parse(original).unwrap();
        assert!(!loaded.shortcuts.contains_key("FUTURE_ACTION"));
        Settings::update_sections_at(
            &path,
            &[
                ("General", &[("filenamePattern", "after".into())]),
                ("Shortcuts", &[("TYPE_COPY", "F10".into())]),
            ],
        )
        .unwrap();
        let expected = original
            .replace("filenamePattern=before", "filenamePattern=\"after\"")
            .replace("TYPE_COPY=F9", "TYPE_COPY=\"F10\"");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), expected);
        // A later rejected edit must preserve both the known and opaque fields.
        assert!(
            Settings::update_sections_at(&path, &[("Shortcuts", &[("TYPE_SAVE", "F10".into())])])
                .is_err()
        );
        assert_eq!(std::fs::read_to_string(path).unwrap(), expected);
    }
    #[test]
    fn form_changes_and_shortcuts_commit_together_or_not_at_all() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("flameshot.ini");
        let original = "[General]\nshowHelp=true\n[Shortcuts]\nTYPE_COPY=F9\n";
        std::fs::write(&path, original).unwrap();
        let general = [("showHelp", "false".into())];
        let conflict = [("TYPE_SAVE", "F9".into())];
        assert!(
            Settings::update_sections_at(&path, &[("General", &general), ("Shortcuts", &conflict)])
                .is_err()
        );
        assert_eq!(std::fs::read_to_string(&path).unwrap(), original);
        let valid = [("TYPE_SAVE", "F10".into())];
        Settings::update_sections_at(&path, &[("General", &general), ("Shortcuts", &valid)])
            .unwrap();
        let ini = IniDocument::parse(std::fs::read_to_string(path).unwrap());
        assert_eq!(
            ini.string("General", "showHelp").unwrap().as_deref(),
            Some("false")
        );
        assert_eq!(
            ini.string("Shortcuts", "TYPE_COPY").unwrap().as_deref(),
            Some("F9")
        );
        assert_eq!(
            ini.string("Shortcuts", "TYPE_SAVE").unwrap().as_deref(),
            Some("F10")
        );
    }
    #[test]
    fn shortcut_updates_validate_the_merged_configuration() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("flameshot.ini");
        let original = "[General]\ndrawThickness=3\n[Shortcuts]\nTYPE_COPY=F9\n";
        std::fs::write(&path, original).unwrap();
        assert!(
            Settings::update_section_at(&path, "Shortcuts", &[("TYPE_SAVE", "F9".into())]).is_err()
        );
        assert_eq!(std::fs::read_to_string(&path).unwrap(), original);
        Settings::update_section_at(&path, "Shortcuts", &[("TYPE_SAVE", "F10".into())]).unwrap();
        let ini = IniDocument::parse(std::fs::read_to_string(&path).unwrap());
        assert_eq!(
            ini.string("Shortcuts", "TYPE_COPY").unwrap().as_deref(),
            Some("F9")
        );
        assert_eq!(
            ini.string("Shortcuts", "TYPE_SAVE").unwrap().as_deref(),
            Some("F10")
        );
    }
    #[test]
    fn numeric_loading_and_configuration_check_agree_on_legacy_ranges() {
        let keys = [
            "drawThickness",
            "drawMarkerSize",
            "drawPixelateSize",
            "drawCircleCounterSize",
            "drawFontSize",
            "drawRectangleSize",
            "showSelectionGeometryHideTime",
            "showSelectionGeometry",
            "contrastOpacity",
            "jpegQuality",
            "undoLimit",
            "arrowStyle",
        ];
        for key in keys.into_iter().chain(if cfg!(feature = "imgur") {
            Some("uploadHistoryMax")
        } else {
            None
        }) {
            for value in [
                "0",
                "-0",
                "+1",
                "1",
                "2",
                "5",
                "6",
                "100",
                "101",
                "255",
                "256",
                "999",
                "1000",
                "2147483647",
                "2147483648",
                "4294967295",
                "-1",
                "1.5",
                "invalid",
            ] {
                let source = format!("[General]\n{key}={value}\n");
                assert_eq!(
                    Settings::parse(&source).is_ok(),
                    crate::config_check::check(&source).is_empty(),
                    "{key}={value}"
                );
            }
        }
    }
    #[test]
    fn numeric_validation_uses_the_last_effective_duplicate() {
        for (first, last, valid) in [("2147483648", "3", true), ("3", "2147483648", false)] {
            let source = format!(
                "[General]\ndrawThickness={first}\n[Other]\nprivate=keep\n[General]\ndrawThickness={last}\n"
            );
            assert_eq!(Settings::parse(&source).is_ok(), valid);
            assert_eq!(crate::config_check::check(&source).is_empty(), valid);
        }
    }
    #[test]
    fn integer_overflow_cannot_replace_valid_configuration() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("flameshot.ini");
        let original = "[General]\ndrawThickness=3\n; keep\n";
        std::fs::write(&path, original).unwrap();
        assert!(Settings::update_at(&path, &[("drawThickness", "2147483648".into())]).is_err());
        assert_eq!(std::fs::read_to_string(path).unwrap(), original);
    }
    #[cfg(feature = "imgur")]
    #[test]
    fn reads_upload_confirmation_copy_and_deletion_preferences() {
        let defaults = Settings::default();
        assert!(!defaults.upload_without_confirmation);
        assert!(defaults.copy_url_after_upload && defaults.history_confirmation_to_delete);
        let settings=Settings::parse("[General]\nuploadWithoutConfirmation=true\ncopyURLAfterUpload=false\nhistoryConfirmationToDelete=false\n").unwrap();
        assert!(settings.upload_without_confirmation);
        assert!(!settings.copy_url_after_upload);
        assert!(!settings.history_confirmation_to_delete);
        assert!(Settings::parse("[General]\nuploadWithoutConfirmation=maybe\n").is_err());
    }
    #[test]
    fn toolbar_updates_use_qt_variant_lists_and_reject_invalid_ids_atomically() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("flameshot.ini");
        std::fs::write(&path, "; comment\n[General]\nunknown=keep\n").unwrap();
        Settings::update_at(&path, &[("buttons", "23,18,0,18,20,21".into())]).unwrap();
        let saved = std::fs::read_to_string(&path).unwrap();
        let ini = IniDocument::parse(saved.clone());
        assert_eq!(
            ini.integer_list("General", "buttons").unwrap(),
            Some(vec![0, 18, 23, 20, 21])
        );
        assert!(saved.contains("unknown=keep"));
        assert!(Settings::update_at(&path, &[("buttons", "7,24".into())]).is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), saved);
        Settings::update_at(&path, &[("buttons", String::new())]).unwrap();
        assert_eq!(
            Settings::parse(&std::fs::read_to_string(path).unwrap())
                .unwrap()
                .buttons,
            Some(vec![])
        );
    }
    #[test]
    fn palette_settings_preserve_qt_lists_alpha_and_default_sizes() {
        let settings = Settings::parse("[General]\nuserColors=#80ff0000, #abc, picker\n").unwrap();
        assert_eq!(
            settings.palette,
            vec![Some([255, 0, 0, 128]), Some([170, 187, 204, 255]), None]
        );
        assert_eq!(Settings::default().palette.len(), 10);
        assert_eq!(
            Settings::parse("[General]\npredefinedColorPaletteLarge=true\n")
                .unwrap()
                .palette
                .len(),
            17
        );
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("flameshot.ini");
        std::fs::write(&path, "; kept\n[General]\nunknown=keep\n").unwrap();
        Settings::update_at(&path, &[("userColors", "#80ff0000, #abc, picker".into())]).unwrap();
        let saved = std::fs::read_to_string(&path).unwrap();
        assert!(saved.contains("unknown=keep"));
        assert_eq!(Settings::parse(&saved).unwrap().palette, settings.palette);
        assert!(
            Settings::update_at(&path, &[("userColors", "red, invalid, picker".into())]).is_err()
        );
        assert_eq!(std::fs::read_to_string(&path).unwrap(), saved);
    }
    #[test]
    fn updates_are_atomic_and_preserve_unrelated_configuration() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("flameshot.ini");
        let original =
            "; comment\n[General]\nbuttons=@Variant(\\0)\n[Shortcuts]\nTYPE_SAVE=Ctrl+S\n";
        std::fs::write(&path, original).unwrap();
        Settings::update_at(&path, &[("jpegQuality", "90".into())]).unwrap();
        let saved = std::fs::read_to_string(&path).unwrap();
        assert_eq!(
            saved,
            original.replace("[Shortcuts]", "jpegQuality=\"90\"\n[Shortcuts]")
        );
        assert_eq!(Settings::parse(&saved).unwrap().jpeg_quality, 90);
        assert!(Settings::update_at(&path, &[("jpegQuality", "999".into())]).is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), saved);
    }
    #[test]
    fn reads_legacy_output_settings_and_ignores_other_typed_values() {
        let settings=Settings::parse("[General]\nsavePath=\"/tmp/截图\"\nfilenamePattern=%F-test\nsaveAsFileExtension=.JPG\njpegQuality=90\nbuttons=@Variant(\\0)").unwrap();
        assert_eq!(settings.save_path, Some(PathBuf::from("/tmp/截图")));
        assert_eq!(settings.extension, "jpg");
        assert_eq!(settings.jpeg_quality, 90);
    }
    #[test]
    fn reads_selection_preferences_and_opacity_bounds() {
        let settings = Settings::parse(
            "[General]\ncopyOnDoubleClick=true\nsaveLastRegion=true\ncontrastOpacity=255\n",
        )
        .unwrap();
        assert!(settings.copy_on_double_click && settings.use_last_region);
        assert_eq!(settings.contrast_opacity, 255);
        assert_eq!(Settings::default().contrast_opacity, 190);
        for value in [
            "contrastOpacity=256",
            "contrastOpacity=-1",
            "saveLastRegion=maybe",
            "copyOnDoubleClick=1",
        ] {
            assert!(Settings::parse(&format!("[General]\n{value}\n")).is_err());
        }
    }
    #[test]
    fn reads_drawing_color_alpha_and_font_family() {
        let settings =
            Settings::parse("[General]\ndrawColor=#00ff0080\nfontFamily=DejaVu Serif\n").unwrap();
        assert_eq!(settings.draw_color, [0, 255, 0, 128]);
        assert_eq!(settings.font_family, "DejaVu Serif");
        assert_eq!(
            Settings::parse("[General]\ndrawColor=green\n")
                .unwrap()
                .draw_color,
            [0, 128, 0, 255]
        );
        for color in ["#123", "#notacolor", "nonsense"] {
            assert!(Settings::parse(&format!("[General]\ndrawColor={color}\n")).is_err());
        }
    }
    #[test]
    fn legacy_history_and_tool_sizes_are_independent() {
        let settings = Settings::parse("[General]\nundoLimit=0\ndrawThickness=3\ndrawMarkerSize=7\ndrawPixelateSize=9\ndrawCircleCounterSize=11\ndrawFontSize=13\n").unwrap();
        assert_eq!(settings.undo_limit, 0);
        assert_eq!(
            [
                settings.draw_thickness,
                settings.marker_size,
                settings.pixelate_size,
                settings.counter_size,
                settings.font_size
            ],
            [3.0, 7.0, 9.0, 11.0, 13.0]
        );
        for value in [
            "undoLimit=1000",
            "undoLimit=-1",
            "drawMarkerSize=0",
            "drawPixelateSize=-2",
            "drawCircleCounterSize=oops",
        ] {
            assert!(
                Settings::parse(&format!("[General]\n{value}\n")).is_err(),
                "{value}"
            );
        }
        assert_eq!(
            Settings::parse("[General]\nundoLimit=999\n")
                .unwrap()
                .undo_limit,
            999
        );
    }
    #[test]
    fn reads_and_validates_legacy_shape_preferences() {
        let settings =
            Settings::parse("[General]\narrowStyle=1\nreverseArrow=true\ndrawRectangleSize=12\n")
                .unwrap();
        assert_eq!(settings.arrow_style, 1);
        assert!(settings.reverse_arrow);
        assert_eq!(settings.rectangle_radius, 12.0);
        for value in [
            "arrowStyle=2",
            "arrowStyle=-1",
            "reverseArrow=maybe",
            "drawRectangleSize=0",
            "drawRectangleSize=-1",
        ] {
            assert!(
                Settings::parse(&format!("[General]\n{value}\n")).is_err(),
                "{value}"
            );
        }
    }
    #[test]
    fn rejects_invalid_quality_and_patterns() {
        for item in [
            "jpegQuality=101",
            "jpegQuality=-1",
            "filenamePattern=../outside",
            "filenamePattern=%Q",
        ] {
            assert!(Settings::parse(&format!("[General]\n{item}")).is_err());
        }
    }
}
