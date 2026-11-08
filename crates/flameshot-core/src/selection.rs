// SPDX-License-Identifier: GPL-3.0-or-later
use crate::geometry::PixelRect;
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Edges {
    pub left: bool,
    pub right: bool,
    pub top: bool,
    pub bottom: bool,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Drag {
    Move,
    Resize(Edges),
}
pub fn hit(region: PixelRect, x: f32, y: f32, tolerance: f32) -> Option<Drag> {
    let left = region.x as f32;
    let top = region.y as f32;
    let right = left + region.width as f32;
    let bottom = top + region.height as f32;
    if x < left - tolerance
        || x > right + tolerance
        || y < top - tolerance
        || y > bottom + tolerance
    {
        return None;
    }
    let edges = Edges {
        left: (x - left).abs() <= tolerance,
        right: (x - right).abs() <= tolerance,
        top: (y - top).abs() <= tolerance,
        bottom: (y - bottom).abs() <= tolerance,
    };
    if edges == Edges::default() {
        Some(Drag::Move)
    } else {
        Some(Drag::Resize(edges))
    }
}
/// Selection adjustment is constrained to a nonempty image and never wraps.
pub fn drag(region: PixelRect, mode: Drag, dx: i64, dy: i64, size: [u32; 2]) -> Option<PixelRect> {
    if region.width == 0 || region.height == 0 || size.contains(&0) {
        return None;
    }
    let width = i64::from(region.width.min(size[0]));
    let height = i64::from(region.height.min(size[1]));
    let mut left = i64::from(region.x).min(i64::from(size[0]) - width);
    let mut top = i64::from(region.y).min(i64::from(size[1]) - height);
    let mut right = left + width;
    let mut bottom = top + height;
    match mode {
        Drag::Move => {
            left = left.saturating_add(dx).clamp(0, i64::from(size[0]) - width);
            top = top.saturating_add(dy).clamp(0, i64::from(size[1]) - height);
            right = left + width;
            bottom = top + height;
        }
        Drag::Resize(e) => {
            if e.left {
                left = left.saturating_add(dx).clamp(0, right - 1);
            }
            if e.top {
                top = top.saturating_add(dy).clamp(0, bottom - 1);
            }
            if e.right {
                right = right.saturating_add(dx).clamp(left + 1, i64::from(size[0]));
            }
            if e.bottom {
                bottom = bottom.saturating_add(dy).clamp(top + 1, i64::from(size[1]));
            }
        }
    }
    Some(PixelRect {
        x: left as u32,
        y: top as u32,
        width: (right - left) as u32,
        height: (bottom - top) as u32,
    })
}
pub fn keyboard(
    region: PixelRect,
    dx: i64,
    dy: i64,
    resize: bool,
    symmetric: bool,
    size: [u32; 2],
) -> Option<PixelRect> {
    if region.width == 0 || region.height == 0 || size.contains(&0) {
        return None;
    }
    // Qt keyboard adjustments intersect the proposed rectangle with the image;
    // mouse dragging instead clamps its position while retaining its dimensions.
    // Use wide arithmetic so public callers cannot overflow with extreme deltas.
    let symmetric = resize && symmetric;
    let dx = i128::from(dx);
    let dy = if symmetric {
        -i128::from(dy)
    } else {
        i128::from(dy)
    };
    let mut left = i128::from(region.x)
        + if !resize {
            dx
        } else if symmetric {
            -dx
        } else {
            0
        };
    let mut top = i128::from(region.y)
        + if !resize {
            dy
        } else if symmetric {
            -dy
        } else {
            0
        };
    let mut right = i128::from(region.x) + i128::from(region.width) - 1 + dx;
    let mut bottom = i128::from(region.y) + i128::from(region.height) - 1 + dy;
    let minimum = Some(PixelRect {
        x: 0,
        y: 0,
        width: 1,
        height: 1,
    });
    if right == left - 1 && bottom == top - 1 {
        return minimum;
    }
    // QRect's inclusive endpoints normalize inverted dimensions before intersection.
    if right < left - 1 {
        (left, right) = (right + 1, left - 1);
    }
    if bottom < top - 1 {
        (top, bottom) = (bottom + 1, top - 1);
    }
    let max_x = i128::from(size[0]) - 1;
    let max_y = i128::from(size[1]) - 1;
    if left > max_x || right < 0 || top > max_y || bottom < 0 {
        return minimum;
    }
    left = left.max(0);
    top = top.max(0);
    right = right.min(max_x);
    bottom = bottom.min(max_y);
    Some(PixelRect {
        x: left as u32,
        y: top as u32,
        width: (right - left + 1).max(1) as u32,
        height: (bottom - top + 1).max(1) as u32,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    fn region() -> PixelRect {
        PixelRect {
            x: 20,
            y: 30,
            width: 40,
            height: 50,
        }
    }
    #[test]
    fn keyboard_matches_qt_reference_at_edges_and_minimum_sizes() {
        let fixture = include_str!("../tests/fixtures/qt6-selection-keyboard.tsv");
        let mut count = 0;
        for line in fixture.lines() {
            let values: Vec<i64> = line
                .split_whitespace()
                .map(|value| value.parse().unwrap())
                .collect();
            assert_eq!(values.len(), 14);
            let rect = |start| PixelRect {
                x: values[start] as u32,
                y: values[start + 1] as u32,
                width: values[start + 2] as u32,
                height: values[start + 3] as u32,
            };
            assert_eq!(
                keyboard(
                    rect(0),
                    values[4],
                    values[5],
                    values[6] != 0,
                    values[7] != 0,
                    [values[8] as u32, values[9] as u32]
                ),
                Some(rect(10)),
                "{line}"
            );
            count += 1;
        }
        assert_eq!(count, 156);
    }
    #[test]
    fn keyboard_extreme_deltas_remain_nonempty_and_in_bounds() {
        for dx in [i64::MIN, -1, 0, 1, i64::MAX] {
            for dy in [i64::MIN, -1, 0, 1, i64::MAX] {
                for (resize, symmetric) in
                    [(false, false), (false, true), (true, false), (true, true)]
                {
                    let value = keyboard(region(), dx, dy, resize, symmetric, [100, 100]).unwrap();
                    assert!(value.width > 0 && value.height > 0);
                    assert!(value.x + value.width <= 100 && value.y + value.height <= 100);
                }
            }
        }
        assert!(keyboard(region(), 1, 0, false, false, [0, 100]).is_none());
    }
    #[test]
    fn moving_clamps_without_changing_dimensions() {
        assert_eq!(
            drag(region(), Drag::Move, i64::MAX, i64::MIN, [100, 100]),
            Some(PixelRect {
                x: 60,
                y: 0,
                width: 40,
                height: 50
            })
        );
    }
    #[test]
    fn edges_and_corners_resize_without_collapsing() {
        let corner = hit(region(), 20.0, 30.0, 3.0).unwrap();
        assert_eq!(
            drag(region(), corner, 10, 5, [100, 100]),
            Some(PixelRect {
                x: 30,
                y: 35,
                width: 30,
                height: 45
            })
        );
        assert_eq!(
            drag(region(), corner, 1000, 1000, [100, 100])
                .unwrap()
                .width,
            1
        );
        assert_eq!(hit(region(), 40.0, 50.0, 3.0), Some(Drag::Move));
        assert_eq!(hit(region(), 0.0, 0.0, 3.0), None);
    }
    #[test]
    fn keyboard_matches_legacy_resize_directions() {
        assert_eq!(
            keyboard(region(), -1, 0, true, false, [100, 100])
                .unwrap()
                .width,
            39
        );
        assert_eq!(
            keyboard(region(), 1, 0, true, true, [100, 100]),
            Some(PixelRect {
                x: 19,
                y: 30,
                width: 42,
                height: 50
            })
        );
        assert_eq!(
            keyboard(region(), 0, -1, true, true, [100, 100]),
            Some(PixelRect {
                x: 20,
                y: 29,
                width: 40,
                height: 52
            })
        );
    }
}
