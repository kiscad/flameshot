// SPDX-License-Identifier: GPL-3.0-or-later
//! Qt 5/6 QDataStream QPixmap + QRect + optional qreal compatibility.
//! Only the wire types used by Flameshot's public D-Bus interface are supported.
use image::RgbaImage;
use std::io::Cursor;

#[derive(Debug, thiserror::Error)]
pub enum WireError {
    #[error("Invalid or unsupported Qt screenshot message")]
    Invalid,
    #[error("Screenshot image: {0}")]
    Image(#[from] image::ImageError),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PinGeometry {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}
pub struct Pin {
    pub image: RgbaImage,
    pub geometry: PinGeometry,
    pub scale: f64,
}
const PNG_SIGNATURE: &[u8] = b"\x89PNG\r\n\x1a\n";
const LIMIT: usize = 128 * 1024 * 1024;

fn png_end(data: &[u8]) -> Result<usize, WireError> {
    if data.len() > LIMIT
        || data.get(..4) != Some(&[0, 0, 0, 1])
        || data.get(4..12) != Some(PNG_SIGNATURE)
    {
        return Err(WireError::Invalid);
    }
    let mut position = 12_usize;
    loop {
        let header = data
            .get(position..position.checked_add(8).ok_or(WireError::Invalid)?)
            .ok_or(WireError::Invalid)?;
        let length =
            u32::from_be_bytes(header[..4].try_into().map_err(|_| WireError::Invalid)?) as usize;
        let end = position
            .checked_add(12)
            .and_then(|n| n.checked_add(length))
            .filter(|n| *n <= data.len())
            .ok_or(WireError::Invalid)?;
        if &header[4..] == b"IEND" {
            if length != 0 {
                return Err(WireError::Invalid);
            }
            return Ok(end);
        }
        position = end;
    }
}
fn image(data: &[u8], end: usize) -> Result<RgbaImage, WireError> {
    let reader =
        image::ImageReader::with_format(Cursor::new(&data[4..end]), image::ImageFormat::Png);
    Ok(reader.decode()?.to_rgba8())
}
pub fn decode_clipboard(data: &[u8]) -> Result<RgbaImage, WireError> {
    let end = png_end(data)?;
    if end != data.len() {
        return Err(WireError::Invalid);
    }
    image(data, end)
}
pub fn decode_pin(data: &[u8]) -> Result<Pin, WireError> {
    let end = png_end(data)?;
    let tail = &data[end..];
    if !matches!(tail.len(), 16 | 24) {
        return Err(WireError::Invalid);
    }
    let mut values = [0_i32; 4];
    for (value, chunk) in values.iter_mut().zip(tail[..16].as_chunks::<4>().0) {
        *value = i32::from_be_bytes(*chunk);
    }
    let [x, y, right, bottom] = values;
    let width =
        u32::try_from(i64::from(right) - i64::from(x) + 1).map_err(|_| WireError::Invalid)?;
    let height =
        u32::try_from(i64::from(bottom) - i64::from(y) + 1).map_err(|_| WireError::Invalid)?;
    if width == 0 || height == 0 {
        return Err(WireError::Invalid);
    }
    let scale = if tail.len() == 24 {
        f64::from_be_bytes(tail[16..].try_into().map_err(|_| WireError::Invalid)?)
    } else {
        1.0
    };
    if !scale.is_finite() || scale <= 0.0 {
        return Err(WireError::Invalid);
    }
    Ok(Pin {
        image: image(data, end)?,
        geometry: PinGeometry {
            x,
            y,
            width,
            height,
        },
        scale,
    })
}
pub fn encode_clipboard(image: &RgbaImage) -> Result<Vec<u8>, WireError> {
    if image.width() == 0 || image.height() == 0 || image.as_raw().len() > LIMIT {
        return Err(WireError::Invalid);
    }
    let mut data = Cursor::new(vec![0, 0, 0, 1]);
    data.set_position(4);
    image.write_to(&mut data, image::ImageFormat::Png)?;
    Ok(data.into_inner())
}
pub fn encode_pin(pin: &Pin) -> Result<Vec<u8>, WireError> {
    if !pin.scale.is_finite()
        || pin.scale <= 0.0
        || pin.geometry.width == 0
        || pin.geometry.height == 0
    {
        return Err(WireError::Invalid);
    }
    let right = i32::try_from(i64::from(pin.geometry.x) + i64::from(pin.geometry.width) - 1)
        .map_err(|_| WireError::Invalid)?;
    let bottom = i32::try_from(i64::from(pin.geometry.y) + i64::from(pin.geometry.height) - 1)
        .map_err(|_| WireError::Invalid)?;
    let mut data = encode_clipboard(&pin.image)?;
    for value in [pin.geometry.x, pin.geometry.y, right, bottom] {
        data.extend_from_slice(&value.to_be_bytes());
    }
    data.extend_from_slice(&pin.scale.to_be_bytes());
    Ok(data)
}
#[cfg(test)]
mod tests {
    use super::*;
    const CLIP: &[u8] = include_bytes!("../tests/fixtures/qt6-clipboard.bin");
    const PIN: &[u8] = include_bytes!("../tests/fixtures/qt6-pin.bin");
    #[test]
    fn reads_real_qt_pixmap_and_pin_messages() {
        let clipboard = decode_clipboard(CLIP).unwrap();
        assert_eq!(clipboard.dimensions(), (3, 2));
        assert!(clipboard.pixels().all(|p| p.0 == [20, 80, 160, 255]));
        let pin = decode_pin(PIN).unwrap();
        assert_eq!(pin.image, clipboard);
        assert_eq!(
            pin.geometry,
            PinGeometry {
                x: -20,
                y: 30,
                width: 3,
                height: 2
            }
        );
        assert_eq!(pin.scale, 1.5);
        assert_eq!(decode_pin(&PIN[..PIN.len() - 8]).unwrap().scale, 1.0);
        let rewritten = decode_pin(&encode_pin(&pin).unwrap()).unwrap();
        assert_eq!(rewritten.image, pin.image);
        assert_eq!(rewritten.geometry, pin.geometry);
        assert_eq!(rewritten.scale, pin.scale);
    }
    #[test]
    fn rejects_truncation_corrupt_chunks_and_invalid_scale() {
        for length in 0..CLIP.len() {
            assert!(decode_clipboard(&CLIP[..length]).is_err());
        }
        let mut corrupt = CLIP.to_vec();
        corrupt[12..16].copy_from_slice(&u32::MAX.to_be_bytes());
        assert!(decode_clipboard(&corrupt).is_err());
        let mut invalid = PIN.to_vec();
        let len = invalid.len();
        invalid[len - 8..].copy_from_slice(&f64::NAN.to_be_bytes());
        assert!(decode_pin(&invalid).is_err());
        assert!(decode_clipboard(PIN).is_err());
    }
}
