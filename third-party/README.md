# Patched Rust dependencies

`winit/` is the crates.io source of winit 0.30.13 (upstream licenses are retained).
The upstream crate SHA-256 is
`a6755fa58a9f8350bd1e472d4c3fcc25f824ec358933bba33306d0b63df5978d`.
The sole local source change is in `src/platform_impl/linux/x11/event_processor.rs`:
ignore releases of legacy X11 wheel buttons 4–7. Upstream emits a scroll for both
press and release; events delivered on separate frames cause two zoom steps.
The private-Xvfb localization fixture exercises a complete XTest wheel click and
requires exactly one 10% zoom, with no sleeps or time-based event suppression in
the application. Ordinary button releases and XI2 axis scrolling are unchanged.
Remove the Cargo patch once an upstream version provides equivalent handling.
