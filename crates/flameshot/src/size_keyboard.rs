// SPDX-License-Identifier: GPL-3.0-or-later
use eframe::egui::{Key, Modifiers};

/// Legacy decimal tool-size entry, capped at the original keyboard range.
#[derive(Default)]
pub struct SizeKeyboard(u32);
impl SizeKeyboard {
    pub fn reset(&mut self) {
        self.0 = 0;
    }
    pub fn press(&mut self, key: Key, modifiers: Modifiers) -> Option<f32> {
        let digit = match key {
            Key::Num0 => 0,
            Key::Num1 => 1,
            Key::Num2 => 2,
            Key::Num3 => 3,
            Key::Num4 => 4,
            Key::Num5 => 5,
            Key::Num6 => 6,
            Key::Num7 => 7,
            Key::Num8 => 8,
            Key::Num9 => 9,
            _ => {
                self.reset();
                return None;
            }
        };
        if modifiers != Modifiers::NONE {
            self.reset();
            return None;
        }
        self.0 = self.0 * 10 + digit;
        let value = self.0.clamp(1, 50);
        if value != self.0 {
            self.reset();
        }
        Some(value as f32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn decimal_entry_clamps_and_resets_like_legacy_keyboard_input() {
        let mut input = SizeKeyboard::default();
        for (key, expected) in [
            (Key::Num2, 2.),
            (Key::Num5, 25.),
            (Key::Num9, 50.),
            (Key::Num3, 3.),
            (Key::Num0, 30.),
        ] {
            assert_eq!(input.press(key, Modifiers::NONE), Some(expected));
        }
        assert_eq!(input.press(Key::P, Modifiers::NONE), None);
        assert_eq!(input.press(Key::Num0, Modifiers::NONE), Some(1.));
        assert_eq!(input.press(Key::Num4, Modifiers::NONE), Some(4.));
        assert_eq!(input.press(Key::Num2, Modifiers::CTRL), None);
        assert_eq!(input.press(Key::Num6, Modifiers::NONE), Some(6.));
    }
}
