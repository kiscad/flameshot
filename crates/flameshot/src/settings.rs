// SPDX-License-Identifier: GPL-3.0-or-later
use anyhow::{Context, Result, ensure};
use flameshot_core::config::IniDocument;
use std::path::PathBuf;
#[derive(Debug, Clone)]
pub struct Settings {
    pub startup_launch: bool,
    pub language: String,
    pub save_path: Option<PathBuf>,
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
            startup_launch: false,
            language: "auto".into(),
            save_path: None,
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
    fn update_at(path: &std::path::Path, values: &[(&str, String)]) -> Result<()> {
        use std::io::Write;
        let source = match std::fs::read_to_string(path) {
            Ok(source) => source,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
            Err(e) => return Err(e.into()),
        };
        let mut ini = IniDocument::parse(source);
        for (key, value) in values {
            ini.set_string("General", key, value)?;
        }
        Self::parse(ini.source())?;
        let parent = path.parent().context("Invalid configuration path")?;
        std::fs::create_dir_all(parent)?;
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
        let mut settings = Self::default();
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
                matches!(value.as_str(), "png" | "jpg" | "jpeg"),
                "Unsupported configured image extension: {value}"
            );
            settings.extension = value;
        }
        if let Some(value) = ini.string("General", "jpegQuality")? {
            settings.jpeg_quality = value.parse().context("Invalid JPEG quality")?;
            ensure!(
                settings.jpeg_quality <= 100,
                "JPEG quality must be between 0 and 100"
            );
        }
        for (key, destination) in [
            ("disabledTrayIcon", &mut settings.disabled_tray),
            ("showDesktopNotification", &mut settings.notifications),
            ("showHelp", &mut settings.show_help),
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
            ("drawFontSize", &mut settings.font_size),
        ] {
            if let Some(value) = ini.string("General", key)? {
                let parsed: u32 = value
                    .parse()
                    .with_context(|| format!("Invalid positive integer {key}"))?;
                ensure!(parsed > 0, "{key} must be positive");
                *destination = parsed as f32;
            }
        }
        Ok(settings)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn updates_are_atomic_and_preserve_unrelated_configuration() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("flameshot.ini");
        let original =
            "; comment\n[General]\nbuttons=@Variant(\\0)\n[Shortcuts]\nTYPE_SAVE=Ctrl+S\n";
        std::fs::write(&path, original).unwrap();
        Settings::update_at(&path, &[("jpegQuality", "90".into())]).unwrap();
        let saved = std::fs::read_to_string(&path).unwrap();
        assert!(saved.starts_with(original));
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
