# Rust migration status

Baseline: `f4f2e4e8`. Branch: `rewrite/rust`.

The completion target is the complete Linux application, not a core-library prototype.
Windows and macOS implementations follow later. Application and UI code must be Rust;
OS APIs and necessary native dependencies are allowed. Existing interaction patterns,
CLI, D-Bus and INI compatibility remain requirements.

## Architecture

Cargo workspace: platform-independent core, Linux platform adapters, desktop UI,
and application entrypoint. Use egui/winit/wgpu for UI, image/tiny-skia/cosmic-text
for composition, ashpd/zbus for portals and IPC, x11rb for direct X11 capture,
and ksni for the tray. Platform interfaces keep future OS support separate.

## Acceptance matrix

| Area | Required evidence | Status |
| --- | --- | --- |
| Core geometry | Negative origin, fractional scale, clipped selection tests | 5 tests passed; fmt and Clippy passed |
| Capture | Portal and direct X11; full/screen/gui; cancel/refusal/timeout | X11 root capture implemented; Portal and application integration pending |
| Editor | All existing tools, selection, undo/redo, magnifier, color picker | Partial: native editor and basic tools; advanced tools/options pending |
| Rasterization | Shared preview/export, text shaping, secure pixelation tests | Pending |
| Clipboard | Image/text transfer after editor closes on X11 and Wayland | X11 image transfer verified; native Wayland pending |
| Daemon | Single instance, pins, tray, idle lifetime | Partial: service, pin windows and tray code; lifecycle/parity tests pending |
| CLI | Existing commands/options, stdout PNG, exit codes | Partial: full/gui/screen/config; remaining options and launcher pending |
| D-Bus | Existing service/path/methods and QDataStream payload fixtures | Pending |
| Configuration | QSettings INI and QVariant round trips; unknown keys preserved | Pending |
| Resources | Existing icons and 49 translation catalogs; locale fallback | Catalogs embedded and tested; full UI translation coverage pending |
| Desktop | Notifications, autostart, launcher, configuration windows | Pending |
| Network | Optional Imgur and history, configurable update checking; mock tests | Pending |
| Packaging | Debian/RPM/Arch/Flatpak/Snap/Nix Cargo migration | Pending |
| CI | fmt, Clippy, tests, default/optional builds, release build | Pending |
| Desktop validation | X11, GNOME/KDE Wayland, wlroots, mixed DPI, IME | Pending |
| Final removal | Remove C++/Qt/CMake after complete Linux acceptance | Pending |

## Compatibility facts from source

- Binary name: `flameshot`; commands: `gui`, `full`, `screen`, `launcher`, `config`.
- Exit codes: success 0, general 1, cancelled 2, D-Bus connection 3; signals 128+signal.
- D-Bus service/interface: `org.flameshot.Flameshot`, object `/`.
- Methods: `captureScreen`, `attachScreenshotToClipboard(ay)`,
  `attachTextToClipboard(s,s)`, `attachPin(ay)`.
- Image byte arrays contain QDataStream-encoded pixmaps, not bare PNG files.
  Pin messages append QRect and optional device-pixel ratio.
- INI tool button data can contain escaped QVariant/QList<int> data.
  Tool numeric IDs must remain stable.
- Linux GUI chooses a monitor before editing; full captures the whole desktop.
- Secure pixelation derives replacement colors from the fringe, not the interior.
- Existing shell tests require desktop interaction; they are not automated CI proof.

## Execution rules

Keep the C++ implementation available as a behavioral reference until Linux parity
is established. Use isolated XDG directories and private D-Bus sessions for tests.
Do not publish or replace the user's installed application during development.
Record actual commands and results; compilation is not desktop validation.

## Validation log

Initial workspace: `cargo fmt --all`, `cargo test --workspace` (5 passed),
`cargo clippy --workspace --all-targets -- -D warnings` passed.
At initial workspace validation no capture or GUI existed.

X11 backend added: root-window capture with TrueColor mask decoding, scanline
padding, byte order and allocation limits. `cargo test --workspace` passed 9 tests;
`cargo clippy --workspace --all-targets -- -D warnings` passed. These validate
synthetic pixel conversion, not a real X11 desktop capture. Portal implementation
is next; ashpd 0.13.13 source was inspected (its send operation awaits the response,
so cancellation/timeout must account for closing the outstanding portal request).
GUI, clipboard, compatibility and packaging remain pending.

Portal request lifecycle implemented in `flameshot-platform`: subscribe before
request, distinguish cancellation/rejection, bound timeout and Close cleanup,
validate local URI including percent-encoded Unicode. This narrow adapter uses
zbus directly because ashpd's Screenshot send awaits the response before exposing
the request handle; explicit request cleanup is needed on timeout. Other portal
integrations may still use ashpd. Image decoding and desktop geometry integration
remain pending. Dropping the future externally is not yet a supported cancellation
API; the current implementation handles portal cancellation and timeout.

Validation: 10 workspace unit tests passed. The isolated integration command
`FLAMESHOT_TEST_PRIVATE_BUS=1 dbus-run-session -- cargo test -p flameshot-platform --test portal -- --ignored`
passed, verifying a response emitted before the method reply, cancellation,
rejection, and timeout causing exactly one Close call. Clippy with warnings denied
passed. This is simulated portal verification, not actual compositor validation.

Rust executable added with `full -p/--path -r/--raw -d/--delay`, version and help.
The capture path is Portal first with direct X11 fallback only for unavailable
portal interfaces/services outside Wayland. PNG stdout is separate from stderr;
PNG/JPEG save uses exclusive file creation with collision suffixes. Known images
are round-tripped in tests. `full` CLI options and filename behavior are not yet
fully compatible: clipboard, INI settings, other image codecs, custom filename
patterns and all other commands are still pending. No-argument daemon startup
is also pending. The current application is not ready to replace Flameshot.

Verification commands are collected in `scripts/check-rust.sh`, including separate
private D-Bus sessions for both lifecycle tests and executable output tests.

Editor core added: immutable source image, ordered pencil/line/arrow/rectangle/
ellipse annotations, inversion and secure pixelation, add/replace/remove commands,
bounded undo history, redo invalidation and selection export. Raster paths use
tiny-skia with explicit premultiplied-alpha composition into RGBA images. Preview
and export can call the same `Document::render` pipeline. GUI integration is pending.
Text, counter, marker-specific behavior, curved arrows, rounded corners and other
advanced tool options still need implementation. Pixelation currently uses a
stable fringe-color mosaic rather than reproducing the C++ noise distribution;
it preserves the interior-independence requirement, including edge/full-image cases.

Five new editor tests passed: raster/export equality and undo restoration,
replacement/removal history, redo invalidation, secure redaction independence,
alpha compositing/inversion and invalid input/history bounds. Core has 10 passing
tests total. Workspace Clippy passed with warnings denied.

Initial native editor wired to `gui -p/-r/-d` using eframe's egui/winit/wgpu
integration. It displays the captured image, supports drag selection and existing
core drawing tools, colors, widths, undo/redo, accept/cancel and selected-image
export through the CLI output pipeline. Escape cancels; Enter accepts; Ctrl+Z and
Ctrl+Shift+Z navigate history. A selection-coordinate test covers reverse dragging
and fractional edges. `cargo check --workspace` and Clippy passed after integration.

This is an initial UI, not parity: image currently fits below a top toolbar,
monitor selection, true desktop overlay coordinates, floating toolbar/side panel,
live stroke preview, text/IME, localization, object manipulation, clipboard and
settings are pending. The UI has not yet been exercised on a compositor. No
screenshots or GUI usability claims can be inferred from compilation.

Lossless INI source storage and targeted string updates added in core. Existing
comments, line endings, unknown values and Qt-serialized entries are preserved.
Supported string decoding includes quoting, backslash/control escapes and hex
escapes. Full QSettings compatibility (lists, all escapes, escaped keys and typed
QVariant decoding) is still pending; typed values are not misread as plain strings.

Application now reads the legacy XDG flameshot/flameshot.ini location for savePath,
filenamePattern, saveAsFileExtension and jpegQuality. Output uses those settings,
with explicit CLI paths retaining priority. JPEG encoding uses configured quality.
Validation rejects malformed quality, unsupported format and unsafe filename paths.
No user configuration files are written. Test CLI processes use an isolated
XDG_CONFIG_HOME. The remaining configuration keys, config CLI/UI, hot reload and
Qt-compatible typed serialization remain outstanding.

Text annotations now use cosmic-text advanced shaping and system-font fallback in
the shared document renderer. Font discovery and glyph rasterization are cached
per document. Multi-line annotations support position, font size/family and RGBA
color, participate in undo/redo, and are baked into exported pixels. GUI includes
a multiline text field and font-size control; clicking the image places the text.
Global Enter/undo shortcuts defer to focused text widgets to allow newline entry
and normal text editing. Existing annotation editing, full IME interaction testing,
font-family configuration UI and translated controls remain pending.

Text raster test requires an installed sans-serif font (Noto Sans is available in
the development environment); it checks both text lines, alpha, export equality,
undo and redo. Future CI must provision fonts explicitly.

Real X11 smoke validation now exists in `tests/rust/desktop_smoke.py`. Xvfb,
xdotool, libxdo3 and libxfont2 packages were extracted under ignored
`target/desktop-test-tools/root` (no system install). The script starts a dedicated
Xvfb and private D-Bus with service activation directories omitted, ensuring host
portal services cannot autostart into the test bus. Initial attempt with default
private-bus configuration timed out because system portal services autostarted;
that was a test-isolation defect, now fixed.

Verified against the actual Rust executable: 1024x768 direct X11 full capture,
visible wgpu editor window, Escape exit code 2 with empty stdout, pointer drag and
Enter acceptance producing a valid 278x279 cropped PNG. This proves basic X11
window interaction only, not GNOME/KDE/wlroots Wayland or multi-monitor behavior.
Command: `python3 tests/rust/desktop_smoke.py target/desktop-test-tools/root target/debug/flameshot`.

X11 platform also now exposes active RandR monitor enumeration and root pointer
position. Screen-command integration and corresponding Wayland monitor mapping
remain pending.

`screen --number/-n`, `--edit/-e`, `--path/-p`, `--delay/-d` and `--raw/-r`
are now wired to platform monitor enumeration and crop mapping. X11 can choose
the monitor containing the pointer. Wayland requires an explicit monitor number
because it does not expose global pointer position. Wayland enumeration uses
wl_output/xdg-output via smithay-client-toolkit, including logical positions and
sizes; absent geometry is reported rather than invented. Screen ordering follows
the backend enumeration. Stable cross-backend numbering/hotplug handling remain
pending. Monitor cropping currently assumes the Portal returns a full-desktop
image, as the old implementation did; portals returning a selected area and
mixed-scale compositing still require backend-specific validation.

Validation: 25 unit tests, both private-bus integration tests, Clippy and format
checks passed. Extended real-X11 smoke test passed explicit screen 0, implicit
pointer screen and out-of-range rejection in addition to full/GUI tests.
On the actual GNOME Wayland session, screen enumeration succeeded and rejected
number 999 before capture. Real `full` and `screen --number 0` Portal captures both
completed successfully with 2560x1440 PNG output in ignored target/ artifacts.
This does not establish multi-monitor, rotated/fractionally scaled layouts, KDE,
wlroots or Wayland GUI/clipboard parity.

Editor UI now uses a movable foreground tool palette over the capture canvas;
controls no longer consume image layout space. Canvas mapping centers and fits the
image to the viewport, with selection shading outside the exported rectangle.
Transient drawing uses `Document::preview` without changing history and commits
on pointer release. Default tool keys P/D/A/S/R/C/T/B/I and Ctrl+A selection are
wired, while text-field keyboard focus retains priority.

New preview test proves equality with committed output and unchanged history.
26 unit tests and both private-bus integration tests passed, plus fmt/Clippy.
Real X11 smoke still passes. The test now explicitly sizes the window to the Xvfb
screen because there is no window manager to honor fullscreen requests; a 200x200
pointer drag exports exactly 200x200 pixels. Prior 278x279/256x257 output was the
expected scaling of the image into a smaller unmanaged test window, not evidence
of a correctly sized desktop overlay. Multi-monitor overlays, custom shortcuts,
live-preview performance, GUI visual review and Wayland interaction remain pending.

Qt wire codec added for legacy clipboard/pin D-Bus payloads: marker + PNG,
QRect inclusive endpoints and optional qreal device-pixel ratio. The parser
bounds payload size, validates chunk lengths, rejects truncated/malformed data,
and supports older pin messages without a scale trailer. Fixtures were generated
with actual Qt 6.4.2 using a synthetic solid-color image; Rust tests validate image
pixels, negative pin coordinates, scale and round trips. This establishes the
payload codec only; D-Bus service registration, clipboard ownership and pin
windows still need implementation. Qt is not a Rust build/test dependency.

Reverse interoperability check also passed: Qt QDataStream read the Rust-reencoded
pin and verified all image pixels, QRect(-20,30,3,2), scale 1.5 and end-of-stream.
28 unit tests and both isolated D-Bus tests passed; fmt/Clippy passed including
the fixture roundtrip example.

No-argument Rust startup now owns org.flameshot.Flameshot at / and stays alive
until termination or bus disconnect. Existing name owners are not replaced or
queued behind. The four legacy method names are implemented: captureScreen
launches an editor child; attachScreenshotToClipboard decodes the Qt fixture
format and retains an arboard owner; attachTextToClipboard also sends an optional
desktop notification; attachPin validates and passes data by stdin to a dedicated
native image window. Child processes are reaped. Pins currently support resize,
drag and Escape/Ctrl+Q closure; zoom and other pin behavior remain pending.

CLI full/gui/screen now accepts -c/--clipboard, starts a daemon if necessary and
copies through the legacy D-Bus method. Explicit save/raw actions may be combined;
clipboard-only no longer implicitly saves a file. Real isolated X11 test verifies
that xclip receives a 1024x768 image/png after the capture process has exited.
All previous smoke and automated checks passed (28 unit tests, two D-Bus tests).
The clipboard backend currently uses X11/XWayland. Native Wayland ownership,
clipboard change/idle tracking, tray, daemon-managed GUI integration, GUI copy
button and full D-Bus method behavioral tests remain pending. No claim of complete
daemon or Wayland clipboard parity is made.

GUI now returns an explicit action with the cropped image and selection. Copy,
Save and Pin buttons are connected to output processing; Ctrl+C and Ctrl+S are
available outside text input. GUI/screen --pin forwards a Qt-compatible message
to the daemon, opening a native pin window. Explicit CLI output tasks combine
with GUI actions, while copy/pin-only actions do not implicitly save.

Real X11 test caught that egui emits Event::Copy rather than a C keypress for
Ctrl+C; handling was corrected. Expanded smoke now verifies a 200x200 selection
copied through Ctrl+C remains available after the editor exits, and a D-Bus pin
window appears and closes with Escape. Existing capture/cancel/export tests pass.
Save-dialog selection/retry behavior, pin placement on scaled/multiple monitors,
pin zoom, and native Wayland clipboard remain incomplete. New CLI-generated pins
currently carry scale 1.0; legacy D-Bus pin decoding preserves supplied scale.

Basic configuration UI and `config --filename/--trayicon/--notifications/--showhelp`
implemented. INI updates read the latest source, preserve unknown settings and
comments, validate changes before writing and atomically replace via a sibling
temporary file. Existing permissions are retained. CLI integration and writer
unit tests prove successful updates and unchanged contents after rejected input.
GUI exposes output settings, notification/help switches and tray visibility.
Config window opens/closes with Escape in real X11 without writing unless saved.
Concurrent writer locking and complete QSettings/schema validation remain pending.

ksni StatusNotifierItem tray is connected to daemon startup, screenshot/config
launch and graceful Quit. Tray visibility is refreshed from configuration once a
second; errors loading configuration leave the current state unchanged. Text-copy
notifications honor the notification setting; capture hint honors showHelp.
Tray registration/menu behavior has not yet been tested against an actual watcher,
and bundled icon fallback, complete menu/history/update actions remain pending.

Validation: 29 unit tests, one config CLI integration test, two private D-Bus
integration tests, Clippy and the expanded real X11 smoke passed. Configuration
window smoke covers appearance and cancel only, not editing every control.

All 49 existing TS catalogs are now parsed by a Rust build script into embedded,
sorted context/source tables; no lrelease or other Qt translation tooling is used.
Unfinished/obsolete entries fall back to source text. Locale resolution supports
encoding/modifier stripping, hyphen normalization and language-level fallback.
Tests verify catalog count, Chinese translations, distinct contexts sharing the
same English source, unknown locale and unfinished fallback. Existing catalogs
contain no plural forms; plural support is not implemented or claimed.

Editor tool/action labels use their original Qt translation contexts. uiLanguage
is loaded from INI (auto honors LC_ALL/LC_MESSAGES/LANG); configuration UI offers
all catalog locales. System CJK/Arabic/Hebrew/Indic/Thai font fallbacks are loaded
for editor and configuration widgets when installed. Newly introduced controls,
help/error strings and tray menu translation coverage remain pending. Language
changes take effect in newly launched processes, not live existing windows.

Tray now includes a 32x32 ARGB fallback pixmap from the existing licensed Flameshot
PNG, independent of an installed icon theme. Watcher/menu verification remains
pending. Validation passed: 31 unit tests, config CLI integration, two private-bus
integration tests and real X11 smoke; fmt and Clippy passed.

Added a private-bus daemon/tray integration test using an actual Rust daemon
process and a simulated StatusNotifierWatcher. It verifies all four legacy method
names through introspection, invalid image/pin message rejection, duplicate startup
exiting successfully without replacing the first owner, tray registration, icon
pixel dimensions/data, menu labels and Quit-event-driven graceful daemon exit.
The test is now part of scripts/check-rust.sh. This verifies protocol behavior;
actual desktop panel appearance and screenshot/config menu activation remain
separate desktop acceptance items.

Enabled arboard's wayland-data-control feature after inspecting its implementation:
clipboard selection prefers wl-clipboard-rs native ext/wlr data-control when the
compositor supports it and otherwise falls back to X11/XWayland. No shell clipboard
utility is used by the application. This code path is compiled but native Wayland
compositor verification and core wl_data_device support are still pending.

Marker tool added with the legacy 35% opacity and multiply composition semantics,
including square caps and width padding. Tests verify dark source pixels remain
dark rather than being painted over. Object manipulation now supports topmost
bounding-box selection, drag translation and Delete, with undo/redo through the
same document history. Marker uses M; object movement uses Ctrl+M. A test verifies
moved output pixels, topmost selection, undo restoration and rejection of NaN moves.

Object hit testing is currently bounding-box based (text bounds estimated), and
movement preview/resize/property editing are not yet complete. Circle counter
implementation remains pending; it was inspected this turn but not added.
Validation passed: 33 unit tests, config integration, three private-bus integration
tests, Clippy and existing real X11 desktop smoke. GUI movement/marker visual
acceptance is still needed beyond these core pixel tests.

Circle Counter implemented in the shared document model: click places a bubble,
drag adds a triangular pointer, numbering increments from current document
contents (bounded at 999), optional contrasting outline and bold centered digits.
Label centering uses actual rasterized glyph bounds. Counter objects participate
in preview, export, hit bounds, movement, undo and redo; undoing an insertion makes
its number available again. GUI exposes the counter and outline toggle.

A new pixel test checks pointer rendering, preview/commit equality, moving and
undoing a counter, number progression and redo. 34 unit tests, config integration,
three private-bus integrations and existing X11 smoke passed; fmt/Clippy passed.
Counter GUI-specific visual acceptance and persistent counter-size/outline settings
are still pending alongside remaining advanced tools and other migration work.

GUI command now supports -s/--accept-on-select and -g/--print-geometry. Valid
selection completion triggers acceptance without Enter when requested. Geometry
is emitted as WxH+X+Y with a newline; --raw takes priority so binary PNG stdout is
never prefixed by geometry. Geometry-only capture does not implicitly save.
Explicit path/copy/pin tasks still combine with acceptance/output options.

Extended real X11 smoke passes auto-accept geometry output exactly
`200x200+100+180`, --raw/--print-geometry precedence and auto-save to an explicit
path, alongside all previous window/clipboard/pin/config cases. Geometry currently
uses capture image pixel coordinates; logical-coordinate semantics for scaled
multi-monitor captures still need compatibility validation. Initial --region,
--last-region, keyboard/mouse selection resizing and remaining CLI options remain
pending.

Selection editing added as a separate core module: bounded movement, edge/corner
resizing, and keyboard adjustments. GUI supports dragging inside an existing
selection to move it, dragging its visible handles to resize, arrow-key movement,
Shift+arrows resizing, and Ctrl+Shift+arrows symmetric adjustment. The unusual
legacy vertical symmetric direction (Ctrl+Shift+Up expands) is preserved from
SelectionWidget. Ctrl+M now selects movement of the screenshot selection; moving
annotation objects remains a separate toolbar tool.

Three core tests cover extreme movement clamping, corner sizing and legacy
keyboard semantics. Extended real X11 smoke combines Right, Shift+Right, Ctrl+M
mouse movement and corner resizing, then verifies exact geometry
`211x205+121+210`. All 37 unit tests, four integration tests and desktop smoke
passed; final handle-rendering addition passed fmt/Clippy. Selection gestures
currently stop at a minimum size of one pixel instead of flipping across edges;
advanced visual/Wayland/multi-DPI acceptance remains pending.

Magnifier and G-key color sampling added from the composed image, with exact RGBA
sampling and clamped pixel neighborhoods. The magnifier uses nearest-neighbor
10x enlargement, a highlighted sampled pixel, hexadecimal color/coordinate label
and square or circular shape; it remains an overlay, excluded from export. The
showMagnifier/squareMagnifier/drawCircleCounterOutline settings are loaded and
editable in configuration. Initial stroke/font sizes now read legacy configuration
(font points converted to 96-DPI logical pixels). Configuration content scrolls so
controls remain reachable in small windows.

New sampling unit test covers exact values and clipped/overflow-radius edges.
38 unit tests and four integrations passed, alongside expanded X11 smoke with
magnifier enabled and G pressed. The synthetic X11 rendered screenshot was viewed
(target/rust-magnifier-x11.png); review prompted adding an outline so the lens stays
visible on black backgrounds. That final visual-only change passed fmt/Clippy.
Color-pick modal UX, persistent last-used colors/sizes and per-monitor text DPI
remain pending; the current G action picks the hovered pixel directly.

Save-dialog work in progress: editor now keeps its document open while browsing
folders/typing a path, cancelling a save, or retrying a failed write. Explicit CLI
paths are saved before closing the editor; the outcome records successful saving
to prevent duplicate files in the CLI output stage. The dialog is Rust egui-based;
Portal file chooser integration for sandboxed filesystem access is still pending.

Expanded X11 regression exposed two issues. Fast shortcut events could lose
Ctrl/Shift when consulting the final frame modifier state; selection/tool/save
handlers now consult modifiers on the key event itself, with a regression test.
The new save retry case still FAILS: after an invalid destination, Ctrl+A then
replacement typing appends instead of replacing the path. Synthetic screenshot
`target/rust-save-debug.png` confirmed the malformed concatenated path. Explicit
TextEdit ID/focus restoration and selection state handling did not yet resolve it.
The save UI must NOT be considered validated until this regression is fixed and
save retry/cancellation cases pass. Existing geometry/capture/clipboard/pin cases
pass; final full desktop smoke is currently red because of save retry.

Next action: inspect egui 0.37 text focus/IME ownership and TextEdit cursor-state
handling around a dialog error, fix path replacement, then rerun the new save tests
and full checks. The first test-edit attempt accidentally sent Enter without
opening the dialog; that test harness issue has been corrected.

Save retry regression RESOLVED. Event tracing showed that synchronous directory
scanning and metadata sorting blocked the UI long enough for Enter and the next
Ctrl+A to arrive together; Enter surrendered text focus before Ctrl+A was handled.
Directory reads now run on a background thread, are cached by folder, precompute
metadata once and use virtualized visible rows. The speculative custom Ctrl+A
selection workaround and temporary input tracing were removed; standard TextEdit
selection now works. Pending folder changes clear old rows instead of presenting
stale entries. Input focus is restored after a rejected save.

Real X11 smoke passes invalid destination -> replacement path -> successful save,
checks no duplicate retry_1.png is created, and verifies cancelling a save returns
to the same exportable 200x200 selection. All prior desktop scenarios also pass.
The shortcut-event regression test passes, including texture-delta cleanup required
by the egui test harness. Automated checks pass (39 unit tests and four integration
tests). The final stale-list clear is a UI-only follow-up checked by fmt/Clippy.

The Rust file chooser is functional outside a sandbox. Portal-mediated filesystem
access, format dropdown parity, saved-path persistence and savePathFixed behavior
still require work; the overall migration remains incomplete.

Linux autostart now uses the historical autostart/Flameshot.desktop filename to
avoid duplicate startup entries. Enable atomically installs a desktop file with
Exec=flameshot; disable removes only that application entry. Configuration reads
the entry's actual existence, and config -a/--autostart plus the UI switch update
both the desktop entry and INI setting (with rollback if the INI write fails).
The binary must be installed on PATH for login launching, as in the old version.
No user's real autostart settings were changed during verification.

Screenshot completion notifications now use org.freedesktop.Notifications with
an application desktop-entry hint, bounded request time and nonfatal service
errors. Save/copy/pin completion respects the notification switch. A private-bus
mock verifies the exact Notify signature, Unicode body and returned notification
ID. Autostart tests cover enable/disable idempotency and unrelated-entry retention;
CLI tests run in isolated XDG directories.

Validation passed: 40 unit tests, two config CLI tests and four isolated D-Bus
tests, fmt/Clippy and the full real X11 smoke including save retry/cancel. Real
login/session autostart and notifications across all target desktops remain
acceptance items, not inferred from filesystem/protocol tests.
