# Local release validation snapshot

Source revision: `ee049bd49ae685233c4b16edda916e862a0862ad`.

The optimized workspace and platform examples were built with the locked dependency
set. The application build enabled all features. The staged executable reports
`flameshot 15.0.0` and has SHA-256:

```text
255bc58c7c88d61a0116566a03fe2c30e29db121585301ba487cecbe8f5bf642
```

The local payload is `target/rust-release-review-qw7wh4vn/usr`; its manifest is
`target/rust-release-review.json`. Staging did not install files into the running
system. The payload contains the executable, desktop entry, D-Bus activation and
interface files, AppStream metadata, PNG/SVG icons, licenses, manual, and Bash/Zsh/
Fish completions. Desktop-file and offline AppStream validation passed. The ELF
runtime dependency listing contains no Qt 5/6 libraries.

The staged executable passed the native headless Sway suite, including mixed-scale
monitor selection, clipboard persistence and confirmation behavior. It also passed
the private GNOME multi-monitor/clipboard suite: real screenshot permission grant
and refusal, native Wayland editor export/cancellation, second-monitor placement,
PNG/JPEG and Unicode clipboard delivery to a Wayland GTK recipient. GNOME's fixture
session-service substitutions and renderer requirements are described in BUILD.md.
Live Bash/Fish completion checks and Fish/Zsh syntax checks passed.

These are local optimized-build and staged-payload checks. They do not establish
clean-distro installation, deployed sandbox confinement, hosted CI, complete IME or
all compositor coverage. The C++ reference tree still exists pending Linux acceptance;
absence of Qt linkage in this executable does not make the repository Rust-only.
This snapshot must be superseded after further production changes before release.

The complete isolated Xvfb suite also passed using this staged executable, including
clipboard/pin lifetime, owned launcher descendants, configuration concurrency,
selection/editing tools, save retry/cancellation and multi-monitor cases. The binary
hash was verified unchanged after all desktop and completion checks.

## Current optimized artifact refresh

The post-audit all-feature optimized build at revision `91fb057a`
reports `flameshot 15.0.0`. Its executable and staged copy both have SHA-256
`31c7d4dddd7576ee6ef6f49299396355bc1875a30bcec2cb0efd38861ecf24c3`.
`desktop-file-validate`, offline `appstreamcli validate`, the staging fixture and
version/help checks pass. ELF linkage and strings contain no Qt 5/6 library names.
The vendored source archive has SHA-256
`bdd735cffcb2a87ec134c01ed38c22830a7a8041458e9bf8cb774c942105e52a`.
Evidence is in `target/current-release-manifest.json`,
`target/current-release-stage/` and `target/current-rust-source.tar.gz`.

This refresh supersedes the older optimized snapshot for source/staging evidence;
it does not turn unrun hosted CI, installed Flatpak/Snap confinement or the pending
Nix binary build into completed gates.
