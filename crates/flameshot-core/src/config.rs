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
    #[error("Malformed Qt integer list")]
    InvalidIntegerList,
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
        self.set_raw(section, key, &encode_string(value))
    }
    pub fn integer_list(&self, section: &str, key: &str) -> Result<Option<Vec<i32>>, ConfigError> {
        self.raw(section, key).map(decode_integer_list).transpose()
    }
    /// Decode QSettings' comma-separated string lists, preserving quoted commas.
    pub fn string_list(
        &self,
        section: &str,
        key: &str,
    ) -> Result<Option<Vec<String>>, ConfigError> {
        let Some(raw) = self.raw(section, key) else {
            return Ok(None);
        };
        let mut values = Vec::new();
        let (mut quoted, mut escaped, mut start) = (false, false, 0);
        for (index, ch) in raw.char_indices() {
            if escaped {
                escaped = false;
                continue;
            }
            match ch {
                '\\' => escaped = true,
                '"' => quoted = !quoted,
                ',' if !quoted => {
                    values.push(decode_string(raw[start..index].trim())?);
                    start = index + 1;
                }
                _ => {}
            }
        }
        if quoted || escaped {
            return Err(ConfigError::InvalidString);
        }
        values.push(decode_string(raw[start..].trim())?);
        Ok(Some(values))
    }
    pub fn set_string_list(
        &mut self,
        section: &str,
        key: &str,
        values: &[String],
    ) -> Result<(), ConfigError> {
        self.set_raw(
            section,
            key,
            &values
                .iter()
                .map(|value| encode_string(value))
                .collect::<Vec<_>>()
                .join(", "),
        )
    }
    pub fn set_integer_list(
        &mut self,
        section: &str,
        key: &str,
        values: &[i32],
    ) -> Result<(), ConfigError> {
        let mut bytes = 127_u32.to_be_bytes().to_vec();
        bytes.extend_from_slice(&11_u32.to_be_bytes());
        bytes.extend_from_slice(b"QList<int>\0");
        bytes.extend_from_slice(
            &u32::try_from(values.len())
                .map_err(|_| ConfigError::InvalidIntegerList)?
                .to_be_bytes(),
        );
        for value in values {
            bytes.extend_from_slice(&value.to_be_bytes());
        }
        // Escaping each byte avoids ambiguity when a hex escape precedes an ASCII hex digit.
        let mut raw = String::from("@Variant(");
        for byte in bytes {
            raw.push_str(&format!("\\x{byte:02x}"));
        }
        raw.push(')');
        self.set_raw(section, key, &raw)
    }
    fn set_raw(&mut self, section: &str, key: &str, value: &str) -> Result<(), ConfigError> {
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
        let replacement = format!("{key}={value}");
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
    let mut decoded = decode_escaped(raw)?;
    if decoded.starts_with("@@") {
        decoded.remove(0);
    } else if decoded.starts_with('@') {
        return Err(ConfigError::TypedValue);
    }
    Ok(decoded)
}
fn decode_escaped(raw: &str) -> Result<String, ConfigError> {
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
            'a' => decoded.push('\x07'),
            'b' => decoded.push('\x08'),
            'f' => decoded.push('\x0c'),
            'v' => decoded.push('\x0b'),
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
    Ok(decoded)
}
fn decode_integer_list(raw: &str) -> Result<Vec<i32>, ConfigError> {
    let decoded = decode_escaped(raw)?;
    let body = decoded
        .strip_prefix("@Variant(")
        .and_then(|s| s.strip_suffix(')'))
        .ok_or(ConfigError::InvalidIntegerList)?;
    let bytes: Vec<u8> = body
        .chars()
        .map(|c| u8::try_from(c as u32).map_err(|_| ConfigError::InvalidIntegerList))
        .collect::<Result<_, _>>()?;
    if bytes.len() < 23
        || bytes[..4] != 127_u32.to_be_bytes()
        || bytes[4..8] != 11_u32.to_be_bytes()
        || &bytes[8..19] != b"QList<int>\0"
    {
        return Err(ConfigError::InvalidIntegerList);
    }
    let count = u32::from_be_bytes(bytes[19..23].try_into().unwrap()) as usize;
    if count > 10000 || bytes.len() != 23 + count * 4 {
        return Err(ConfigError::InvalidIntegerList);
    }
    Ok(bytes[23..]
        .as_chunks::<4>()
        .0
        .iter()
        .map(|chunk| i32::from_be_bytes(*chunk))
        .collect())
}

#[cfg(test)]
mod tests {
    #[test]
    fn string_lists_handle_quoted_commas_and_reject_unclosed_quotes() {
        let ini = super::IniDocument::parse(
            r##"[General]
colors=red, "#abc", picker
words="a,b", "c\\d", plain
broken="unfinished
"##
            .into(),
        );
        assert_eq!(
            ini.string_list("General", "colors").unwrap().unwrap(),
            ["red", "#abc", "picker"]
        );
        assert_eq!(
            ini.string_list("General", "words").unwrap().unwrap(),
            ["a,b", r"c\d", "plain"]
        );
        assert!(ini.string_list("General", "broken").is_err());
    }

    use super::*;
    #[test]
    fn reads_actual_qsettings_list_and_roundtrips_without_touching_other_values() {
        let source = include_str!("../tests/fixtures/qt6-settings.ini");
        let mut ini = IniDocument::parse(source.into());
        assert_eq!(
            ini.integer_list("General", "buttons").unwrap().unwrap(),
            vec![
                0, 1, 2, 3, 4, 5, 6, 8, 9, 10, 11, 12, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24
            ]
        );
        let quoted = ini.string("General", "quoted").unwrap();
        ini.set_integer_list("General", "buttons", &[18, 0, -1, 255])
            .unwrap();
        assert_eq!(
            ini.integer_list("General", "buttons").unwrap().unwrap(),
            vec![18, 0, -1, 255]
        );
        assert_eq!(ini.string("General", "quoted").unwrap(), quoted);
        if let Some(path) = std::env::var_os("FLAMESHOT_INI_ARTIFACT") {
            std::fs::write(path, ini.source()).unwrap();
        }
        assert!(
            IniDocument::parse("buttons=@Variant(\\0)".into())
                .integer_list("General", "buttons")
                .is_err()
        );
    }
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
