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
`org_kde_kwin_fake_input` interface. The fixture does not disable KWin interface
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
coverage includes native IME, mixed-scale/negative-origin
outputs, hotplug and installed sandbox behavior. The plain `screen` command cannot
infer the pointer's monitor on this Wayland backend; the existing diagnostic asks
for `--number`. Hosted CI has not run this fixture.
