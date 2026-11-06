// SPDX-License-Identifier: GPL-3.0-or-later
use eframe::egui;
use std::time::{Duration, Instant};

#[derive(Default)]
pub struct ToolWheel {
    last_touchpad: Option<Instant>,
}
impl ToolWheel {
    pub fn step(&mut self, unit: egui::MouseWheelUnit, y: f32, now: Instant) -> i32 {
        if !y.is_finite() || y == 0.0 {
            return 0;
        }
        if unit == egui::MouseWheelUnit::Point {
            if self.last_touchpad.is_some_and(|last| {
                now.saturating_duration_since(last) < Duration::from_millis(200)
            }) {
                return 0;
            }
            self.last_touchpad = Some(now);
        }
        if y > 0.0 { 1 } else { -1 }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn wheel_ticks_are_discrete_and_touchpad_events_are_throttled() {
        let now = Instant::now();
        let mut wheel = ToolWheel::default();
        assert_eq!(wheel.step(egui::MouseWheelUnit::Line, 5.0, now), 1);
        assert_eq!(wheel.step(egui::MouseWheelUnit::Line, -1.0, now), -1);
        assert_eq!(wheel.step(egui::MouseWheelUnit::Point, 2.0, now), 1);
        assert_eq!(
            wheel.step(
                egui::MouseWheelUnit::Point,
                2.0,
                now + Duration::from_millis(199)
            ),
            0
        );
        assert_eq!(
            wheel.step(
                egui::MouseWheelUnit::Point,
                -2.0,
                now + Duration::from_millis(200)
            ),
            -1
        );
        assert_eq!(wheel.step(egui::MouseWheelUnit::Point, f32::NAN, now), 0);
        assert_eq!(wheel.step(egui::MouseWheelUnit::Line, 0.0, now), 0);
    }
}
