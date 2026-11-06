// SPDX-License-Identifier: GPL-3.0-or-later
use anyhow::{Context, Result, bail, ensure};
use image::RgbaImage;
use x11rb::{
    connection::Connection,
    protocol::xproto::{ConnectionExt, ImageFormat, ImageOrder, VisualClass},
};

/// Capture the complete X11 root window. Do not use for Wayland capture:
/// an XWayland root window need not contain the compositor's desktop.
pub fn capture_desktop() -> Result<RgbaImage> {
    let (connection, screen_number) = x11rb::connect(None).context("Cannot connect to X11")?;
    let setup = connection.setup();
    let screen = &setup.roots[screen_number];
    let visual = screen
        .allowed_depths
        .iter()
        .flat_map(|depth| &depth.visuals)
        .find(|visual| visual.visual_id == screen.root_visual)
        .context("X11 root visual is missing")?;
    ensure!(
        visual.class == VisualClass::TRUE_COLOR,
        "Unsupported X11 visual class"
    );
    let format = setup
        .pixmap_formats
        .iter()
        .find(|format| format.depth == screen.root_depth)
        .context("X11 pixel format is missing")?;
    let layout = PixelLayout {
        bits_per_pixel: format.bits_per_pixel,
        scanline_pad: format.scanline_pad,
        little_endian: setup.image_byte_order == ImageOrder::LSB_FIRST,
        masks: [visual.red_mask, visual.green_mask, visual.blue_mask],
    };
    let reply = connection
        .get_image(
            ImageFormat::Z_PIXMAP,
            screen.root,
            0,
            0,
            screen.width_in_pixels,
            screen.height_in_pixels,
            u32::MAX,
        )?
        .reply()
        .context("X11 desktop capture failed")?;
    layout.decode(
        u32::from(screen.width_in_pixels),
        u32::from(screen.height_in_pixels),
        &reply.data,
    )
}

#[derive(Clone, Copy)]
struct PixelLayout {
    bits_per_pixel: u8,
    scanline_pad: u8,
    little_endian: bool,
    masks: [u32; 3],
}

impl PixelLayout {
    fn decode(self, width: u32, height: u32, data: &[u8]) -> Result<RgbaImage> {
        ensure!(
            matches!(self.bits_per_pixel, 16 | 24 | 32),
            "Unsupported X11 pixel width"
        );
        ensure!(
            matches!(self.scanline_pad, 8 | 16 | 32),
            "Unsupported X11 scanline alignment"
        );
        ensure!(width > 0 && height > 0, "Empty X11 image");
        let mut used = 0;
        for mask in self.masks {
            ensure!(mask != 0 && used & mask == 0, "Invalid X11 color mask");
            let shifted = mask >> mask.trailing_zeros();
            ensure!(
                shifted & shifted.wrapping_add(1) == 0,
                "Noncontiguous X11 color mask"
            );
            ensure!(
                u64::from(mask) < (1_u64 << self.bits_per_pixel),
                "X11 mask exceeds pixel width"
            );
            used |= mask;
        }
        let stride = (u64::from(width) * u64::from(self.bits_per_pixel))
            .div_ceil(u64::from(self.scanline_pad))
            * u64::from(self.scanline_pad)
            / 8;
        let required = stride
            .checked_mul(u64::from(height))
            .context("X11 image size overflow")?;
        ensure!(required <= data.len() as u64, "Truncated X11 image");
        let output_size = u64::from(width) * u64::from(height) * 4;
        // Bound allocation before trusting any external dimensions.
        ensure!(
            output_size <= 1024 * 1024 * 1024,
            "X11 image exceeds 1 GiB limit"
        );
        let stride = usize::try_from(stride)?;
        let bytes = usize::from(self.bits_per_pixel / 8);
        let mut output = Vec::new();
        output.try_reserve_exact(usize::try_from(output_size)?)?;
        for row in data.chunks_exact(stride).take(height as usize) {
            for pixel in row[..width as usize * bytes].chunks_exact(bytes) {
                let value = if self.little_endian {
                    pixel
                        .iter()
                        .enumerate()
                        .fold(0_u32, |v, (i, byte)| v | (u32::from(*byte) << (i * 8)))
                } else {
                    pixel
                        .iter()
                        .fold(0_u32, |v, byte| (v << 8) | u32::from(*byte))
                };
                for mask in self.masks {
                    let shift = mask.trailing_zeros();
                    let maximum = u64::from(mask >> shift);
                    let channel = u64::from((value & mask) >> shift);
                    output.push(((channel * 255 + maximum / 2) / maximum) as u8);
                }
                output.push(255);
            }
        }
        match RgbaImage::from_raw(width, height, output) {
            Some(image) => Ok(image),
            None => bail!("Invalid decoded X11 image size"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn rgb(bits_per_pixel: u8, little_endian: bool) -> PixelLayout {
        PixelLayout {
            bits_per_pixel,
            scanline_pad: 32,
            little_endian,
            masks: [0xff0000, 0xff00, 0xff],
        }
    }
    #[test]
    fn decodes_bgrx_without_using_unused_alpha() {
        assert_eq!(
            rgb(32, true)
                .decode(2, 1, &[3, 2, 1, 0, 30, 20, 10, 9])
                .unwrap()
                .as_raw(),
            &[1, 2, 3, 255, 10, 20, 30, 255]
        );
    }
    #[test]
    fn decodes_big_endian_and_padded_rows() {
        assert_eq!(
            rgb(24, false)
                .decode(1, 2, &[1, 2, 3, 99, 4, 5, 6, 99])
                .unwrap()
                .as_raw(),
            &[1, 2, 3, 255, 4, 5, 6, 255]
        );
    }
    #[test]
    fn scales_rgb565_channels() {
        let layout = PixelLayout {
            bits_per_pixel: 16,
            scanline_pad: 16,
            little_endian: true,
            masks: [0xf800, 0x7e0, 0x1f],
        };
        assert_eq!(
            layout
                .decode(3, 1, &[0, 0xf8, 0xe0, 7, 31, 0])
                .unwrap()
                .as_raw(),
            &[255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255]
        );
    }
    #[test]
    fn rejects_truncated_or_invalid_layout() {
        assert!(rgb(32, true).decode(2, 2, &[0; 15]).is_err());
        assert!(rgb(32, true).decode(0, 1, &[]).is_err());
        let mut layout = rgb(32, true);
        layout.masks[0] = 0;
        assert!(layout.decode(1, 1, &[0; 4]).is_err());
        layout.masks = [0xff, 0xff, 0xff];
        assert!(layout.decode(1, 1, &[0; 4]).is_err());
    }
}

#[derive(Debug, Clone)]
pub struct Monitor {
    pub name: String,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub primary: bool,
}
/// Active RandR monitor geometry in root-window pixels (not UI logical units).
pub fn monitors() -> Result<Vec<Monitor>> {
    let (connection, screen_number) = x11rb::connect(None)?;
    let root = connection.setup().roots[screen_number].root;
    let reply = x11rb::protocol::randr::get_monitors(&connection, root, true)?.reply()?;
    let mut monitors = Vec::new();
    for monitor in reply.monitors {
        let name = connection.get_atom_name(monitor.name)?.reply()?.name;
        monitors.push(Monitor {
            name: String::from_utf8_lossy(&name).into_owned(),
            x: i32::from(monitor.x),
            y: i32::from(monitor.y),
            width: u32::from(monitor.width),
            height: u32::from(monitor.height),
            primary: monitor.primary,
        });
    }
    ensure!(!monitors.is_empty(), "X11 has no active monitors");
    Ok(monitors)
}
pub fn pointer_position() -> Result<(i32, i32)> {
    let (connection, screen_number) = x11rb::connect(None)?;
    let pointer = connection
        .query_pointer(connection.setup().roots[screen_number].root)?
        .reply()?;
    ensure!(pointer.same_screen, "Pointer is on a different X11 screen");
    Ok((i32::from(pointer.root_x), i32::from(pointer.root_y)))
}
