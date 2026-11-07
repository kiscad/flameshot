// SPDX-License-Identifier: GPL-3.0-or-later
//! Input for the private KWin acceptance fixture only.
use std::io::{BufRead, Write};
use wayland_client::{
    Connection, Dispatch, QueueHandle,
    globals::{GlobalListContents, registry_queue_init},
    protocol::wl_registry,
};
use wayland_protocols_plasma::fake_input::client::org_kde_kwin_fake_input::OrgKdeKwinFakeInput;
use wayland_protocols_plasma::plasma_window_management::client::{
    org_kde_plasma_window as window, org_kde_plasma_window_management as management,
};
#[derive(Default)]
struct WindowInfo {
    app_id: String,
    geometry: Option<(i32, i32, u32, u32)>,
}
#[derive(Default)]
struct State {
    windows: std::collections::HashMap<u32, WindowInfo>,
}
impl Dispatch<management::OrgKdePlasmaWindowManagement, ()> for State {
    fn event(
        state: &mut Self,
        manager: &management::OrgKdePlasmaWindowManagement,
        event: management::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if let management::Event::Window { id } = event {
            state.windows.insert(id, WindowInfo::default());
            manager.get_window(id, qh, id);
        }
    }
}
impl Dispatch<window::OrgKdePlasmaWindow, u32> for State {
    fn event(
        state: &mut Self,
        _: &window::OrgKdePlasmaWindow,
        event: window::Event,
        id: &u32,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            window::Event::AppIdChanged { app_id } => {
                if let Some(window) = state.windows.get_mut(id) {
                    window.app_id = app_id;
                }
            }
            window::Event::Geometry {
                x,
                y,
                width,
                height,
            } => {
                if let Some(window) = state.windows.get_mut(id) {
                    window.geometry = Some((x, y, width, height));
                }
            }
            window::Event::Unmapped => {
                state.windows.remove(id);
            }
            _ => {}
        }
    }
}
impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for State {
    fn event(
        _: &mut Self,
        _: &wl_registry::WlRegistry,
        _: wl_registry::Event,
        _: &GlobalListContents,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}
wayland_client::delegate_noop!(State: OrgKdeKwinFakeInput);
fn main() -> anyhow::Result<()> {
    anyhow::ensure!(
        std::env::var("FLAMESHOT_TEST_KDE_INPUT").as_deref() == Ok("1"),
        "Run only inside the isolated KWin fixture"
    );
    let connection = Connection::connect_to_env()?;
    let (globals, mut queue) = registry_queue_init::<State>(&connection)?;
    let input: OrgKdeKwinFakeInput = globals.bind(&queue.handle(), 4..=4, ())?;
    input.authenticate(
        "Flameshot isolated test".into(),
        "Input in disposable compositor".into(),
    );
    let _manager: management::OrgKdePlasmaWindowManagement =
        globals.bind(&queue.handle(), 6..=6, ())?;
    let mut state = State::default();
    queue.roundtrip(&mut state)?;
    println!("ready");
    std::io::stdout().flush()?;
    for line in std::io::stdin().lock().lines() {
        let line = line?;
        let words: Vec<_> = line.split_whitespace().collect();
        match words.as_slice() {
            ["move", x, y] => input.pointer_motion_absolute(x.parse()?, y.parse()?),
            ["key", key, state] => input.keyboard_key(key.parse()?, state.parse()?),
            ["button", button, state] => input.button(button.parse()?, state.parse()?),
            ["geometry", app_id] => {
                queue.roundtrip(&mut state)?;
                queue.roundtrip(&mut state)?;
                if let Some((x, y, width, height)) = state
                    .windows
                    .values()
                    .find(|window| window.app_id == *app_id)
                    .and_then(|window| window.geometry)
                {
                    println!("{x} {y} {width} {height}");
                } else {
                    println!("none");
                }
                std::io::stdout().flush()?;
                continue;
            }
            ["quit"] => break,
            _ => anyhow::bail!("Unsupported KWin test input"),
        }
        queue.roundtrip(&mut state)?;
        println!("ok");
        std::io::stdout().flush()?;
    }
    Ok(())
}
