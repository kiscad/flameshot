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
    if !resize {
        return drag(region, Drag::Move, dx, dy, size);
    }
    let dy = if symmetric { -dy } else { dy };
    let mut result = region;
    if symmetric {
        result = drag(
            result,
            Drag::Resize(Edges {
                left: true,
                top: true,
                ..Default::default()
            }),
            -dx,
            -dy,
            size,
        )?;
    }
    drag(
        result,
        Drag::Resize(Edges {
            right: true,
            bottom: true,
            ..Default::default()
        }),
        dx,
        dy,
        size,
    )
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
