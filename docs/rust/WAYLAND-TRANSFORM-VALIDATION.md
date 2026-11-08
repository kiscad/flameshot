# Wayland output transforms and installed Fedora runtime

`tests/rust/wayland_smoke.py` now checks output transforms at 2x scale beside a 1x
output, with a negative horizontal origin. An asymmetric four-color wallpaper with
an off-center stripe prevents orientation errors from hiding behind a uniform image.
The fixture waits for a valid nonuniform reference before testing Flameshot.

For each transform it checks:

- Logical output dimensions and origin reported by Sway.
- Independent per-output `grim` reference dimensions and distinct quadrant colors.
- Full-desktop capture dimensions and exact placement/pixels of the transformed output.
- Single-screen capture dimensions and exact reference pixels.
- Native editor placement on that output and exact cropped export pixels.

Ordinary 90/180/270-degree rotations pass on the extracted Sway 1.9/Pixman stack.
All seven non-normal Wayland transforms, including four mirrored variants, pass on
Fedora 44's Sway 1.11 and wlroots 0.19.3, enabled with
`FLAMESHOT_TEST_MIRRORED_OUTPUTS=1`. Both full suites also pass their remaining
clipboard, pin, monitor selection, close-confirmation, fractional-scale boundary,
grid and precision-color cases. IME was not enabled in these runs.

The older stack produced a uniform gray reference for mirrored outputs even in a
minimal Sway/grim probe without Flameshot. Its software GLES2 alternative could not
start in the device-free environment. These observations do not prove that every
Sway 1.9 renderer has the same limitation; they explain why the older fixture runs
ordinary rotations while the newer stack provides mirrored-output acceptance.

## Fedora environment

The tested application was the installed RPM from source `47e1e77e`, with executable
SHA-256 `20bcf02d0884e5ff664b1c6092bebef67726f848bb267bd5d0d7409b24f49c09`.
The Fedora image ID is
`sha256:15da97ed9aa94dd3d92c3925eb89520f6c80e07ea7ee24cba848d1f8b6912b3e`.
The container had no host mounts and used Docker init plus an unprivileged test user.
It installed Sway, swaybg, grim, wtype, wl-clipboard, both portal backends, PipeWire,
Pillow and Mesa Vulkan drivers through DNF. Only the test script and Rust virtual
pointer helper were copied in.

Fedora's Sway executable has `cap_sys_nice`, which cannot execute under this Docker
capability bounding set. The capability was removed from Sway **inside the disposable
container** before running the unprivileged, headless Pixman fixture; no host setting
or Flameshot package file changed. Post-run `rpm -V flameshot` was empty. The
container was removed after copying artifacts.

Local evidence: `target/wayland-output-transforms-final.log`,
`target/sway-output-probe.log`, `target/sway-output-probe-fedora.log`,
`target/fedora-wayland-transforms.log` and
`target/rpm-install-validation/fedora-wayland-transforms/` (report and images).

This establishes installed Fedora wlroots capture/editor/clipboard behavior and
the transform cases above. It does not imply all GNOME/KDE/IME combinations or
physical display hardware. Those remain separately tracked desktop acceptance.
