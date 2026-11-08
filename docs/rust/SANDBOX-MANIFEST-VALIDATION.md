# Sandbox manifest validation

The current source snapshot at `91fb057a` was packaged with
`scripts/package-rust-source.sh` and parsed by the extracted Flatpak Builder 1.4.2
tool. `flatpak-builder --show-manifest` passed without downloading an SDK. The
parsed manifest confirms app ID `org.flameshot.Flameshot`, Freedesktop runtime
26.08, command `flameshot`, local Rust source archive, and the declared X11/Wayland,
Dri, Pictures, network, notification and tray permissions. The source archive is
vendored and the Rust compiler module is cleanup-only; the application payload does
not retain `/app/lib/rust`.

A Python structural parse of `snapcraft.yaml` also passed: strict confinement,
core24 base, amd64/arm64 declarations, `usr/bin/flameshot` command, GPU extension,
D-Bus slot `org.flameshot.Flameshot`, and the declared desktop/Wayland/X11/network
plugs. No Snapcraft provider was available, so this does not claim a snap build or
installed strict-confinement runtime.

These checks are static/manifest evidence only. Full Flatpak SDK compilation is
blocked by the local bubblewrap UID-map restriction documented in BUILD.md, and full
Snap construction requires a provider unavailable in this environment. The hosted
Flatpak/Snap workflows remain the authoritative build gates.

Evidence: `target/flatpak-manifest-current/parsed.json`,
`target/flatpak-manifest-current/flameshot-15.0.0-rust.tar.gz`, and
`target/flatpak-manifest-current/parse.log`.

## Local full-build attempt

Using the extracted Flatpak 1.14.6 tools and the current source archive, I also ran
`flatpak-builder --disable-rofiles-fuse --force-clean --user --install-deps-from=flathub`
with isolated XDG/Flatpak directories and no host installation. The tool terminated
with SIGSEGV (exit 139) before producing a build result; `target/flatpak-validation/current-build.log`
contains the empty diagnostic. This is a builder/environment failure, not a passing
build or an application compile failure. The manifest parser result above remains
valid, while full SDK construction still requires the hosted Flatpak workflow or a
working builder/runtime environment.
