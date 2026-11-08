# Final migration audit

Audit branch: `rewrite/rust`. The current tracked project implementation contains
zero `.cpp`, C++ header, `.cmake` or `CMakeLists.txt` files. Legacy Qt/CMake workflows,
Windows installer assets, AppVeyor config and C++ formatting/lint configs are removed.
The behavioral baseline remains available through Git history and frozen fixtures.

| Requirement | Current evidence | Result |
| --- | --- | --- |
| Pure Rust application/workspace | Cargo workspace, current offline vendored archive, no project C++/CMake files | Passed |
| Linux capture/editor/clipboard/daemon behavior | DESKTOP-ACCEPTANCE.md; X11, GNOME, Sway/wlroots, KWin native fixtures | Passed for explicitly tested compositor versions |
| CLI/D-Bus/INI/resource compatibility | CLI-COMPATIBILITY.md, DBUS-COMPATIBILITY.md, resource/config audits and final local gate | Passed locally |
| Network/upload/history flows | Native private loopback update/upload/history fixtures | Passed without external services |
| Debian/RPM/Arch package paths | Debian install/purge/reinstall, Fedora RPM install/X11/Wayland, Arch package evidence | Passed for recorded environments |
| Flatpak/Snap package definitions | SANDBOX-MANIFEST-VALIDATION.md and current manifest parser checks | Manifest passed; provider build unavailable locally |
| Nix package | NIX-VALIDATION.md | Both targets evaluate; binary build stopped at uncached nixpkgs GCC bootstrap download |
| Hosted CI | `.github/workflows/rust.yml`, `flatpak-rust.yml`, `snap-rust.yml`, `nix-rust.yml` parse and point to current Rust paths | Workflow definitions passed static checks; hosted execution unverified |
| Final source cleanup | Git tracked-file audit reports zero project C++/CMake files; README/packaging/CI are Rust-only | Passed |

The final local gate is `scripts/check-rust.sh`; its latest successful output is
`target/pure-rust-cleanup-gate.log`. The final optimized/staged artifact and source
archive are recorded in RELEASE-VALIDATION.md and SOURCE-VALIDATION.md. The final
cleaned archive passed offline all-feature workspace tests after a serial rustc retry
for a known parallel LLVM crash.

The remaining “unverified” entries are external execution requirements rather than
missing project implementation: they require a hosted runner, a Flatpak/Snap
provider, or a reachable Nix binary/source cache. They are deliberately not reported
as passing based on static manifest or local partial evidence.
