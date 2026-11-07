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

The tray monitor test also exercises actual D-Bus menu activation and output removal.
Build its test executable with `cargo test --locked -p flameshot --all-features
--test daemon --no-run`, then pass the executable path printed by Cargo to
`python3 tests/rust/tray_smoke.py / TEST_EXECUTABLE`.

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

## Debian source and binary packages

From a Git checkout, prepare a portable source archive with the locked Cargo
registry dependencies included:

```sh
scripts/package-rust-source.sh target/flameshot_15.0.0.orig.tar.gz
```

This exports repository files, vendors dependencies and normalizes archive metadata.
Dependency preparation may download crates; the resulting archive configures Cargo
for offline builds. Install the build dependencies listed in
`packaging/debian/control`, including Rust/Cargo 1.99 or newer, then build from the
unpacked archive:

```sh
cp -a packaging/debian debian
dpkg-buildpackage -b -us -uc
```

The Debian recipe runs Cargo tests and installs through `stage-rust.sh`. It includes
Imgur support and disables the application's update checker. Vendored files are
excluded from debhelper cleanup so their Cargo checksums remain valid. `CARGO_TARGET_DIR`
may point to an external build cache.

To run desktop tests against an unpacked package executable, set
`FLAMESHOT_TEST_HELPER_DIR` to the directory containing the Rust test helpers and
`FLAMESHOT_TEST_ARTIFACT_DIR` to a writable directory for diagnostic screenshots.
The test scripts locate fixtures relative to their own repository location.

## RPM packages

Prepare `flameshot-15.0.0.tar.gz` with `scripts/package-rust-source.sh`, place it in
the RPM build tree's `SOURCES` directory, and copy `packaging/rpm/flameshot.spec`
into `SPECS`. With the declared build dependencies installed:

```sh
rpmbuild -ba SPECS/flameshot.spec
```

The spec uses offline Cargo, runs the workspace tests and validates the desktop
entry. Imgur is enabled and application update checking is disabled. Dynamically
loaded graphics libraries are declared explicitly by SONAME, with the correct
32/64-bit capability suffix; they cannot be inferred from `ldd` alone.

Driver recommendations use the distro packages
[Fedora mesa-vulkan-drivers](https://packages.fedoraproject.org/pkgs/mesa/mesa-vulkan-drivers/index.html)
and [openSUSE libvulkan_lvp](https://slc-downloadcontent.opensuse.org/repositories/X11%3A/XOrg/openSUSE_Factory/x86_64/).
Select a screenshot portal backend appropriate for the running desktop.

## Arch development packages

The PKGBUILD consumes a local snapshot produced by the source-archive helper:

```sh
mkdir -p target/arch-build
scripts/package-rust-source.sh target/arch-build/flameshot-15.0.0-rust.tar.gz
cp PKGBUILD target/arch-build/
cd target/arch-build
makepkg
```

The archive records a Git revision for `pkgver()`; dirty working trees are marked.
The local snapshot checksum is skipped because each revision produces different
content. For publishing, pin the prepared archive/checksum and use an appropriate
release source location. Cargo builds and tests run offline once the required Rust
toolchain and declared package dependencies are installed. Imgur is enabled and
application update checking is disabled.

Arch's [libx11 package](https://archlinux.org/packages/extra/x86_64/libx11/files/)
includes X11-XCB, and its [wayland package](https://archlinux.org/packages/extra/x86_64/wayland/files/)
includes the Wayland EGL library. A GPU driver or
[vulkan-swrast software driver](https://archlinux.org/packages/extra/x86_64/vulkan-swrast/)
is needed for Vulkan rendering; OpenGL is the alternative backend.

## Flatpak

The manifest uses Freedesktop 26.08 and a checksum-pinned Rust 1.99.1 build module.
The compiler is removed from the application payload. This avoids relying on an SDK
extension older than the workspace MSRV. Vendored archives are an
[upstream-supported Cargo packaging approach](https://github.com/flatpak/flatpak-builder-tools/blob/master/cargo/README.md).

```sh
mkdir -p target/flatpak-manifest
scripts/package-rust-source.sh target/flatpak-manifest/flameshot-15.0.0-rust.tar.gz
cp packaging/flatpak/org.flameshot.Flameshot.yml target/flatpak-manifest/
flatpak-builder --user --install-deps-from=flathub --force-clean target/flatpak-app target/flatpak-manifest/org.flameshot.Flameshot.yml
```

The builder needs working user namespaces and the 26.08 SDK/runtime. Local source
preparation may download dependencies; application compilation is offline. The
manifest keeps Pictures access and portal/tray/notification integration. X11 stays
available alongside Wayland for clipboard fallback on compositors without data-control.
Saving elsewhere and full host-app launching still require the remaining sandbox
integration work; broad home access is not added as a substitute.

The development environment currently rejects bubblewrap UID-map setup. Manifest
parsing, source downloads and unconfined build/runtime checks therefore do not prove
a successful Flatpak sandbox build. The dedicated CI workflow provides that build
gate on a runner with the required namespace support; it has not yet run remotely.

## Nix

The Linux flake uses `rustPlatform.buildRustPackage`, dependencies locked by
`Cargo.lock`, and the shared Rust package staging script. Its pinned nixpkgs
provides Rust 1.99.0, satisfying the workspace's Rust 1.99 minimum (the non-Nix
rustup toolchain remains 1.99.1). No Qt or CMake build hooks are used. Imgur is
enabled; the application update checker is disabled. Runtime wrapping supplies
the dynamically loaded X11/Wayland/OpenGL/Vulkan libraries and the `gio` launcher.
The desktop session must provide its graphics driver, fonts, D-Bus and portals.

```sh
nix flake check --all-systems --no-build
nix build .#flameshot
./result/bin/flameshot --version
nix develop
```

Both `x86_64-linux` and `aarch64-linux` expressions are evaluated locally. The
portable application-launch test also passes after removing its hardcoded Python
path. A complete Nix build is **not yet verified**: the available Nix 2.18 client
can fetch/evaluate in a relocated private store, but cannot execute builders at
the canonical `/nix/store` paths in this environment. Enabling its sandbox option
does not resolve that mapping. The `Rust Nix` workflow uses a normal Nix store to
build and test the native package; that hosted workflow has not run yet. Evaluation
of aarch64 does not constitute an aarch64 compilation or desktop test.

## Snap

The root `snapcraft.yaml` is the canonical configuration; `data/snap/snapcraft.yaml`
is a compatibility symlink. It uses core24, the Rust plugin pinned to 1.99.1, and
[Snapcraft's GPU extension](https://ubuntu.com/docs/snapcraft/9/reference/extensions/gpu-extension/)
to obtain the graphics driver stack without a KDE/Qt runtime. Use Snapcraft 9.x
(the configuration is locally expanded with 9.2.1). Build on the target architecture;
amd64 and arm64 are declared, but local native compilation covers amd64 only.

Prepare a small, vendored source tree rather than copying local build artifacts
into Snapcraft's source part:

```sh
mkdir -p target/snap-project
scripts/package-rust-source.sh target/flameshot-15.0.0-rust.tar.gz
tar -xzf target/flameshot-15.0.0-rust.tar.gz --strip-components=1 -C target/snap-project
cd target/snap-project
snapcraft expand-extensions
snapcraft
```

The Rust plugin installs the locked app with Imgur enabled and update checking
disabled. The build override runs workspace tests, stages desktop/D-Bus/icon files,
and removes Cargo installation bookkeeping. Rust crates in the prepared archive
are offline; acquiring the compiler, base, stage packages and graphics provider
still needs network access. The launcher keeps the GPU wrapper's library precedence
and supplies app libraries, fonts, XKB data and `gio`. D-Bus activation enters through
`/snap/bin/flameshot`. The tray uses a unique connection name under Snap confinement.

A full Snapcraft provider build and installed strict-confinement desktop run are
still required. This workstation has no LXD or Multipass build provider; no Snap
was installed into the host for testing. Configuration expansion, direct Cargo
installation, staging and simulated Snap environment checks do not establish
confinement compatibility. The dedicated `Rust Snap` workflow builds the prepared
source and inspects the package for unexpected Qt libraries; it has not run remotely.
Portal file access, host application launching and autostart under confinement
remain part of the final sandbox acceptance work.

## Shell completions

The shared staging script installs Bash, Zsh and Fish completions in their standard
`share` locations. Bash and Fish completions are exercised against the executable;
optional `history`/`updates` suggestions follow that executable's enabled features.
The basic check is included in `scripts/check-rust.sh`:

```sh
python3 tests/rust/completions.py target/debug/flameshot
```

An optional second argument supplies an extracted Ubuntu shell-tool root containing
Fish, Zsh and Fish's helper functions for additional native-shell checks. Local
validation covers Bash and Fish completion results, Zsh syntax, and staged file
contents. Interactive Zsh completion and refreshed distribution package builds
remain part of packaging acceptance. The Rust CLI manual is installed in `share/man/man1/flameshot.1`; package tools
may compress it. Its options are checked against the executable help in the Rust
integration tests. Final exit-code compatibility remains under audit.
