// SPDX-License-Identifier: GPL-3.0-or-later
//! Input for the private KWin acceptance fixture only.
use std::io::{BufRead, Write};
use wayland_client::{
    Connection, Dispatch, QueueHandle,
    globals::{GlobalListContents, registry_queue_init},
    protocol::wl_registry,
};
use wayland_protocols_plasma::fake_input::client::org_kde_kwin_fake_input::OrgKdeKwinFakeInput;
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
    let mut state = State;
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
            ["quit"] => break,
            _ => anyhow::bail!("Unsupported KWin test input"),
        }
        queue.roundtrip(&mut state)?;
        println!("ok");
        std::io::stdout().flush()?;
    }
    Ok(())
}
