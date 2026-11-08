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

The fixture does not exercise GUI rendering, portals, clipboard, or an upgrade
from an older Fedora Flameshot package. Those require separate runtime/upgrade
validation; the host's native desktop tests cannot establish installed Fedora GUI
behavior.

Both install/remove cycles passed. The container was removed after validation.

- RPM SHA-256: `666cd905f54c8425601b00f5a1b5ef7e9c0401c63df950bb6f832ac92a29c877`
- Installed executable SHA-256: `20bcf02d0884e5ff664b1c6092bebef67726f848bb267bd5d0d7409b24f49c09`
- Fedora image ID: `sha256:15da97ed9aa94dd3d92c3925eb89520f6c80e07ea7ee24cba848d1f8b6912b3e`
- Version: `flameshot 15.0.0`

Detailed local evidence: `target/rpm-refresh-build.log`,
`target/rpm-install-validation/install.log` and `report.json`.
