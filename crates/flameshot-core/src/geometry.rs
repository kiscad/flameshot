// SPDX-License-Identifier: GPL-3.0-or-later
//! Half-open rectangles: right and bottom edges are excluded.
//! Desktop coordinates may be negative. Image coordinates are always unsigned.

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DesktopRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PixelRect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GeometryError {
    InvalidDesktopBounds,
    EmptyImage,
    InvalidSelection,
    OutsideCapture,
}

/// Mapping supplied by a capture backend, not inferred from monitor DPI.
/// A portal image can use a scale different from the display's scale factor.
#[derive(Debug, Clone, Copy)]
pub struct CaptureMapping {
    desktop: DesktopRect,
    image_width: u32,
    image_height: u32,
}

impl DesktopRect {
    fn valid(self) -> bool {
        [
            self.x,
            self.y,
            self.width,
            self.height,
            self.x + self.width,
            self.y + self.height,
        ]
        .into_iter()
        .all(f64::is_finite)
            && self.width > 0.0
            && self.height > 0.0
            && self.x + self.width > self.x
            && self.y + self.height > self.y
    }
}

impl CaptureMapping {
    pub fn new(
        desktop: DesktopRect,
        image_width: u32,
        image_height: u32,
    ) -> Result<Self, GeometryError> {
        if !desktop.valid() {
            return Err(GeometryError::InvalidDesktopBounds);
        }
        if image_width == 0 || image_height == 0 {
            return Err(GeometryError::EmptyImage);
        }
        Ok(Self {
            desktop,
            image_width,
            image_height,
        })
    }

    /// Clip to the capture and round outward so selected edge pixels survive.
    pub fn crop(self, selection: DesktopRect) -> Result<PixelRect, GeometryError> {
        self.crop_edges(selection, false)
    }

    /// Map a desktop partition with shared, nearest-pixel boundaries. Unlike a
    /// freehand selection, adjacent monitors must not both include the same edge
    /// pixel when the portal's composite dimensions imply a fractional scale.
    pub fn crop_partition(self, selection: DesktopRect) -> Result<PixelRect, GeometryError> {
        self.crop_edges(selection, true)
    }

    fn crop_edges(
        self,
        selection: DesktopRect,
        partition: bool,
    ) -> Result<PixelRect, GeometryError> {
        if !selection.valid() {
            return Err(GeometryError::InvalidSelection);
        }
        let left = selection.x.max(self.desktop.x);
        let top = selection.y.max(self.desktop.y);
        let right = (selection.x + selection.width).min(self.desktop.x + self.desktop.width);
        let bottom = (selection.y + selection.height).min(self.desktop.y + self.desktop.height);
        if right <= left || bottom <= top {
            return Err(GeometryError::OutsideCapture);
        }
        let start = |value: f64| if partition { value.round() } else { value.floor() } as u32;
        let end = |value: f64| if partition { value.round() } else { value.ceil() } as u32;
        let x = start((left - self.desktop.x) / self.desktop.width * f64::from(self.image_width));
        let y = start((top - self.desktop.y) / self.desktop.height * f64::from(self.image_height));
        let end_x =
            end((right - self.desktop.x) / self.desktop.width * f64::from(self.image_width))
                .min(self.image_width);
        let end_y =
            end((bottom - self.desktop.y) / self.desktop.height * f64::from(self.image_height))
                .min(self.image_height);
        if end_x <= x || end_y <= y {
            return Err(GeometryError::OutsideCapture);
        }
        Ok(PixelRect {
            x,
            y,
            width: end_x - x,
            height: end_y - y,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn rect(x: f64, y: f64, width: f64, height: f64) -> DesktopRect {
        DesktopRect {
            x,
            y,
            width,
            height,
        }
    }

    #[test]
    fn negative_desktop_origin_maps_to_image_origin() {
        let mapping =
            CaptureMapping::new(rect(-1920.0, -200.0, 3840.0, 1080.0), 4800, 1350).unwrap();
        assert_eq!(
            mapping.crop(rect(-1920.0, -200.0, 1920.0, 1080.0)),
            Ok(PixelRect {
                x: 0,
                y: 0,
                width: 2400,
                height: 1350
            })
        );
        assert_eq!(
            mapping.crop(rect(0.0, -200.0, 1920.0, 1080.0)),
            Ok(PixelRect {
                x: 2400,
                y: 0,
                width: 2400,
                height: 1350
            })
        );
    }

    #[test]
    fn fractional_selection_rounds_outward_and_clips() {
        let mapping = CaptureMapping::new(rect(0.0, 0.0, 100.0, 100.0), 125, 150).unwrap();
        assert_eq!(
            mapping.crop(rect(0.5, 0.5, 1.0, 1.0)),
            Ok(PixelRect {
                x: 0,
                y: 0,
                width: 2,
                height: 3
            })
        );
        assert_eq!(
            mapping.crop(rect(-5.0, 95.0, 110.0, 10.0)),
            Ok(PixelRect {
                x: 0,
                y: 142,
                width: 125,
                height: 8
            })
        );
    }

    #[test]
    fn monitor_partitions_share_one_boundary_without_neighbor_pixels() {
        // Observed KDE 1x/1.5x desktop: logical width 1707, portal width 2561.
        let mapping = CaptureMapping::new(rect(-683.0, 0.0, 1707.0, 768.0), 2561, 1152).unwrap();
        let left = mapping
            .crop_partition(rect(-683.0, 0.0, 683.0, 512.0))
            .unwrap();
        let right = mapping
            .crop_partition(rect(0.0, 0.0, 1024.0, 768.0))
            .unwrap();
        assert_eq!(
            left,
            PixelRect {
                x: 0,
                y: 0,
                width: 1025,
                height: 768
            }
        );
        assert_eq!(
            right,
            PixelRect {
                x: 1025,
                y: 0,
                width: 1536,
                height: 1152
            }
        );
        assert_eq!(left.x + left.width, right.x);
        assert_eq!(right.x + right.width, 2561);
        let vertical = CaptureMapping::new(rect(0.0, -683.0, 768.0, 1707.0), 1152, 2561).unwrap();
        let upper = vertical
            .crop_partition(rect(0.0, -683.0, 512.0, 683.0))
            .unwrap();
        let lower = vertical
            .crop_partition(rect(0.0, 0.0, 768.0, 1024.0))
            .unwrap();
        assert_eq!(upper.height, 1025);
        assert_eq!(lower.y, 1025);
        assert_eq!(lower.height, 1536);
        assert_eq!(upper.y + upper.height, lower.y);
        assert_eq!(lower.y + lower.height, 2561);
        assert_eq!(mapping.crop(rect(0.0, 0.0, 1024.0, 768.0)).unwrap().x, 1024);
        assert_eq!(
            mapping.crop_partition(rect(-682.9, 0.0, 0.01, 1.0)),
            Err(GeometryError::OutsideCapture)
        );
    }
    #[test]
    fn touching_edge_is_not_an_intersection() {
        let mapping = CaptureMapping::new(rect(0.0, 0.0, 100.0, 100.0), 100, 100).unwrap();
        assert_eq!(
            mapping.crop(rect(100.0, 0.0, 1.0, 1.0)),
            Err(GeometryError::OutsideCapture)
        );
    }

    #[test]
    fn rejects_invalid_and_unrepresentable_coordinates() {
        for bad in [
            rect(f64::NAN, 0.0, 1.0, 1.0),
            rect(0.0, 0.0, -1.0, 1.0),
            rect(f64::MAX, 0.0, 1.0, 1.0),
            rect(0.0, 0.0, f64::INFINITY, 1.0),
        ] {
            assert!(CaptureMapping::new(bad, 100, 100).is_err());
        }
        assert!(CaptureMapping::new(rect(0.0, 0.0, 100.0, 100.0), 0, 100).is_err());
    }

    #[test]
    fn crops_never_exceed_buffer_at_multiple_scales() {
        for size in [100, 125, 150, 200, 8000] {
            let mapping =
                CaptureMapping::new(rect(-50.0, -50.0, 100.0, 100.0), size, size).unwrap();
            for x in -100..100 {
                if let Ok(crop) = mapping.crop(rect(f64::from(x), -25.0, 25.0, 40.0)) {
                    assert!(crop.x + crop.width <= size);
                    assert!(crop.y + crop.height <= size);
                    assert!(crop.width > 0 && crop.height > 0);
                }
            }
        }
    }
}
