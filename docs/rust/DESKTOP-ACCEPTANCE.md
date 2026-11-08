# Linux desktop acceptance audit

This audit checks the functional requirements in MIGRATION.md against the native
fixtures and their recorded results. The tested matrix is X11/Xvfb, GNOME
46/Mutter 46, Sway 1.9 and 1.11, and KWin/KDE 5.27. It is not a guarantee for every
compositor release, physical GPU, input device or IME engine. Installed sandbox
confinement and final release artifacts remain separate acceptance gates.

| Requirement | Evidence and result |
| --- | --- |
| Direct X11 and portal capture; full/screen/gui | Full Xvfb suite and installed Fedora X11 suite; real GNOME, wlr and KDE portals all pass capture dimensions and image checks |
| Cancel, refusal, timeout and invalid requests | Xvfb/GNOME editor cancellation; mapped fullscreen KWin editor Escape; real GNOME permission refusal; private portal lifecycle tests verify refusal/timeout and Close cleanup; CLI tests check statuses and empty output on failure |
| Multiple monitors and placement | X11 monitor tests; GNOME integer/fractional scaling with a secondary left of the primary; KWin integer/fractional placement and virtual output disable/re-enable; Sway negative-origin mixed/fractional scale and all output transforms on a capable backend |
| Native Wayland execution | Sway/KWin run with DISPLAY absent; GNOME asserts native Wayland client type. Its clipboard fixture additionally enables the private Xwayland bridge |
| Clipboard survives editor/process exit | X11 PNG/JPEG, large INCR and in-flight transfer ownership; Sway/KWin native ownership and replacement; GNOME PNG/JPEG/Unicode delivery into an independent native GTK4 receiver |
| Text input and composition | X11/Sway/KWin Fcitx Pinyin and GNOME IBus libpinyin; commit, cancel, exact copied text and rendered annotation export pass. GNOME also passes at fractional scale |
| Selection, drawing and image export | EDITOR-COMPATIBILITY.md maps tools, geometry, history, palette, pixelation, text, grid and actions to unit/native evidence |
| Pins, launcher, tray and process lifetime | Full X11 suite, private D-Bus tray actions and KWin/Sway ownership tests; DAEMON-COMPATIBILITY.md covers queued work, child shutdown, independent service lifetime and signal exits |
| Configuration windows and repair | Native conflict/recovery/fallback/save-directory/localization fixtures plus CLI/INI tests; invalid settings, draft discard/repair, external changes and atomic writes are covered |
| Notifications | Private real D-Bus services verify the standard Notify signature, no-reply behavior, translated save/copy/pin/cancel messages, preference suppression and nonblocking delivery. Desktop notification styling belongs to the recipient service |
| Autostart and desktop resources | CLI tests verify the isolated legacy autostart entry and rollback on config-write failure; staging/installed-package tests verify desktop/service paths, executable availability, icons, translations and metadata |
| Open With, save dialogs and links | Native chooser/terminal/save-retry cases, private portal protocols and real document FUSE saves; endpoints for uploads/updates remain loopback-only |

## Current KWin refresh

The older KDE results were refreshed using the current all-feature executable and
input helper, copied with the current fixture into the dependency-only
`flameshot-test-kde:5.27-ime` image. The disposable container had no network or host
mounts. Clipboard, fractional/hotplug and Pinyin modes all passed.

The cancellation fixture previously sent Escape while a request could still be
waiting for portal consent. It now preauthorizes only its private permission store,
waits for the native application's geometry and fullscreen state, then sends Escape.
All three modes verify status 2, empty stdout and that cancellation actually reached
the mapped fullscreen editor. This closes that explicit gap in the older evidence.

Application source: `07564353`. Executable SHA-256:
`96b0c14b472811272b239a9b6d1b93d71c94ee704b7e2b21bea542583346fd01`.
Logs: `target/kde-final-clipboard.log`, `target/kde-final-fractional-hotplug.log`,
`target/kde-final-ime.log`. Images and a hash/environment report are in
`target/cross-desktop-audit/`. Only Docker init and sleep remained before the
container was removed.

## Evidence boundaries

- GNOME rejects negative global origins; its test verifies atomic rejection and
  normalized left-of-primary placement. KWin likewise normalizes the requested
  left-hand layout. Sway supplies actual negative-global-origin coverage.
- Sway 1.9/Pixman's mirrored reference frames are gray even without Flameshot;
  Sway 1.11/wlroots 0.19 supplies the mirrored-transform cases.
- KDE evidence is for version 5.27, not KDE 6. GNOME input evidence uses IBus Pinyin;
  other IME engines are not implied. Hotplug evidence uses virtual output lifecycle,
  not physical cable removal.
- GNOME clipboard persistence uses its Xwayland bridge. A compositor that exposes
  neither data-control nor an X11 bridge cannot support that background ownership
  path; the implementation reports capability failure instead of pretending success.
- Package/runtime checks do not establish installed Flatpak/Snap confinement.
  The old optimized release snapshot is not the final rewritten release artifact.

The capture, clipboard, desktop integration and named cross-desktop functional
requirements are covered by this evidence. Packaging/provider completion, final
artifact refresh, hosted CI evidence and final legacy-source removal are still
tracked separately; this audit does not mark the whole migration complete.
