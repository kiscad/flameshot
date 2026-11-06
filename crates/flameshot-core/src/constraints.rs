// SPDX-License-Identifier: GPL-3.0-or-later
//! Legacy Ctrl-drag drawing constraints, preserving fractional image coordinates.
use crate::editor::Point;

#[derive(Clone, Copy)]
pub enum DrawingConstraint {
    AxisAndDiagonal,
    Diagonal,
}

pub fn endpoint(start: Point, end: Point, constraint: DrawingConstraint) -> Point {
    let dx = end.x - start.x;
    let dy = end.y - start.y;
    // Preserve the legacy angle-sector tie breaking, including axis-aligned
    // square drags. Canonical zero avoids atan2 distinguishing -0 from +0.
    let angle = (if dy == 0.0 { 0.0 } else { -f64::from(dy) }).atan2(f64::from(dx));
    let down = || {
        let d = (dx + dy) / 2.0;
        Point { x: d, y: d }
    };
    let up = || {
        let d = (dx - dy) / 2.0;
        Point { x: d, y: -d }
    };
    let vector = match constraint {
        DrawingConstraint::AxisAndDiagonal => {
            match ((angle / std::f64::consts::FRAC_PI_4).round() as i32).rem_euclid(4) {
                0 => Point { x: dx, y: 0.0 },
                1 => up(),
                2 => Point { x: 0.0, y: dy },
                _ => down(),
            }
        }
        DrawingConstraint::Diagonal => {
            let sector = ((angle - std::f64::consts::FRAC_PI_4) / std::f64::consts::FRAC_PI_2)
                .round() as i32;
            if sector.rem_euclid(2) == 0 {
                up()
            } else {
                down()
            }
        }
    };
    Point {
        x: start.x + vector.x,
        y: start.y + vector.y,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn axes_and_diagonals_follow_the_nearest_direction_in_all_quadrants() {
        let start = Point { x: 100.0, y: 100.0 };
        for sx in [-1.0, 1.0] {
            for sy in [-1.0, 1.0] {
                for (dx, dy, expected) in [
                    (80.0, 20.0, (80.0, 0.0)),
                    (20.0, 80.0, (0.0, 80.0)),
                    (60.0, 40.0, (50.0, 50.0)),
                ] {
                    assert_eq!(
                        endpoint(
                            start,
                            Point {
                                x: start.x + sx * dx,
                                y: start.y + sy * dy
                            },
                            DrawingConstraint::AxisAndDiagonal
                        ),
                        Point {
                            x: start.x + sx * expected.0,
                            y: start.y + sy * expected.1
                        }
                    );
                }
                assert_eq!(
                    endpoint(
                        start,
                        Point {
                            x: start.x + sx * 80.0,
                            y: start.y + sy * 20.0
                        },
                        DrawingConstraint::Diagonal
                    ),
                    Point {
                        x: start.x + sx * 50.0,
                        y: start.y + sy * 50.0
                    }
                );
            }
        }
    }
    #[test]
    fn axis_aligned_square_drags_keep_legacy_sector_ties() {
        let start = Point { x: 0.0, y: 0.0 };
        for (end, expected) in [
            (Point { x: 0.0, y: 100.0 }, Point { x: -50.0, y: 50.0 }),
            (Point { x: -100.0, y: 0.0 }, Point { x: -50.0, y: 50.0 }),
            (Point { x: 100.0, y: 0.0 }, Point { x: 50.0, y: 50.0 }),
            (Point { x: 0.0, y: -100.0 }, Point { x: -50.0, y: -50.0 }),
        ] {
            assert_eq!(endpoint(start, end, DrawingConstraint::Diagonal), expected);
        }
    }
    #[test]
    fn zero_and_fractional_drags_preserve_the_anchor() {
        let start = Point { x: 3.25, y: 9.5 };
        assert_eq!(endpoint(start, start, DrawingConstraint::Diagonal), start);
        assert_eq!(
            endpoint(
                start,
                Point { x: 7.75, y: 11.0 },
                DrawingConstraint::Diagonal
            ),
            Point { x: 6.25, y: 12.5 }
        );
    }
}
