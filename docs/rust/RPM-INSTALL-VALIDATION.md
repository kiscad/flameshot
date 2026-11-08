# RPM installation validation

The RPM for source snapshot `r2512.47e1e77e` was rebuilt from the clean,
vendored source archive through `packaging/rpm/flameshot.spec`. The release build,
workspace tests and desktop-file validation in the spec passed. Imgur is enabled;
application update checking is disabled.

Build environment: the repository's extracted RPM 4.18 tools on the Ubuntu host,
with the existing Rust toolchain and native build libraries. `--nodeps` bypassed
RPM database build-dependency checks because the host uses dpkg; it did not bypass
Cargo compilation or the spec's tests. This is not a native Fedora build claim.

Installation uses `tests/rust/rpm_install.py` and a disposable Fedora 44 container.
The container has no host mounts. DNF resolves runtime dependencies from Fedora
repositories; weak recommendations are disabled to exercise required dependencies.
The fixture checks two install/remove cycles, RPM integrity, command features,
ELF dependencies, license/patch notices, D-Bus interface and unchanged synthetic
user configuration. It removes only its own generated container.

The default fixture does not exercise GUI rendering. The optional X11 mode is
covered below. Fedora Wayland/portals and an upgrade from an older Fedora Flameshot
package remain separate runtime/upgrade validation.

Both install/remove cycles passed. The container was removed after validation.

- RPM SHA-256: `666cd905f54c8425601b00f5a1b5ef7e9c0401c63df950bb6f832ac92a29c877`
- Installed executable SHA-256: `20bcf02d0884e5ff664b1c6092bebef67726f848bb267bd5d0d7409b24f49c09`
- Fedora image ID: `sha256:15da97ed9aa94dd3d92c3925eb89520f6c80e07ea7ee24cba848d1f8b6912b3e`
- Version: `flameshot 15.0.0`

Detailed local evidence: `target/rpm-refresh-build.log`,
`target/rpm-install-validation/install.log` and `report.json`.

## Installed X11 validation

The complete private Xvfb suite passed against the installed `/usr/bin/flameshot`
from the same RPM above, using Fedora Mesa Vulkan rendering. This covers capture,
selection/export, editing tools, shortcut overrides, save dialogs, configuration,
clipboard/INCR, pins, launcher/owner-death cleanup and monitor selection. RPM
integrity verification remained empty after the run. No host display, clipboard or
directory was mounted; the diagnostic container was removed afterwards.

The initial run timed out at the custom F8 accept shortcut. A native `xev` probe
showed that Fedora's xdotool/XKB combination sent Alt+F8 for `xdotool key F8`
(keycode 74, state 0x8); F9–F12 had the same extra modifier. The application correctly
rejected the mismatched chord. A second native probe confirmed physical XTEST input
sends state 0x0 for plain F8–F12 and only state 0x40 for Super+F8. The test now uses
that input path for F1–F12, including requested modifiers, on its own Xvfb display.
No application shortcut behavior or keyboard map was changed.

The Fedora suite then passed in full. The corresponding Ubuntu/Xvfb fixture also passed in full with the same helper,
checking the older keyboard environment. Input
probes and the installed run are recorded in `target/fedora-keyprobe.log`,
`target/fedora-keyprobe-physical.log` and `target/fedora-desktop-physical.log`;
artifact hashes and screenshots are under
`target/rpm-install-validation/fedora-x11-physical/`.

The reusable `--desktop` install wrapper copies this helper with its fixtures,
retains timeout screenshots and keeps each invocation's logs separately. An earlier
diagnostic attempt timed out downloading dependencies; that result is not counted
as GUI coverage. Subsequent invocations keep package downloads within the disposable
container to avoid duplicate downloads during remove/reinstall.
