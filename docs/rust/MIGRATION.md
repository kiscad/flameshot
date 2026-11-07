# Rust migration status

Baseline: `f4f2e4e8`. Branch: `rewrite/rust`.

The completion target is the complete Linux application, not a core-library prototype.
Windows and macOS implementations follow later. Application and UI code must be Rust;
OS APIs and necessary native dependencies are allowed. Existing interaction patterns,
CLI, D-Bus and INI compatibility remain requirements.

## Architecture

Cargo workspace: platform-independent core, Linux platform adapters, desktop UI,
and application entrypoint. Use egui/winit/wgpu for UI, image/tiny-skia/cosmic-text
for composition, zbus for portals and IPC, x11rb for direct X11 capture/clipboard,
and ksni for the tray. Platform interfaces keep future OS support separate.

## Acceptance matrix

| Area | Required evidence | Status |
| --- | --- | --- |
| Core geometry | Negative origin, fractional scale, clipped selection tests | Core tests and mixed-scale crop tests pass; final desktop placement audit remains |
| Capture | Portal and direct X11; full/screen/gui; cancel/refusal/timeout | Implemented; mock portal, Xvfb and Sway cases pass; other compositor acceptance remains |
| Editor | All existing tools, selection, undo/redo, magnifier, color picker | Drawing/text editing, rendered picking, live movement and selected color/size implemented/tested; remaining object-editing parity pending |
| Rasterization | Shared preview/export, text shaping, pixelation tests | Shared renderer and secure/insecure pixelation tests pass; exact Qt raster equivalence is not a goal |
| Clipboard | Image/text transfer after editor closes on X11 and Wayland | PNG/JPEG/text, X11 INCR and ownership lifecycle verified on Xvfb/Sway; other compositor paths remain |
| Daemon | Single instance, pins, tray, idle lifetime | Idle ownership, direct-child shutdown and launcher descendants tested; independent CLI/sandbox lifecycle audit remains |
| CLI | Existing commands/options, stdout PNG, exit codes | Commands, configuration flags, completions and manual implemented/tested; final syntax/exit-code and shell acceptance remain |
| D-Bus | Existing service/path/methods and QDataStream payload fixtures | Signatures, singleton behavior and real Qt payload fixtures tested; final compatibility audit remains |
| Configuration | QSettings INI and QVariant round trips; unknown keys preserved | All Linux legacy keys have read paths; typed fixtures/atomic updates tested; live-reload/semantic gaps remain |
| Resources | Existing icons and 49 translation catalogs; locale fallback | Catalogs embedded and tested; full UI translation coverage pending |
| Desktop | Notifications, autostart, launcher, configuration windows | Implemented with isolated service/window tests; full desktop integration acceptance remains |
| Network | Optional Imgur/history and update checking; mock tests | Local HTTP pipelines and preferences tested; complete interactive mocked upload/history coverage remains |
| Packaging | Debian/RPM/Arch/Flatpak/Snap/Nix Cargo migration | Debian/RPM/Arch locally validated; Flatpak/Nix/Snap definitions migrated; sandbox/provider builds and clean-distro installs pending |
| CI | fmt, Clippy, tests, optional builds, release build | Rust workflow added; local gates pass; hosted workflow unverified and final release rebuild pending |
| Desktop validation | X11, GNOME/KDE Wayland, wlroots, mixed DPI, IME | Xvfb and native Sway/mixed-DPI suites pass; full GNOME/KDE and IME coverage pending |
| Final removal | Remove C++/Qt/CMake after complete Linux acceptance | Pending; legacy source remains a reference, so this is not yet a pure-Rust-only repository |

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

Region CLI implemented: gui/screen --region accepts legacy numeric separators,
negative coordinates/dimensions, all, and (for gui) screen<N>. Negative dimensions
normalize like Qt; screen command rejects --region screen<N> as before. Explicit
region takes precedence over gui --last-region. GUI begins with the requested
selection; noninteractive screen capture crops before output. Out-of-image regions
are reported instead of producing an empty image.

Actual Qt 6.4.2 was used to confirm cache location
$XDG_CACHE_HOME/flameshot/flameshot/region.txt and negative-width normalization.
Rust reads/writes the existing 16-byte big-endian QRect endpoints atomically.
Successful GUI captures update the cache; missing caches simply start unselected.
Malformed cache data is reported. No user's real cache was changed in tests.

Two new unit tests cover parser validation, negative dimensions, clipping and
roundtrip against the existing actual-Qt fixture. Extended X11 smoke verifies
screen --region exports 40x30 pixels, preset GUI selection outputs
80x60+20+30, --last-region restores it, and cache bytes exactly match Qt endpoints.
42 unit tests, six integration cases and full X11 smoke passed; fmt/Clippy passed.
Global logical coordinates vs local captured-image pixels on mixed-DPI/multiple
outputs remain a compatibility gap requiring dedicated platform validation.

Capture launcher implemented and exposed via `flameshot launcher` and tray menu.
It offers region/full capture, output selection, initial rectangle (loaded from
legacy region cache), countdown delay, clipboard and optional save path. It builds
argument vectors without a shell, hides before capture with the legacy 600 ms
settling delay, reaps its capture child and returns after completion/cancellation.
The launcher refreshes the last accepted rectangle when the child finishes.

Argument construction has a unit test including a path containing spaces and
literal shell metacharacters. Extended real X11 smoke opens the launcher, starts
an editor, cancels that capture and verifies launcher restoration and closure.
Tray protocol test now checks the new menu entry. 43 unit tests, six integration
cases, fmt/Clippy and full X11 smoke passed. Countdown timing on real desktop
compositors, monitor hotplug/default-active-monitor selection, full translation
coverage and other platform acceptance remain outstanding.

Optional Imgur transport added behind the default-disabled `imgur` feature on the
application/platform crates. It sends PNG multipart uploads with anonymous Client-ID
authentication, validates returned image links/IDs/deletion tokens, and implements
confirmed API deletion. TLS uses rustls; requests have connection/overall timeouts,
redirects are disabled and response parsing is capped. Reqwest errors have URLs
removed to avoid exposing deletion tokens through error messages.
API behavior was checked against the [official Imgur documentation](https://apidocs.imgur.com/).

Three localhost HTTP mock tests verify PNG/authentication/multipart contents,
anonymous deletion tokens, DELETE requests and failure handling (HTTP 429, API
failure and malformed JSON). No images were uploaded to an external service.
Both default and all-feature workspace tests and Clippy pass; check-rust.sh now
includes the optional-feature matrix. The all-feature unit count is 46 (default
43), with six existing integration cases. Upload UI, confirmation, progress,
history and deletion interactions are NOT connected yet and remain required work.

Optional upload UI now connects selected-image preview, explicit confirmation,
asynchronous request status and success/error display to Imgur transport. Upload
success is retained even if writing history fails. Copy-link actions use the daemon
text-clipboard method so ownership survives the dialog. The imgur feature adds a
history command/tray entry with links, thumbnail previews and confirmed remote
deletion; local history is removed only after successful API deletion.

History uses the legacy $XDG_CACHE_HOME/flameshot/history directory and
imgur-<deletehash>-<image-id>.png filenames. It reads older entries without tokens,
creates at-most-250x100 thumbnails, writes atomically and enforces uploadHistoryMax
without deleting unrelated files. uploadClientSecret is read as the existing
anonymous Client-ID setting. Default builds still contain no upload controls.

All-feature tests passed (47 unit cases), including history/thumbnail/retention
compatibility and localhost upload/delete HTTP tests. Optional-feature X11 smoke
opened a legacy synthetic history record and closed without modifying it; existing
capture/clipboard/launcher/save tests passed. Thumbnail UI addition then passed
all-feature Clippy. No real uploads/deletions were performed. Mock-driven end-to-end
upload UI success/retry/deletion flows, auto-copy/close preferences, interruption
handling and full history UI parity remain required acceptance work.

Update checking migrated behind the default-enabled `update-check` Cargo feature;
--no-default-features excludes it. The client reads the existing upstream GitHub
latest-release endpoint, compares semantic versions numerically, skips drafts and
prereleases, honors ignoreUpdateToVersion and validates release links. Requests
are bounded by timeout/response size and do not download or install updates.
The service checks at startup and every 24 hours when checkForUpdates is enabled.
A manual updates window/tray entry displays results and can persist an ignored
version; configuration exposes the automatic-check switch.

Tests use localhost release JSON, not live GitHub data. All daemon/desktop test
configurations explicitly disable automatic checks. Tests cover numeric version
ordering, installed/ignored/prerelease handling and HTTP retrieval. Default and
all-feature tests/Clippy pass (45/49 unit cases respectively), six integration
cases pass, and the full X11 desktop smoke passes. --no-default-features checks
pass and are now included in check-rust.sh. Manual update-window interaction and
24-hour scheduling behavior still need dedicated acceptance coverage. The endpoint
remains upstream Flameshot's release feed, matching the original implementation;
this migration branch has not published Rust release packages.

Native Wayland smoke test added in tests/rust/wayland_smoke.py. It starts a fully
isolated headless Sway/wlroots session with XWayland disabled and DISPLAY removed,
private D-Bus, temporary XDG runtime/config/cache/data directories, PipeWire,
portal-wlr, GTK Access backend and private permission store. Only the synthetic
session is granted screenshot permission. The existing user GNOME session and
permission data are not touched. Test tools were extracted under ignored target/
from distribution packages rather than installed globally.

Validated real Portal full/screen capture at 1024x768, native data-control clipboard
ownership after capture-process exit, native wgpu editor display, preset-region
Enter export and Ctrl+C transfer of a 100x80 image after editor closure. No X11
clipboard fallback was possible. Focus establishment requires a short delay after
creating the virtual keyboard; service startup waits for PipeWire and D-Bus owners.

The frontend requires an Access backend to expose Screenshot; configuring only
wlr was insufficient. This was verified against the official portal source:
https://raw.githubusercontent.com/flatpak/xdg-desktop-portal/1.18.4/src/screenshot.c
The isolated bus logs nonfatal old-backend UnknownMethod cleanup reply rejections;
all asserted capture/editor/clipboard results pass. This proves Sway/wlroots only,
not GNOME native clipboard without XWayland, KDE, real IMEs or mixed-DPI layouts.

Reproduction: `python3 tests/rust/wayland_smoke.py target/wayland-test-tools/root target/debug/flameshot`.
Required extracted packages: sway, swaybg, libwlroots12t64, libseat1,
libdisplay-info1, libliftoff0, libxcb-composite0, xdg-desktop-portal-wlr, grim,
wl-clipboard, wtype. Host xdg-desktop-portal, portal-gtk, permission-store and
PipeWire are used, each with the isolated environment.

Multi-monitor editor selection implemented. GUI captures without a preselected
monitor show frozen screen thumbnails and numbered choices; selecting a screen
rebuilds the document from that crop and targets its borderless fullscreen output.
Explicit screen --edit and gui --region screen<N> target the requested output at
window creation. Single-monitor behavior remains direct. The implementation uses
egui/winit monitor indices, which match protocol enumeration in the tested Sway
session; other backends/order changes still require validation.

Native Wayland smoke now hotplugs a second headless output at logical x=-640,
using 1x and 2x scales with distinct blue/red backgrounds. It verifies a 3328x1536
full image, 2048x1536 and 1280x960 monitor crops, and their actual center RGB pixels
using Pillow. Explicit and interactive editor selection both place the window on
HEADLESS-2 (verified via Sway tree) and export the expected red 100x80 region.
The test explicitly focuses the window for virtual-keyboard input; this does not
prove activation-token handling for arbitrary real desktop launch contexts.

Default/all-feature tests, Clippy and disabled-feature checks passed. Remaining
coordinate work includes global print-geometry/pin origins, fractional/rotated
outputs and backend-specific image-layout assumptions. GNOME/KDE/native IME
acceptance is still required; this successful mixed-scale case is specific to
Sway/wlr Portal composition. Wayland smoke now also requires Python Pillow.

Image export expanded after querying actual Qt 6.4.2 encoders. All baseline formats
are now supported: bmp, cur, ico, jpeg/jpg, pbm, pgm, png, ppm, xbm and xpm. Rust
also supports tiff/tif, webp and gif. Raster codecs use image; PNM/XBM/XPM writers
and CUR headers are implemented in Rust. Format selection is exposed in both
configuration and the save dialog, updating the selected file suffix. Empty
images are rejected. savePathFixed now bypasses the dialog for a configured fixed
destination, while write errors still return to the retry dialog.

A new test encodes every supported extension, decodes available Rust raster
formats and checks exact pixels for lossless RGB formats. Synthetic files were
also decoded using actual Qt: every baseline format passed 16x12 size validation,
with BMP/CUR/ICO/PNG/PPM/XPM sample colors verified exactly. Qt remains a one-time
compatibility check tool, not a Rust build/runtime dependency. Fixtures are under
ignored target/format-fixtures (generated with FLAMESHOT_FORMAT_ARTIFACT_DIR).

Default/all-feature suites pass (46/50 unit cases), six integrations pass, and full
X11 smoke includes Ctrl+S saving directly to configured fixed.png. Palette/mono
quantization may differ from Qt, and optional third-party Qt encoder plugins are
not a preserved plugin ABI. Large-icon limits and Portal save-dialog integration
remain acceptance work alongside other migration gaps.

Qt QSettings QList<int> compatibility implemented using an actual Qt-generated
INI fixture. Rust now handles the control escapes used in QVariant byte payloads,
decodes/encodes the serialized integer list and preserves unrelated settings.
Qt independently read a Rust-written list containing positive, negative and 255
values. This is the concrete tool-button QVariant format used by Flameshot, not
a claim of supporting arbitrary QVariant types.

Editor tool visibility and configured order now use the legacy button ID list;
malformed list data is preserved and falls back to default visibility. Drawing
now distinguishes outline rectangle (ID 3) from filled rectangle (ID 4), with R
selecting filled rectangle. Crop-region and object-manipulation utilities remain
available separately. Remaining compatibility includes the original S shortcut
semantics, full configurable shortcut mapping, tool customization UI and all
recognized-config validation; these are not considered complete yet.

Default/all-feature checks passed (47/51 unit cases), six integrations passed and
full X11 smoke passed. Fixture generation/Qt verification remain temporary tools
under target/, while the committed tests require only Rust and the fixture data.

Shortcut configuration is now loaded from the legacy [Shortcuts] section and
used by editor actions, drawing tools, selection movement/resizing, undo/redo,
copy/save/pin, color sampling, size adjustments and an object/property panel.
Portable Return/Esc/direction aliases, Ctrl/Alt/Shift, literal plus, empty disabled
bindings and up-to-four-stroke sequences are parsed. Press-event modifiers are
used rather than final frame state. S now selects the legacy outline rectangle;
selection resize handles remain usable while a drawing tool is selected.

Configuration exposes editable shortcut strings and validates conflicts before
saving. Existing ambiguous bindings execute neither action. Pin-window close
bindings use their own context so Ctrl+Q does not conflict with editor exit.
Space opens the basic object/property panel; full property editing is still pending.

Tests cover overrides, disabled defaults, sequences, aliases, conflicts and pin
scope. Real X11 smoke verifies TYPE_ACCEPT=F8 replaces Enter without losing the
selection. Full suites/desktop smoke passed before the final conflict validation
follow-up, which passed targeted tests and all-feature Clippy. Linux Meta/Super,
localized Qt key names, live binding reload and shortcuts for still-unimplemented
operations remain compatibility gaps; this is not full shortcut parity yet.

Meta/Super/Win modifier aliases added to shortcut parsing. Left/right Super key
state is tracked independently, cleared on window focus loss and reset when
entering save/upload dialogs. Text-widget focus resets sequence progress while
continuing to observe physical modifier transitions. Sequence parsing now accepts
commas with or without following spaces while preserving literal comma/plus keys.
Prefix-conflicting bindings are rejected when saving; existing ambiguous/prefix
bindings do not prematurely execute a shorter action. Upload shortcut dispatch is
connected when the imgur feature is enabled.

Targeted tests cover Meta binding, focus reset, comma/plus sequences, duplicate and
prefix conflicts, pin scope and disabled bindings. All-feature Clippy passed.
Expanded real X11 smoke verifies Meta+F8 does not activate with plain F8 and does
activate with Super_L+F8; the complete smoke passed. One first run hit an Xvfb
process/log-pipe stall; Xvfb stderr is now directed to a temporary file instead of
an unread pipe, with bounded terminate/kill cleanup. No orphan Xvfb remained.
Native Wayland Meta/IME and localized Qt key spelling still require acceptance.

Open With added to the editor and TYPE_OPEN_APP/Ctrl+O. Applications are scanned
on a worker thread from XDG desktop-entry locations with user override precedence,
hidden/NoDisplay filtering, localized names and PNG-capable apps sorted first.
The Rust picker supports search and the legacy keepOpenAppLauncher setting. GIO's
system launcher handles Desktop Entry field codes, D-Bus activation and declared
terminal behavior via argument vectors (no shell). A bounded launcher wait reports
failure without discarding the capture. Successful opening completes the capture
without an unwanted implicit save, while explicit CLI output tasks still apply.

Temporary PNG files are private and retained after successful handoff so viewers
can read asynchronously, matching the old autoRemove(false) behavior. Failed or
cancelled attempts clean up when no launch is in flight. Packaging must supply
the `gio` executable; category tabs, icons, force-terminal override and sandboxed
Portal AppChooser integration remain pending.

Tests verify user override/hidden entries and actually launch a synthetic Python
receiver via GIO, proving spaced/literal-metacharacter image paths remain one
argument. Full default/all-feature suites, Clippy and disabled-feature checks pass.
X11 smoke verifies Ctrl+O cancellation returns to an intact exportable selection.
Final successful-handoff action routing passed all-feature Clippy; complete picker
launch/failure GUI automation still needs acceptance coverage.

Pinned images now preserve aspect ratio, support wheel/pinch zoom, double-click
original-size reset, rotation via context menu, opacity controls and legacy 0–9
opacity keys. Transparent-window rendering is requested, and the original
antialiasingPinZoom setting selects linear vs nearest texture sampling. Source
pixels are retained so repeated zooming does not resample an already-scaled image.
Window sizing validates finite device scale and bounds maximum dimensions.

A unit test verifies aspect ratio, DPR conversion and size limits. Extended real
X11 smoke checks wheel enlargement, proportional geometry, double-click reset and
closure. Default/all-feature suites and Clippy pass. Rotation-menu interaction,
compositor-level opacity blending, native pinch gestures and mixed-DPI/global pin
placement still require acceptance; Xvfb does not prove composited transparency.

Native Wayland pin rendering acceptance expanded with a synthetic red/green image
sent through the Qt-compatible pin wire format. Sway compositor screenshots verify
opaque pixels, expected 50% blending over the blue desktop and restoration to full
opacity. A real context-menu rotation is invoked and the resulting 300x400 window
and top/bottom red/green pixels are checked. Existing capture, clipboard and
mixed-DPI monitor scenarios continue to pass.

A persistent Rust virtual-pointer example was added for isolated tests. Earlier
Sway IPC/transient-pointer attempts did not deliver usable pointer events to the
client; holding the virtual device open resolves capability/event timing. The
helper requires explicit FLAMESHOT_TEST_WAYLAND_INPUT=1 and is launched only in the
private test runtime. Build with `cargo build -p flameshot-platform --example wayland_pointer`
before running wayland_smoke.py. No user desktop input was injected.

All-feature Clippy and Python syntax checks passed. This establishes rendered pin
opacity/rotation under Sway; very small pin-menu layout, other compositors, native
pinch hardware and global mixed-DPI pin positioning remain acceptance items.

Rust CI workflow added for Ubuntu 24.04. It installs native runtime/test tools
without requesting Qt development packages, runs default/all-feature/no-default
checks, isolated service tests, real X11/Wayland smoke and a locked release build.
The workflow also checks the executable's version/linkage and staged desktop/
AppStream metadata. YAML parses locally; the hosted GitHub workflow has NOT yet
been run, so remote runner success is not claimed.

Actual `cargo build --locked --release --all-features` succeeded in 1m36s. The
42 MB optimized executable reports 15.0.0 and ldd lists only libc, libm and libgcc_s
(plus loader/vDSO), with no direct Qt dependency. Runtime graphics libraries may
still be loaded dynamically. The optimized executable passed full X11 smoke,
including optional history, clipboard, pin zoom, launcher, save retry and custom
shortcuts. Rust build instructions now appear at the top of the branch README.

scripts/stage-rust.sh assembles an explicit DESTDIR payload without installing to
the live system: binary, desktop entry, D-Bus service, embedded-catalog-compatible
AppStream metadata, icons and licenses. Actual staging passed desktop-file-validate,
appstreamcli validate --no-net and activation-path checks; the staged binary runs.
Distribution-specific recipes, old-C++ removal and complete feature acceptance
remain unfinished. This is not a release/publication or a system installation.

The optimized executable also passed the complete native Wayland smoke, including
clipboard persistence, composited pin opacity/rotation and mixed-DPI monitor
pixels/placement. An initial optimized run selected Rotate Left rather than Right:
the test sent both menu navigation presses within one frame. Explicit inter-key
frame waits corrected the test, after which expected rotation pixels passed.
CI now includes both debug and optimized desktop regressions. Package staging
permissions are normalized to 0644 for metadata and 0755 for the executable.

Default single-editor behavior implemented for gui and screen --edit using a
kernel file lock in XDG_RUNTIME_DIR/flameshot/gui.lock (cache fallback). The
allowMultipleGuiInstances setting bypasses the lock and is exposed in configuration.
The lock is acquired before capture/delay and held through output processing;
noninteractive captures and pinned windows are not blocked. A stale lock file
is harmless because ownership is held by the kernel, not inferred from existence.

A parallel unit run exposed transient fork/exec descriptor inheritance retaining
the lock after a normal handle drop. The guard now explicitly unlocks on Drop,
then closes its descriptor. Unit coverage confirms exclusivity and stale-file
reuse. Expanded real X11 tests verify duplicate rejection, opt-in simultaneous
editors, normal release and reopening after killing an editor. The test's window
lookup was corrected to combine PID and title using xdotool --all. Test runtime
directories are now isolated as well as config/cache/bus state.

Full default/all-feature checks, Clippy and disabled-feature checks passed. Full
X11 smoke passed after the guard change. This lock coordinates Rust editors;
interoperability with the old Qt shared-memory mutex is not implemented.

Arrow rendering now uses a filled head instead of an open V. Legacy arrowStyle=1
selects the concave curved head (the shaft stays straight), and reverseArrow
swaps endpoints. The editor exposes both options and persists changes. Filled
rectangles read drawRectangleSize and expose a persisted corner-radius control.
Preview and exported pixels continue to share the same renderer. Pixel tests
cover filled/reversed/curved arrows, zero-length arrows and clipped rounded
corners; configuration tests cover legacy shape preferences and invalid values.
Full default/all-feature/disabled-feature checks and Clippy passed. These changes
do not claim exact Qt raster parity: short-arrow rounding and rounded-rectangle
stroke expansion still differ from Qt geometry.
The rebuilt all-feature executable passed the complete isolated X11 desktop smoke
after these shape changes. Existing Wayland/release results above predate this
renderer change; they have not been rerun for this revision.

`config --check` now runs without a display, reads the legacy configuration path,
prints the success message or collected diagnostics, and returns 0/1. Like the
old command, --check takes priority over mutation flags. It never creates or
rewrites configuration. The validator enumerates the Linux legacy schema,
checks booleans, integer bounds, existing save directories, output extensions,
colors/palettes, serialized button IDs and shortcut syntax/conflicts. Unknown
General and Shortcuts keys are reported; unrelated groups are preserved/ignored.
QSettings comma-separated string-list decoding now handles quoted commas and
malformed quoting. Unit/CLI tests cover multiple simultaneous errors, no display,
missing files, read-only behavior, palette/button values and legacy settings.
Default/all-feature/no-default checks and Clippy passed for the initial validator;
focused checks passed after aligning filename/extension validation with runtime.
Validation does not imply that every recognized preference is implemented in the
GUI. Rust's shortcut validation currently also rejects collisions with implicit
defaults and sequence prefixes; the old checker only compares explicit bindings.
Filename validation follows Rust's existing stricter filename rules. Those
compatibility differences remain pending; no complete config parity is claimed.

Shortcut precedence now follows ConfigHandler::shortcut: a valid explicit binding
suppresses an identical implicit default. For example TYPE_ACCEPT=Ctrl+S accepts
the screenshot and disables the unconfigured Save binding; it no longer leaves
both actions ambiguous. Validation compares explicit bindings rather than adding
implicit defaults to its conflict set. Explicit duplicates still fail validation
and do not execute ambiguously. Prefix conflicts between explicit sequences remain
stricter than Qt's old configuration checker. Pin/editor action scopes remain
separate. Merely opening the shortcuts configuration section no longer inserts
all defaults into the override map; only edited fields become explicit settings.
Unit tests cover precedence and restoration of defaults, with a CLI check case
for a valid override of an implicit default. Full workspace checks passed.
The rebuilt all-feature executable passed the expanded isolated X11 desktop smoke,
including TYPE_ACCEPT=Ctrl+S overriding the default Save action and exporting the
selection directly. Configuration CLI integration tests also passed after adding
the precedence case. git diff --check reports no whitespace errors.

Editor history now reads undoLimit (0..999) both for initial capture and after
monitor selection; the config window can edit it. Zero now means unlimited, as
in QUndoStack, instead of disabling undo. Core tests undo/redo 105 edits with zero
limit and retain the existing bounded-history checks. Marker, pixelate and counter
sizes now read separate legacy keys; text and rounded-rectangle sizes also use
the shared tool-size control without changing other tools' sizes. Size shortcuts
and controls persist the active tool's setting. Text uses point units in settings
and pixel units in rendering. Pixelation block width follows 2*(configured size+1).
Settings tests cover independent values and invalid ranges. Full workspace checks
passed. Exact legacy pixelation appearance and unlimited-history memory usage are
not optimized by this change.
Tool controls respect the renderer's current bounds (counter size <=985, font
size <=750 points, other sizes <=10000); larger stored values are clamped for
rendering rather than rewritten on load. The first desktop size test reached
persisted values but its Python parser treated valid quoted INI integers as
literal integers. The test now removes QSettings string quotes before comparing.
Expanded full X11 smoke passed on the rebuilt all-feature binary: selecting six
tools and increasing their sizes writes six independent legacy values, while
undoLimit remains unchanged. A pixel regression with undoLimit=1 draws two filled
rectangles, sends Undo twice, and verifies only the first rectangle remains.
Clippy and whitespace checks also passed after the renderer-bound adjustment.

Canvas wheel events now adjust the active tool's independent size and persist its
legacy setting. Discrete wheel events change by one step; point-based touchpad
events are rate-limited to one step per 200 ms, with zero/nonfinite deltas ignored.
The handler requires canvas hover and no text-input focus, so popup/panel controls
keep their own input. Ctrl+wheel with Circle Counter adjusts the next number
(clamped 1..999), then explicit numbering increments after committing a counter.
Tool-size changes also invalidate the transient preview. Unit tests cover wheel
steps and touchpad timing, and the full Rust verification script passed. Physical
touchpad hardware and exact Qt event conversion are not yet acceptance-tested.
The first desktop wheel test exposed duplicate input: temporary event tracing
showed two identical Line(+1) events in the same frame for a single Xvfb/xdotool
wheel notch. The trace has been removed. The editor now coalesces wheel events
within a frame and guards against egui replaying input on subsequent layout passes.
The test checks each upward/downward step separately instead of only the final
value. This also means multiple physical notches delivered within one rendered
frame are coalesced to one adjustment.
The rebuilt all-feature binary passed full X11 smoke after event coalescing,
including per-notch size persistence (up/up/down), Ctrl+wheel leaving counter
size unchanged, independent tool sizes and the bounded-undo pixel regression.
Clippy and focused wheel tests passed again. Counter-number glyphs after Ctrl+
wheel have not yet received a separate pixel assertion.

saveAfterCopy and copyPathAfterSave now affect output processing and are exposed
in configuration. Copy actions attempt an automatic save before placing the image
on the persistent clipboard; automatic-save failure is reported but does not
prevent image copy. An unset savePath uses an existing Pictures/Home directory
or the temporary directory. Explicit save destinations retain failure behavior.
Save actions can copy the actual saved filename, including collision suffixes;
GUI outcomes now carry that filename rather than only a saved boolean. If copy
and save are both requested, the image remains the final clipboard content.
The output settings are reloaded after editing, so changes made while the editor
is open take effect for its eventual output. This does not provide general live
reload for toolbar/other capture preferences. Full Rust checks passed before the
default-directory/output-reload follow-ups; final desktop regressions are pending.
Final full Rust checks and expanded X11 smoke passed. Real clipboard tests verify
CLI copy creates a PNG, save publishes the pathname, combined preferences retain
the image, missing configured directory still permits copy, and unset savePath
falls back to an isolated test home. GUI Ctrl+C saves the selected 200x200 image
using preferences changed while editing; GUI Save copies the actual suffixed
filename for a 40x30 preset region. Existing desktop regressions also passed.

drawColor and fontFamily now initialize the editor rather than always using red
and the default font. The text tool exposes a persisted family-name field; text
annotations pass the configured family into the existing cosmic-text renderer.
Color picker changes and G color sampling persist drawColor. A shared Rust color
parser supports legacy #RRGGBB/#RRGGBBAA and all 148 named QColor colors. Actual
Qt 6.4.2 output was compared successfully and stored as qt6-colors.txt; normal
Rust tests need no Qt. Config validation uses the same named-color parser.
An alpha bug was corrected: egui Color32 channels are premultiplied, whereas the
annotation renderer expects straight RGBA. Annotations now receive unmultiplied
channels, preventing a second application of opacity. Full workspace checks and
color fixture tests passed; desktop pixel regression is pending. User palette,
UI theme colors and exhaustive font/IME acceptance remain separate work.
Expanded full X11 smoke passed on the rebuilt all-feature executable. A filled
rectangle drawn with configured #00ff0080 exports as approximately (0,128,0,255)
over the black capture background, proving opacity is applied once through the
actual editor/export path. Existing save/clipboard, wheel, undo and shortcut
regressions also passed. The selected font-family path is covered by settings
and renderer integration code; exhaustive per-family glyph comparison is pending.

Selection preferences are now connected to behavior. saveLastRegion restores the
cached selection for gui when no explicit --region is supplied; explicit regions
still take priority. copyOnDoubleClick copies from a primary double-click inside
the selected area and ignores clicks outside it or over existing annotations,
reserving object clicks for editing behavior. Full text-object double-click editing
is still pending. contrastOpacity controls the outside-selection shade (0..255,
legacy default 190 rather than the previously hardcoded 150). All three settings
are exposed in configuration. Settings validation and full Rust checks passed.
Expanded desktop tests cover enabled/disabled double-click, cached-region defaults,
explicit-region precedence, and opaque shading over a white test background.
The first shade check failed because Xvfb reset its root background after the last
client (xsetroot) disconnected, so the intended white source was black. The isolated
server now uses -noreset. A focused repeat passed: full opacity makes pixels outside
the selection black while selected pixels remain white. The CI test dependencies
now explicitly include x11-xserver-utils for xsetroot. Initial double-click and
cached/explicit-region cases passed before this test-environment correction.
The full expanded X11 suite passed with -noreset, including disabled and enabled
copy-on-double-click, outside-selection rejection, persistent clipboard contents,
saveLastRegion restoration, explicit --region precedence and opaque-shade pixels.
The complete Rust verification script and whitespace checks also passed.

Legacy config -m/--maincolor and -k/--contrastcolor now update uiColor and
contrastUiColor without opening a window. Values use the shared named/RGBA parser;
invalid colors reject the entire settings write, including simultaneous unrelated
updates. Integration tests verify both flags, unknown-value preservation and
unchanged files on invalid input. Settings read the legacy purple defaults.
Editor selected/hovered/active controls use the configured theme, with dark/light
foreground contrast; selection borders and handles use uiColor. Configuration has
color controls with a local preview and saves both values. The theme is applied to
both egui dark/light styles. It currently covers editor/configuration windows;
other utility-window theme consistency and live editor theme reload remain open.
Full Rust checks passed; desktop selection-color pixel regression is pending.
The rebuilt all-feature executable passed the full expanded X11 suite. A real
window capture verifies configured cyan uiColor at the selection handle while
opaque outside shading remains black and selected source pixels remain white.
Previously added clipboard/save, region, wheel, undo and RGBA drawing checks
also passed. CLI usage for the legacy color options is documented in BUILD.md.

Existing text annotations can now be double-clicked to edit their text, family,
size and RGBA color in a focused dialog. Drafts do not mutate the document; Apply
replaces the original annotation in one undo entry, while Escape/Cancel leaves it
unchanged. Empty replacement text removes the annotation; unchanged Apply creates
no extra history entry. Position and other annotation properties are preserved.
Stale annotation indices are rejected. Text editing takes priority over double-
click copy. With an empty repeated-label field, the Text tool now also opens this
dialog after clicking a placement point, so new text can be typed at that point.
The configured commit-tool shortcut commits the dialog; default Ctrl+Enter is
consumed before the text field can insert a newline. Font/size/color edits here
change this annotation rather than global defaults. A focused X11 regression
verified creation, cancellation, replacement, undo and redo using captured pixels.
Full Rust checks passed, including empty commit/history and isolated-draft tests.
Full desktop regression is pending. In-place live text preview, full IME hardware
acceptance and precise glyph-based hit testing remain outstanding.
The rebuilt all-feature executable passed the full expanded X11 suite, including
the text-edit flow with copyOnDoubleClick enabled. It proves text double-click
edits rather than copies, Escape retains the capture, applied text changes pixels,
one Undo restores the original pixels, Redo restores the edited pixels, and final
export retains the 400x200 selected region. Earlier desktop cases also passed.

Text drafts now render live on the capture canvas before Apply. The core exposes
non-mutating replacement/removal previews that preserve annotation layer order;
new text uses the existing transient-add preview. Text editing caches the preview
texture until the draft changes, and uses the same renderer as final export.
The editor window is movable rather than fixed at screen center so it can be moved
away from the edited text. Cancel drops the preview and refreshes the unchanged
document; Apply refreshes the single committed replacement. Same-frame keyboard
replay after closing the text dialog is suppressed to prevent Escape also closing
the capture during an egui layout retry. Core layer-order/history tests and full
Rust checks passed; expanded live-preview desktop pixel assertions are pending.
The full X11 suite passed after the preview changes. The text case now captures
pixels while a changed draft is still open, confirms they differ from the original,
then verifies Cancel restores the original and Apply produces exactly the previewed
pixels. Undo/redo and selected-region export assertions continue to pass. Final
all-feature Clippy and whitespace checks also passed.

userColors now drives editor swatches and the canvas right-click palette. Entries
retain their order, the picker sentinel opens the color chooser, and selecting a
swatch persists drawColor. The config window edits comma-separated entries and
provides small/large default resets; predefinedColorPaletteLarge selects the legacy
default when no custom list is present. Default Qt global-color values are stored
explicitly (Qt::green differs from the SVG name green). Atomic INI updates now
serialize userColors as a real QSettings string list and validate 3..17 entries.
Palette parsing deliberately differs from drawColor: eight hex digits are Qt ARGB,
and short/12-bit/16-bit RGB entries are accepted. Actual Qt 6 fixtures verify these
forms; exhaustive local comparison of all 12-bit/16-bit channel values confirmed
rounding. Rust unit/CLI/service checks passed. Focused X11 testing selected a custom
green swatch via right-click, verified persisted RGBA and exported green pixels.
Full X11 regression is pending; the picker is an egui menu rather than a radial
Qt widget, and palette-specific interaction on other compositors remains to verify.
The rebuilt all-feature executable passed the full expanded X11 suite with the
palette row and right-click menu, including exported swatch pixels and all prior
text preview/edit, theme, opacity, region, save/clipboard and shortcut cases.
Whitespace checks passed as well.

Configuration now exposes toolbar visibility checkboxes and Restore defaults.
Opening the section alone does not rewrite buttons; edits serialize a real Qt
QList<int> variant atomically. Unknown requested IDs fail without modifying the
file; valid lists are deduplicated and sorted by legacy CaptureToolButton priority.
The editor now applies that priority for drawing tools and action buttons rather
than depending on list storage order. Optional increase/decrease-size buttons are
implemented, hidden by default as in Qt. Missing feature-specific tools remain
hidden but their stored IDs survive other edits. Base selection/object-move controls
remain available independently of the configurable legacy buttons.
Tests cover QVariant serialization, empty/custom/default lists, invalid IDs,
normalization and optional upload availability. Full Rust checks passed. An actual
X11 screenshot with only Accept/+/- enabled verified the reduced toolbar; expanded
full desktop checks now also click +/- and verify persisted drawThickness.
The first complete desktop run reached the final toolbar case but its test assumed
a black desktop when measuring toolbar width; an earlier opacity test had left a
white background. The measurement now compares against the actual background.
A focused white-background repeat passed, including +/- persistence.

A fresh source inventory found these Linux legacy option keys are still not read
by Rust Settings (recognition by config --check is not implementation):
showSidePanelButton, showAbortNotification, historyConfirmationToDelete,
autoCloseIdleDaemon, showStartupLaunchMessage, showQuitPrompt, copyURLAfterUpload,
useJpgForClipboard, uploadWithoutConfirmation, showSelectionGeometry,
showSelectionGeometryHideTime, insecurePixelate, captureActiveMonitor and
useX11LegacyScreenshot. This is a configuration-read inventory only, not a full
feature/desktop/distribution completion audit. Other acceptance gaps recorded
above, including legacy C++ removal and distro recipe migration, remain open.
The final full X11 run passed with the corrected toolbar measurement, including
visibility, optional size buttons, default priority ordering and all previously
added editor/output regressions. Full Rust checks and whitespace checks passed.

useX11LegacyScreenshot now bypasses the portal on X11 and uses direct root capture;
on Wayland it is ignored, preserving portal capture. captureActiveMonitor selects
the X11 monitor under the pointer for graphical captures with multiple monitors,
while explicit monitor requests take precedence. Failed pointer lookup retains the
manual chooser. As in the Qt implementation, this preference reports unsupported
on multi-monitor Wayland rather than attempting to inspect an XWayland pointer;
single-monitor and explicit-monitor captures still work. Both options are exposed
in configuration. Monitor selection currently uses the pointer position determined
before the requested capture delay, as the existing screen command did.
Full Rust checks passed. A private portal mock verifies forced X11 makes no portal
request (and fails honestly without DISPLAY), while Wayland still requests/decodes
the portal PNG with the same flag. Focused Xvfb/RandR tests use blue and red monitors
to verify pointer-selected red pixels and explicit left-monitor blue pixels, both
at 512x768. Full X11 and native Wayland regressions are pending.
Full expanded X11 and native Wayland suites passed, including X11 forced capture,
pointer-selected colored monitors, Wayland portal operation with the X11 flag set,
unsupported active-monitor rejection, and explicit/interactive mixed-scale output
selection. A follow-up moved captureActiveMonitor pointer lookup until after the
requested delay, correcting the earlier timing note above. Full Rust checks passed
again; a focused delayed-pointer-movement regression is running. The pre-existing
implicit `screen` command pointer timing is unchanged by this follow-up.
The final focused X11 run passed all three monitor cases: automatic right monitor,
explicit left monitor, and moving from left to right during an 800 ms delay. The
delayed capture exported the right monitor's red pixels at 512x768.

showSelectionGeometry now supports None, four corners and Center with pixel
width/height/x/y text. showSelectionGeometryHideTime restarts only when the region
changes, not on normal repaint; zero remains visible indefinitely. Repaint is
scheduled at expiry so the overlay hides without further input. Labels are clamped
to the canvas and painted separately from the document/export. showSidePanelButton
now controls an Objects toggle beside the magnifier control, while the existing
Space shortcut remains available. All options are exposed in configuration.
Full Rust checks passed. Deterministic timer tests verify change/repaint/expiry
semantics. Focused X11 pixels verify appearance, timed hiding, reappearance after
keyboard movement, and an unchanged white export. Other pixel tests explicitly
turn off geometry overlays when measuring unrelated text/shading behavior. Full
expanded X11 regression is pending.
The complete expanded X11 suite passed with geometry timing enabled and the new
panel toggle present, including prior toolbar, palette, text, clipboard/save and
multi-monitor cases. Whitespace checks passed.

insecurePixelate now selects a separate annotation rendering mode and is exposed
in configuration with its retained-image-information meaning. Default annotations
still use only outside-fringe pixels (neutral fill for an all-image selection).
Explicit insecure mode samples the selected image: smallest size uses blur, larger
sizes downsample smoothly then enlarge with nearest-neighbor blocks. Each annotation
retains its chosen mode for stable previews/history. The shared renderer handles
clipping, preview, export and undo for both paths. The blur approximates the Qt
visual behavior using Rust image filters; exact Qt blur-kernel parity is not claimed.
Full Rust checks passed, including the existing interior-independence tests and
new sampled-mode/blur/clip/undo tests. Focused X11 tests draw red on white, verify
the secure mode replaces it from white fringe while insecure mode preserves red,
and compare drag-preview pixels with exported pixels. Full X11 regression pending.
The full expanded X11 suite passed on the rebuilt all-feature binary, including
both pixelation modes and all prior editor, clipboard/output and multi-monitor
regressions. Whitespace checks passed.

Optional Imgur preferences now affect the application: uploadWithoutConfirmation
starts one upload when the upload dialog opens (not repeated after failure),
copyURLAfterUpload controls automatic persistent-link clipboard transfer, and
historyConfirmationToDelete controls the extra confirmation after a Delete click.
All three appear in feature-enabled configuration. Manual upload retry remains
available after failure. Clipboard status is separate from upload/history warnings,
and normal dialog closing waits for pending link transfer; disconnected worker
channels now become errors rather than permanently pending state.
Upload/deletion work was factored for dependency injection. A loopback-only HTTP
constructor is available to tests; production UI still constructs the standard
Imgur API and has no endpoint environment override. Local HTTP tests verify PNG
multipart upload, automatic-start-once, auto-copy intent for both preference values,
history persistence, API failure/retry, history-write warnings, deletion confirmation
policy and retaining local history until remote deletion succeeds. A rendered egui
confirmation test verifies no request starts by default. Full Rust checks passed;
final all-feature Clippy/focused tests passed after worker-close handling. No real
Imgur uploads or deletions occurred. Expanded X11 confirmation-cancel regression
is pending; complete interactive mocked upload/delete window coverage remains open.
The full expanded X11 suite passed, including opening upload confirmation via its
shortcut, cancelling it without closing the capture, then exporting the image.
All other desktop regressions passed. Local upload/deletion tests passed again
with proxy use explicitly disabled for the loopback-only test client.

showAbortNotification now controls a best-effort screenshot-cancel notification,
also gated by showDesktopNotification. Cancellation still exits with status 2 and
never writes notification text to PNG stdout; notification transport errors do not
change the result. showStartupLaunchMessage now sends the legacy tray greeting only
after tray creation succeeds. Like Qt's tray greeting, its switch is independent
of the capture-notification switch. Duplicate daemon launches do not repeat it.
Both options are exposed in configuration. Private D-Bus integration tests verify
all abort-notification flag combinations, notification-server failure, startup on,
startup off and singleton behavior. The check script runs the new isolated test.
Full Rust checks passed; focused startup/abort tests passed after expanding cases.
The complete X11 regression is running; no test notification reached the real bus.
The first desktop attempt was invalidated by a concurrent default-feature Cargo
integration-test build replacing target/debug/flameshot after the script had
observed all-feature help. The later upload-cancel case therefore ran a binary
without upload support. This was a test orchestration issue, not evidence of a
modal-close bug; no speculative GUI change was made. The all-feature binary was
rebuilt and the complete desktop suite restarted with no concurrent Cargo builds.
The uncontended full X11 run passed, including upload-confirmation cancellation,
normal capture cancellation without a notification server, and every prior desktop
case. Final all-feature Clippy and whitespace checks passed. All notification
behavior tests used isolated mock services.

showQuitPrompt now controls capture-exit confirmation (not daemon/tray exit).
Escape first leaves the active tool/object selection/panel; actual exit requests,
Ctrl+Q and the toolbar Cancel action then use the prompt. No is the default.
Do not show again persists the legacy setting immediately. Accepted/saved captures
close without a prompt. Native window-close input is consumed once at the raw-input
hook and converted into the same application request; confirmed/programmatic close
is allowed through. Keyboard events after a quit request cannot accept the capture
in the same action batch. A unit test verifies one-shot native-close consumption.
Focused X11 tests passed default No/resume, explicit confirmation, and preference
persistence. Full X11 passed before the final native-input guard refinement.
Native Wayland initially timed out on keyboard confirmation: diagnostic captures
showed the dialog unchanged. Giving the test virtual keyboard a settling interval,
as used by other wtype cases, resolved it. The complete native Wayland suite then
passed compositor close, default No and confirmed cancellation with status 2.
Final Rust checks and X11 verification are in progress; this is not full migration
completion.
Final full Rust checks and the rebuilt all-feature X11 suite passed after the
native-close refinement. The native Wayland suite also passed compositor close,
No/resume and Yes/cancel. X11 additionally verifies Ctrl+Q, Escape/tool deselection,
normal acceptance without a prompt, and the persisted Do not show again option.

Clipboard ownership is now explicit. A Rust X11 provider serves PNG/JPEG/text,
TARGETS/TIMESTAMP and bounded INCR transfers; a native Wayland data-control provider
runs until its source is replaced, with X11 fallback when available. Each provider
exposes a live lease. The daemon retains all active leases, including older INCR
pastes that outlive selection ownership, and tracks children launched by its service
or tray. autoCloseIdleDaemon exits after hosted activity ends only when no clipboard
provider/transfer or child remains. Initial service activation stays alive to receive
its first request. Explicit daemon shutdown behavior still needs the broader audit.
useJpgForClipboard now publishes actual image/jpeg data instead of PNG. arboard
remains enabled for egui text-widget clipboard support, not screenshot ownership.

Full Rust checks passed. Full X11 tests verify JPEG, exact large PNG INCR transfer,
retaining ownership, pin lifetime and idle exit. A gated X11 probe holds an old INCR
paste open while newer clipboard contents are installed and replaced: the daemon
stays alive, delivers every original PNG byte, then exits. The helper is now built
alongside the Wayland pointer helper in CI. Native Sway verifies PNG/JPEG/text MIME,
text replacing an image, and idle exit after another application takes ownership;
the complete native suite passed. A wl-copy test invocation was changed to avoid
waiting on output pipes inherited by its background provider. No test touches the
real desktop clipboard. All Linux legacy option keys now appear in Settings, which
is read-path coverage rather than proof of full feature acceptance.

Debian build/install rules now use locked offline Cargo, with Imgur enabled and
application update checking disabled. Qt/CMake build/runtime dependencies were
replaced with Rust and native graphics/runtime requirements; shlibs dependencies
are generated from the executable. The staging script now installs the legacy
D-Bus interface XML. A source-archive script exports tracked project files and
vendors locked dependencies, normalizes tar metadata and publishes the archive
atomically. It does not include the old root CMake/C++ project.

Actual validation built an optimized executable from the archive using an empty
Cargo home and offline mode. Extracted debhelper tools were used without installing
them on the host; dpkg-buildpackage -d bypassed package-database checks for these
externally supplied tools/toolchain. The standard build, Cargo tests, stripping,
debug-symbol package and .deb generation succeeded. Debhelper cleanup initially
removed vendor Cargo.toml.orig files; excluding vendor and disabling irrelevant
autotools hooks fixed checksum failures. The existing upstream changelog emits a
version-ordering warning for 15.0.0 versus its earlier 15.0.rc2 entry; it was not
silently rewritten. Desktop/AppStream metadata validated and the package binary
has no direct Qt linkage. Other distro recipes remain pending.

Release package testing exposed an X11 INCR termination race: all data chunks
arrived, but after ownership loss the provider could drop its connection before
the empty terminator reached the server. It now waits for the receiver's final
property deletion (or window destruction/timeout), and synchronizes with the X
server before dropping its connection. The paused-reader package test passed after
this fix; final package desktop suites are pending. Test helpers now emit progress
on failure and can be supplied independently of the executable installation path.
Final stripped Debian package executable passed the complete X11 and native Sway
suites, including the paused INCR paste that exposed the release-only race. Final
workspace checks passed. Desktop/AppStream validation and Qt-linkage checks passed.
Two source archive runs produced the same SHA-256 at the validation snapshot:
7493e16d4768cb039dde017c70127df4193d4024d27d60b1b87e115a1459dab8.
The archive was inspected for portable vendor configuration, absence of the legacy
root C++/CMake project, and inclusion of the final clipboard fix. Debian packaging
is migrated/locally validated; RPM, Arch, Flatpak, Snap and Nix remain pending.

RPM rules now build/test with locked offline Cargo and use the shared staging
script. Qt/CMake dependencies, QM installation and Qt-specific cleanup were removed.
The file list matches the Rust payload. Runtime libraries loaded via dlopen are
explicit SONAME dependencies with architecture-correct capability suffixes; distro
font/driver recommendations retain separate openSUSE and other-RPM branches.
Source review identified libX11-xcb and libxkbcommon-x11 as required winit libraries;
Debian and CI dependencies were corrected too. The RPM changelog's incorrect weekday
for 21 June 2022 was corrected without changing the release date/version history.
Spec parsing passed for x86_64, i686, aarch64 and the openSUSE branch. An actual RPM
and source RPM were built with locally extracted RPM tools and --nodeps, supplying
the Rust toolchain/dependencies outside an RPM database. Build, workspace tests and
metadata validation passed. This is local package construction, not a clean Fedora
or openSUSE installation test; packaged-binary desktop suites are in progress.

Final RPM/source RPM builds completed without the old changelog weekday warning.
The packaged executable passed full X11 and native Sway suites. After final metadata
updates, its bytes matched the desktop-tested executable exactly. RPM dependency
and file-list checks verified dynamic X11/XKB libraries, the session-bus utility,
IPC metadata and absence of Qt linkage. Debian was rebuilt with the same dynamic
library corrections and session-bus dependency. Native Fedora/openSUSE clean-root
installation and non-x86 binary execution remain unverified; macro parsing alone
is not claimed as those tests. Arch/Flatpak/Snap/Nix migration remains pending.

PKGBUILD now consumes the vendored local Rust snapshot, builds/tests offline with
Cargo, and installs through the shared staging script. It preserves the development
package name and derives its version from a generated .git-version snapshot file.
Qt/KDE/CMake build dependencies were replaced by native libraries and Cargo tools.
The source helper exports PKGBUILD and records dirty worktree state. An actual
makepkg run with externally supplied tools and an isolated empty pacman database
(--nodeps) produced a .pkg.tar.zst; release build, workspace tests, stripping and
metadata generation passed. This does not claim a clean Arch dependency-install
or non-x86 runtime test. Desktop/AppStream metadata and executable startup passed.
Source inspection also found the Wayland EGL library needed by the GLES backend;
Debian/RPM runtime declarations were corrected. Packaged-binary X11, native Wayland
and forced GLES desktop regressions are running.
The packaged Arch executable passed full X11 and native Sway desktop suites, and
also the complete native suite with WGPU_BACKEND=gl. This verifies the GLES fallback
rather than only Vulkan on this environment. The same fallback check was added to
the Rust CI workflow. Runtime dependency names/file ownership were checked against
Arch's published libx11 and wayland file lists; vulkan-swrast is offered as an
optional software driver. Final package metadata is being rebuilt and checked.

The final Arch package was rebuilt with the completed metadata; its executable
bytes exactly match the version tested under X11, native Wayland and forced GLES.
Package metadata, Git snapshot provenance and no-Qt linkage checks passed. Debian
and RPM metadata rebuilds also contain the Wayland EGL runtime dependency. The
workflow YAML and shell recipes parse successfully. These validations used extracted
tools on the development host; clean Arch installation and non-x86 execution are
still not claimed. Flatpak, Snap and Nix migration remains pending.

Flatpak manifest migrated from KDE/Qt/CMake to Freedesktop 26.08, a pinned Rust
1.99.1 build-only toolchain, vendored offline Cargo and the shared /app staging
layout. The published 26.08 rust-stable extension was inspected and still contains
Rust 1.98.0, so it is not silently assumed to satisfy the MSRV. Official 1.99.1
archive checksums are pinned for x86_64/aarch64; flatpak-builder downloaded and
verified the x86_64 source, and its selected compiler/Cargo/std components installed
successfully into a local test prefix. Manifest parsing passed with flatpak-builder.

Application windows now use the packaged icon and Wayland desktop app ID, while
standalone X11 keeps the flameshot WM_CLASS. Flatpak X11 uses the app ID and the
manifest adjusts StartupWMClass accordingly. The tray uses ksni's unique-name mode
under Flatpak instead of requesting a sandbox-forbidden dynamic well-known name.
Private D-Bus tests verify this path; full X11 and native Sway suites pass with
new identity assertions. All Rust checks passed. The local bubblewrap UID-map
probe fails with Permission denied, so actual sandbox build/run acceptance remains
unproven. A dedicated CI build workflow was added but not executed remotely.
The pinned official toolchain also built the vendored app offline in a separate
host test prefix. /app staging, D-Bus activation path, desktop/AppStream metadata
and executable startup passed. A full X11 suite with FLATPAK_ID set passed against
that staged release executable, exercising the sandbox identity branch without
claiming filesystem or bus confinement. Normal X11/native Sway suites passed too.
Actual SDK-sandbox compilation, cleanup/bundle execution and sandbox file/app-chooser
flows remain unverified because user-namespace setup is denied here. The goal stays
open; this environment limitation is not treated as complete Flatpak acceptance.

### Nix Cargo migration

Replaced the Qt/CMake flake with `rustPlatform.buildRustPackage`, Cargo-lock
vendoring, the shared Rust staging script, and native library/`gio` wrapping.
Removed obsolete Qt dependency fetches, flake-parts, treefmt-nix and Darwin outputs;
Linux x86_64/aarch64 match the current rewrite acceptance scope. The updated lock
pins nixpkgs `1d060086d60cb76a40cf37109f979d88a5bd6396` with Rust 1.99.0.
The development shell supplies Cargo, rustc, rustfmt, Clippy and launcher tools.

Validation: Nix 2.18.1 `flake check --all-systems --no-build` succeeds; evaluating
native build inputs confirms Rust/Cargo 1.99.0 and no Qt/CMake hooks. The launcher
test passes using PATH-resolved Python, and Rust formatting remains clean. Actual
Nix builds stop before project compilation: the relocated private store does not
make canonical `/nix/store/.../bin/bash` builder paths executable, including with
`sandbox = true`. No system store or host namespace policy was changed. Added an
unrun hosted workflow for normal-store builds and installed executable checks.
Nix package compilation/runtime acceptance and clean-distro installs remain open;
Snap still needs migration.

### Snap Cargo migration

The canonical core24 manifest now builds the current Rust workspace with
Snapcraft's Rust plugin and pinned Rust 1.99.1, enables Imgur, disables the app
update checker, and runs workspace tests before installing the shared package
payload. The obsolete core18/Qt copy is a symlink to the canonical manifest.
The GPU extension replaces KDE, while explicit native libraries, fonts, XKB data
and MIME data supply the remaining runtime resources. A small launcher preserves
GPU library order and literal arguments and connects a parent-directory Wayland
socket into Snap's per-app runtime directory without overwriting existing entries.
D-Bus activation calls `/snap/bin/flameshot`; the tray uses its unique connection
name in both Flatpak and Snap. The private-bus test covers both cases.

Validation completed locally: Snapcraft 9.2.1 extension expansion (GPU wrapper
precedes the app wrapper), locked release `cargo install` with package features,
manifest installation commands and resulting version/desktop/service payload,
workspace tests with no default features plus Imgur, private-bus daemon test,
Clippy with all features, and isolated launcher tests covering sockets, existing
files/broken symlinks, absolute Wayland socket paths and literal arguments. The
full Xvfb suite and the staged release binary’s native Sway/Wayland suite pass
with the Snap environment simulated. The vendored archive contains the canonical
manifest, valid compatibility symlink and executable launcher. These native checks
are not evidence of installed strict confinement. There is no LXD/Multipass build
provider on this workstation; no host snap installation was performed. A dedicated
unrun CI workflow now builds the vendored source and inspects its payload, replacing
the old Snap job that read its version from CMake. Provider build, installed desktop
acceptance, arm64 compilation, portal file/host-app access and confined autostart
remain open.

### Live object movement and cancellation

Moving an annotation now uses an immutable replacement preview at its original
layer. The selected object's outline follows that preview; release commits one
replacement. No-op edits no longer create undo entries or discard redo. Invalid
moves leave the document/history intact. Escape clears the pending movement and
its drag state; the subsequent mouse release is ignored, preserving the existing
capture selection instead of replacing it with an empty one.

Core tests cover overlapping layers, preview/export agreement, single-step undo,
redo preservation after no-op movement, and rejected movement. The X11 desktop
case draws a rectangle, holds the pointer down to inspect intermediate pixels,
compares release with preview, checks undo/redo, cancels another drag, then verifies
redo and the exported selection size. Its reduced English toolbar provides stable
coordinates, and pointer events are separated by rendering delays. This closes the
live-movement gap, not all object-editing parity: legacy selection uses rendered
search areas with near/far tolerances, while Rust hit testing still uses bounds;
resize/property editing also needs further compatibility work.

Validation passed: `cargo test --locked --workspace --all-features`, all-target
all-feature Clippy with warnings denied, Rust formatting, the complete isolated
Xvfb suite including the new movement case, and the native Sway/Wayland suite.
The Wayland run exercises existing capture/clipboard/pin/selection regressions;
the new held-pointer movement assertions currently run on X11.

### Selecting rendered objects

Replaced bounding-box-only picking with rendered coverage. Search follows the
legacy near-then-far passes (3 and 5 image pixels); text receives another 5 pixels
on the near pass for glyph spacing and is excluded from the far pass. Each pass
visits layers from top to bottom, so a near hit on a lower layer wins over a far
hit on an upper one. Invert and pixelate retain filled rectangular search areas.
Transparent shapes, hollow interiors and empty portions of diagonal bounding boxes
no longer intercept objects underneath.

Vector bounds are only a rejection filter. Candidate vector paths are rendered in
an object-sized scratch surface before extracting the small query mask: directly
clipping a curve to the query surface changed tiny-skia's edge coverage, discovered
by the pixel comparison test. Text uses its actual glyph rendering, not estimated
character widths. Only query masks survive for reuse by the second search pass.
The shared export renderer still uses its original identity transform.

Core regression coverage includes near/far priority, layer order, filled effect
regions, transparency, empty glyph spaces and invalid coordinates. A grid comparison
checks picking against exported alpha pixels across pencil, marker, line, straight
and curved/reversed arrows, outlined/filled rounded rectangles and ellipses, counter
and multiline text. The X11 movement case now puts an outline above a filled shape
and selects through the hollow center. This addresses geometry-based picking;
complete object resizing/property editing and final mixed-DPI interaction acceptance
remain separate pending requirements.

Validation passed: 44 core tests, `cargo test --locked --workspace --all-features`,
all-target/all-feature Clippy with warnings denied, formatting, and the complete
isolated Xvfb suite including overlapping-object movement and double-click text
editing. No GNOME/KDE or mixed-DPI picking parity is claimed by these checks.

### Selected-object color editing

The Move object tool now displays the selected annotation's own color and applies
palette/color-control changes to that annotation as undoable replacements. The
replacement keeps its geometry, layer and other tool properties. Drawing tools
continue to change the drawing default without recoloring a stale selected object.
Counter/text colors and stroke/fill colors share the same core accessor; invert
and pixelate do not acquire artificial color properties or undo entries.

The core test checks alpha-aware recoloring, unchanged bounds, undo/redo, and
unmodified pixelation parameters. The X11 object-editing case changes a moved
rectangle to green, undoes/redoes the color change, then switches to drawing with
blue while checking that the existing rectangle stays green. A palette choice
is one undoable operation. Continuous color-picker gesture history grouping,
tool-specific size editing, and resizing are still pending; this change does not
claim complete object-properties parity.

Validation passed: `cargo test --locked --workspace --all-features`, all-target
all-feature Clippy with warnings denied, formatting, and the complete isolated
Xvfb suite with the added recolor/undo/redo/drawing-default assertions. The test
palette respects the existing minimum of three configured color entries.

### Selected-object size editing

The Move object tool now reads the selected annotation's own size and applies
size-control, wheel and size-shortcut changes to it through undoable replacements.
Stroke width, marker width, text points, counter size, filled-rectangle corner
radius and pixelation size are distinct properties. Text points map to raster
font size by 4/3, counters include the existing 15-pixel radius offset, and pixel
blocks retain the `2 * (size + 1)` conversion. Updates preserve all unrelated
geometry, colors, layer order and effect flags. Invert has no size control and
ignores size shortcuts while selected.

Selected-object sizes persist to the corresponding existing per-tool INI key,
rather than always writing `drawThickness` because the active tool is Move.
The core tests cover each size kind, limits/non-finite values, undo/redo, and
filled-rectangle radius changes without changing stroke width or endpoints.
The X11 object-editing case increases roundness on the selected moved rectangle,
checks a corner pixel and `drawRectangleSize`, and undoes/redoes the individual
keyboard increments. These edits do not resize an object's geometry; drag handles
and continuous-property gesture undo grouping remain pending.

Validation passed: `cargo test --locked --workspace --all-features`, all-target
all-feature Clippy with warnings denied, formatting, and the complete isolated
Xvfb suite including selected-object roundness, configuration persistence and
incremental undo/redo. The INI assertion accepts QSettings-compatible quoted values
and repeated sections; pixel assertions sample outside the antialiased edge.

### Continuous property edits in undo history

The core document now supports a live edit group: intermediate replacements render
normally but retain one starting snapshot. Finishing records one undo entry and
honors the history limit. Cancellation restores the snapshot, and finishing a
no-op group preserves redo. Undo/redo first finish any active group so pending
changes cannot leak across history operations.

The editor begins a group when a selected-object color or size changes while the
primary pointer is held. It flushes on the frame after release, including changes
reported on the release frame itself. A repaint ensures that flush happens even
if the user stops interacting. A subsequent press flushes the prior group before
starting another. Keyboard increments and palette clicks remain discrete edits.
The cancellation API is core behavior; this change does not introduce an Escape
rollback of already-applied property values in the UI.

Core tests cover repeated starts, mixed color/size changes, bounded history,
cancellation/no-op redo preservation and undo during a live group. The X11 test
holds the size control across several motion frames, inspects the held preview,
then verifies single-step undo/redo; existing discrete-increment checks remain.
The color picker shares the same grouping path, but its entire popup interaction
has not separately been automated in this change. Geometry resize handles and
remaining object-specific properties are still pending.

Validation passed: 48 core tests within the all-feature workspace test run,
all-target/all-feature Clippy with warnings denied, formatting, and the complete
isolated Xvfb suite including held-pointer preview and grouped undo/redo.

### Baseline audit: geometry editing and Ctrl drawing constraints

The earlier log entries that listed post-creation annotation resize handles as a
missing legacy feature were an incorrect inference. In the baseline implementation,
`CaptureTool::drawObjectSelectionRect` draws an outline, and the selected-object
branch of `CaptureWidget::mouseMoveEvent` calls `move`; `AbstractTwoPointTool::move`
translates both points. There is no corresponding handle-resize interface for
completed annotations. Capture-region resizing and pinned-image scaling are real,
separate baseline behaviors and remain within acceptance scope. This correction
does not waive any baseline functionality or the full Linux migration gates.

A real geometry-interaction gap was found and implemented: holding **Ctrl**, not
Shift, during drawing constrains line/arrow/marker directions to axes or diagonals,
and outline/filled rectangles and ellipses to equal width/height. Pencil, counter,
effects, capture regions and object movement keep their existing behavior. The
helper follows the baseline angular sectors and rounding direction at sector ties;
image-space fractional coordinates are retained rather than truncated to integers.

The pending annotation uses the current modifier without rewriting raw pointer
positions, so releasing Ctrl restores free drawing. Pointer-release modifiers win
over the final frame's keyboard state, including a Ctrl release in the same frame.
This applies before preview rendering as well as commit to avoid a release-frame
shape change. The core tests cover all quadrants, zero/fractional drags and axis
ties; the GUI unit test covers release-event modifier retention. The X11 case
checks live free/constrained/free switching without pointer movement, constrained
commit/undo/redo, and a horizontal constrained line. Full object-property and
cross-desktop acceptance is still incomplete; unsupported extra resize handles
are not a substitute for those requirements.

Validation passed: the all-feature workspace tests (including 51 core tests and
the mouse-release modifier unit test), all-target/all-feature Clippy with warnings
denied, formatting, and the complete isolated Xvfb suite with live Ctrl constraints,
release, undo/redo and line snapping. Other compositor/modifier acceptance remains
part of the final desktop audit.

### Counter deletion and history numbering

Deleting a counter now decrements remaining counter values at or above the deleted
value, preserving layer positions and unrelated annotations. The removal and all
renumbering share one history entry. Removal preview uses the same transformation.
Automatically chosen next numbers follow the remaining counters. A manually selected
next number is decremented on counter deletion; toolbar and shortcut undo/redo now
both clear that override and recalculate from existing counters, matching the
baseline `restoreCircleCountState` behavior.

The core test covers intervening non-counter layers, deletion preview/commit,
undo/redo, the next automatic number, non-counter removal and duplicated labels.
The baseline decrement can produce zero when deleting a manually duplicated first
counter; the Rust implementation keeps such duplicates at one to preserve the
existing valid counter range instead of constructing an invalid annotation.

The X11 case compares rendered labels after deleting the middle of three counters,
undoing/redoing, creating the next counter, and adjusting the next number manually
before undo. It waits for exact expected image content rather than assuming software
rendering finishes within a fixed quarter second. Diagnosis also found needless
full-image redraws on Ctrl changes outside drawing; modifier changes now invalidate
the preview only during a supported pending constrained stroke. Existing Ctrl-drag
coverage is retained in the complete desktop suite.

Validation passed: `cargo test --locked --workspace --all-features`, all-target
all-feature Clippy with warnings denied, formatting, and the complete isolated
Xvfb suite including counter deletion/history/manual-override reset, Ctrl drawing
constraints and grouped property undo. Temporary diagnostic logging was removed.

### Object layer ordering

Added Move Up/Move Down controls to the object panel using existing translations.
They move the selected entry within the bottom-to-top drawing list, matching the
baseline index direction: moving down in the list brings an object forward in the
composition. Selection follows the object to its new index. Boundary controls are
disabled; invalid/no-op core requests do not damage history. Reordering finishes
an active property group before recording its own single undo entry and preserves
annotation contents, including counter labels.

The core test checks overlapping fill composition, hit-test ordering, up/down
movement, invalid/no-op requests and undo/redo. The X11 test draws overlapping red
and green rectangles, selects the lower layer outside the overlap, moves it both
ways through the panel, and checks exact overlap pixels after each move and each
undo/redo. Other object-property and final desktop acceptance work remains open.

Validation passed: all-feature workspace tests, all-target/all-feature Clippy with
warnings denied, formatting, and the complete isolated Xvfb suite with panel-driven
layer movement, selection retention, pixel composition and multi-step undo/redo.

### INI insertion without new duplicate sections

New settings now enter the last existing occurrence of their section instead of
appending another section header for each key. Existing duplicate sections/keys
remain intact, and updates still replace only the last effective matching key.
Implicit General keys are preserved; empty/new files receive an explicit section.
Insertion keeps comments, opaque Qt variants, unrelated sections and existing line
endings unchanged, including CRLF and files without a trailing newline.

The atomic-settings and actual CLI tests now verify preservation around the inserted
keys and that repeated updates do not create extra headers. Core tests cover repeated,
implicit, empty and unterminated sections, Unicode comments and opaque values. Invalid
CLI/settings updates still leave the file unchanged. Validation passed: all-feature
workspace tests (including configuration/CLI integration), all-target/all-feature
Clippy with warnings denied, and formatting. This is a storage-format improvement;
remaining live-reload semantics and the overall migration gates are not marked complete.

### Shell completion and package installation

Bash completion now keeps subcommand context after flags/values, distinguishes
`config -n` booleans from `screen -n` indices and `gui -s` switches, completes files
with spaces for output paths, treats filename patterns as text, and stops after
`--`. Fish/Zsh use the actual plural `--notifications`; completion version flags
match the current Rust CLI's `-V`. Zsh value-taking flags declare arguments rather
than treating delays, indices and booleans as switches. Optional history/update
commands are offered according to the installed executable's help output.

All three scripts are installed by shared Rust staging, with RPM file ownership
updated. Added a CLI-parity/Bash test to the standard Rust check script plus optional
Fish live-completion/Zsh syntax checks. Validation passed against all-feature,
no-default-feature, and the existing Imgur-only staged executable; staged files
match their sources and the RPM spec parses with the new entries. No screenshot,
daemon, host installation or remote workflow was started by completion testing.
Interactive Zsh behavior, the manual page, final CLI compatibility audit and fresh
package builds remain pending; these checks do not close the entire CLI gate.

### CLI manual and legacy version option

Replaced the stale manual with documentation of the Rust command surface: capture
and configuration options, region/monitor rules, optional-feature commands, PNG
stdout and geometry priority, XDG files, examples, and current actual exit statuses.
The shared staging script installs it and RPM includes compressed or uncompressed
man-page output. Historical manual attribution and CC0 licensing are retained;
legacy C++ implementation-specific license descriptions are not presented as Rust
application components.

The baseline parser exposes `-v`, which the initial Clap-derived CLI had lost.
Restored `-v` and retained `-V` as an alias for `--version`; all shell completions
were updated. A regression test runs each spelling with an unusable session-bus
address and verifies version output, no stderr, and no configuration writes.
The command-options test compares executable help with the manual. Normal config
integration and private-bus daemon tests confirm the version argument is optional.

Validation passed: all-feature workspace tests, all-target/all-feature Clippy,
formatting, Bash/Fish completion tests and Zsh syntax, the private-bus daemon test,
shared staging/content comparison, RPM spec parsing, and groff UTF-8 rendering
with no warnings. The earlier manual claimed D-Bus exit 3 and signal-specific
statuses that the current Rust paths do not yet consistently implement; those
remain explicit compatibility gaps rather than being claimed by this manual.
Argument-error priority and final command syntax also remain in the CLI audit.

### Daemon signal and disconnection exit status

The background service now returns the baseline statuses for caught SIGINT (130),
SIGTERM (143), and loss of an established session-bus connection (3), instead of
reporting all three as successful exits. Signal handlers are registered before
publishing the D-Bus service name. Normal tray and idle exits retain success status;
signal shutdown does not print an operation-failure diagnostic, while bus loss does.

An ignored integration test creates its own private bus and configuration for each
case, waits for service ownership, signals only the spawned child (or kills its own
bus), and checks the actual process status. It is included in `check-rust.sh` behind
the private-bus test guard. The existing private-bus tray/single-instance test still
passes. All-feature workspace tests, all-target/all-feature Clippy, formatting and
warning-free manual rendering pass as well.

This closes established-daemon shutdown statuses, not every bus failure or child
lifecycle path. Initial connection failures, capture-client failures and explicit
shutdown of spawned windows remain under audit. Baseline argument parsing also
needs case-by-case treatment: `parse` failure can reach `finish` and return zero in
`src/main.cpp`, so changing every parser failure to one purported legacy error code
would not be evidence-based. Current Rust argument rejection remains nonzero.

### Initial D-Bus connection failure classification

Initial session-bus connection errors now carry an explicit connection-failure
context and return 3 for daemon startup, capture startup and service-client connection
setup. Both address construction and connection/authentication setup are covered
for the daemon builder. The shared connection helper preserves the underlying
error chain in stderr; it does not classify unrelated operation/configuration errors
as bus failures. Cancellation notification remains best-effort and retains cancellation
status rather than replacing it with a notification transport error.

Integration tests use a missing private socket and a malformed address, with no
display, and verify daemon/full-raw status 3, empty stdout and no configuration
writes. A separate invalid-configuration case with the same bad bus address remains
status 1 and read-only. Validation passed: all-feature workspace tests, private-bus
capture/daemon regression tests, daemon signal/disconnection tests, all-target Clippy,
formatting and warning-free manual rendering. Post-connect method-call failures,
child shutdown lifecycle and the remaining CLI syntax cases are still under audit.

### Owned direct-child shutdown

Daemon-launched windows and pin processes now use a shared registry of owned Child
handles rather than only an atomic count. Registration and shutdown are serialized,
new launches are rejected once shutdown starts, and a scope guard terminates and
waits for registered children on daemon exit, including signal/bus-error returns.
Reapers use short nonblocking polls; payload writes do not hold registry/child
locks, so a child that never reads its pin input cannot deadlock shutdown. Completed
Child handles are removed before they could be confused with a reused PID. Idle
checks also reject a busy spawn registry instead of racing an in-progress launch.

Tests cover natural exit, failed spawn, blocked input, idempotent shutdown and
concurrent spawn/shutdown. The isolated X11 suite starts a daemon and a pin, sends
SIGTERM to that known daemon process, checks status 143, and verifies its pin process
was reaped and its window disappeared. Full X11 and native Sway/Wayland suites pass,
as do workspace tests, private-bus daemon/signal tests, Clippy and formatting.

This registry covers directly owned children. Windows subsequently launched by a
launcher child and other descendant/independent CLI ownership cases still need the
remaining lifecycle audit. This is not a claim that the full shutdown gate is closed.

### Ownership across launcher descendants

Owned Flameshot window commands now receive a hidden internal parent identifier.
Before doing work, the child checks its actual parent, installs Linux parent-death
termination through rustix's safe API, then checks the parent again to cover exit
before registration. This is applied both to daemon-owned windows and captures
created by a launcher. It is deliberately an argument, not an inherited environment
setting: independently activated clipboard services and external applications do
not acquire an accidental lifetime dependency.

Linux associates this notification with the spawning thread, not an arbitrary
process-wide lease; these call sites use the long-lived UI/runtime threads. See the
[Linux parent-death signal contract](https://man7.org/linux/man-pages/man2/PR_SET_PDEATHSIG.2const.html).
The application keeps its unsafe-code prohibition. Rustix 1.1.6 was already locked
transitively; the direct process-feature dependency adds no new package version.

The X11 test now starts a real owner/launcher/capture chain, kills the owner, and
waits for both descendant processes and windows to disappear. Existing normal
launcher cancellation/return behavior remains tested. Another case copies from an
owned capture and checks that the independent clipboard service survives its exit.
The no-longer-present owner path is tested headlessly with an invalid bus address:
it exits before capture or configuration writes, and the internal flag stays out
of public help. Sandbox execution and independently invoked CLI window ownership
remain in final lifecycle/packaging acceptance; those are not implied by this test.

Validation passed: the complete isolated Xvfb suite, including two-level owner
termination and independent clipboard survival; the native Sway/Wayland suite;
all-feature workspace tests; private-bus signal/disconnection tests; all-target
Clippy and formatting. No host process policy or installed application was changed.

### Complete-file publication for saved captures

File saves now encode first, write and sync a same-directory staging file, and then
publish it with no-clobber persistence. Collision retries retain the completed staged
file and choose the existing numbered-name convention. The final image name is never
used for an in-progress write, including when an owned capture is terminated with its
parent. Normal output permissions continue to follow the process umask.

Tests cover injected write failure and cleanup, destination invisibility during
partial writes, eight concurrent writers with an existing destination, broken symlink
collisions, normal file modes, supported image formats and actual capture/save output.
A subprocess test stops a writer after partial staging and forcibly terminates it;
the final PNG name remains absent. Forced termination can leave a hidden
`.flameshot-*.tmp` staging file, since destructors cannot run in that case. This change
does not claim power-loss durability of directory entries or universal filesystem
support beyond the no-clobber primitive's behavior.

Validation passed: the all-feature workspace tests, dedicated interruption/output
tests, the private-bus full-capture/save regression, all-target/all-feature Clippy
with warnings denied, and formatting. No installed application or user capture was
modified during these tests; final filesystem/sandbox packaging acceptance remains.

### Scalar integer validation parity

Runtime settings now parse scalar integers through a shared signed 32-bit range
validator, matching the checker and the baseline Qt integer domain. This closes
cases where `config --check` rejected values above i32::MAX but runtime loading
accepted them through u32/usize and later clamped them. It also makes valid signed
zero consistent for nonnegative settings. Existing per-setting limits for opacity,
JPEG quality, geometry mode, arrow style and undo history are preserved. Error
messages state both ends of the accepted range.

The matrix test compares checker/runtime acceptance across all active integer keys,
boundaries, signed zero, overflow, fractional and invalid strings; upload-history
validation is included only when that feature is enabled. Repeated-section tests
verify last-effective-value behavior. Failed updates preserve the original file,
and an oversized drawing setting is rejected by both `config --check` and `full
--raw` before any bus access or capture. Validation passed with no default features,
the all-feature workspace/CLI tests, all-target/all-feature Clippy and formatting.
Legacy fallback/error presentation, inactive optional settings and live-reload
semantics remain in the broader configuration audit.

### Concurrent configuration updates

Rust settings writers now hold an exclusive sibling-file lock across reading,
merging, validation and atomic replacement. The stable `flameshot.ini.rust-lock`
file is separate from the replaced INI inode and is not unlinked after use, so
queued writers continue to coordinate on the same file. The OS lock is released
with the file handle; see [Rust File locking](https://doc.rust-lang.org/std/fs/struct.File.html#method.lock).
General settings and shortcut updates use the same path. Shortcut changes also
validate the complete merged shortcut map while locked, preventing individually
valid partial updates from creating a conflict with existing bindings.

Integration tests launch six CLI writers with disjoint settings and verify every
value plus comments/opaque data survives. A controlled-lock test waits until the
child opens the lock, changes the file while holding it, then releases the child
and confirms it reads the new content rather than a stale snapshot. A shortcut
merge test checks conflict rejection without modifying the configuration and a
successful nonconflicting update. All-feature workspace tests, all-target Clippy
with warnings denied and formatting pass.

The auxiliary lock file may persist after updates, including rejected updates;
read-only configuration checks do not create it. This coordinates cooperating Rust
writers, not arbitrary external editors or a legacy Qt process using a different
locking protocol. GUI form/live-reload conflict handling remains in the remaining
configuration acceptance work.

### Configuration form saves preserve newer unrelated edits

The configuration form records an initial value snapshot and saves only effective
field/shortcut changes. An unchanged save reports that there is nothing to save
and does not rewrite the INI or autostart entry. Untouched fields therefore do not
replace newer CLI/other-window values. Explicit Restore default toolbar remains an
intentional update even when the original form already showed defaults.

General and shortcut patches now merge and validate in one locked INI transaction:
a merged shortcut conflict cannot leave general fields partially committed. File
snapshots advance after that transaction, separately from autostart success, so a
later autostart failure does not break change tracking or subsequent reverts. This
is not a cross-file atomic transaction with the desktop autostart file.

Model tests cover effective defaults, incremental edits, reset intent and pending
autostart state. Settings tests cover all-or-nothing general/shortcut updates. The
native X11 configuration case opens the form, updates the filename pattern through
a separate CLI process, then actually clicks Save both unchanged and after changing
a counter preference. It checks status-label pixels, the requested saved value and
preservation of the newer filename. Untouched form fields are not yet refreshed live;
same-field concurrent changes and complete error/reload presentation remain in the
configuration acceptance audit.

Validation passed on the final implementation: no-default-feature form-model tests,
all-feature workspace tests (including combined INI transaction rejection), Clippy
with warnings denied, formatting, and the complete isolated Xvfb suite with actual
unchanged/edited form saves after an external CLI update.
