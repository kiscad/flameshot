# Flameshot (Rust)

Flameshot is a Linux screenshot and annotation tool implemented in Rust. It
supports X11 and Wayland screenshot portals, image editing, clipboard ownership,
pinned screenshots, D-Bus integration, optional Imgur uploads and desktop
configuration.

The repository is a Cargo workspace:

```sh
cargo build --locked -p flameshot --all-features
cargo test --locked --workspace --all-features
cargo run --locked -p flameshot -- --help
```

The installed command is `flameshot`. Linux packaging and runtime instructions are
in [docs/rust/BUILD.md](docs/rust/BUILD.md). The compatibility and native desktop
evidence is indexed in [docs/rust/MIGRATION.md](docs/rust/MIGRATION.md), including
X11, GNOME, Sway/wlroots and KDE/KWin validation.

The application does not use Qt, CMake or a C++ build. Native graphics, portal,
clipboard and desktop services remain system dependencies. Optional Imgur support
is enabled with `--all-features`; distribution builds disable application update
checking through their package configuration.

## Development checks

Run the complete local gate with:

```sh
FLAMESHOT_COMPLETION_TOOL_ROOT=target/package-validation/tools scripts/check-rust.sh
```

Native fixtures use private Xvfb, Sway, GNOME or KWin sessions and isolated XDG and
D-Bus directories. They never contact public upload/update services. Release and
sandbox manifest validation records limitations where a local provider or namespace
is unavailable.

## License

Flameshot is distributed under the GNU General Public License, version 3 or later.
See [LICENSE](LICENSE) and the third-party notices in `third-party/`.
