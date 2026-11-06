// SPDX-License-Identifier: GPL-3.0-or-later
//! Lossless INI storage: unknown Qt variants and comments stay byte-for-byte intact.
#[derive(Debug, Clone)]
pub struct IniDocument {
    source: String,
}
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("Malformed quoted or escaped INI string")]
    InvalidString,
    #[error("Qt typed value cannot be read as a string")]
    TypedValue,
    #[error("Invalid INI section or key")]
    InvalidKey,
}
impl IniDocument {
    pub fn parse(source: String) -> Self {
        Self { source }
    }
    pub fn source(&self) -> &str {
        &self.source
    }
    pub fn raw(&self, section: &str, key: &str) -> Option<&str> {
        let mut current = "General";
        let mut found = None;
        for line in self.source.lines() {
            let line = line.trim();
            if line.starts_with(';') || line.starts_with('#') {
                continue;
            }
            if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
                current = name;
            } else if current == section
                && let Some((name, value)) = line.split_once('=')
                && name.trim() == key
            {
                found = Some(value.trim());
            }
        }
        found
    }
    pub fn string(&self, section: &str, key: &str) -> Result<Option<String>, ConfigError> {
        self.raw(section, key).map(decode_string).transpose()
    }
    /// Replace only the last effective value. Other lines, including earlier
    /// duplicates and unknown serialized Qt data, retain their original bytes.
    pub fn set_string(&mut self, section: &str, key: &str, value: &str) -> Result<(), ConfigError> {
        if section.is_empty()
            || key.is_empty()
            || section.contains(['[', ']', '\n', '\r'])
            || key.contains(['=', '\n', '\r'])
        {
            return Err(ConfigError::InvalidKey);
        }
        let mut current = "General";
        let mut offset = 0;
        let mut location = None;
        for line in self.source.split_inclusive('\n') {
            let trimmed = line.trim();
            if let Some(name) = trimmed.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
                current = name;
            } else if current == section
                && !trimmed.starts_with([';', '#'])
                && let Some((name, _)) = trimmed.split_once('=')
                && name.trim() == key
            {
                let end = offset + line.trim_end_matches(['\n', '\r']).len();
                location = Some(offset..end);
            }
            offset += line.len();
        }
        let replacement = format!("{key}={}", encode_string(value));
        if let Some(range) = location {
            self.source.replace_range(range, &replacement);
        } else {
            let newline = if self.source.contains("\r\n") {
                "\r\n"
            } else {
                "\n"
            };
            if !self.source.is_empty() && !self.source.ends_with('\n') {
                self.source.push_str(newline);
            }
            self.source
                .push_str(&format!("[{section}]{newline}{replacement}{newline}"));
        }
        Ok(())
    }
}
fn encode_string(value: &str) -> String {
    let mut encoded = String::from("\"");
    if value.starts_with('@') {
        encoded.push('@');
    }
    for ch in value.chars() {
        match ch {
            '\\' => encoded.push_str("\\\\"),
            '"' => encoded.push_str("\\\""),
            '\n' => encoded.push_str("\\n"),
            '\r' => encoded.push_str("\\r"),
            '\t' => encoded.push_str("\\t"),
            ch => encoded.push(ch),
        }
    }
    encoded.push('"');
    encoded
}
fn decode_string(raw: &str) -> Result<String, ConfigError> {
    let raw = if raw.starts_with('"') {
        raw.strip_prefix('"')
            .and_then(|r| r.strip_suffix('"'))
            .ok_or(ConfigError::InvalidString)?
    } else {
        raw
    };
    let mut decoded = String::new();
    let mut chars = raw.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            decoded.push(ch);
            continue;
        }
        match chars.next().ok_or(ConfigError::InvalidString)? {
            'n' => decoded.push('\n'),
            'r' => decoded.push('\r'),
            't' => decoded.push('\t'),
            '0' => decoded.push('\0'),
            '\\' => decoded.push('\\'),
            '"' => decoded.push('"'),
            'x' => {
                let mut hex = String::new();
                while hex.len() < 4 && chars.peek().is_some_and(|c| c.is_ascii_hexdigit()) {
                    hex.push(chars.next().unwrap());
                }
                let value =
                    u32::from_str_radix(&hex, 16).map_err(|_| ConfigError::InvalidString)?;
                decoded.push(char::from_u32(value).ok_or(ConfigError::InvalidString)?);
            }
            _ => return Err(ConfigError::InvalidString),
        }
    }
    if decoded.starts_with("@@") {
        decoded.remove(0);
    } else if decoded.starts_with('@') {
        return Err(ConfigError::TypedValue);
    }
    Ok(decoded)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preserves_unknown_variants_comments_and_crlf() {
        let original = "; keep\r\n[General]\r\nbuttons=@Variant(\\0\\0\\x7f)\r\nsavePath=/old\r\n[Other]\r\nvalue=42\r\n";
        let mut doc = IniDocument::parse(original.into());
        doc.set_string("General", "savePath", "/截图/new").unwrap();
        assert_eq!(
            doc.source(),
            original.replace("savePath=/old", "savePath=\"/截图/new\"")
        );
        assert!(doc.string("General", "buttons").is_err());
    }
    #[test]
    fn round_trip_special_strings_and_duplicate_keys() {
        for value in [
            "@literal",
            "a=b,c;d",
            " spaces ",
            "quote\" and \\ slash",
            "line\nnext",
            "中文",
        ] {
            let mut doc = IniDocument::parse("[General]\nx=old\nx=effective\n".into());
            doc.set_string("General", "x", value).unwrap();
            assert_eq!(doc.string("General", "x").unwrap().as_deref(), Some(value));
            assert!(doc.source().contains("x=old\n"));
        }
    }
    #[test]
    fn appends_missing_keys_and_decodes_hex_escape() {
        let mut doc = IniDocument::parse("[Shortcuts]\nTYPE_SAVE=Ctrl+S".into());
        doc.set_string("General", "filenamePattern", "%F_%H-%M")
            .unwrap();
        assert_eq!(
            doc.string("Shortcuts", "TYPE_SAVE").unwrap().as_deref(),
            Some("Ctrl+S")
        );
        assert_eq!(decode_string("\\x4e2d文").unwrap(), "中文");
        assert!(doc.set_string("General", "bad\nkey", "value").is_err());
    }
}
