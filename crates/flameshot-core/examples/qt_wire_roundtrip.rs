// SPDX-License-Identifier: GPL-3.0-or-later
//! Re-encode a Qt-generated pin fixture for an external Qt compatibility check.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args_os().skip(1);
    let input = args.next().ok_or("expected input path")?;
    let output = args.next().ok_or("expected output path")?;
    let pin = flameshot_core::qt_wire::decode_pin(&std::fs::read(input)?)?;
    std::fs::write(output, flameshot_core::qt_wire::encode_pin(&pin)?)?;
    Ok(())
}
