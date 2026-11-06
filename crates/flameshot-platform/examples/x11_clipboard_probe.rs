// SPDX-License-Identifier: GPL-3.0-or-later
//! Controlled slow INCR recipient for the isolated desktop regression.
use anyhow::{Context, Result, ensure};
use std::io::Write;
use x11rb::{
    connection::Connection,
    protocol::{Event, xproto::*},
};
fn main() -> Result<()> {
    ensure!(
        std::env::var("FLAMESHOT_TEST_X11_INPUT").as_deref() == Ok("1"),
        "Only run on an isolated test X server"
    );
    let output = std::env::args_os().nth(1).context("Missing output path")?;
    let (connection, screen) = x11rb::connect(None)?;
    let window = connection.generate_id()?;
    connection
        .create_window(
            x11rb::COPY_DEPTH_FROM_PARENT,
            window,
            connection.setup().roots[screen].root,
            0,
            0,
            1,
            1,
            0,
            WindowClass::INPUT_OUTPUT,
            0,
            &CreateWindowAux::new().event_mask(EventMask::PROPERTY_CHANGE),
        )?
        .check()?;
    let atom = |name: &str| -> Result<u32> {
        Ok(connection
            .intern_atom(false, name.as_bytes())?
            .reply()?
            .atom)
    };
    let clipboard = atom("CLIPBOARD")?;
    let png = atom("image/png")?;
    let property = atom("FLAMESHOT_TEST_TRANSFER")?;
    let incr = atom("INCR")?;
    connection
        .convert_selection(window, clipboard, png, property, x11rb::CURRENT_TIME)?
        .check()?;
    loop {
        if let Event::SelectionNotify(event) = connection.wait_for_event()? {
            ensure!(event.property == property, "Clipboard request refused");
            break;
        }
    }
    let header = connection
        .get_property(false, window, property, AtomEnum::ANY, 0, u32::MAX)?
        .reply()?;
    ensure!(header.type_ == incr, "Expected a large INCR transfer");
    let started = std::time::Instant::now();
    println!("ready");
    std::io::stdout().flush()?;
    let mut command = String::new();
    std::io::stdin().read_line(&mut command)?;
    ensure!(command.trim() == "read", "Expected read command");
    eprintln!("Resuming INCR after {:?}", started.elapsed());
    connection.delete_property(window, property)?;
    connection.flush()?;
    let mut data = Vec::new();
    loop {
        if let Event::PropertyNotify(event) = connection.wait_for_event()?
            && event.atom == property
            && event.state == Property::NEW_VALUE
        {
            let part = connection
                .get_property(true, window, property, AtomEnum::ANY, 0, u32::MAX)?
                .reply()?;
            eprintln!(
                "INCR received {} bytes at {:?}",
                part.value.len(),
                started.elapsed()
            );
            ensure!(
                part.type_ == png && part.format == 8,
                "Wrong clipboard data format"
            );
            if part.value.is_empty() {
                break;
            }
            data.extend(part.value);
            ensure!(
                data.len() <= 128 * 1024 * 1024,
                "Clipboard data exceeds test bound"
            );
        }
    }
    std::fs::write(output, &data)?;
    println!("done");
    Ok(())
}
