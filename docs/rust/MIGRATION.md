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
| Capture | Portal and direct X11; full/screen/gui; cancel/refusal/timeout | Implemented; mock portal, Xvfb, Sway, headless GNOME and KWin 5 capture cases pass; broader compositor acceptance remains |
| Editor | All existing tools, selection, undo/redo, magnifier, color picker | Drawing/text formatting, rendered picking, live movement, selected properties, numeric size entry and drawing grid implemented/tested; remaining parity audit pending |
| Rasterization | Shared preview/export, text shaping, pixelation tests | Shared renderer and secure/insecure pixelation tests pass; exact Qt raster equivalence is not a goal |
| Clipboard | Image/text transfer after editor closes on X11 and Wayland | PNG/JPEG/text and ownership verified on Xvfb/Sway/KWin 5 and the GNOME Xwayland bridge; X11 INCR tested; broader compositor/version paths remain |
| Daemon | Single instance, pins, tray, idle lifetime | Idle ownership, child shutdown, launcher descendants and About/save-directory/monitor tray actions tested; independent CLI/sandbox lifecycle audit remains |
| CLI | Existing commands/options, stdout PNG, exit codes | Commands, numeric/repetition/help rules, parse statuses and completions implemented/tested; final command/shell behavior audit remains |
| D-Bus | Existing service/path/methods and QDataStream payload fixtures | Signatures, NoReply metadata, singleton behavior and real Qt payload fixtures tested; final compatibility audit remains |
| Configuration | QSettings INI and QVariant round trips; unknown keys preserved | All Linux legacy keys have read paths; typed fixtures, atomic updates, idle reload and locked form conflict checks tested; remaining semantic audit pending |
| Resources | Existing icons and 49 translation catalogs; locale fallback | Catalogs embedded and tested; full UI translation coverage pending |
| Desktop | Notifications, autostart, launcher, configuration windows | Tray/windows, sandbox Open With/SaveFile/web links tested with private services; real document FUSE saves verified; installed sandbox and host backend UI acceptance remains |
| Network | Optional Imgur/history and update checking; mock tests | Local HTTP pipelines/preferences and upload/history/update widget flows tested; native network-window integration audit remains |
| Packaging | Debian/RPM/Arch/Flatpak/Snap/Nix Cargo migration | Debian/RPM/Arch locally built; Ubuntu Base Debian install/purge/reinstall and X11 runtime verified; other distro installs and sandbox/provider builds pending |
| CI | fmt, Clippy, tests, optional builds, release build | Rust workflow added; local gates pass; hosted workflow unverified and final release rebuild pending |
| Desktop validation | X11, GNOME/KDE Wayland, wlroots, mixed DPI, IME | Xvfb, Sway/mixed-DPI, X11/Sway/KWin Fcitx Pinyin, headless GNOME and KWin 5 capture/editor cases pass; GNOME IME/mixed-scale/negative-origin and broader scale/rotation acceptance remain |
| Final removal | Remove C++/Qt/CMake after complete Linux acceptance | Pending; legacy source remains a reference, so this is not yet a pure-Rust-only repository |

## Compatibility facts from source

- Binary name: `flameshot`; commands: `gui`, `full`, `screen`, `launcher`, `config`.
- Exit codes: success/help/version and legacy parse rejection 0, runtime general failure 1, cancelled 2, D-Bus connection 3; signals 128+signal. Parse rejection is diagnosed on stderr.
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

### Tray menu and startup-message translations

Tray actions now use the existing TrayIcon catalog entries, including the baseline
Latest Uploads label, and the startup notification uses its QObject translation.
Qt ampersand mnemonics are converted to the D-Bus menu underscore convention instead
of being displayed literally or discarded; escaped ampersands and literal underscores
remain correct. Locale normalization also makes language fallback case-insensitive
for mixed-case tags with an unknown territory.

The private-bus daemon test checks English menu/action behavior, Chinese published
menu labels with explicit `uiLanguage` overriding `LC_ALL=C`, optional-feature labels,
and the Chinese startup notification. Both no-default-feature and all-feature daemon
tests pass, together with workspace tests, core mnemonic/fallback tests, Clippy and
formatting. These are actual D-Bus payload checks, not a claim of native host rendering
or keyboard navigation coverage on every desktop.

The baseline VisualsEditor explicitly requires restart for language changes; this
change retains the process-local translator rather than inventing a hot-reload
requirement. The configuration notice and remaining UI translation coverage still
need completion. The source audit also confirms additional baseline tray actions
(About, Open Save Path and monitor selection) that are not supplied by this translation
change and must remain in the complete tray-compatibility audit.

### Language restart notice in configuration

Restored the baseline VisualsEditor restart instruction using its existing translated
message. The form compares its initial language with the last committed INI snapshot:
unsaved selections do not trigger the notice, repeated/no-op saves do not hide it,
and saving the original language clears it. The notice also survives an autostart
failure after the INI was successfully committed. It scrolls into view on Save rather
than forcing the user to discover a message below the viewport.

Model tests cover the commit/revert lifecycle and catalog lookup. The native X11
configuration test selects a different catalog through the actual language dropdown,
saves it, checks the stored value and visible notice pixels, and the resulting
screenshot was inspected. This retains restart-required language behavior; it does
not add live translation switching or finish the remaining UI translation coverage.

Validation passed: no-default-feature form tests, all-feature workspace tests,
all-target Clippy with warnings denied, formatting and the complete isolated Xvfb
suite, including preservation of newer external settings while saving language.

### About window and diagnostic clipboard

The tray now exposes the translated baseline About action. Its Rust window shows
the application icon, GPLv3+ license, version/revision, actual Rust compiler version,
and kernel/distribution information. Existing InfoWindow catalog entries translate
its labels. Escape closes the window; Copy Info uses the daemon's text clipboard
service so ownership survives the window closing. The hidden child command uses
the existing owned-process launcher and is excluded from public help.

Build metadata accepts the existing GIT_HASH override, then the source archive's
.git-version, then the current repository revision. Git lookup checks the repository
root to avoid borrowing an enclosing repository's revision for extracted sources;
missing Git metadata falls back to a dash. OS release values are parsed as data,
without invoking a shell, and diagnostics exclude the host name.

Validation passed: all-feature workspace tests and all-target Clippy with warnings
denied, all-feature and no-default-feature private-bus daemon tests (English/Chinese
About labels), formatting, the complete isolated Xvfb suite including keyboard Copy
Info, Escape and clipboard persistence, and the headless Sway regression suite.
The About clipboard interaction itself was exercised on X11; this does not establish
native host tray rendering, complete translation coverage or GNOME/KDE acceptance.
Open Save Path and monitor-selection tray actions remain in the compatibility audit.

### Open Save Path tray action

Restored the translated tray action through a hidden owned child command. Each
activation reloads settings, so a changed savePath takes effect without restarting
the daemon. An unset/empty savePath remains a no-op, matching the C++ action.
Existing directories are dispatched to the desktop's default handler through GIO;
relative paths are made absolute before launch to prevent option/URI interpretation.
The launcher shares the bounded process wait used by Open With. The external file
manager is independent of the Flameshot window/process registry. Missing directories
produce a diagnostic and are not created automatically.

A real GIO integration test installs a private directory MIME handler and verifies
single-argument delivery of Unicode, spaces, shell metacharacters and leading-dash
relative paths, unset configuration, and a missing directory. The private D-Bus tray
test clicks the actual menu action after changing savePath while the daemon is
running, with an isolated launcher recording the received arguments. English and
Chinese menu labels and hidden public-help behavior are checked as well.

Validation passed: all-feature workspace tests and all-target Clippy, formatting,
and all-feature/no-default-feature private D-Bus tests plus real GIO dispatch tests.
Existing package manifests already supply GIO. This establishes native desktop
dispatch; sandbox OpenURI/AppChooser portal integration and installed Flatpak/Snap
acceptance remain separate unfinished gates. Monitor-selection tray actions remain.

### Tray monitor selection

The tray now includes the baseline Select Screen submenu when more than one output
is available, with translated monitor descriptions and logical dimensions. Names
preserve literal ampersands, underscores and placeholder-like text. Opening the
root menu refreshes the output list, including removal of the submenu when only one
output remains; unavailable enumeration leaves other tray actions usable.

Each monitor action uses the existing owned child registry and `gui --region
screen<N> --delay 500` capture path, preserving the baseline combined menu-hide and
capture delay. X11 and Wayland share the same screen enumeration/cropping already
used by the CLI. The menu snapshot uses the baseline's numeric monitor selection;
output reordering between activation and capture remains subject to the existing
CLI screen-index semantics.

The isolated two-monitor tray test registers a real private D-Bus watcher, activates
the published second-monitor action, copies the resulting editor selection and
checks both its 512x768 dimensions and red pixels (the first monitor is blue).
Removing the second output and invoking AboutToShow removes the submenu. The test
also keeps exercising normal save-directory and Quit actions after the menu changes.
Reproduction is documented in BUILD.md.

Validation passed: all-feature workspace tests and all-target Clippy, all-feature
and no-default-feature private D-Bus tests, the isolated Xvfb tray activation/removal
test, formatting and the headless Sway regression suite. Actual tray activation was
tested on X11; Sway validation covers the shared Wayland capture/monitor paths rather
than native host tray rendering. Full GNOME/KDE, sandbox and release-package acceptance
remain unfinished, as does final removal of the C++ reference implementation.

### Portal dispatch for opening the save directory

Flatpak/Snap directory actions now use the OpenURI portal's OpenFile method with
a directory descriptor; native desktop launches retain GIO. This opens the directory
itself. OpenDirectory has different semantics: it reveals the supplied item in its
parent. The choice follows the [official OpenURI interface](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.OpenURI.html).
No path string is interpreted by a shell or passed as a file URI. The opened descriptor
is checked to refer to a directory, and the tray request has no parent window.

The request subscribes before calling, carries a unique handle token, distinguishes
user cancellation from rejection, rejects mismatched handles and invalid statuses,
and closes timed-out requests with bounded cleanup. Cancellation returns silently;
portal errors are reported without falling back to a local launcher. The helper
retains the connection until completion, with a two-minute production timeout.

A private-bus mock verifies the received descriptor's device/inode and directory
type, immediate responses preceding method replies, cancellation, rejection, invalid
status/handle and timeout Close. CLI integration covers both sandbox markers with
GIO unavailable and confirms success, quiet cancellation and rejection diagnostics.
Missing paths and non-directory paths are rejected locally. The protocol test is
included in scripts/check-rust.sh. This proves protocol and routing behavior, not
installed Flatpak/Snap confinement or a real host file-manager launch through a portal;
those environment acceptance gates remain open alongside FileChooser/AppChooser work.

Validation passed: all-feature workspace tests, all-target Clippy with warnings
denied, formatting, private-bus daemon/portal tests, and no-default-feature portal
and native GIO directory-dispatch tests. No user desktop file manager was launched.

### Sandbox Open With application chooser

In Flatpak/Snap, Ctrl+O now requests the system application chooser through OpenURI
OpenFile with `ask=true` and a writable screenshot descriptor. It does not enumerate
sandbox-local desktop files. Native desktops retain the existing GIO application
list. Open With reuses existing translated title, instruction and keep-open strings;
portal-specific implementation details do not appear in the user flow.

The asynchronous result preserves the editor on cancellation or errors. Success
honors keepOpenAppLauncher, allowing the chooser to be invoked again or the dialog
to be closed after launch. Unaccepted temporary PNGs are removed when the dialog
closes; accepted images remain available for delayed reads by the external application.
The modal spinner repaints while waiting, and errors permit retry.

The isolated Xvfb/portal fixture drives the actual editor using Ctrl+O, checks that
the received FD decodes as the selected 80x60 PNG, verifies ask/writable options,
and covers success, cancellation, rejection, keep-open behavior and file lifetime.
All external application dispatch is mocked on a private bus. This is not installed
Flatpak/Snap or real host AppChooser UI acceptance, which remains an environment gate.
The new dbus_ui_smoke.py runner builds matching test executables and runs both this
fixture and the tray monitor fixture. It is wired into Rust CI; remote CI has not run.

Validation passed: all-feature workspace tests, all-target Clippy with warnings
denied, no-default-feature portal tests, the complete isolated Xvfb regression and
the combined private-bus UI suite. Both native fixture harnesses and their child
processes now set XDG_CACHE_HOME explicitly: an initial local fixture run exposed
that last-selection caching was otherwise escaping isolation. The corrected suite
was rerun with the default region cache's bytes and modification time verified
unchanged. Formatting and diff checks also passed.

### Sandbox SaveFile chooser and exact-path publication

Non-fixed interactive saves in Flatpak/Snap now use the FileChooser SaveFile portal.
It supplies the suggested name/folder, supported image filters with case-insensitive
patterns, and the current format. Responses must contain one local file URI; remote,
missing and multiple targets are rejected. User cancellation returns to the editor,
while portal/write failures retain the selection and offer retry or cancellation.
Explicit/fixed paths retain their existing direct-save behavior and fall back to the
chooser on failure. Native desktop file browsing remains available.

A selected portal path is saved exactly, without appending an extension or numbered
collision suffix. A filename extension determines encoding; without one, the returned
supported filter determines the format. The SaveFile choice authorizes replacement
of that selected file. Encoding and syncing occur in a temporary sibling before
atomic replacement, and existing permissions are preserved. Other save paths retain
their prior no-clobber numbering behavior. This matches the document portal's
[temporary-file replacement support](https://raw.githubusercontent.com/flatpak/xdg-desktop-portal/main/document-portal/document-portal-fuse.c)
and the [SaveFile contract](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.FileChooser.html).

Private-bus tests cover options, immediate responses, local Unicode URI decoding,
invalid responses, cancellation, rejection, timeout and mismatched-handle cleanup.
The actual editor fixture covers exact-path JPEG replacement without a suffix,
cancellation/rejection preserving the old file and editor, and successful retry after
a missing-parent write failure. Output tests verify existing-file permissions and
preservation on encoding failure. The native no-clobber suite remains in regression.

Real document-portal validation is also available with `dbus_ui_smoke.py TOOL_ROOT
--documents`. The fixture starts its own permission store and document portal with
isolated XDG directories and a private bus, exports existing and new filenames,
grants the application read/write access, and returns paths in the per-application
FUSE view from the mock chooser. Actual editor saves then verify the host file was
created/replaced with the expected JPEG bytes and dimensions. Both cases passed
locally. This validates real document filesystem publication, but not a deployed
Flatpak/Snap container or the host's actual file chooser UI. Rust CI now installs
FUSE utilities and requests this fixture; remote CI remains unrun.

Validation passed: all-feature workspace tests (including prior atomic/no-clobber
publication tests), all-target Clippy, all-feature/no-default-feature private portal
tests, the combined D-Bus UI suite with real document FUSE creation/replacement,
and the complete isolated Xvfb regression. Formatting and diff checks passed.

### Browser links through the sandbox portal

Upload results, upload history and release links now share a link opener. Native
desktops retain egui's existing hyperlink behavior; Flatpak/Snap use OpenURI with an
HTTP(S) URL, retaining hover text and a copy-link context menu. Pending requests are
disabled and displayed with a spinner; cancellation is quiet, and errors allow retry.
This does not launch a sandbox-local browser or fall back after portal rejection.

OpenURI and OpenFile share the request lifecycle, preserving pre-call subscription,
unique handles, cancellation/error distinctions, mismatch cleanup and timeout Close.
Private-bus tests verify exact URL delivery (including query/fragment), immediate
responses, cancellation/rejection, malformed statuses, timeout/mismatch cleanup and
local rejection of non-web schemes. The native Xvfb fixture clicks an actual local
history record's link for success/cancel/rejection with local browser commands
unavailable, verifies the request and retains the window and record. No Imgur upload
or real browser launch is involved. Upload-result/release windows use the same widget;
their complete interactive network flows and actual host browser acceptance remain
separate requirements.

Validation passed: all-feature workspace tests and all-target Clippy with warnings
denied, default workspace compilation, no-default-feature private portal tests,
formatting and the combined isolated D-Bus/Xvfb UI suite. Remote CI remains unrun.

### Launcher and update-window localization

Launcher controls now use existing CaptureLauncher, PinWidget and cancellation
catalog entries. Monitor descriptions preserve output names while translating the
template; delay displays use the baseline No Delay and singular/plural suffixes.
Countdown, process-status prefix, optional path, initial region and title strings
are registered in the catalogs. Update-window titles, status text and actions also
use catalog lookups, and the window now installs the shared script fallback fonts.
Raw OS/network error details remain diagnostic text.

New strings were registered in all 49 catalogs without rewriting existing entries.
Simplified Chinese and traditional Chinese (Taiwan/Hong Kong) translations were
provided for the new strings. Other new translations remain marked unfinished and
use the established English source fallback; this is not a claim that every language
or every Rust UI surface has complete translation coverage.

The new localization_smoke.py uses explicit zh_CN/zh_TW settings with LC_ALL=C,
checks UTF-8 window titles, drives capture/cancellation/return, and records screenshots.
Both rendered launcher screenshots were inspected: Chinese glyphs and controls are
visible without truncation. Updates share the translated catalog/font path but their
actual network-result window was not driven by this fixture. Placeholder/fallback
checks cover the new catalog entries. The fixture is included in Rust CI; remote CI
has not run.

Validation passed: all-feature workspace tests, all-target Clippy with warnings
denied, no-default-feature workspace compilation, focused catalog tests, zh_CN/zh_TW
native launcher lifecycle/rendering, and the complete isolated Xvfb suite in English.
Formatting and diff checks passed. No live update API was called for this UI work.

### Native GNOME Wayland capture and editor fixture

Added gnome_smoke.py using a disposable headless GNOME Shell, virtual monitor,
private D-Bus session, isolated XDG/cache/Pictures/temporary directories and explicit
GNOME/GTK portal backends. System-bus requests from its compositor/services are routed
to the private bus as well. A test-only extension enables evaluation and virtual
input only in that compositor, following [GNOME's debug interface](https://raw.githubusercontent.com/GNOME/gnome-shell/46.0/js/ui/shellDBus.js); it is never installed
in the user's extension directory or enabled in the user's shell. Processes are
tracked and terminated through their owned process groups.

The fixture handles the real GNOME screenshot permission dialog, verifies 1024x768
full and numbered-screen captures, confirms the editor is a native Wayland client,
and exports an 80x60 selection with the expected controlled-background pixels. It
also verifies Escape cancellation (status 2, empty stdout), then removes only the
private screenshot permission and exercises actual Deny (status 1, rejection message,
empty stdout). GNOME startup/overview readiness is explicitly awaited before testing;
window creation is checked before focusing the editor for keyboard input.

Validation passed on GNOME Shell 46.0 with Mutter 46.2. The fixture runs with DISPLAY
unset and Xwayland disabled. It does not establish GNOME clipboard fallback, input
method behavior, mixed-monitor placement, native tray hosts or installed-package
acceptance. Those remain open alongside KDE coverage. No production Rust changes
were needed for the capture/export/cancellation cases exercised here. Hosted CI was
not run, and this renderer-dependent fixture is currently documented for local runs.

### GNOME clipboard persistence and native Wayland recipient

The GNOME fixture now has a `--clipboard` mode. It obtains DISPLAY/XAUTHORITY from
its own compositor and requires the authentication file to be inside its private
runtime directory. All Flameshot editor and GTK receiver windows are verified to be
Wayland clients. The existing Rust clipboard provider uses GNOME's Xwayland bridge
when the compositor lacks the data-control interface.

Actual tests verify full PNG copying after the CLI exits, cropped PNG copying via
Ctrl+C after the native editor process exits, JPEG MIME delivery/decoding and Unicode
text delivery. A focused GTK4 Wayland receiver reads the clipboard asynchronously;
image dimensions and known background pixels are checked, with a small JPEG tolerance.
These tests never use the user's display, bus or clipboard. The default mode continues
to run without Xwayland.

Fixture setup needed two adjustments: the inherited force-software GL setting
crashed Xwayland's explicit-device rendering path, so clipboard mode uses the normal
renderer. This host also makes the private shell appear systemd-managed through its
inherited process scope; its X11 readiness target is absent on the private bus. The
disposable test extension acknowledges that empty session-services target instead
of contacting the host manager. Mutter/Xwayland clipboard handling itself is not
mocked. This is not a full systemd-session test or proof of clipboard support without
Xwayland on GNOME 46. No production Rust changes were required for these cases.

Initial fixture failures generated system crash reports. Their private Xauthority
path/known fixture-parent PIDs established ownership, and only those test reports were
removed. The installed IBus daemon also proved unreliable in the no-Xwayland fixture. That
basic capture-only mode now uses a no-op IBus launcher, explicitly excluding input
method validation. Clipboard mode still runs real IBus. Cleanup stops only the
private shell's IBus child before the compositor, checking its parent/executable name
and using a PID fd to avoid signalling a reused PID. No user IBus session is affected.
Successful final runs left no private GNOME test processes behind. GNOME
IME, multi-monitor behavior, installed packages and remote CI remain unfinished gates.

Validation passed in final runs for both the basic no-Xwayland fixture and the
clipboard-enabled fixture, including the existing capture/export/refusal cases.
A post-run audit found no surviving private GNOME processes and no remaining fixture
Xwayland/IBus crash reports. Diff checks passed; hosted CI remains unrun.


### GNOME multi-monitor capture and fullscreen placement

The private GNOME fixture accepts `--multi-monitor`, adding a 640x480 output beside
its 1024x768 primary output. Distinct background colors identify each output. Tests
verify the 1664x768 desktop capture, both numbered captures, and second-monitor
selection through both `--region screen1` and the editor's number-key chooser.
They check exported dimensions/pixels and the native window's actual monitor index
and fullscreen rectangle, rather than inferring placement from exported data alone.

The ordinary region/cancellation cases now explicitly select the first monitor when
the chooser is present. The clipboard fixture derives full-desktop dimensions from
the monitor layout, allowing both options to be combined. A timed-out editor also
records a private compositor screenshot for diagnosis. This adds same-scale GNOME
coverage; mixed scaling, negative origins, hotplug and IME are not established by it.

Validation passed in both multi-monitor-only and combined multi-monitor/clipboard
runs, including the existing capture, cancellation, refusal and clipboard checks.
The post-run process audit found no private GNOME processes remaining. Diff checks
passed. No production Rust changes were required; hosted CI remains unrun.

### Update-window completion and long-result handling

The update window now distinguishes a pending worker from a disconnected response
channel. A worker that stops without sending a result produces an error instead of
leaving a permanent spinner. Buffered results are still consumed before disconnect
is reported. Result/link content uses a bounded scroll area so the Close control has
reserved space even when diagnostic text is long.

State tests cover pending, disconnected, no-update and available-update completion.
All-feature workspace tests, all-target Clippy, no-default-feature compilation,
formatting and diff checks passed. No live update endpoint was contacted for this
change. Native long-message layout and complete interactive update-server scenarios
remain part of the UI acceptance work; these state tests do not establish them.

### Open With completion/close temporary-file ownership

Closing Open With now consumes a buffered cancellation/failure before releasing its
temporary image. A worker completing after the receiver is gone removes an image
that was never accepted. A shared completion lock closes the race between checking
an empty receiver and dropping it. It is held only for result handoff/cleanup, never
while launching an application or awaiting a portal response. Successful launches,
including an earlier success followed by another failed attempt, retain their image
for the external consumer. Unknown outcomes after process termination or worker
failure still conservatively retain the file.

Tests cover buffered and late completions, success/cancellation/failure and prior
acceptance. These and the combined isolated D-Bus/Xvfb UI suite passed, including
Open With cancellation, keep-open behavior and file lifetime. All-target all-feature
Clippy passed; no real external application or browser was launched by the UI suite.
No-default-feature compilation, formatting and diff checks also passed.

### Disconnected save/copy workers

The save portal, native folder listing and About clipboard job now treat a dropped
worker channel as a terminal error instead of an indefinitely pending operation.
This releases pending controls through their existing error paths. A shared polling
helper distinguishes a live empty channel, a buffered success/error, normal portal
cancellation and disconnection; buffered results retain priority over sender exit.

Validation passed: channel-state tests, all-feature workspace tests, all-target
Clippy, no-default-feature compilation and the isolated D-Bus/Xvfb UI suite covering
normal portal cancellation, saving/retry, Open With and history links. Formatting
and diff checks passed. These checks do not complete the remaining desktop or
packaging acceptance gates.

### Open With interrupted-worker recovery

A disconnected application-list worker now ends loading and displays an error.
A disconnected launch worker releases pending controls for closing or retrying,
but records an uncertain launch separately from confirmed success. Its temporary
image is retained because a consumer may already have received it. Later failed
retries also retain that image; uncertainty alone never marks the capture accepted.
Known cancellation/failure cleanup remains unchanged.

Tests verify pending/disconnected transitions, no false success, retention across a
failed retry, buffered completion and late-result cleanup. They passed with the
isolated D-Bus/Xvfb UI suite, all-feature all-target Clippy, no-default-feature
compilation, formatting and diff checks. This is conservative file retention for an
unknown outcome, not a claim that a stopped worker's external side effect can be
reliably determined.

### Current optimized payload validation

Built the locked all-feature release application and platform examples at
`ee049bd49ae685233c4b16edda916e862a0862ad`, staged a fresh /usr payload, and validated
its metadata, installed resource set, executable identity and absence of Qt linkage.
The staged binary passed the full Xvfb and Sway suites, GNOME multi-monitor/clipboard
fixture, and shell completion checks. Its SHA-256 remained unchanged throughout.
See RELEASE-VALIDATION.md for provenance and scope. This updates local release-build
evidence but does not complete installation, sandbox, hosted CI or final source-removal
gates, and later production changes require a new release validation snapshot.

### Staging-prefix boundary and reproducible payload checks

The staging helper now rejects parent-directory components and line breaks in PREFIX
before creating output. The previous absolute-path-only check allowed a parent
component to escape the intended DESTDIR, and line breaks could corrupt activation
metadata. Normal absolute prefixes, including spaces, remain supported.

Added staging.py to verify installed binary bytes, modes, completion/manual/D-Bus
resources, quoted activation paths, absence of Qt catalog files and desktop/AppStream
metadata. Invalid prefixes must fail without creating even a partial destination.
The regression passed against the current release executable and is wired into Rust
CI; hosted CI remains unrun. All test payloads were temporary, with no live install.

### Vendored archive built without a registry cache

Generated the source archive at `r2440.55168bb3`, extracted it into a fresh directory,
and built/tested all workspace features with --frozen, offline mode, an empty Cargo
home and an independent build directory. Both commands passed. The extracted staging
regression passed as well. Build metadata used the archived revision, and no registry
source cache was populated. SOURCE-VALIDATION.md records the artifact hash and the
installed-toolchain/system-dependency limitations of this evidence. This does not
complete clean-system installs, hosted CI or removal of the checkout's C++ reference.

### Upload and history control localization

Upload confirmation, progress, copy-URL feedback, close/delete controls and history
headings now reuse their existing ImgUploadDialog, ImgUploaderBase, UploadHistory,
UploadLineItem and generic-control catalog entries. The upload window retains Imgur
in its title so the destination remains explicit. The shared link context menu also
uses the existing Copy URL translation. Network/worker diagnostic details and some
compound status messages remain outside this localization change.

The Chinese localization fixture now opens a synthetic local history record when
Imgur support is built, checks the translated native title, captures the window,
and closes it while verifying the record is unchanged. Both simplified/traditional
Chinese screenshots were inspected with readable controls and no truncation.
Five upload/history state and local-HTTP tests, the isolated D-Bus/Xvfb suite,
all-target Clippy, default/no-default-feature compilation, formatting and diff checks
passed. No real upload, deletion or browser launch was performed. Complete UI
translation and remaining interactive/network acceptance are still unfinished.

### Text annotation shortcuts during IME composition

The text dialog now tracks composition events before interpreting annotation-level
Escape/commit shortcuts. Active preedit and the frame that commits or dismisses it
are protected; Apply is disabled for that frame. Explicit dialog cancellation remains
available, and a subsequent Escape can cancel the annotation normally. This prevents
composition handling from accidentally closing or applying the entire annotation.

Synthetic egui UI frames verify Chinese preedit/commit without duplicate text,
protection against the commit shortcut, one-step undo, and Escape dismissal followed
by normal annotation cancellation. Existing text-edit tests also pass, together with
all-feature all-target Clippy, no-default-feature compilation, formatting and diff
checks. These event-level tests do not establish native IBus/Fcitx integration or
candidate-window placement; those remain required desktop acceptance work.

### Native Fcitx/XIM and text-field clipboard integration

A real Fcitx Pinyin fixture exposed that eframe's clipboard feature was missing:
text widgets generated CopyText commands, but no OS clipboard integration handled
them. Depending on arboard directly had not enabled eframe's integration. The
application now enables eframe/clipboard and removes that redundant direct dependency;
Cargo.lock includes the resulting Wayland clipboard integration dependencies.
Screenshot clipboard ownership continues to use the existing daemon service.

ime_smoke.py runs Fcitx on a private Xvfb/D-Bus session with isolated configuration,
data and caches, and disables Cloud Pinyin. It verifies Chinese “你好” committed via
XIM, exact text copying, pasting over replacement text, cancellation of an uncommitted
Pinyin sequence without changing committed text, annotation application and PNG export.
The exported glyphs were visually inspected and checked to be contained in the crop.
The fixture explicitly sizes the window because Xvfb has no fullscreen window manager.
It is wired into Rust CI with the Pinyin engine dependency; hosted CI remains unrun.

Validation passed: the native IME fixture, all-feature workspace tests, all-target
Clippy, no-default-feature compilation, the isolated D-Bus/Xvfb UI suite and native
Sway regression. Temporary event tracing was removed. This establishes the tested
X11/Fcitx path, not all input engines, candidate positioning or native Wayland IME
acceptance. Earlier release/source snapshots predate this dependency change and must
be regenerated before final release.

### Feature-matrix runtime checks after clipboard integration

The complete existing check-rust.sh gate passed after enabling eframe's clipboard
integration: formatting, default/all-feature Clippy and workspace tests, private-bus
portal/service/notification checks, daemon signal/bus-loss statuses and completion
checks. Minimal builds previously only received cargo check in this gate.

The gate now runs the no-default-feature workspace tests and private-bus daemon test
as well, and checks completions before and after compiling that minimal binary. The
new minimal commands passed locally, followed by live Bash/Fish completion checks
and Fish/Zsh syntax checks against the minimal executable. This extends repeatable
feature-matrix evidence; it does not complete native Wayland IME, packaging installs,
hosted CI or the final removal of C++ reference sources.


### Native Wayland Fcitx Pinyin verification

The isolated Sway suite now accepts --ime. It starts Fcitx with a private Pinyin
profile and Cloud Pinyin disabled, while DISPLAY remains unset and Xwayland stays
disabled. Latin key events are sent through the virtual keyboard; the actual input
engine must produce Chinese text. The test verifies the exact copied string, cancels
a second uncommitted composition without losing the committed text, applies the text
annotation and checks the exported dimensions and text pixels. The resulting Chinese
glyphs were visually inspected.

The full Sway suite passed with this option, including its existing image clipboard,
pin, monitor scaling, selection and confirmation cases. No production Rust changes
were needed. Rust CI now requests this option, but hosted CI remains unrun. This
establishes the tested native Wayland/Sway/Fcitx path; GNOME input-method integration,
other engines, candidate-window placement and broad language coverage remain separate
acceptance requirements.


### Current Debian package clean-root installation

Built the current vendored snapshot through the Debian recipe and installed the
result in a new signed Ubuntu Base 24.04.5 filesystem, using APT to resolve declared
runtime dependencies. Installed CLI checks, payload verification and the complete
Xvfb desktop suite passed without Qt runtime packages. Purge/reinstall preserved
user configuration and restored the verified package payload.

The test container required an init process for orphan reaping. The desktop fixture
now accounts for the absent update-check row in distro builds when selecting a
language. DEBIAN-INSTALL-VALIDATION.md records hashes, root-image provenance, build
and install distinctions, and remaining platform gates. Test containers were removed;
no host installation was replaced.

### Actual Qt-package to Rust-package upgrade

Verified an APT upgrade from Ubuntu's Qt-based Flameshot 12.1.0-2build2 to the local
Rust 15.0.0-1 package in a separate container. The old executable generated the
configuration through its CLI. Upgrade retained the complete file, Rust validation
accepted it, and a subsequent Rust filename update preserved every unrelated line.
Installed package checksums and absence of Qt linkage also passed. The test container
was removed. DEBIAN-INSTALL-VALIDATION.md records versions, hashes and scope; this
is not evidence for all legacy settings or upgrading a running Qt GUI.

### Text editor and monitor chooser localization

Text-dialog title, font/size controls, empty-text guidance and Apply now use the
RustEditor catalog context; Cancel reuses the existing catalog entry. Monitor-picker
title/instructions and the editor's font-family label use the same translation
mechanism. Seven new strings were registered in all 49 catalogs, with simplified
and traditional Chinese translations and unfinished/source fallback elsewhere.

The native Fcitx fixture now accepts FLAMESHOT_TEST_LANGUAGE for localized UI checks.
Its Chinese run passed commit/copy/paste/composition cancellation/export, and the
rendered text dialog was inspected with readable, unclipped controls. Text-edit
state tests, all-feature Clippy, no-default-feature compilation, and the full Sway
suite with native IME also passed. This does not complete translation coverage of
all remaining controls or all languages.

### Refresh configuration after successful saves

After a successful Save (including a no-change Save), the form reloads the actual
settings and rebuilds its field/shortcut/startup baselines. Untouched fields changed
by another process are therefore displayed correctly instead of remaining stale
after the save merged them on disk. The original window language is retained for
restart-notice comparisons. A failed reload reports that separately from successful
persistence, while retaining the committed form snapshot.

Model tests cover changed external fields, path/shortcut/startup baselines and the
language notice. They and the full isolated Xvfb suite passed, along with all-feature
all-target Clippy. This is post-save synchronization, not completion of background
refresh or same-field concurrent-edit handling while a local draft is pending.
No-default-feature form tests, formatting and diff checks also passed.

### Opaque configuration preservation regression

Added a transaction-level regression for CRLF INI files containing an opaque future
General QVariant value, an unrecognized shortcut and an application-specific group.
Updating a known General setting and known shortcut changes only those lines. The
unknown shortcut is not activated, and a subsequent conflicting shortcut edit leaves
the complete file unchanged. The existing implementation passed; no production
behavior change was needed.

All settings tests and all-feature all-target Clippy passed. This verifies opaque
preservation during writes, not that the diagnostic config checker must accept
unknown keys or that future shortcut formats are supported.
The focused no-default-feature test, formatting and diff checks also passed.

### Preserve autostart entries when configuration commit fails

Startup changes now snapshot the original entry before applying the change and
committing startupLaunch. The former boolean-only rollback could replace a custom
desktop file with default contents, or lose a dangling symlink. Linux hard-link
snapshots preserve the original entry and its permissions; failures restore it by
rename. If restoration itself fails, the backup is retained and its location/error
is reported instead of silently discarding the original. Cooperating Rust startup
updates use a persistent lock. Backup failure aborts before changing the entry.

Tests cover custom content/mode, symlinks, previously absent entries, successful
commit and failed restoration with recoverable backup. An actual CLI test forces
INI validation failure and verifies both the original startup entry and invalid INI
remain intact. Platform tests, full/minimal config integration checks, all-feature
Clippy, formatting and diff checks passed. This is rollback on reported failure,
not crash/power-loss atomicity across two filesystem resources or coordination with
legacy Qt/external writers.

### Refresh idle configuration windows without replacing drafts

Configuration windows now poll settings once per second while open. A clean form
adopts external changes and updates its committed baseline without writing the file.
Any pending General, shortcut, startup or explicit toolbar-reset edit postpones the
refresh, so background reads cannot erase a local draft. Read/parse failure preserves
the current form and reports an error; a later successful refresh clears the error.
Language changes continue to show the restart notice against the window's original
language. This does not add same-field conflict detection: saving a pending local
change still uses the existing delta-update semantics.

Seven all-feature configuration model tests and all-feature all-target Clippy passed.
The private Xvfb fixture additionally checks rendered fields change after an external
CLI filename update without clicking Save and verifies the INI remains unchanged.
The full Xvfb suite and all seven no-default-feature form tests passed, along with
formatting and diff checks.

### Localize configuration controls and shortcut descriptions

The configuration window now translates its title, field labels, choices, toolbar
names, save status and reload-error prefix. All 79 RustConfig messages are registered
in all 49 existing catalogs; Simplified/Traditional Chinese entries are supplied,
and 822 unambiguous existing translations are reused across other locales. Remaining
unfinished entries retain source-text fallback. No Qt translation build dependency
or runtime converter is introduced. Shortcut settings display human-readable action
names, reusing legacy ShortcutsWidget translations for selection operations, instead
of exposing internal TYPE identifiers. Stored shortcut keys and INI syntax are unchanged.

Catalog tests check Chinese coverage and error placeholders; configuration tests
ensure every configurable shortcut has a readable label. All-feature form tests and
all-target Clippy pass. Private Xvfb localization checks cover simplified/traditional
titles, scrolling, Save/Escape and unchanged INI, alongside launcher/history checks.
Rendered top/bottom configuration screenshots were inspected. The fixture's pointer
was moved inside the narrower Chinese content area so wheel events actually scroll.
Backend diagnostic details and other editor windows still need translation review;
this is not completion of full application translation coverage.
The eight no-default-feature form tests, formatting and diff checks also passed.
The rendered Chinese unchanged-save status was inspected after clicking Save.

### Translate remaining capture controls and native Save UI

Capture help, size controls, arrow options, object panel, selection errors, quit
buttons and native Save dialog labels now use translation catalogs. Twenty additional
RustEditor messages are registered in every catalog, with Simplified/Traditional
Chinese translations and reuse of unambiguous existing translations elsewhere.
Object rows reuse translated tool names. Incorrect source/context pairs for capture
selection, object/selection movement, Upload and Open With were corrected; these had
silently fallen back to English even in Chinese sessions.

The private localization fixture now captures the actual editor and Save dialog in
both Chinese locales. It selects the screen, opens Save, cancels back to the editor,
and cancels capture back to the launcher. Its save directory is inside the disposable
XDG tree, and no screenshot is saved through the dialog. Rendered screenshots were
inspected; this caught the Upload/Open With fallbacks that static compilation missed.
All 49 catalogs parse with 27 distinct RustEditor messages. Six translation tests and
73 all-feature application unit tests pass (one existing ignored test); all-feature
all-target Clippy passes. Backend diagnostics and other windows remain separate
translation audit work, so this does not close the global resources acceptance gate.
Final Chinese editor screenshots confirm Upload/Open With are translated. The
65 no-default-feature application unit tests also pass (one existing ignored test),
as do formatting and diff checks.

### Localize application chooser feedback and record terminal parity gap

The native application chooser now translates its search hint, empty-list feedback,
Close button and disconnected-worker errors. Filtering an existing list to no matches
shows a distinct translated message instead of a blank list. Six messages were added
to all 49 AppLauncherWidget catalogs, with Chinese translations and existing
unambiguous translations reused elsewhere. Catalog XML and duplicate keys checked.

The private localization fixture opens the native chooser after cancelling Save,
captures it in Simplified/Traditional Chinese, closes it with Escape and returns to
the launcher. No listed external application is started. Screenshots were inspected;
application names themselves follow the desktop entries/environment locale. The
three Open With lifecycle tests and all-feature all-target Clippy pass, along with
the expanded localization fixture, formatting and diff checks. Search no-match
feedback is implemented; this fixture does not automate entering the search query.

Source comparison found a remaining behavior gap: legacy applauncherwidget.cpp
provides a manual “Launch in terminal” checkbox in addition to honoring the desktop
entry's Terminal flag. Rust currently delegates the entry's Terminal behavior to
GIO but has no manual override. Implement and validate this native option, including
Desktop Entry argument expansion, spaced image paths and launcher failures, before
closing the desktop compatibility gate. Sandbox application selection remains owned
by the OpenFile portal and cannot promise that native override.

### Restore native manual terminal launch

The native Open With dialog now exposes the legacy translated “Launch in terminal”
checkbox. Choosing an application with it enabled stages a private Desktop Entry
with Terminal=true and DBusActivatable=false, then delegates execution to GIO.
The original entry and action groups remain untouched. Exec %k is bound to the
original entry path; remaining argument/field-code expansion stays with GIO.
Temporary entries are removed after launcher completion. Missing/empty Exec commands
and unavailable terminal launchers report failure through the existing dialog path.
Portal selection remains compositor-owned and does not show this native-only option.

Quoting follows the Desktop Entry Exec rules:
https://specifications.freedesktop.org/desktop-entry/latest/exec-variables.html
Tests use real GIO with a synthetic terminal and verify paths containing spaces,
percent signs, quotes, backslashes and shell metacharacters, plus %f/%k/%c/%i/%%
expansion, unchanged source/action groups and missing-terminal failure. The four
platform application tests and three Open With lifecycle tests (full and minimal
features) pass. All-feature all-target Clippy, formatting and diff checks pass.

The new private-Xvfb terminal_smoke.py clicks the native terminal checkbox and launches
a synthetic receiver through a synthetic terminal. The receiver verifies the 80x60
PNG and original desktop path; the fixture verifies terminal dispatch occurred.
No real user terminal/application is started. The fixture is added to Rust CI;
hosted execution remains unverified. Terminal discovery is GIO's host policy rather
than the legacy hardcoded preference list; acceptance across installed terminal
emulators is still part of broader desktop validation. GIO launch metadata may name
the temporary entry, although the application's %k argument names the original.

### Revalidate combined changes and include all packaged shell completions

After the configuration refresh, UI localization and manual terminal launch changes,
the complete scripts/check-rust.sh gate passed: default/all-feature lint and workspace
tests, private-bus portal/capture/daemon/notification tests, daemon exit cases,
minimal-feature workspace/daemon tests and Bash completion checks. The native terminal
fixture also passed against the minimal executable left by the gate.

The gate previously tested Fish/Zsh only when developers invoked completions.py
separately. It now accepts FLAMESHOT_COMPLETION_TOOL_ROOT and uses that root for both
all-feature and minimal completion checks. Rust CI installs Fish/Zsh and sets the
root to /, so those checks are explicit rather than silently absent. BUILD.md lists
the option, its requirements and the native terminal fixture. With the extracted
local tool root, the entire updated gate passed again and logged Fish live completion
plus Fish/Zsh syntax success twice. Shell syntax and diff checks also pass.

These are local results; hosted CI and the remaining CLI behavior audit are not
closed. The existing parser-status discrepancy remains: legacy parse rejection can
return zero through main.cpp's finish label, while Clap rejects arguments nonzero.
No blanket exit-status change was made during this verification pass.

### Real KDE portal and native Wayland editor validation

Added a guarded KWin input helper using the already-locked Plasma Wayland protocol
crate as a test-only dependency, plus a private headless KWin fixture. In a disposable
Ubuntu container, real KDE portal full and explicit-monitor screenshots match
1024x768/background pixels; the native Wayland editor exports an accepted 80x60
region; pending-request cancellation returns 2 with empty stdout. No host display,
bus, XDG data or GPU device was shared. Details, versions, evidence limits and
minimal-container setup are in KDE-VALIDATION.md.

Environment issues were resolved inside the container: remove the unusable KWin
file capability, install its separate virtual backend, use software OpenGL rather
than QPainter, and provide the normal Plasma QML/theme/package components. The input
helper is granted its interface through a private desktop entry, without disabling
KWin interface checks. Final cleanup left no test desktop processes. Helper build,
all-feature all-target Clippy, formatting, Python syntax and diff checks pass.

This adds actual KDE 5 evidence to the acceptance matrix; it does not claim KDE 6,
clipboard/IME/mixed-DPI acceptance or a complete Plasma desktop session. Cancellation
can occur at the portal consent stage. Hosted execution remains unverified.

### Native KWin clipboard ownership and transfer

Extended the private KDE fixture with --clipboard. A tracked daemon owns the private
session clipboard; independent Qt clients explicitly verify they use Wayland.
After the capture/editor process exits, receivers validate PNG/JPEG MIME, actual
encoding, dimensions and pixels, plus exact Unicode text sent through the legacy
D-Bus method. No DISPLAY or Xwayland fallback is available in the fixture.

The test then enables idle auto-close, verifies the Rust owner remains alive while
holding the selection, and replaces it using a separate native client and private-seat
keyboard event. The daemon releases its bus name and exits with status zero; another
reader obtains the replacement text. All cases passed against KWin 5.27.11, with
no desktop processes left in the disposable container. Python syntax and diff checks
pass. KDE-VALIDATION.md records reproduction and limits; IME/mixed-DPI, other KWin
versions and installed sandbox behavior remain unverified.

### Fix interactive monitor switching on KWin and verify mixed scaling

A two-output KWin fixture now verifies scales 1/2 and the compositor's actual layout,
full-desktop and individual-monitor pixels, explicit second-screen selection, and
interactive keyboard selection. A guarded test helper reads native window geometry,
so accepted export pixels alone cannot conceal a window on the wrong monitor.
KWin normalizes negative requested positions; this is not negative-origin evidence.

The new check found that interactive selection changed the screenshot but left the
fullscreen editor on the first monitor. On Wayland the editor now leaves fullscreen,
waits for the reported state transition, then targets the chosen monitor. A bounded
wait avoids indefinite stalls if the compositor does not acknowledge the request.
The existing X11 path is unchanged. Both explicit and interactive selection now
produce the correct native (0,0,512,384) logical frame and 1024x768 orange export.

KWin mixed-scale/clipboard, GNOME dual-monitor and Sway mixed-scale/Pinyin regressions
passed. All-feature application unit tests (73 passed, one existing ignored),
all-feature all-target Clippy, formatting and Python syntax pass. Details and snapshot
hash are in KDE-VALIDATION.md. Fractional-scale/hotplug/KDE IME work remains open.
The minimal-feature application unit tests also pass (65 passed, one existing
ignored test). The disposable container was stopped and removed after verifying
that no test desktop processes remained.

### Prevent adjacent-monitor pixels in fractional-scale crops

The KWin 1x/1.5x fixture found a one-column leak: the full portal image separates
outputs at x=1025, but outward rounding started the first monitor at x=1024.
Monitor crops now use shared nearest-pixel partition boundaries. Ordinary selection
crops retain outward rounding. A core regression covers both horizontal and vertical
shared boundaries, negative desktop origin, full coverage and non-overlap.

KWin fractional full/screen/editor/interactive-choice cases now pass, including native
window geometry and edge colors. The compositor itself assigns 1025 columns to the
second monitor in its composite; the fixture verifies that actual image partition,
not an assumed 1024 physical-mode width. KWin integer-scale and GNOME multi-monitor
regressions also pass. Sway now verifies 1x/1.5x negative-origin geometry and exact
pixel equality with disjoint regions of its full portal image. Initial uniform-color
assertions were replaced with source-image equality because resampling/previously
rendered content was already present in that source, not introduced by cropping.
The complete Sway/IME run passes.

All-feature workspace tests, focused core geometry tests, all-feature all-target
Clippy, formatting, Python syntax and diff checks pass. KDE-VALIDATION.md records
hashes, dimensions, reproduction and evidence limits. The container was stopped and
removed after cleanup verification. Hotplug/KDE IME and broader scale combinations
remain open; this is not completion of the whole migration.

### KWin output disable/re-enable with active editors and chooser

Extended the native window probe to report the fullscreen protocol flag and added
--hotplug to the KWin fixture. Both integer and fractional mixed-scale configurations
pass: an editor on a disabled virtual output moves to the remaining output, remains
fullscreen and exports its captured image unchanged. Fresh captures observe the
single-output layout, reject its obsolete screen number with status 1/empty stdout,
and recognize the second output again after re-enabling it. Disabling an output
while the chooser is open also preserves its snapshot and fullscreen placement when
that snapshot screen is chosen afterward.

The successful final driver gives portal consent controls one second before sending
Enter. An earlier fractional run returned actual cancellation and was not counted as
success or automatically retried by application code. The application itself needed
no change for these lifecycle cases. Tests cover connected virtual outputs being
disabled/re-enabled, not physical connector removal/GPU behavior. Detailed evidence
and reproduction are in KDE-VALIDATION.md. Helper build, all-feature all-target Clippy,
formatting, Python syntax and diff checks pass; the private container was cleaned up.

### Native KDE Wayland Chinese input and composition lifecycle

Added --ime to the isolated KWin fixture. KWin launches Fcitx on its dedicated input
method connection, with a private Pinyin profile and cloud input disabled. ASCII key
events commit exact Chinese text, native clipboard reads verify UTF-8, Escape cancels
a subsequent preedit without closing the editor or changing committed text, and the
annotation exports with visible Chinese glyphs. The 400x200 artifact was inspected.
No DISPLAY, Xwayland or user input-method session is involved.

The fixture now waits for the private input-method PID and bus name to disappear
before compositor cleanup; the final run has no duplicate termination signal/crash
report and leaves no desktop processes. Native KWin capture/editor/cancel checks
also pass with the input method enabled. Python syntax and diff checks pass. No
production application changes were required. KDE-VALIDATION.md records versions,
setup and limits, including untested candidate geometry/other engines/KDE 6. The
container was stopped and removed after saving local evidence.

### Preserve legacy command-line numeric bounds

Source audit of main.cpp's numericChecker shows delay and screen number are parsed
through QString::toInt and must be nonnegative. Rust's u64/usize parsing previously
accepted values beyond that signed 32-bit range. All three delay options and the
screen-number option now require 0..2147483647, while retaining surrounding whitespace
and a leading plus sign. Oversized delays are rejected before capture waiting begins.
The manual documents the bounds and the additional active-monitor requirement.

Parser tests cover all capture commands, zero/max bounds, whitespace/plus, overflow,
negative, fractional and empty inputs without executing large delays. Executable tests
verify overflow fails with empty stdout before attempting a deliberately unavailable
bus or writing configuration. All-feature parser/six CLI integration tests and Clippy
pass. This restores accepted numeric values, not legacy parser failure statuses:
Rust still reports Clap's nonzero rejection, whereas baseline parse failure can return
zero. Repeated options and help/version trailing-argument behavior also remain in the
CLI audit. In particular baseline value() selects the first repeated value; a generic
last-value-wins override would not preserve that behavior.
The same parser and six executable CLI tests pass with no default features.
Formatting, diff checks and warning-free man-page rendering also pass.

### Preserve repeated capture/config options and QColor CLI validation

The baseline parser records repeated options and value() selects the first matching
value. Capture/config value arguments now use append parsing with first-value
extraction, so every occurrence still passes its value parser. Switches allow repeated
occurrences without changing their enabled state. No global last-value-wins override
is used. Help/version behavior is separate and remains under audit.

This exposed two prerequisite validator gaps. CLI interface colors now use the
existing QColor-compatible palette decoder, require opacity, and normalize to
#RRGGBB. Qt shorthand/high-precision forms and opaque #AARRGGBB are accepted; the
former integration test incorrectly accepted a translucent Qt ARGB value and rejected
#RGB. Expectations were corrected against the baseline colorChecker and an isolated
Qt 5.15 runtime probe. Named-color whitespace/case handling is supported. This is
CLI normalization; the existing stored/drawing color decoder is not rewritten.

Output paths validate the baseline existing-directory/existing-parent condition for
every occurrence before desktop actions. The typed path parser retains non-UTF-8
paths. Filename-pattern strings have no legacy parser checker; their selected first
value still undergoes the existing settings validation when applied.

Four parser tests, six CLI-help integration tests and nine config tests pass with
all features and no default features. Actual config invocations verify first values
are saved, aliases participate, and later invalid typed values prevent all writes.
The private D-Bus mock-portal full-capture integration test verifies PNG stdout and
collision-safe saving. All-feature all-target Clippy and Bash/Fish/Zsh completion
checks pass; the manual documents repetitions and opaque QColor formats and renders
without warnings. Formatting/diff checks pass. Legacy parse-error status and remaining
help/version/syntax semantics are still unclosed CLI acceptance items.

### Restore legacy help/version grammar and parse-rejection status

Compiled the unchanged CommandLineParser/CommandArgument/CommandOption sources against
Qt 6 with a stream-only logger and a small headless harness under target/qt-fixtures.
The harness covers parser behavior, not capture/config execution; main.cpp's finish
path separately establishes zero return after parse failure. Local output is retained
in target/cli-reference-results.json. Cases confirm immediate/final help placement,
version-with-trailing-arguments rejection, rejection of clustered short switches,
and literal --help consumed as a filename option's value.

The Rust entry point now checks legacy option shape using its existing Clap schema,
then delegates typed validation to Clap. Separate values are normalized to inline
values without losing native path bytes, including dash-leading values and the
legacy empty-equals-plus-next-value rule. Earlier invalid values retain priority
over later help/syntax errors. The Rust help subcommand and -V alias remain documented
extensions. Parser errors print to stderr and return zero as the baseline does;
help/version print to stdout. No command executes after a parse error. Runtime
operation failure/cancellation/bus statuses keep their separate handling.

Tests exercise the real entry point with unavailable desktop services and isolated
configuration: valid help/version, invalid combinations, unknown arguments, literal
help/version filename values, no side effects on rejection, and the existing typed
validation/repetition/non-UTF-8 cases. Config parser-error tests now require stderr,
empty stdout and unchanged files instead of assuming a nonzero status.

The complete check-rust.sh gate passed with FLAMESHOT_COMPLETION_TOOL_ROOT enabled:
default/all-feature lint and workspace tests, private-bus integration/daemon exit
tests, minimal builds, Bash/Fish live completions and Fish/Zsh syntax. Formatting,
diff checks and warning-free man rendering pass. The manual explicitly documents
legacy zero-status parse errors. This closes the previously recorded blanket parse
status discrepancy, not every remaining command/config semantic audit item.

### X11 regression after CLI compatibility changes; concrete text-tool gap

The full private-Xvfb desktop suite passed against the current all-feature executable
after the CLI grammar/status changes. It covers valid capture options, save retry and
cancel, image/text clipboard ownership, launcher/pin/daemon lifetime, configuration
refresh and preservation, selection, drawing, object movement/size, undo/redo and text
editing. The run is recorded in target/cli-desktop-regression.log.

The first run exposed a stale test coordinate: the expanded localized capture-label
width moved the Size control. The test pressed the gap before it and dragged the
entire toolbar, so its size-preview assertion failed. The saved image showed that
movement. The fixture now presses the Size control's center; the subsequent complete
run verifies live size preview and single-step undo/redo. This was a test targeting
fix, not an application drag behavior change. Python syntax and diff checks pass.

Source comparison also turned the generic editor-parity remainder into a concrete
missing feature: src/tools/text/textconfig.cpp exposes Bold (normal/bold toggle),
Italic, Underline, StrikeOut, Left Align, Center Align and Right Align. TextTool copies
and draws these font/alignment properties. Rust Annotation::Text currently stores
only position, string, size, family and color, and its renderer/editor expose none of
those formatting properties. They must be represented per annotation and supported
in shared preview/export/picking, edit cancellation and undo/redo, with the existing
TextConfig translations reused. This remains implementation work, not a completed
editor acceptance claim; arbitrary font-weight choices are not a legacy requirement.

### Implement legacy text formatting in the Rust editor

Text annotations now carry bold, italic, underline, strikeout and left/center/right
alignment. Defaults are normal, undecorated and left aligned. The shared cosmic-text
renderer selects font weight/style and draws font-metric decorations. Multiline
alignment uses the annotation's natural widest line rather than the capture canvas;
preview, export and rendered picking all consume the same per-annotation properties.
Move/size/color operations preserve them through the existing clone/edit model.

The text tool and text-edit dialog expose all seven legacy TextConfig controls and
reuse their existing catalogs. Draft formatting previews without changing history;
Apply creates/replaces one annotation, Cancel preserves the document, and undo/redo
restores the complete formatting state. Applied new-text formatting becomes the
current capture tool's defaults; editing an existing annotation does not change those
defaults. A changed family on new-text Apply is persisted through the existing settings
transaction. Formatting does not introduce new INI keys or an external Qt dependency.

Core tests verify pixel changes for each emphasis/decoration, multiline alignment,
selection on decorations, exact preview/export equality and undo/redo. Text-edit tests
include formatting in isolated draft/replacement transactions. The new private Xvfb
text_format_smoke.py clicks every format control and alignment, applies/cancels an
existing-text edit, checks undo/redo, and exports the image. Screen comparisons omit
the selection border; final screenshot-vs-PNG comparison allows one channel level for
display quantization, while core image comparisons remain byte-exact. The rendered
export target/rust-text-formatted-export.png was inspected. The fixture is listed in
BUILD.md and Rust CI; hosted execution is still unverified.

The complete check-rust.sh gate passed with all packaged shell checks enabled. The
full Xvfb desktop suite, native X11 Fcitx Pinyin fixture and Sway/IME/fractional-output
suite also passed. Python syntax, formatting and diff checks pass. This closes the
specific text-formatting implementation gap identified above; it does not establish
completion of every remaining editor or migration acceptance item.

### Edit the curved style of an existing arrow

The next tool-configuration audit compared ArrowTool::configurationWidget and
setArrowStyle with the Rust UI. The existing Rust toolbar checkbox changed drawing
defaults but did not edit an already-drawn selected arrow. The Objects and properties
panel now exposes its actual curved-head state when Move object selects an arrow.
A checkbox change replaces only that annotation as one undoable operation, redraws
it, and saves arrowStyle/current drawing defaults, matching the baseline setter's
preference update. The selected object's state is read independently of the default,
so undo restores its appearance without being overwritten by the saved preference.

The new private Xvfb arrow_style_smoke.py draws two arrows, edits one through the
real property checkbox, verifies the other is unchanged, checks the saved preference,
undoes/redoes the single edit, and compares export against the displayed result
(with the same one-level display-quantization allowance as the text fixture).
The export was inspected. All-feature workspace tests, all-feature all-target Clippy,
formatting, Python syntax and diff checks pass. The fixture is listed in BUILD.md
and Rust CI; hosted execution remains unverified. This closes selected-arrow style
editing, not the remaining global configuration semantics/parity audit.

### Detect conflicting configuration drafts under the write lock

Configuration Save now compares changed fields against the last loaded baseline and
the current/proposed settings while the existing persistent write lock is held. A
same-field conflict rejects the entire General/Shortcuts transaction before publication,
retaining both the external file and local draft. Unrelated external fields are merged;
a current value already equal to the proposed value is accepted. Shortcut comparisons
use parsed chords so equivalent Ctrl/Control spellings do not create false conflicts.
Plain CLI/other updates retain their existing path, including the ability to repair
invalid old values without first parsing the entire old document.

An explicit “Discard edits and reload” action lets users adopt saved settings and
reset the baseline. Read failures preserve the draft. The conflict message and action
are registered in all 49 catalogs, with Simplified/Traditional Chinese translations.
Startup desktop-entry handling remains its separate rollback-protected transaction.
As with the existing lock, this coordinates cooperating Rust writers; it cannot make
an uncooperative editor's writes participate in that protocol.

Tests verify the guard runs under the lock, rejects without changing any INI bytes,
allows successful atomic General/Shortcuts updates, preserves drafts and handles
unrelated/same-value/chord-equivalent updates. The private Xvfb config_conflict_smoke.py
edits a real form, modifies the same field through another CLI process, verifies the
file and draft survive rejection, clicks Reload, then saves a new edit while preserving
an unrelated external change and an unknown key. The fixture is added to BUILD.md
and Rust CI; hosted execution is unverified.

The complete Rust gate passed, followed by focused full/minimal-feature conflict
checks including shortcut equivalence and all-feature Clippy. Full Xvfb desktop and
Simplified/Traditional Chinese UI regressions passed. Catalog tests, formatting,
Python syntax and diff checks passed. This closes same-field draft overwrite handling,
not the remaining configuration fallback/live-capture semantics audit.

### Exercise upload/history controls and fix deletion edge cases

Added egui-frame interaction tests that locate rendered button labels and deliver
pointer press/release events through the production widget code. A 1024x768 synthetic
image remains unsubmitted until confirmation, a failed request permits retry, and a
successful retry writes history. The received multipart PNG is decoded and compared
with the original image. History tests click Delete/Confirm/Cancel, preserve files
on remote failure, retry successfully and remove the record only after success.
These are real widget-frame tests with loopback HTTP, not native-window uploads.

Client/history-path injection exists only under cfg(test). Missing injected clients
fail closed instead of falling back to production endpoints or user cache locations.
The existing loopback constructor disables proxies/redirects. No real screenshot or
external upload was used. History's native App method delegates to the same rendering
method used by the frame tests; production endpoint setup is unchanged.

A new failing regression demonstrated that retention cleanup could remove a thumbnail
while confirmation was open, causing successful remote deletion to leave a stale row
with a local NotFound error. Missing local files are now treated as already cleaned
up after remote success; other filesystem errors remain visible. Escape also dismisses
an active deletion confirmation without closing the history window, matching the
baseline modal confirmation behavior. Tests cover both fixes and preserved file data.

Eight upload/history tests pass with all features and with only Imgur enabled.
All-feature workspace tests and all-target Clippy pass. Native simplified/traditional
Chinese history-window open/close checks pass, along with formatting/diff checks.
The frame tests do not establish native-window upload/clipboard/browser integration
or every partial remote/local failure recovery path; those limits remain in the
network/desktop audit.

### Retry local history cleanup without repeating a confirmed remote deletion

History deletion now distinguishes a failed remote request from remote success
followed by local cleanup failure. The current window remembers confirmed remote
success by record path, reports that partial result, and presents a translated
“Retry local cleanup” action. That retry bypasses client creation and only removes
the local file; it does not re-confirm or reissue the remote deletion. Successful
cleanup removes the record and its remembered state. Unknown/disconnected worker
outcomes are not guessed to be remote success.

The widget-frame regression forces a local directory error after a successful
loopback DELETE, then retries once while the error remains and again after repairing
the path. No test API remains configured during either retry, so an attempted remote
retry would fail the test guard. The record stays visible until local cleanup succeeds.
The partial-result message and retry label are registered in all 49 catalogs, with
Chinese translations and placeholder/fallback tests.

All-feature and Imgur-only upload/history tests, all-feature workspace tests and
Clippy pass. Native simplified/traditional Chinese history open/close checks, catalog
tests, formatting and diff checks pass. No external requests or user history were
used. The remembered remote-success state is scoped to the open window; persistent
recovery after closing/crashing the application is not established by this change.

### Add update-window retry and verify its controls

The manual update window now offers Check for updates again after completion or
failure. It disables the action while a request is pending, clears old release data
when checking, and clears stale results on empty/error responses. Initial checking
starts after window creation and worker completion requests repaint. Ignore-version
writes still use the normal atomic settings path and report success only after the
write succeeds.

A loopback-only update transport helper reuses production response parsing with
proxies disabled and redirects refused. Test-only window fields inject that endpoint
and an isolated configuration path; missing test configuration never falls back to
real update requests or user settings. Shared test-only egui helpers now drive both
upload/history and update controls through pointer press/release events.

The update interaction test gates the mock response to exercise the disabled pending
button, receives an invalid response, retries to an available release, verifies failed
and successful Ignore writes, then checks an older release and confirms stale links
are gone. It also verifies Close emits the viewport-close command. Transport tests
reject non-loopback/credential-bearing endpoints. No external release service or
browser is contacted.

The complete Rust gate passed with shell completion checks enabled, including the
upload/history regression tests after helper extraction. Focused update tests and
all-feature Clippy also pass after adding the Ignore-write failure case. Formatting
and diff checks pass. These are widget-frame and loopback transport checks, not proof
of native update-window networking on every desktop; that integration scope remains
explicitly separate.

### Keep existing annotations in sync with global rendering preferences

The legacy arrow, counter and pixelation implementations read `reverseArrow`,
`drawCircleCounterOutline` and `insecurePixelate` while painting. The Rust document
now supports equivalent global render overrides, shared by preview, export and
rendered hit testing. Changing these preferences leaves annotation data and undo/redo
history intact. Per-object curved arrow style remains an editable annotation property.

The editor refreshes these three preferences from valid settings once per second,
including when otherwise idle. Toolbar reverse-arrow and counter-outline changes
persist and repaint immediately. Active text drafts invalidate their document preview
cache when these preferences change. Monitor selection initializes the replacement
document with the same preferences. This is a targeted live-settings correction;
it does not establish live reload parity for all configuration keys.

Core regressions compare effective rendering with independently configured annotations,
exercise picking at a reversed arrowhead, verify preview/export agreement, and verify
that global changes do not create undo entries. Pixelation tests check switching an
existing region between secure and insecure rendering without editing the annotation.
The private-Xvfb `global_render_smoke.py` edits an isolated INI file while an editor
is open, verifies both arrows and the counter repaint, exercises undo/redo, changes
preferences during text entry, cancels that draft, and compares exported pixels.
The fixture is registered in the Rust workflow and build instructions.

Validation passed: the complete `scripts/check-rust.sh` gate with Fish/Zsh completion
checks, the new live-render fixture, the full Xvfb desktop suite, text-format and
selected-arrow fixtures, X11 Fcitx Pinyin, and Sway Wayland with Pinyin and mixed-scale
monitor cases. Desktop checks used isolated displays, sessions and XDG directories.
Hosted CI and remaining migration acceptance gates are still pending.

### Restore decimal keyboard entry for tool sizes

The legacy capture widget accumulates unmodified digit presses into a decimal tool
size, clamps it to 1–50, and resets the accumulator on a non-digit key or clipping.
The Rust editor now supports this input alongside its existing wheel and size control.
It uses the existing per-tool persistence and selected-object replacement paths,
so a selected object's size changes remain undoable. Text-entry widgets and modal
dialogs do not receive canvas size handling; focus loss and modal transitions reset
the accumulator. Input is processed once per egui frame, including multi-pass layouts.

Shortcut dispatch now retains ordered unbound key events for the editor. Bound digit
shortcuts and multi-stroke prefixes consume their keys before decimal handling;
interrupted prefixes do not accidentally resize a tool. Other shortcut clients retain
the existing action-only interface. Unit tests cover decimal accumulation, clipping,
modifier rejection, shortcut precedence and interrupted sequences.

The private-Xvfb `size_keyboard_smoke.py` verifies decimal and physical keypad input,
clipping/reset, numeric Undo and multi-stroke bindings, isolation during text entry,
selected-object size changes, undo restoration and preview/export pixels. Physical
keypad events use XTest because xdotool's keysym helper toggles NumLock around each
key, which would correctly reset the accumulator but does not model ordinary typing.
The test is registered in the Rust workflow and build instructions.

Validation passed: the new numeric-input native fixture, the complete Rust gate
with Fish/Zsh checks, the full Xvfb desktop suite, X11 Fcitx Pinyin, Sway Wayland
with Pinyin and mixed-scale cases, and the live-global-render regression. Tests
used private displays, D-Bus sessions and XDG directories. Hosted CI and the other
migration acceptance gates remain pending.

### Restore the drawing grid and snapping controls

The Objects and properties panel now exposes the existing translated Display grid
control and a 5–50 spacing control (default 10). This restores the legacy side-panel
feature that was missing from the Rust editor. The grid is a UI overlay clipped to
the selection, including during text drafts; it never changes document pixels or
appears in exports. Dots are batched into a mesh to avoid a separate shape per dot.

New annotation positions snap to the grid in global logical window coordinates;
image scaling is applied after snapping. Drawing endpoints use normal snapping unless
Control enables the existing constrained-drawing behavior. Selection manipulation
and movement of existing annotations remain unsnapped. Where the compositor does
not report the window position, the selected monitor's logical origin supplies the
grid origin, including after the monitor chooser crops the capture.

Unit tests cover disabled snapping and positive/negative global coordinates, including
half-step rounding. The private-Xvfb drawing-grid fixture compares snapped lines at
default and custom spacing with lines drawn at exact coordinates, checks undo and
unsnapped object movement, and verifies that grid-enabled export contains only the
annotations. The fixture is registered in the Rust workflow and build instructions.

Validation passed: the complete Rust gate with Fish/Zsh checks, the new grid fixture
(including text-draft grid visibility), text formatting, selected-arrow style, the
full Xvfb desktop suite, and Sway Wayland with Pinyin and mixed-scale cases. All desktop
fixtures used private displays, D-Bus sessions and XDG directories. Enabled-grid native
validation is currently X11; the Wayland regression does not establish enabled-grid
behavior at every scale or monitor origin. Hosted CI and final migration acceptance
remain pending.

### Validate enabled grids on native scaled Wayland outputs

The Sway fixture now exercises enabled-grid rendering and drawing at 1.5x and 2x,
with monitor origins -853 and -640. It compares complete exported PNG pixels against
independently positioned reference strokes, checks that an unsnapped stroke differs,
and verifies the same result when entering through the monitor chooser. A preview
patch away from annotations verifies the grid is visible while absent from export.
The existing private virtual-pointer helper now accepts an explicit desktop extent
and separate press/release commands for accurate multi-monitor drags.

The fixture waits for virtual-keyboard focus and parks the pointer at the same
position before each capture, so portal cursor pixels cannot masquerade as a drawing
difference. It accounts for the side panel moving upward on a shorter logical output.

Concurrent renderer runs exposed a backend fixture limitation: portal-wlr 0.7.1
[hard-codes `/tmp/out.png`](https://github.com/emersion/xdg-desktop-portal-wlr/blob/v0.7.1/src/screenshot/screenshot.c),
so private buses alone do not isolate screenshot files. A per-user advisory lock now
serializes these tests and remains held until fixture cleanup completes.
The backend and application are unchanged; this is test coordination, not a production
portal concurrency fix.

Validation passed: complete Sway runs using the default renderer with Fcitx Pinyin
and the OpenGL fallback, including all new grid scenarios. A simultaneous launch
verified one runner waiting on the lock while the other ran; both then completed
successfully in sequence. The virtual-pointer example build and focused Clippy,
formatting and diff checks pass. This extends native grid coverage to Sway at the
specified scales/origins, not every compositor or output arrangement. Hosted CI
and the remaining migration acceptance gates are still pending.

### Restore confirmed color grabbing and selected-object recoloring

The previous Rust G shortcut immediately saved the hovered color and bypassed the
selected object's color-edit path. G and the translated side-panel Grab Color button
now enter a picking mode. Hovering does not mutate settings or annotations; a left
click or Enter confirms, and Escape cancels without exiting the editor. Space/right
click toggle the magnifier, using the existing ColorGrabWidget translations for the
on-screen controls. Canvas shortcuts and wheel size changes are suppressed while
picking; confirmation events are not reused to export or start a drawing operation.

Confirmation uses the same color persistence and object replacement path as palette
edits. A selected object therefore changes in one undoable edit, while choosing a
color with a drawing tool changes its default without editing the previously selected
object. Capture tool controls are hidden during picking and restored afterward.

The new private-Xvfb fixture exercises confirmation/cancellation, selected-object
undo/redo, button entry, magnifier toggling, default-only picking and preview/export
agreement. The existing magnifier smoke case now explicitly confirms its sampled
color. The legacy held-button enlarged pixel-selection interaction is not implemented
by this change and remains an explicit editor acceptance item.

Validation passed: the complete Rust gate with Fish/Zsh checks, the new color-grab
fixture (including right-click/Space magnifier toggling and Enter confirmation),
drawing-grid and text-format fixtures, the full Xvfb suite, and Sway with Pinyin and
the scaled negative-origin grid scenarios. All GUI checks used private displays,
D-Bus sessions and XDG directories. Native color-grab-specific coverage is X11;
Wayland picking and the held-button precision mode remain acceptance work.

### Add held-button pixel selection to color grabbing

Holding the primary button while picking now freezes an 11×11 source-pixel crop.
Each pixel occupies a 15×15 logical cell; moving inside the enlarged region selects
that source pixel, and release or Enter confirms. The initial pixel stays under the
cursor when the crop is clipped at image edges. Leaving the enlarged region resumes
ordinary picking without committing a dragged release. Escape discards the frozen
region. Confirmation/cancellation while the button remains held suppresses subsequent
release handling so it cannot accidentally draw or change the selection.

The precision region is rendered from its frozen pixels as an overlay, independently
of the document image. The existing translated held-click instruction is now shown.
Unit tests use distinct source pixels to check cell mapping, clipping, bounds and
frozen-source behavior. The Xvfb fixture uses a narrow green stripe to distinguish a
selected enlarged cell from the black image pixel physically under the cursor; it
also checks release/Enter confirmation, held cancellation, leaving the region, edge
selection, settings timing and unchanged document/export pixels.

The Sway fixture is extended with corresponding narrow-stripe picking at 1.5x and 2x,
plus Enter confirmation, held cancellation and exclusion of the precision overlay
from exported screenshots. This addresses the held-button interaction left open by
the preceding color-grab change.

Validation passed: the complete Rust gate with Fish/Zsh completion checks, the expanded
Xvfb color-grab fixture, the full Xvfb desktop suite, and complete Sway runs with the
default renderer/Pinyin and OpenGL fallback. Both Wayland runs passed precision picking
at 1.5x and 2x along with the existing grid cases. GUI validation used private displays,
D-Bus sessions and XDG directories. This establishes the tested X11/Sway interactions;
other compositor/version acceptance and the overall migration gates remain pending.

### Preserve asynchronous D-Bus notifications and clipboard ordering

The legacy text-clipboard service asynchronously sends its notification before
replacing clipboard contents. The Rust implementation replaced the clipboard first
and waited up to two seconds for a notification reply before finishing its D-Bus
method. It now sends Notify without requesting a reply, then replaces the clipboard.
A slow or rejecting notification service cannot delay completion of the copy operation;
the existing timeout only bounds notification transport setup/sending.

Text and screenshot clipboard requests share an asynchronous ordering lock, preserving
replacement order across transport awaits. Public service calls hold pending-request
guards so queued/executing operations are not considered idle. A lifecycle test polls
an executing request and a queued request, then cancels each and verifies idle state
is restored only after both have ended.

The private-Xvfb fixture holds synthetic notification handlers behind an explicit gate.
Clipboard calls must finish before that gate is released, and the captured D-Bus headers
must carry NoReplyExpected. It also checks rejection, disabled notifications, concurrent
text/text and text/image replacements, a legacy no-reply text call, and clipboard/daemon
survival while notification handling remains pending. Actual copied image pixels are
compared. The fixture continues through tray actions and explicit daemon shutdown.

A separate private-bus introspection snapshot confirms the four legacy method names
and argument signatures (`ay`, `s,s`, `ay`, and no arguments for captureScreen). The
Rust XML currently has no Method.NoReply annotations, while the Qt adapter declares
these slots Q_NOREPLY. Exact introspection-metadata parity remains a specific follow-up;
the successful no-reply text call above proves dispatch behavior, not metadata parity.

Validation passed for the final asynchronous-send implementation: the complete Rust
gate with Fish/Zsh checks, private D-Bus/native UI tests including real document-portal
saves, the full Xvfb desktop suite, and Sway with Pinyin, scaled grids and precision
picking. Existing clipboard ownership and automatic-idle-exit cases still pass on
X11 and Wayland. Tests used private sessions/displays/XDG directories; hosted CI and
remaining migration acceptance, including the metadata item above, are still pending.

### Match the legacy D-Bus method introspection metadata

A Qt 6.4.2 QCoreApplication harness using the unchanged legacy adaptor header and
moc-generated metadata confirmed that all four public methods export
`org.freedesktop.DBus.Method.NoReply=true`. Only the slot bodies were stubbed; the
interface declarations were the original source. The observed Flameshot interface
is now checked in as `crates/flameshot/tests/fixtures/qt-dbus-interface.xml`, with
header provenance and regeneration notes beside it. Qt tools were extracted and run
as local reference fixtures, not added to Rust build/runtime requirements.

The locked zbus 5.19 interface macro does not expose arbitrary method annotations.
A forwarding Interface implementation therefore retains the generated Service's
method/property dispatch and task policy, while adding the four legacy annotations
to its generated XML. It does not suppress replies requested by existing Rust callers;
actual no-reply calls remain handled by zbus's request flags.

The private-bus daemon test now structurally compares the complete public method
contract with the Qt reference: names, argument attributes/order, annotations and
absence of unexpected/duplicate members. Standard empty Properties.GetAll and
UnknownProperty errors are also checked. The XML parser is a test-only dependency
already present in the workspace lockfile. This addresses the method-metadata gap
recorded in the preceding D-Bus audit.

Ordinary gdbus calls to the Qt reference's captureScreen and attachTextToClipboard
methods returned empty replies despite their NoReply annotations, confirming that
retaining replies for callers which request them matches the observed Qt behavior.
Validation passed: the full Rust gate with Fish/Zsh checks, private D-Bus/native UI
fixtures with real document-portal saves, and Sway with Pinyin, scaled grids and
precision picking. The contract checks run in default, all-feature and minimal builds.
All reference/native checks used private buses and isolated desktop fixtures. Hosted
CI and the remaining migration acceptance gates are still pending.

### Localize the existing pinned-image menu

The pin window's rotation, original-size, opacity and close labels previously bypassed
the translator, and its egui context did not install the script-specific fonts used
by other windows. It now uses the existing PinWidget translations for rotation/close,
registers Original size and Opacity in all 49 catalogs, and installs the shared fonts
and configured UI theme when creating the window. Simplified/traditional Chinese
translations are supplied; unfinished languages retain source-text fallback. The
Hong Kong rotation/close entries used here are now completed instead of falling back
to English because of unfinished translation markers.

The Chinese catalog coverage check includes all five pin-menu labels. The private-Xvfb
localization fixture now opens a synthetic pinned image in Simplified and Traditional
Chinese, captures its menu, rotates it, zooms, restores original size and closes it.
The captured menus were visually checked for readable Chinese glyphs.

This closes the identified pin-menu localization gap, not pin feature parity. Source
comparison also identified missing Copy to clipboard and Save to file menu actions;
legacy increment/decrement opacity actions remain to be reconciled with the current
slider/keyboard controls. Those are explicit remaining pin acceptance items.

Validation passed: Chinese catalog/fallback checks, the expanded Simplified/Traditional
Chinese native fixture, the complete Rust gate with Fish/Zsh checks, and the full Sway
fixture with Pinyin, pin compositing/rotation, scaled grids and precision color picking.
GUI tests used private displays, D-Bus sessions and XDG directories. This verifies the
existing localized pin controls; full pin menu parity, hosted CI and other migration
acceptance gates remain pending.

### Restore pinned-image copying and opacity increment actions

The pin context menu now includes Copy to clipboard and the legacy Increase/Decrease
Opacity actions. Copy snapshots the current rotated image independently of display
zoom/opacity. The normal daemon clipboard format preference still applies; PNG retains
source resolution and alpha. Copy runs asynchronously, disables duplicates while pending, and
retains the image with a retryable error dialog if copying fails. Escape dismisses that
error without closing the image.

Keyboard/menu close requests and native close events wait for a pending copy to finish.
A failed copy keeps the image open; successful handoff allows the requested close. The
shared clipboard daemon retains ownership after the pin process exits. Opacity menu
steps are 0.1 with the legacy 0–1 limits. Existing slider/number-key controls remain.
The newly used Hong Kong menu translations are completed and included in catalog checks.

A private-Xvfb integration fixture sends the real pin UI's clipboard requests to a gated
D-Bus test service. It checks rotated pixel data while the window is zoomed and half
transparent, opacity-menu compositing, synthetic rejection and retry, delayed completion,
Escape/native WM_DELETE_WINDOW close deferral, and clipboard data after process exit.
The fixture is included in the existing dbus_ui_smoke runner. The Sway fixture also now
copies a rotated half-transparent pin through the real daemon and verifies full-opacity
pixels remain available after closing the pin. Chinese menu navigation was updated for
the added actions. Save to file remains an explicit pin feature-parity task.

Error-dialog shortcut state is isolated from image editing, and close shortcuts are
processed once per egui frame so dismissing/retrying a dialog cannot reuse the same
input to close the pin. Validation passed: the complete Rust gate with Fish/Zsh checks,
the private D-Bus/native UI runner (including document-portal saves and the new gated
pin test), Simplified/Traditional Chinese windows, and Sway with Pinyin and actual pin
clipboard persistence. GUI tests used private displays, buses and XDG directories.
Pin file saving, hosted CI and the remaining migration gates are still pending.

### Restore saving pinned images through the shared save workflow

The pin menu now exposes Save to file. The screenshot editor's file browser and portal
chooser have been extracted into a shared SaveDialog, retaining its asynchronous folder
listing, format choices, portal filtering and write-error retry. Pins use that dialog in
an independent native viewport, sized independently of the image. While it is open the
pin surface is transparent/non-interactive and its topmost level is relaxed; normal pin
input and presentation return after save or cancellation. Closing the save window leaves
the image open. Parent close requests wait for the save operation to resolve.

Saving snapshots the rotated source pixels without display zoom/opacity. Fixed-path
settings use the existing non-clobbering writer directly, with dialog recovery on write
failure. Successful saves remember the destination directory and honor path-copy and
notification preferences. Notification sending does not wait for the desktop service's
reply, and path-copy failures retry the path operation rather than rewriting the image.
The save-time settings snapshot is retained for post-save actions.

Manual saves now ask before replacing an existing file, defaulting to No. Confirmed
replacement uses the exact-path atomic writer; portal selections continue to rely on
the system chooser's authorization. Relative destinations use the actual working
folder for listing and save-directory preferences. The confirmation prompt is registered
in all catalogs with Chinese translations. Confirmation/dialog events are not replayed
as editor or pin close actions within the same frame.

The new private-Xvfb pin-save fixture covers cancellation, failure/retry, rotated pixels,
confirmed/declined overwrite, relative paths, fixed-path BMP saves and copied paths after
pin closure. The private portal fixture additionally exercises pin acceptance, cancellation,
rejection and write retry while honoring exact extensionless names and the selected JPG
filter. Sway now also saves a pin through the native Rust dialog before exercising its
clipboard handoff. These additions address the previously missing pin save menu action;
installed sandbox/provider and broader desktop acceptance remain separate gates.

Directory remembering is shared by interactive editor and pin saves, including fixed
GUI destinations; successful explicit CLI output paths keep their previous preference
behavior. Widget-frame tests additionally verify that Enter/Escape in the format popup
do not submit/cancel the save dialog, and renaming a destination without a suffix keeps
the chosen format. Native tests cover both editor and pin directory preferences.

Validation passed for the final shared workflow: the full Rust gate with Fish/Zsh
checks, multi-pass format-widget tests, private portal/native UI tests with real
document-portal editor saves, the new pin-save fixture, the complete Xvfb suite and
Chinese window checks. Sway passed with Pinyin/default rendering and OpenGL, including
native pin saving and restored visible pixels afterward; the X11 pin-save fixture also
passed under OpenGL and checks restored opacity after cancellation. All GUI tests used
private displays, buses and XDG directories. Pin-save behavior is implemented and these
specified paths are verified; hosted CI, installed packaging and the other migration
acceptance gates are still pending.

### Keep all best-effort application notifications nonblocking

The remaining application notification call sites now use the no-reply transport:
screenshot completion, capture cancellation, daemon startup and background update
announcements. They enqueue notifications without waiting for the notification service
to finish handling them. Preference conditions and existing bounded transport setup
remain intact; the reply-returning platform API is retained for callers needing an ID.

The private notification fixture records NoReplyExpected headers and can keep handlers
blocked behind a gate. Both cancellation and successful save/raw commands must exit
before that gate is released, preserving cancellation status 2, success status 0, PNG
stdout and saved image pixels. Preference combinations and rejecting handlers remain
covered. Notification observations are awaited rather than assumed to have arrived
synchronously with command completion. The daemon fixture also requires no-reply
headers for startup messages, without enabling real update-network requests.

Validation passed: the complete Rust gate with Fish/Zsh checks, including private-bus
startup and capture-notification tests, plus focused notification tests under both
minimal and all-feature builds. The blocked-handler cases use synthetic local services
and images; no desktop notification or external update request is sent to the user's
session. Hosted CI and the remaining migration acceptance gates are still pending.

### Cancel pending save portals without publishing late results

The shared save dialog now offers Cancel/Escape while the portal request is pending,
and closing a pin's save viewport cancels that request instead of waiting for selection.
Cancellation is signalled to the worker; dropping the dialog also signals cancellation.
The UI handles cancellation before consuming a queued result, so a late successful
selection cannot reach the file writer after the user cancels.

The platform chooser retains its original API and adds a cancellable variant. It avoids
dispatching an already-cancelled operation, sends Request.Close when cancelling an active
request, and cleans up the eventual handle if the SaveFile method was still starting.
When an initial Close cannot find a ready request, the method reply is drained and the
returned handle is closed. The original timeout bounds that background cleanup; the UI
can return to the image immediately rather than waiting for it.

Protocol fixtures cover cancellation before dispatch, pending selection, a request whose
first Close fails while it is starting, and a successful Response emitted during Close.
Widget tests prove cancellation wins over an already queued selection and dialog drop
signals its worker. Native private-Xvfb tests cancel the screenshot save dialog with Esc
and the pin save viewport with WM_DELETE_WINDOW, verify Request.Close, retain the parent
image/editor, and confirm the original destination file stays unchanged despite a late
acceptance response.

The starting-request fixtures cover both a late successful method reply and a late
initialization error. Mouse cancellation also has a widget test that queues a successful
selection on the button-release frame. Validation passed: the complete Rust gate with
Fish/Zsh checks, private portal/native UI tests including real document-portal saves,
and the normal pin-save regression (retry, overwrite, formats and directory preferences).
All GUI checks used private displays, D-Bus sessions and XDG directories. These tests
establish the specified request/cancellation behavior; hosted CI, installed sandbox
validation and the other migration acceptance gates remain pending.

### Synchronize the installed D-Bus interface description

The installed XML still lacked the four NoReply annotations and named the screenshot
argument `screenshot`, unlike the verified runtime/Qt `data` argument. The resource now
matches the public runtime contract. The daemon integration test compares this shipped
resource with runtime introspection, and staging checks compare the installed copy with
the checked-in Qt reference. Wire argument types and method behavior are unchanged.

The private-bus contract test and staging checks passed, including normal and spaced
installation prefixes, permissions, desktop/AppStream resources and activation quoting.
This corrects the published interface resource; a refreshed optimized-build validation
follows separately and does not replace remaining install/sandbox acceptance.

### Pin zoom input across repeated layout passes

The optimized `1ecfd4cc` review build passed X11, Sway/IME and pin-save smoke
checks, but the Chinese-window fixture exposed a duplicate zoom: one wheel click
changed a rotated 300×400 pin to 363×484 instead of 330×440. Pin zoom now consumes
wheel/pinch input once per egui frame, including frames occupied by a modal dialog.
A subsequent frame can still apply a new input. The widget regression forces two
layout passes and verifies wheel directions, successive frames, pinch and limits;
both pin unit tests pass. Native/release revalidation is recorded separately.

The initial optimized compilation remained busy in LLVM without completing. An
isolated compilation with the same optimization settings completed, after which
the original owned build was stopped and an unchanged standard Cargo retry passed
in 10.58 seconds. This does not establish the cause of the compilation anomaly.

The `97281a0d` optimized retest still reproduced double zoom, so repeated layout
was not the sole cause. Inspection of winit 0.30.13's X11 handler showed that legacy
wheel buttons 4–7 generate a wheel event on both press and release. The existing
XTest fixture sends a full click; scheduling determines whether those events are
processed together or in separate frames. A local Rust dependency patch now ignores
only wheel-button releases, preserving ordinary buttons and XI2 axis scrolling.
Its original source and licenses are retained under `third-party/winit`; Cargo,
source-archive export and Nix source filtering include this dependency. The native
fixture's expected single-step size remains unchanged.

### Patched-wheel release validation (`96c4c60c`)

The debug and optimized builds now both pass the unchanged one-step pin zoom
expectation. The native Chinese-window fixture also verifies a second complete
wheel click and reverse scrolling. The optimized staged executable has SHA-256
`1c3dd702093b067936fab5084d784194ebc2f5df02683a6eec8475acdfa356b9`;
its dynamic dependencies contain no missing libraries or Qt libraries. Staging,
Chinese-window lifecycle, pin save/overwrite/retry/clipboard, X11 full/screen/editor
and Sway/IME suites pass. Sway includes mixed-scale negative-origin capture, grid, pixel picking,
pin-save restoration and clipboard ownership after closing the pin.

The full local `scripts/check-rust.sh` gate passes with the patched dependency:
formatting, Clippy, workspace tests, private-bus integration, default/all/minimal
feature configurations and shell completions. Hosted CI remains unverified.

The first winit optimized compilation remained active in LLVM. A separate invocation
with the same compiler arguments and environment, changing only its output directory,
completed successfully. After stopping only that original owned build, the standard
Cargo retry completed in 19.55 seconds without changing optimization settings. The
compiler anomaly's underlying cause remains undetermined.

The clean `96c4c60c` source archive has SHA-256
`e88735c8231815f9f294a4112f5e81b53ed0eff70509edb876423b7d6d7cddba`.
All 508 tracked files under crates, third-party, data, docs/rust and tests/rust
were compared byte-for-byte after extraction. With a new empty Cargo home,
`cargo metadata --frozen --offline` resolves the local patched winit and
`cargo build --frozen --offline -p flameshot --all-features -j 8` succeeds
in 61 seconds. This verifies source-archive compilation, not installed sandbox
runtime behavior or a fresh optimized build from the archive.
