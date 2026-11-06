# Building the Rust migration branch

The Rust application is developed on `rewrite/rust`. It is not yet a complete
replacement for the C++ application; see [the migration matrix](MIGRATION.md).
Existing C++ files remain a behavior reference until Linux parity is verified.

## Build

Install Rust through rustup. `rust-toolchain.toml` pins the compiler and components.

```sh
cargo build --locked --release
./target/release/flameshot --help
```

The application code, widgets, codecs and translation compiler are Rust. Qt and
CMake are not used by this build. System graphics/window libraries and a C linker
are required by native dependencies. On Linux, install X11/Wayland and xkbcommon
runtime libraries, a working Vulkan/OpenGL driver, and appropriate fonts. `gio`
(from GLib's utilities) is used for desktop application dispatch.

Features:

- `update-check` is enabled by default. Disable it with `--no-default-features`.
- `imgur` is disabled by default. Enable with `--features imgur`.
- `--all-features` builds both optional integrations.

On Wayland, install the desktop's Screenshot portal backend. A wlr backend also
needs a compatible Access backend (such as GTK). Native windowless clipboard
ownership uses data-control when available; XWayland is the current fallback on
compositors lacking that protocol.

## Verification

```sh
scripts/check-rust.sh
cargo build --locked -p flameshot --all-features
cargo build --locked -p flameshot-platform --examples
python3 tests/rust/desktop_smoke.py / target/debug/flameshot
python3 tests/rust/wayland_smoke.py / target/debug/flameshot
```

The test tool root `/` uses installed system tools. A directory containing an
extracted `usr/bin`, `usr/libexec` and libraries can also be supplied. X11 tests
need Xvfb, xdotool, xclip, xrandr and xsetroot. Wayland tests need Sway, swaybg, grim, wtype,
wl-clipboard, PipeWire, xdg-desktop-portal, portal-wlr, portal-gtk and the permission
store. Python Pillow verifies rendered pixels. The workflows install these tools
without Qt development packages.

Desktop tests create private display/bus sessions and isolated XDG directories.
They disable automatic update checking and never perform real Imgur uploads.
The Wayland test injects input only into its own compositor via the Rust helper.
Desktop test success on Sway/Xvfb is not proof of every GNOME/KDE/IME combination.

## Package staging

After a release build, assemble the Linux install payload without modifying a
live installation:

```sh
DESTDIR="$PWD/target/rust-stage" PREFIX=/usr scripts/stage-rust.sh
```

The payload includes the executable, desktop launcher, D-Bus activation service,
AppStream metadata, icons and licenses. Translation catalogs are embedded. This
staging step is a foundation for the distribution-specific recipes; their
migration and validation are still tracked separately.

Validate an existing Linux configuration without opening a window:

```sh
./target/release/flameshot config --check
```

This reads `$XDG_CONFIG_HOME/flameshot/flameshot.ini` (normally
`~/.config/flameshot/flameshot.ini`), reports configuration errors and returns
exit status 1 for invalid configuration or 0 for a valid/missing file. It does
not edit the file, even when combined with configuration mutation flags.

Legacy theme-color options are supported without starting the GUI:

```sh
./target/release/flameshot config --maincolor '#740096' --contrastcolor '#270032'
```

The short forms are `-m` and `-k`. Colors accept Qt/SVG names, `#RRGGBB`, or
Flameshot's `#RRGGBBAA` form. Quote hexadecimal values in shell commands so `#`
is not interpreted as a shell comment. Invalid values leave the INI file intact.
