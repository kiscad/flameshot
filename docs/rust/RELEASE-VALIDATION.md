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
