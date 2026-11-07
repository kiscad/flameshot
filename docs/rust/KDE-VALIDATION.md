# KWin Wayland validation

Local validation on 2026-11-07 used a disposable Ubuntu 24.04.5 container, KWin
5.27.11, xdg-desktop-portal-kde 5.27.11, xdg-desktop-portal 1.18.4 and Mesa
25.2.8 LLVMpipe. No host display, session bus, configuration directory or GPU device
was mounted. This is a headless KDE 5 backend check, not KDE 6 or hardware acceptance.

The all-feature debug executable built at `a7f89c20` had SHA-256
`be39c0f7da73238bad516603cb6dded80933a3a780ac2589e09d31025070e921`.
Its dynamic dependencies contain no Qt library. Qt is used by the external KWin,
portal and synthetic background processes, not the Rust application.

## Results

- `full --raw`: real KDE portal, 1024×768 PNG and expected background pixels.
- `screen --number 0 --raw`: 1024×768 PNG and expected pixels.
- `gui --region 80x60+500+450 --raw`: native Wayland editor accepted through test
  keyboard input, 80×60 PNG with expected pixels. DISPLAY is absent and Xwayland
  is not enabled.
- Cancellation: status 2 and empty stdout. The fixture sends Escape while the
  request is pending; this can cancel portal consent, so it is not separate proof
  of an already-mapped editor's Escape behavior.
- Cleanup: no KWin, portal, background or input processes remained in the container
  after the successful run. The container was subsequently stopped and removed.

Artifacts are local under `target/kde-artifacts` and `target/kde-smoke.log`.
The prepared test-only environment was cached as `flameshot-test-kde:5.27`.
It contains synthetic fixtures and is not a release image.

## Native Wayland clipboard follow-up

The `--clipboard` extension passed in another disposable container from the cached
KDE environment. It starts a tracked Rust daemon on the private bus and uses an
independent PyQt5 client that asserts its platform is `wayland`. DISPLAY remains
absent and no Xwayland server is enabled.

- Full-capture PNG and 80×60 editor PNG transfer after the capture process exits.
- JPEG MIME transfer, actual JPEG decoding, dimensions and pixel tolerance.
- Exact Unicode text including Chinese, punctuation and an emoji over the legacy
  D-Bus clipboard method and native Wayland selection.
- With autoCloseIdleDaemon enabled, the daemon stays alive while owning data.
  A separate Qt Wayland writer uses a private-seat key event to replace the
  selection; the Rust daemon then exits successfully and releases its bus name.
  Another native reader verifies the replacement text remains available.

This verifies native KDE 5 ownership and handoff, including capture-process exit;
it does not exercise the GNOME Xwayland bridge or an installed sandbox. Logs are
in `target/kde-clipboard.log` and `target/kde-clipboard-artifacts`. No test processes
remained after cleanup. Use the latest checkout's script when reusing the cached
environment image, which is a dependency cache rather than the authoritative test.

## Mixed-scale outputs and interactive monitor placement

The `--multi-monitor` fixture uses two 1024×768 virtual outputs, with scales 1 and 2.
`kscreen-doctor` requests a left-hand second monitor. KWin normalizes the requested
negative origin to nonnegative positions: Virtual-1 is at (0,0), Virtual-0 at (512,0).
The test reads and asserts the resulting JSON layout; it does **not** claim negative
origin coverage. A separate colored Qt background on each screen distinguishes crops.

The KDE portal returns a 3072×1536 composite at the maximum output scale. First and
second monitor crops are 2048×1536 and 1024×768 respectively, with expected colors.
Both `--region screen1` and keyboard selection of monitor 2 are exercised. The input
helper reads the native window-management protocol and verifies the editor's actual
logical frame is (0,0,512,384) before accepting the 1024×768 export. This establishes
window placement as well as image selection.

This caught a production bug: changing an already-fullscreen window's output on
KWin 5.27 left it on the old monitor while showing the new monitor's image. Sending
leave-fullscreen and target-fullscreen requests in the same frame did not fix it.
The Rust editor now requests leaving fullscreen, waits for the compositor's reported
non-fullscreen state, then requests the selected output in a later frame. The wait
is bounded to two seconds so an uncooperative compositor cannot stall editing
indefinitely. X11 keeps its existing direct monitor-switch path.

All mixed-scale cases passed after the fix, as did KDE clipboard, GNOME dual-monitor
and Sway mixed-scale/IME regressions. The tested all-feature debug binary SHA-256 was
`4a44e14af8dc85d96f4bfaa994b52bfe600e4cbdd7e110f6fc4ea4cc84d4b1a1`.
Logs/artifacts are `target/kde-monitors.log` and `target/kde-monitor-artifacts`;
`chooser-debug.png` is the pre-fix diagnostic, not accepted output. The test-only
container with KScreen tools is cached as `flameshot-test-kde:5.27-monitors`; copy
current checkout binaries/scripts into it before reuse.

Run this mode separately from `--clipboard`. It additionally requires
`libkf5screen-bin` (`kscreen-doctor`):

```sh
python3 tests/rust/kde_smoke.py target/debug/flameshot target/debug/examples/kde_input --multi-monitor
```

Fractional scaling and virtual-output lifecycle are covered in the follow-ups below; native KDE IME remains separate acceptance work.

## Fractional-scale crop boundaries

The `--fractional` mode uses scales 1 and 1.5 and implicitly enables the multi-monitor
fixture. KWin reports a logical second-output width of 683 and normalizes positions
to (0,0) and (683,0). Its actual portal image is 2561×1152: the second monitor occupies
columns 0–1024 (1025 pixels), and the first occupies columns 1025–2560 (1536 pixels).
The compositor's composite rounding is authoritative here; forcing the second crop
to its physical mode width of 1024 would discard a column of the portal image.

This exposed another production bug: outward rounding, appropriate for an arbitrary
selection, started the first monitor at column 1024. Its 1537-pixel crop included one
orange column from the adjacent monitor. Monitor partitions now use consistent
nearest-pixel boundaries, while ordinary selections retain outward rounding. The
regression checks shared horizontal and vertical boundaries and keeps the original
selection-rounding test.

After the fix, full capture, both monitor crops, explicit/interactive editor export
and native second-monitor placement all pass. The second editor has a 683×512
logical frame and exports the correct 1025×768 portal-image region. Crop edge colors
are checked as well as interior pixels. Local evidence is in `target/kde-fractional.log`
and `target/kde-fractional-artifacts`; `target/kde-fractional-before.png` shows the
pre-fix adjacent column. Tested debug executable SHA-256:
`74f2e0b6f6cc555a242b1c32e7a522d47648dbeb8c0f36b22ee09b0a4cffa460`.

```sh
python3 tests/rust/kde_smoke.py target/debug/flameshot target/debug/examples/kde_input --fractional
```

KWin 1×/2× and GNOME dual-monitor regressions pass. Sway's fixture additionally checks
1×/1.5× at a negative origin: it asserts compositor output geometry, disjoint crop
sizes, byte-for-byte equality with corresponding full-portal rectangles, and the
color distinction across their shared boundary. Its source can contain resampling
and earlier rendered content, so uniform wallpaper color across every edge is not
assumed. The complete Sway/IME fixture passes with these checks.

All-feature workspace tests, focused geometry tests, all-target Clippy, formatting,
Python syntax and diff checks pass. These are representative fractional-scale
cases, not every scale/rotation combination or a KDE hotplug/IME acceptance claim.

## Output disable/re-enable while editing

The `--hotplug` extension passed with both scales 1/2 and 1/1.5. It uses KScreen to
disable and re-enable a connected virtual output. This exercises the compositor's
logical output lifecycle; it is not a physical cable-removal or GPU-driver test.

The test opens the second-screen editor, disables that output, and reads both native
window geometry and the KWin fullscreen state flag. The window moves to the remaining
output and remains fullscreen. Its captured image is still exported with the original
second-screen dimensions/colors even after the synthetic background process is stopped.
A fresh capture sees only the remaining screen, and requesting screen number 1 returns
status 1 with empty stdout and the out-of-range diagnostic. Re-enabling the output
restores correct fresh screenshot dimensions/colors.

A second case disables the output while the monitor chooser is open. Choosing that
screen from the already-captured snapshot still preserves its image and stays fullscreen
on the remaining output. The helper now explicitly checks the fullscreen protocol flag,
so a merely screen-sized floating window would not satisfy the check.

```sh
python3 tests/rust/kde_smoke.py target/debug/flameshot target/debug/examples/kde_input --hotplug
python3 tests/rust/kde_smoke.py target/debug/flameshot target/debug/examples/kde_input --hotplug --fractional
```

Logs and layout records are in `target/kde-hotplug.log`,
`target/kde-hotplug-fractional.log` and the matching artifact directories. The hotplug
consent driver polls at one-second intervals: an earlier faster attempt received an
actual portal cancellation after reconfiguration and was counted as a failed run,
not silently retried. Both final modes pass; application cancellation handling was
not changed. No production application code needed modification for these cases.
Helper compilation/Clippy, formatting, Python syntax and diff checks pass. No test
desktop processes remained, and the disposable container was stopped and removed.

## Reproduce

Build the application and guarded input helper:

```sh
cargo build --locked -p flameshot --all-features
cargo build --locked -p flameshot-platform --example kde_input
```

On a complete KDE 5 test system with its normal runtime packages:

```sh
python3 tests/rust/kde_smoke.py target/debug/flameshot target/debug/examples/kde_input --clipboard
```

The script creates private XDG trees, a private session bus, a virtual 1024×768
KWin output and a solid-color Qt Wayland background. The input helper requires
`FLAMESHOT_TEST_KDE_INPUT=1`; only its disposable Desktop Entry requests the
`org_kde_kwin_fake_input` and window-management interfaces. The fixture does not disable KWin interface
permission checks globally. It uses an explicit software OpenGL compositor;
QPainter cannot provide the KWin screenshot effect.

For the minimal container used here, the runtime preparation was:

```sh
apt-get install -y --no-install-recommends \
  kwin-wayland kwin-wayland-backend-virtual xdg-desktop-portal-kde \
  xdg-desktop-portal dbus-x11 libglib2.0-bin pipewire \
  python3-pyqt5 python3-pil qtwayland5 libgl1-mesa-dri \
  plasma-framework plasma-workspace-data
```

These commands were run **inside the disposable container**, not on the host.
Ubuntu's KWin executable carried `cap_sys_resource=ep`, which the container's
capability bounds reject at exec. Removing that file capability inside the
container (`setcap -r /usr/bin/kwin_wayland`) allowed unprivileged virtual rendering;
no host/container capability was added.

The minimal installation also needed QML and package plugins supplied by the
`plasma-workspace` package. Instead of starting a complete Plasma session, its
5.27.12 package was downloaded with APT and extracted into
`/validation/workspace-extracted`. Set these paths when running the fixture in
that container:

```sh
QML2_IMPORT_PATH=/validation/workspace-extracted/usr/lib/x86_64-linux-gnu/qt5/qml \
QT_PLUGIN_PATH=/validation/workspace-extracted/usr/lib/x86_64-linux-gnu/qt5/plugins \
FLAMESHOT_TEST_ARTIFACT_DIR=/validation/kde-artifacts \
python3 /validation/kde_smoke.py /usr/bin/flameshot /validation/kde_input --clipboard
```

No real terminal, browser, uploader or user desktop is invoked. Remaining KDE
coverage includes native IME, additional scale/rotation combinations and negative-origin
outputs, physical hardware removal and installed sandbox behavior. The plain `screen` command cannot
infer the pointer's monitor on this Wayland backend; the existing diagnostic asks
for `--number`. Hosted CI has not run this fixture.
