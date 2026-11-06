// SPDX-License-Identifier: GPL-3.0-or-later
//! Persistent virtual pointer for the isolated Wayland smoke test.
use std::io::{BufRead, Write};
use wayland_client::{
    Connection, Dispatch, QueueHandle,
    globals::{GlobalListContents, registry_queue_init},
    protocol::{wl_pointer, wl_registry},
};
use wayland_protocols_wlr::virtual_pointer::v1::client::{
    zwlr_virtual_pointer_manager_v1::ZwlrVirtualPointerManagerV1,
    zwlr_virtual_pointer_v1::ZwlrVirtualPointerV1,
};
struct State;
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
wayland_client::delegate_noop!(State: ZwlrVirtualPointerManagerV1);
wayland_client::delegate_noop!(State: ZwlrVirtualPointerV1);
fn main() -> anyhow::Result<()> {
    anyhow::ensure!(
        std::env::var("FLAMESHOT_TEST_WAYLAND_INPUT").as_deref() == Ok("1"),
        "Run only inside the isolated smoke test"
    );
    let connection = Connection::connect_to_env()?;
    let (globals, mut queue) = registry_queue_init::<State>(&connection)?;
    let handle = queue.handle();
    let mut state = State;
    let manager: ZwlrVirtualPointerManagerV1 = globals.bind(&handle, 1..=2, ())?;
    let pointer = manager.create_virtual_pointer(None, &handle, ());
    queue.roundtrip(&mut state)?;
    println!("ready");
    std::io::stdout().flush()?;
    for line in std::io::stdin().lock().lines() {
        let line = line?;
        let words: Vec<_> = line.split_whitespace().collect();
        match words.as_slice() {
            ["move", x, y] => pointer.motion_absolute(1, x.parse()?, y.parse()?, 1024, 768),
            ["right"] => {
                pointer.button(2, 273, wl_pointer::ButtonState::Pressed);
                pointer.frame();
                pointer.button(3, 273, wl_pointer::ButtonState::Released);
            }
            ["left"] => {
                pointer.button(4, 272, wl_pointer::ButtonState::Pressed);
                pointer.frame();
                pointer.button(5, 272, wl_pointer::ButtonState::Released);
            }
            ["quit"] => break,
            _ => anyhow::bail!("Unsupported test pointer command"),
        }
        pointer.frame();
        queue.roundtrip(&mut state)?;
        println!("ok");
        std::io::stdout().flush()?;
    }
    Ok(())
}
