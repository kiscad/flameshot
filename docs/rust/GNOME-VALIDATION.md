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
a native GTK4 Wayland receiver via GNOME's Xwayland clipboard bridge. The separate real IBus input-method cases below also pass. The basic
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

## Real IBus Pinyin

`--ime` passed on the single-output layout and together with `--fractional-scale`.
It selects the real libpinyin engine, types `nihao` with the compositor's virtual
keyboard, observes visible GNOME preedit `你好`, and commits through Space. It then
copies committed text to a native GTK4 Wayland receiver, composes `zhong`, cancels
with Escape and verifies that the preedit is hidden and committed text unchanged.
Finally it applies the text annotation and exports a 400x200 image containing the
Chinese glyphs. The ordinary two-monitor fixture also passes after these additions.
Cloud input is explicitly configured and read back as disabled.

The first test click hit the toolbar's font-family field rather than the canvas.
Screenshots identified that fixture error; the corrected region/click lies below
the toolbar, and successful annotation export verifies the intended text tool.
No application input handling was changed.

This workstation lacks an installed Pinyin engine. Ubuntu packages
`ibus-libpinyin 1.15.7-1build2`, `libpinyin15/libpinyin-data 2.8.1-1build3` and
`proot 5.1.0-1.3` were extracted into a test directory. PRoot maps only the engine's
hard-coded dictionary/resource paths for that child process; the compositor,
application and IBus protocol are native. Schemas, component XML, engine caches and
configuration remain test-local, and the private session uses the installed IBus
1.5.29 daemon. No host package or input-source preference was changed.

Example preparation on the tested Ubuntu x86_64 environment (normal engine runtime
libraries must already be available):

```sh
mkdir -p target/ibus-test-tools/downloads target/ibus-test-tools/root
(cd target/ibus-test-tools/downloads && apt-get download ibus-libpinyin libpinyin15 libpinyin-data proot)
for package in target/ibus-test-tools/downloads/*.deb; do
    dpkg-deb -x "$package" target/ibus-test-tools/root
done
glib-compile-schemas target/ibus-test-tools/root/usr/share/glib-2.0/schemas
FLAMESHOT_TEST_IBUS_ROOT="$PWD/target/ibus-test-tools/root" \
    python3 tests/rust/gnome_smoke.py target/debug/flameshot --ime --fractional-scale
```

With the engine installed normally, omit `FLAMESHOT_TEST_IBUS_ROOT`. The test's
portable-root mode is x86_64-specific and is not an application runtime dependency.
Evidence: `target/gnome-ime.log`, `target/gnome-fractional-ime.log`,
`target/gnome-ime-baseline.log`, `target/gnome-ime-artifacts/` and
`target/gnome-fractional-ime-artifacts/`. No private engine process remained after
fixture cleanup.
