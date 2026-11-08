# Linux editor compatibility audit

Baseline: `f4f2e4e8`. Scope: the existing Linux capture/editor tools, selection,
history and input contracts. This is functional compatibility; exact Qt raster
output is not a requirement. Compositor placement, font/IME environments and package
installation retain their separate acceptance gates.

The frozen `legacy-editor-tools.tsv` and `legacy-editor-shortcuts.tsv` fixtures
preserve 23 configurable toolbar IDs and 42 default shortcut entries independently
of the Rust implementation. Unit tests compare the full sets/order and drawing-tool
ID mappings. Internal values, optional Imgur and Linux Delete behavior are documented
alongside the fixtures. Existing INI/QVariant button lists retain their numeric IDs.

| Behavior | Rust implementation | Verification |
| --- | --- | --- |
| Pencil, line, arrow, outline/filled rectangle, ellipse | `editor.rs`, `constraints.rs`, `gui.rs` | Core preview/export and visible-geometry picking tests; native constraints/drawing and arrow-style fixtures |
| Straight/curved arrows, filled/reversed heads; rounded rectangles | Shared render options in core/editor and settings | Core head/corner tests; `arrow_style_smoke.py`, `global_render_smoke.py` |
| Multiply marker, inversion, secure/insecure pixelation | Shared core raster pipeline | Multiply/alpha/edge/interior-independence tests; native pixelation preview/export checks |
| Text, editing existing text, font/size, bold/italic/underline/strikeout, alignment | `text.rs`, editor text draft and configuration | Core shaping/decoration/multiline tests; native text editing, `text_format_smoke.py`; IME environment coverage remains in desktop validation |
| Counter values, outline, pointer, manual next value and renumbering | Counter annotation and editor override | Core counter/undo/delete tests; native counter-delete and Ctrl-wheel cases |
| Capture rectangle selection, movement, resize, select-all, cancellation | Core `selection.rs`, editor state | Frozen Qt geometry fixtures; native keyboard geometry, region/cache and cancellation tests |
| Object picking, movement, property edits, ordering, deletion | Core document transactions and layer panel | Core pixel-hit/near-hit/alpha tests; native object movement, properties, layers and text editing |
| Undo/redo, edit grouping, redo invalidation, configurable history limit | Core document history and transactions | Core no-op/cancel/live-edit tests; native undo-limit/property/layer cases |
| Decimal size entry and per-tool size limits | `size_keyboard.rs`, core annotation size properties, editor size dispatch | Unit bounds/reset tests; `size_keyboard_smoke.py`, native wheel/size persistence |
| Wheel sizing and counter number adjustment | `wheel.rs`, editor per-frame input guard | Discrete/throttled input tests and native wheel cases; physical touchpad hardware is not claimed |
| Palette, alpha, rendered color picking, magnifier/held-button precision | Editor palette and color-grab state | Native palette/alpha/magnifier cases, `color_grab_smoke.py`; shared-renderer pixel comparisons |
| Grid and constrained drawing | Core constraints and editor grid mapping | Core constraints; `drawing_grid_smoke.py`; Sway 1.5×/2× negative-origin grid/export cases |
| Save/copy/accept/pin/open-with/upload/cancel | Editor outcome dispatch and platform adapters | Native save retry/cancel/collision, clipboard lifetime, pins, Open With/portal and native upload fixtures |
| Default/custom/multi-stroke shortcuts, explicit default replacement | `shortcuts.rs` | Frozen Linux default map; dispatch/modifier/release tests; native plain/Meta/default-conflict shortcuts |
| Configurable toolbar order and optional actions | `buttons.rs`, editor toolbar | Frozen IDs/priority; INI list tests; native filtered toolbar and feature-gated upload |
| Quit confirmation and geometry overlay | Editor close-request/dialog and overlay state | Native default-No/confirmed/disabled quit cases; overlay timing and exclusion from export |

Primary native suites are `desktop_smoke.py`, the focused fixtures listed above,
and `wayland_smoke.py`; each uses private displays/buses/configuration. Tool and
render tests are in the workspace's normal test suite. The Fedora installed-X11
run also confirmed the tool/shortcut paths after correcting the fixture's accidental
Alt modifier for function keys; application shortcut semantics did not change.

The complete local Rust gate and fresh arrow-style, text-format and keyboard
geometry native fixtures passed. Together with the full X11/Fedora and Sway
regressions recorded in MIGRATION.md, this closes the software editor inventory
audit. It does not imply physical touchpad equivalence,
all compositor/IME combinations, identical text rasterization to Qt, or installed
sandbox integration. Those boundaries remain explicit in MIGRATION.md.
