// SPDX-License-Identifier: GPL-3.0-or-later
use eframe::egui;
use std::{
    collections::BTreeMap,
    time::{Duration, Instant},
};
pub const DEFAULTS: &[(&str, &str)] = &[
    ("TYPE_PENCIL", "P"),
    ("TYPE_DRAWER", "D"),
    ("TYPE_ARROW", "A"),
    ("TYPE_SELECTION", "S"),
    ("TYPE_RECTANGLE", "R"),
    ("TYPE_CIRCLE", "C"),
    ("TYPE_MARKER", "M"),
    ("TYPE_MOVESELECTION", "Ctrl+M"),
    ("TYPE_UNDO", "Ctrl+Z"),
    ("TYPE_COPY", "Ctrl+C"),
    ("TYPE_SAVE", "Ctrl+S"),
    ("TYPE_ACCEPT", "Return"),
    ("TYPE_EXIT", "Ctrl+Q"),
    ("TYPE_CANCEL", "Ctrl+Backspace"),
    ("TYPE_IMAGEUPLOADER", ""),
    ("TYPE_OPEN_APP", "Ctrl+O"),
    ("TYPE_PIXELATE", "B"),
    ("TYPE_INVERT", "I"),
    ("TYPE_REDO", "Ctrl+Shift+Z"),
    ("TYPE_TEXT", "T"),
    ("TYPE_TOGGLE_PANEL", "Space"),
    ("TYPE_GRAB_COLOR", "G"),
    ("TYPE_RESIZE_LEFT", "Shift+Left"),
    ("TYPE_RESIZE_RIGHT", "Shift+Right"),
    ("TYPE_RESIZE_UP", "Shift+Up"),
    ("TYPE_RESIZE_DOWN", "Shift+Down"),
    ("TYPE_SYM_RESIZE_LEFT", "Ctrl+Shift+Left"),
    ("TYPE_SYM_RESIZE_RIGHT", "Ctrl+Shift+Right"),
    ("TYPE_SYM_RESIZE_UP", "Ctrl+Shift+Up"),
    ("TYPE_SYM_RESIZE_DOWN", "Ctrl+Shift+Down"),
    ("TYPE_SELECT_ALL", "Ctrl+A"),
    ("TYPE_MOVE_LEFT", "Left"),
    ("TYPE_MOVE_RIGHT", "Right"),
    ("TYPE_MOVE_UP", "Up"),
    ("TYPE_MOVE_DOWN", "Down"),
    ("TYPE_COMMIT_CURRENT_TOOL", "Ctrl+Return"),
    ("TYPE_DELETE_CURRENT_TOOL", "Delete"),
    ("TYPE_PIN", ""),
    ("TYPE_SIZEINCREASE", ""),
    ("TYPE_SIZEDECREASE", ""),
    ("TYPE_CIRCLECOUNT", ""),
    ("TYPE_CLOSE_ACTIVE_PIN_WINDOW", "Ctrl+Q"),
];
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct Chord {
    key: egui::Key,
    ctrl: bool,
    alt: bool,
    shift: bool,
    meta: bool,
}
fn parse(sequence: &str) -> Result<Vec<Chord>, String> {
    if sequence.trim().is_empty() {
        return Ok(vec![]);
    }
    let mut parts = Vec::new();
    let mut part = String::new();
    for ch in sequence.chars() {
        let comma_key = part.trim().is_empty()
            || (part.ends_with('+') && !part.ends_with("++") && part.trim() != "+");
        if ch == ',' && !comma_key {
            parts.push(std::mem::take(&mut part));
        } else {
            part.push(ch);
        }
    }
    parts.push(part);
    let mut chords = Vec::new();
    for part in &parts {
        let part = part.trim();
        let (modifiers, key) = if part == "+" {
            ("", "+")
        } else if let Some(prefix) = part.strip_suffix("++") {
            (prefix, "+")
        } else {
            part.rsplit_once('+').unwrap_or(("", part))
        };
        let key = match key {
            "Return" => "Enter",
            "Esc" => "Escape",
            "Left" => "ArrowLeft",
            "Right" => "ArrowRight",
            "Up" => "ArrowUp",
            "Down" => "ArrowDown",
            "Del" => "Delete",
            "PgUp" => "PageUp",
            "PgDown" => "PageDown",
            key => key,
        };
        let mut chord = Chord {
            key: egui::Key::from_name(key).ok_or_else(|| format!("Unknown key: {key}"))?,
            ctrl: false,
            alt: false,
            shift: false,
            meta: false,
        };
        for modifier in modifiers.split('+').filter(|s| !s.is_empty()) {
            match modifier.to_ascii_lowercase().as_str() {
                "ctrl" | "control" => chord.ctrl = true,
                "alt" => chord.alt = true,
                "shift" => chord.shift = true,
                "meta" | "super" | "win" => chord.meta = true,
                _ => return Err(format!("Unsupported modifier: {modifier}")),
            }
        }
        chords.push(chord);
    }
    if chords.len() > 4 {
        return Err("A shortcut can contain at most four strokes".into());
    }
    Ok(chords)
}
pub(crate) fn equivalent(left: &str, right: &str) -> bool {
    left == right || matches!((parse(left), parse(right)), (Ok(left), Ok(right)) if left == right)
}

struct Binding {
    name: String,
    chords: Vec<Chord>,
    progress: usize,
    last: Instant,
}
/// Ordered shortcut dispatch, retaining unbound keys for editor input.
/// Claimed sequence prefixes reset decimal input instead of leaking into it.
pub enum InputAction {
    Shortcut(String),
    Key(egui::Key, egui::Modifiers),
    ResetInput,
}

pub struct Shortcuts {
    bindings: Vec<Binding>,
    super_left: bool,
    super_right: bool,
}
impl Shortcuts {
    pub fn new(overrides: &BTreeMap<String, String>) -> Self {
        Self::scoped(overrides, false)
    }
    pub fn for_pin(overrides: &BTreeMap<String, String>) -> Self {
        Self::scoped(overrides, true)
    }
    fn scoped(overrides: &BTreeMap<String, String>, pin: bool) -> Self {
        let bindings = DEFAULTS
            .iter()
            .filter(|(name, _)| (*name == "TYPE_CLOSE_ACTIVE_PIN_WINDOW") == pin)
            .map(|(name, default)| {
                let mut chords = parse(overrides.get(*name).map(String::as_str).unwrap_or(default))
                    .unwrap_or_else(|_| parse(default).unwrap());
                // Explicit bindings take precedence over an implicit default,
                // as ConfigHandler::shortcut did in the Qt implementation.
                if !overrides.contains_key(*name)
                    && overrides
                        .values()
                        .any(|value| parse(value).is_ok_and(|explicit| explicit == chords))
                {
                    chords.clear();
                }
                Binding {
                    name: (*name).into(),
                    chords,
                    progress: 0,
                    last: Instant::now(),
                }
            })
            .collect();
        Self {
            bindings,
            super_left: false,
            super_right: false,
        }
    }
    pub fn reset(&mut self) {
        self.super_left = false;
        self.super_right = false;
        self.reset_sequences();
    }
    pub fn reset_sequences(&mut self) {
        for binding in &mut self.bindings {
            binding.progress = 0;
        }
    }
    pub fn actions(&mut self, ctx: &egui::Context) -> Vec<String> {
        self.events(&ctx.input(|i| i.events.clone()))
    }
    fn events(&mut self, events: &[egui::Event]) -> Vec<String> {
        self.dispatch_events(events)
            .into_iter()
            .filter_map(|action| match action {
                InputAction::Shortcut(name) => Some(name),
                _ => None,
            })
            .collect()
    }
    pub fn dispatch(&mut self, ctx: &egui::Context) -> Vec<InputAction> {
        self.dispatch_events(&ctx.input(|i| i.events.clone()))
    }
    fn dispatch_events(&mut self, events: &[egui::Event]) -> Vec<InputAction> {
        let mut actions = Vec::new();
        for event in events {
            match event {
                egui::Event::WindowFocused(false) => {
                    self.reset();
                    actions.push(InputAction::ResetInput);
                    continue;
                }
                egui::Event::Key {
                    key: egui::Key::SuperLeft,
                    pressed,
                    ..
                } => {
                    self.super_left = *pressed;
                    actions.push(InputAction::ResetInput);
                    continue;
                }
                egui::Event::Key {
                    key: egui::Key::SuperRight,
                    pressed,
                    ..
                } => {
                    self.super_right = *pressed;
                    actions.push(InputAction::ResetInput);
                    continue;
                }
                _ => {}
            }
            let chord = match event {
                egui::Event::Key {
                    key,
                    pressed: true,
                    modifiers,
                    ..
                } => Chord {
                    key: *key,
                    ctrl: modifiers.ctrl,
                    alt: modifiers.alt,
                    shift: modifiers.shift,
                    meta: self.super_left || self.super_right || modifiers.mac_cmd,
                },
                egui::Event::Copy => Chord {
                    key: egui::Key::C,
                    ctrl: true,
                    alt: false,
                    shift: false,
                    meta: self.super_left || self.super_right,
                },
                _ => continue,
            };
            // Modifier transitions are not strokes in a multi-key sequence.
            if matches!(
                chord.key,
                egui::Key::ControlLeft
                    | egui::Key::ControlRight
                    | egui::Key::ShiftLeft
                    | egui::Key::ShiftRight
                    | egui::Key::AltLeft
                    | egui::Key::AltRight
            ) {
                actions.push(InputAction::ResetInput);
                continue;
            }
            let mut claimed = false;
            let mut completed = Vec::new();
            for binding in &mut self.bindings {
                if binding.chords.is_empty() {
                    continue;
                }
                if binding.last.elapsed() > Duration::from_secs(1) {
                    binding.progress = 0;
                }
                claimed |= binding.progress > 0 || binding.chords[0] == chord;
                if binding.chords[binding.progress] == chord {
                    binding.progress += 1;
                } else {
                    binding.progress = usize::from(binding.chords[0] == chord);
                }
                binding.last = Instant::now();
                if binding.progress == binding.chords.len() {
                    completed.push(binding.name.clone());
                    binding.progress = 0;
                }
            }
            if completed.len() == 1 && !self.bindings.iter().any(|binding| binding.progress > 0) {
                actions.extend(completed.into_iter().map(InputAction::Shortcut));
            }
            if claimed || chord.meta {
                actions.push(InputAction::ResetInput);
            } else if let egui::Event::Key { key, modifiers, .. } = event {
                actions.push(InputAction::Key(*key, *modifiers));
            }
        }
        actions
    }
}
pub fn validate_overrides(overrides: &BTreeMap<String, String>) -> Result<(), String> {
    let mut used: std::collections::HashMap<Vec<Chord>, &str> = std::collections::HashMap::new();
    for (name, sequence) in overrides {
        if !DEFAULTS.iter().any(|(known, _)| *known == name) {
            return Err(format!("Unrecognized shortcut name: {name}"));
        }
        let chords = parse(sequence)?;
        if chords.is_empty() || name == "TYPE_CLOSE_ACTIVE_PIN_WINDOW" {
            continue;
        }
        for (existing, previous) in &used {
            if chords.starts_with(existing) || existing.starts_with(&chords) {
                return Err(format!(
                    "Shortcut sequence {sequence} conflicts with {previous}"
                ));
            }
        }
        if let Some(previous) = used.insert(chords, name.as_str()) {
            return Err(format!(
                "Shortcut {sequence} conflicts between {previous} and {name}"
            ));
        }
    }
    Ok(())
}
#[cfg(test)]
pub fn validate(sequence: &str) -> Result<(), String> {
    parse(sequence).map(|_| ())
}
#[cfg(test)]
mod tests {
    use super::*;
    fn key(key: egui::Key, ctrl: bool, shift: bool) -> egui::Event {
        egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers {
                ctrl,
                command: ctrl,
                shift,
                ..Default::default()
            },
        }
    }
    #[test]
    fn numeric_bindings_and_sequences_claim_keys_before_size_entry() {
        let mut shortcuts = Shortcuts::new(&BTreeMap::from([
            ("TYPE_UNDO".into(), "7".into()),
            ("TYPE_ARROW".into(), "8,9".into()),
        ]));
        let actions = shortcuts.dispatch_events(&[
            key(egui::Key::Num2, false, false),
            key(egui::Key::Num7, false, false),
            key(egui::Key::Num8, false, false),
            key(egui::Key::Num9, false, false),
            key(egui::Key::Num3, false, false),
        ]);
        let unbound: Vec<_> = actions
            .iter()
            .filter_map(|action| match action {
                InputAction::Key(key, _) => Some(*key),
                _ => None,
            })
            .collect();
        let names: Vec<_> = actions
            .iter()
            .filter_map(|action| match action {
                InputAction::Shortcut(name) => Some(name.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(unbound, [egui::Key::Num2, egui::Key::Num3]);
        assert_eq!(names, ["TYPE_UNDO", "TYPE_ARROW"]);
        let interrupted = shortcuts.dispatch_events(&[
            key(egui::Key::Num8, false, false),
            key(egui::Key::Num4, false, false),
        ]);
        assert!(
            interrupted
                .iter()
                .all(|action| matches!(action, InputAction::ResetInput))
        );
    }
    #[test]
    fn super_keys_and_focus_reset_do_not_leave_stuck_modifiers() {
        let overrides = BTreeMap::from([("TYPE_ACCEPT".into(), "Meta+F8".into())]);
        assert!(validate_overrides(&overrides).is_ok());
        let mut shortcuts = Shortcuts::new(&overrides);
        assert!(
            shortcuts
                .events(&[key(egui::Key::F8, false, false)])
                .is_empty()
        );
        let held = key(egui::Key::SuperLeft, false, false);
        assert!(
            shortcuts
                .events(&[held.clone(), key(egui::Key::F8, false, false)])
                .contains(&"TYPE_ACCEPT".into())
        );
        shortcuts.events(&[egui::Event::WindowFocused(false)]);
        assert!(
            shortcuts
                .events(&[key(egui::Key::F8, false, false)])
                .is_empty()
        );
        assert!(
            shortcuts
                .events(&[held, key(egui::Key::F8, false, false)])
                .contains(&"TYPE_ACCEPT".into())
        );
    }
    #[test]
    fn comma_and_plus_keys_do_not_conflict_with_sequence_delimiters() {
        assert_eq!(parse("Ctrl+K,Ctrl+C").unwrap().len(), 2);
        assert_eq!(parse("Ctrl+,").unwrap()[0].key, egui::Key::Comma);
        assert_eq!(parse("Ctrl++, Ctrl+C").unwrap().len(), 2);
        assert_eq!(parse("Ctrl+,, Ctrl+C").unwrap().len(), 2);
    }
    #[test]
    fn conflicts_are_rejected_and_existing_ambiguous_bindings_do_not_execute() {
        let overrides = BTreeMap::from([
            ("TYPE_COPY".into(), "Ctrl+S".into()),
            ("TYPE_SAVE".into(), "Ctrl+S".into()),
        ]);
        assert!(validate_overrides(&overrides).is_err());
        assert!(
            Shortcuts::new(&overrides)
                .events(&[key(egui::Key::S, true, false)])
                .is_empty()
        );
        let prefix = BTreeMap::from([
            ("TYPE_COPY".into(), "Ctrl+K".into()),
            ("TYPE_SAVE".into(), "Ctrl+K, Ctrl+S".into()),
        ]);
        assert!(validate_overrides(&prefix).is_err());
        let mut shortcuts = Shortcuts::new(&prefix);
        assert!(
            shortcuts
                .events(&[key(egui::Key::K, true, false)])
                .is_empty()
        );
        assert_eq!(
            shortcuts.events(&[key(egui::Key::S, true, false)]),
            vec!["TYPE_SAVE"]
        );
        assert!(validate_overrides(&BTreeMap::new()).is_ok());
        assert_eq!(
            Shortcuts::for_pin(&BTreeMap::new()).events(&[key(egui::Key::Q, true, false)]),
            vec!["TYPE_CLOSE_ACTIVE_PIN_WINDOW"]
        );
    }
    #[test]
    fn explicit_binding_replaces_an_implicit_default() {
        let overrides = BTreeMap::from([("TYPE_COPY".into(), "Ctrl+S".into())]);
        assert!(validate_overrides(&overrides).is_ok());
        let mut shortcuts = Shortcuts::new(&overrides);
        assert_eq!(
            shortcuts.events(&[key(egui::Key::S, true, false)]),
            vec!["TYPE_COPY"]
        );
        assert!(shortcuts.events(&[egui::Event::Copy]).is_empty());
        let mut defaults = Shortcuts::new(&BTreeMap::new());
        assert_eq!(
            defaults.events(&[key(egui::Key::S, true, false)]),
            vec!["TYPE_SAVE"]
        );
    }
    #[test]
    fn overrides_disable_defaults_and_support_sequences() {
        let mut shortcuts = Shortcuts::new(&BTreeMap::from([
            ("TYPE_COPY".into(), "Ctrl+K, Ctrl+C".into()),
            ("TYPE_SAVE".into(), String::new()),
        ]));
        assert!(
            !shortcuts
                .events(&[egui::Event::Copy])
                .contains(&"TYPE_COPY".into())
        );
        assert!(
            shortcuts
                .events(&[key(egui::Key::K, true, false), egui::Event::Copy])
                .contains(&"TYPE_COPY".into())
        );
        assert!(
            !shortcuts
                .events(&[key(egui::Key::S, true, false)])
                .contains(&"TYPE_SAVE".into())
        );
    }
    #[test]
    fn portable_names_and_plus_key_are_supported() {
        for (_, sequence) in DEFAULTS {
            assert!(validate(sequence).is_ok(), "{sequence}");
        }
        assert_eq!(parse("Ctrl++").unwrap()[0].key, egui::Key::Plus);
        assert_eq!(parse("Ctrl+Return").unwrap()[0].key, egui::Key::Enter);
        assert!(validate("NotAKey").is_err());
        let mut shortcuts = Shortcuts::new(&BTreeMap::new());
        assert!(
            shortcuts
                .events(&[key(egui::Key::S, false, false)])
                .contains(&"TYPE_SELECTION".into())
        );
    }
}
