// SPDX-License-Identifier: GPL-3.0-or-later
use anyhow::{Context, Result, ensure};
use flameshot_core::geometry::{CaptureMapping, DesktopRect, PixelRect};
use smithay_client_toolkit::{
    delegate_output, delegate_registry,
    output::{OutputHandler, OutputState},
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
};
use wayland_client::{Connection, QueueHandle, globals::registry_queue_init, protocol::wl_output};

#[derive(Debug, Clone)]
pub struct Screen {
    pub name: String,
    pub bounds: DesktopRect,
}
pub fn list(wayland: bool) -> Result<Vec<Screen>> {
    if !wayland {
        return Ok(crate::x11::monitors()?
            .into_iter()
            .map(|m| Screen {
                name: m.name,
                bounds: DesktopRect {
                    x: f64::from(m.x),
                    y: f64::from(m.y),
                    width: f64::from(m.width),
                    height: f64::from(m.height),
                },
            })
            .collect());
    }
    let connection = Connection::connect_to_env()?;
    let (globals, mut queue) = registry_queue_init::<Outputs>(&connection)?;
    let handle = queue.handle();
    let mut state = Outputs {
        registry: RegistryState::new(&globals),
        outputs: OutputState::new(&globals, &handle),
    };
    queue.roundtrip(&mut state)?;
    queue.roundtrip(&mut state)?;
    let mut screens = Vec::new();
    for output in state.outputs.outputs() {
        let info = state
            .outputs
            .info(&output)
            .context("Wayland output information is unavailable")?;
        let (x, y) = info
            .logical_position
            .context("Compositor does not expose logical screen positions")?;
        let (width, height) = info
            .logical_size
            .context("Compositor does not expose logical screen sizes")?;
        ensure!(width > 0 && height > 0, "Invalid Wayland output dimensions");
        screens.push(Screen {
            name: info.name.unwrap_or(info.model),
            bounds: DesktopRect {
                x: f64::from(x),
                y: f64::from(y),
                width: f64::from(width),
                height: f64::from(height),
            },
        });
    }
    ensure!(!screens.is_empty(), "No active Wayland outputs");
    Ok(screens)
}
/// Map a screen into a full-desktop portal image. The caller must establish that
/// the image represents the full desktop, rather than a portal-selected area.
pub fn crop_bounds(screens: &[Screen], index: usize, width: u32, height: u32) -> Result<PixelRect> {
    let selected = screens
        .get(index)
        .context("Screen number is out of range")?;
    let left = screens
        .iter()
        .map(|s| s.bounds.x)
        .fold(f64::INFINITY, f64::min);
    let top = screens
        .iter()
        .map(|s| s.bounds.y)
        .fold(f64::INFINITY, f64::min);
    let right = screens
        .iter()
        .map(|s| s.bounds.x + s.bounds.width)
        .fold(f64::NEG_INFINITY, f64::max);
    let bottom = screens
        .iter()
        .map(|s| s.bounds.y + s.bounds.height)
        .fold(f64::NEG_INFINITY, f64::max);
    let mapping = CaptureMapping::new(
        DesktopRect {
            x: left,
            y: top,
            width: right - left,
            height: bottom - top,
        },
        width,
        height,
    )
    .map_err(|e| anyhow::anyhow!("Invalid desktop mapping: {e:?}"))?;
    mapping
        .crop(selected.bounds)
        .map_err(|e| anyhow::anyhow!("Invalid monitor crop: {e:?}"))
}
pub fn at_point(screens: &[Screen], x: f64, y: f64) -> Option<usize> {
    screens.iter().position(|s| {
        x >= s.bounds.x
            && y >= s.bounds.y
            && x < s.bounds.x + s.bounds.width
            && y < s.bounds.y + s.bounds.height
    })
}
struct Outputs {
    registry: RegistryState,
    outputs: OutputState,
}
impl OutputHandler for Outputs {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.outputs
    }
    fn new_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
    fn update_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
    fn output_destroyed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
}
delegate_output!(Outputs);
delegate_registry!(Outputs);
impl ProvidesRegistryState for Outputs {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry
    }
    registry_handlers![OutputState];
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn selects_and_crops_negative_origin_monitors_at_fractional_scale() {
        let screens = vec![
            Screen {
                name: "left".into(),
                bounds: DesktopRect {
                    x: -100.0,
                    y: 0.0,
                    width: 100.0,
                    height: 100.0,
                },
            },
            Screen {
                name: "right".into(),
                bounds: DesktopRect {
                    x: 0.0,
                    y: 0.0,
                    width: 100.0,
                    height: 100.0,
                },
            },
        ];
        assert_eq!(at_point(&screens, -1.0, 50.0), Some(0));
        assert_eq!(at_point(&screens, 0.0, 50.0), Some(1));
        assert_eq!(at_point(&screens, 100.0, 50.0), None);
        assert_eq!(
            crop_bounds(&screens, 1, 250, 125).unwrap(),
            PixelRect {
                x: 125,
                y: 0,
                width: 125,
                height: 125
            }
        );
        assert!(crop_bounds(&screens, 2, 250, 125).is_err());
    }
}
