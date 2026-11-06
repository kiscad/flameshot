// SPDX-License-Identifier: GPL-3.0-or-later
use crate::geometry::PixelRect;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Region {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Spec {
    All,
    Screen(usize),
    Rectangle(Region),
}
#[derive(Debug, thiserror::Error)]
#[error("Invalid region: expected WxH+X+Y, all, or screen<N>")]
pub struct RegionError;
impl std::str::FromStr for Spec {
    type Err = RegionError;
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        if text == "all" {
            return Ok(Self::All);
        }
        if let Some(number) = text.strip_prefix("screen") {
            return number.parse().map(Self::Screen).map_err(|_| RegionError);
        }
        static PATTERN: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
        let pattern = PATTERN.get_or_init(|| {
            regex::Regex::new(r"^(-?\d+)[x,.\s](-?\d+)[+,.\s]*(-?\d+)[+,.\s]*(-?\d+)$").unwrap()
        });
        let captures = pattern.captures(text).ok_or(RegionError)?;
        let parse = |index: usize| captures[index].parse::<i32>().map_err(|_| RegionError);
        let w = parse(1)?;
        let h = parse(2)?;
        let x = parse(3)?;
        let y = parse(4)?;
        Ok(Self::Rectangle(Region {
            x: if w < 0 {
                x.checked_add(w).ok_or(RegionError)?
            } else {
                x
            },
            y: if h < 0 {
                y.checked_add(h).ok_or(RegionError)?
            } else {
                y
            },
            width: w.unsigned_abs(),
            height: h.unsigned_abs(),
        }))
    }
}
impl Region {
    pub fn clip(self, size: [u32; 2]) -> Option<PixelRect> {
        let left = i64::from(self.x).max(0);
        let top = i64::from(self.y).max(0);
        let right = (i64::from(self.x) + i64::from(self.width)).min(i64::from(size[0]));
        let bottom = (i64::from(self.y) + i64::from(self.height)).min(i64::from(size[1]));
        (right > left && bottom > top).then(|| PixelRect {
            x: left as u32,
            y: top as u32,
            width: (right - left) as u32,
            height: (bottom - top) as u32,
        })
    }
    pub fn from_pixels(rect: PixelRect) -> Result<Self, RegionError> {
        Ok(Self {
            x: rect.x.try_into().map_err(|_| RegionError)?,
            y: rect.y.try_into().map_err(|_| RegionError)?,
            width: rect.width,
            height: rect.height,
        })
    }
    pub fn encode(self) -> Result<[u8; 16], RegionError> {
        let right = i32::try_from(i64::from(self.x) + i64::from(self.width) - 1)
            .map_err(|_| RegionError)?;
        let bottom = i32::try_from(i64::from(self.y) + i64::from(self.height) - 1)
            .map_err(|_| RegionError)?;
        let mut data = [0; 16];
        for (chunk, value) in data
            .as_chunks_mut::<4>()
            .0
            .iter_mut()
            .zip([self.x, self.y, right, bottom])
        {
            chunk.copy_from_slice(&value.to_be_bytes());
        }
        Ok(data)
    }
    pub fn decode(data: &[u8]) -> Result<Self, RegionError> {
        let bytes: &[u8; 16] = data.try_into().map_err(|_| RegionError)?;
        let fields: Vec<_> = bytes
            .as_chunks::<4>()
            .0
            .iter()
            .map(|c| i32::from_be_bytes(*c))
            .collect();
        Ok(Self {
            x: fields[0],
            y: fields[1],
            width: (i64::from(fields[2]) - i64::from(fields[0]) + 1)
                .try_into()
                .map_err(|_| RegionError)?,
            height: (i64::from(fields[3]) - i64::from(fields[1]) + 1)
                .try_into()
                .map_err(|_| RegionError)?,
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_legacy_separators_negative_coordinates_and_dimensions() {
        for s in ["40x30+-10+20", "40,30,-10,20", "40 30 -10 20"] {
            assert_eq!(
                s.parse::<Spec>().unwrap(),
                Spec::Rectangle(Region {
                    x: -10,
                    y: 20,
                    width: 40,
                    height: 30
                })
            );
        }
        assert_eq!(
            "-5x4+10+20".parse::<Spec>().unwrap(),
            Spec::Rectangle(Region {
                x: 5,
                y: 20,
                width: 5,
                height: 4
            })
        );
        for s in ["oops", "screen-1", "5x4+10+20junk", "99999999999x4+0+0"] {
            assert!(s.parse::<Spec>().is_err());
        }
    }
    #[test]
    fn cache_matches_actual_qt_rect_payload_and_clips() {
        let pin = include_bytes!("../tests/fixtures/qt6-pin.bin");
        let data = &pin[pin.len() - 24..pin.len() - 8];
        let r = Region::decode(data).unwrap();
        assert_eq!(
            r,
            Region {
                x: -20,
                y: 30,
                width: 3,
                height: 2
            }
        );
        assert_eq!(r.encode().unwrap(), data);
        let r = Region {
            x: -5,
            y: 2,
            width: 10,
            height: 10,
        };
        assert_eq!(
            r.clip([8, 8]),
            Some(PixelRect {
                x: 0,
                y: 2,
                width: 5,
                height: 6
            })
        );
        assert!(Region::decode(&[0; 15]).is_err());
    }
}
