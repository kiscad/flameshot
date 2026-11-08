# Private GNOME layout and clipboard validation

Environment: GNOME Shell `46.0-0ubuntu6~24.04.17`, Mutter
`46.2-1ubuntu0.24.04.18`. The all-feature debug executable tested here has SHA-256
`6ef45666dd2dfe677abaf0517ff65584e6a9bbc3bfbab1def56a07424c440988`.

The fixture creates its own GNOME compositor, bus routing, XDG directories,
permission store and temporary extension for virtual input/evaluation. No layout
or GSettings call addresses the user's desktop. Mixed modes enable fractional
framebuffer scaling only in the private keyfile settings and apply temporary
monitor configurations through Mutter's DisplayConfig interface.

| Case | Logical layout (primary first) | Portal PNG size | Result |
| --- | --- | --- | --- |
| Baseline 1x/1x | 1024x768 at (0,0); 640x480 at (1024,0) | 1664x768 | Pass |
| Mixed 2x/1x | 1024x768 at (1280,0); 1280x960 at (0,0) | 4608x1920 | Pass |
| Fractional ~1.5x/1x | 1364x1023 at (1280,0); 1280x960 at (0,0) | 3970x1536 | Pass |

The fractional scale is Mutter's advertised `1.5014662742614746`, not an assumed
exact 1.5. Output modes are 2048x1536 and 1280x960 for both mixed cases. The fixture
asserts the actual scale and logical layout, then obtains independent GNOME Shell
screenshot dimensions as the capture reference. It checks full capture dimensions,
per-output pixel partition sizes/colors, native Wayland windows, both monitors'
fullscreen rectangles, explicit/interactive monitor choice, crop export and cancel.
Real screenshot permission refusal must produce status 1 with no PNG output.

Mutter rejects a logical monitor at (-1280,0) with InvalidArgs. The fixture verifies
that the rejected request leaves the complete display state unchanged. It then
normalizes the layout to put the secondary at (0,0) and primary at (1280,0). This
verifies a monitor left of the primary; it does not pretend GNOME accepted negative
global origins. Negative global origins are exercised separately on wlroots/KWin.

Fractional mode with `--clipboard` also passes PNG persistence after capture exit,
editor Ctrl+C after window/process exit, JPEG decode and Unicode text delivery into
a native GTK4 Wayland receiver via GNOME's Xwayland clipboard bridge. This does not
establish an input-method test: GNOME Chinese IME remains pending. The basic
no-Xwayland fixture suppresses the known failing IBus startup in that environment.

Commands:

```sh
python3 tests/rust/gnome_smoke.py target/debug/flameshot --multi-monitor
python3 tests/rust/gnome_smoke.py target/debug/flameshot --mixed-scale
python3 tests/rust/gnome_smoke.py target/debug/flameshot --fractional-scale --clipboard
```

Local evidence: `target/gnome-layout-baseline.log`,
`target/gnome-mixed-scale-final.log`, `target/gnome-fractional-clipboard-final.log`,
`target/gnome-mixed-scale-artifacts/` and `target/gnome-fractional-artifacts/`.
The final mixed/fractional runs include explicit primary and secondary fullscreen
placement assertions. Their compositor/process groups were cleaned up by the fixture.
