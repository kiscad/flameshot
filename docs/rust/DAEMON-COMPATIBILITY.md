# Linux daemon lifecycle audit

Baseline: `f4f2e4e8`; implementation: `crates/flameshot/src/daemon.rs`,
`children.rs`, `launcher.rs` and `tray.rs`. This audit covers the Linux process and
IPC contract. Baseline ownership/idle behavior is described in
`src/core/flameshotdaemon.cpp`; signal exit behavior is in `signaldaemon.cpp`
and `main.cpp`. Actual Flatpak/Snap installation and confinement are separate package
gates; setting their environment markers does not emulate those sandboxes.

| Contract | Implementation and evidence |
| --- | --- |
| A duplicate daemon exits successfully without replacing the owner | D-Bus `DoNotQueue`; `daemon_exit.rs` checks the unique service owner before/after duplicate startup and that the original process stays alive |
| SIGINT / SIGTERM / bus loss return 130 / 143 / 3 | Signal handlers precede name publication; the exit integration runs all three events on newly created private buses |
| Native and restricted-name startup use the same lifecycle | Exit matrix runs ordinary, FLATPAK_ID and SNAP marker modes; tray integration separately checks unique-name registration for both restricted modes |
| A fresh unused daemon stays available | `Activity::idle` requires `used`; readiness and duplicate tests observe the live original owner |
| Queued/in-progress work prevents idle exit | Pending-request guard and serialized clipboard requests; `executing_and_queued_requests_prevent_idle_until_cancelled` and private D-Bus clipboard tests |
| Clipboard/pin resources retain the daemon until released | Active clipboard leases and child count; native X11 tests cover PNG/JPEG, INCR and in-flight older transfers, pin close and eventual idle exit; Wayland tests cover clipboard ownership and idle exit |
| Daemon shutdown reaps owned direct windows | `Children` shutdown guard; race/blocked-input tests and native pin shutdown after signals/quit |
| Abrupt owner death closes launcher/capture descendants | Internal parent PID check before and after kernel death-signal registration; native X11 kills only its fixture owner and observes launcher and capture disappearance |
| Background clipboard service outlives a capturing CLI process | `connect_service` deliberately starts an independent daemon; native X11 owned capture exits and clipboard remains available |
| External applications are not registered as owned Flameshot windows | Owned-command call sites are internal capture, pin, tray windows and launcher capture; external Open With/link actions use platform/portal adapters |

The exit matrix uses isolated configuration/cache directories, removes inherited
display and sandbox markers, disables update checks/tray/notifications, and applies
one explicit marker per case. Every spawned process has a cleanup guard. Normal
signal exits must also release the service name. Each case has bounded readiness
and exit waits; the entire matrix runs for default, all-feature and minimal builds
through `scripts/check-rust.sh`.

Native ownership checks run through `tests/rust/desktop_smoke.py` on a private Xvfb
display and session bus. They do not signal user processes or operate the user's
clipboard. D-Bus method and wire compatibility are documented separately in
[DBUS-COMPATIBILITY.md](DBUS-COMPATIBILITY.md).

Validation: the full local Rust gate (including all three feature matrices) and
the private X11 desktop suite passed for this audit.
