# Linux CLI compatibility

The baseline is `f4f2e4e8`. The original command/option scopes are preserved in
`crates/flameshot/tests/fixtures/legacy-cli-options.tsv`, with source hashes and
provenance in the adjacent Markdown file. A test compares all six scopes (root and
five commands), primary short names and flag/value shapes with the built Clap schema.
Help descriptions and section labels use the translation catalogs; this fixture
continues to validate syntax independently of language.

| Surface | Contract and evidence |
| --- | --- |
| No command | Starts the single-instance daemon; private D-Bus tests verify ownership and duplicate invocation |
| gui | Region/last-region, accept-on-select, geometry, raw, path, clipboard, pin and delay options; native desktop and keyboard-geometry fixtures exercise capture, cancel and export |
| screen | Explicit/default monitor selection, edit, region, pin and output options; private X11/Sway capture fixtures cover monitor selection and rejection |
| full | Raw PNG, save path, clipboard and delay; the private portal test verifies pixels, stdout routing and collision-safe saves |
| config | Headless atomic changes, opaque data preservation, colors, booleans, autostart and read-only check; config integration tests cover failures, repetition and cooperating writers |
| launcher | Native launcher and child lifetime are covered by the desktop fixture |

Help and version requests finish without desktop services or configuration writes.
The original -v/--version flags are retained; -V and the help subcommand are Rust
extensions. Optional history/updates commands follow their build features. Both
available help and unavailable-command rejection are tested in all-feature and
minimal builds, without a display or usable session bus.

Parsing retains the legacy restrictions on option shape, argument ordering and
repetition: the first option value is used, every occurrence is validated, and
malformed later values cannot cause earlier configuration writes. Tests cover i32
numeric bounds, signed zero/whitespace, opaque QColor inputs, region grammar,
non-UTF-8 paths and strings resembling options used as values.

| Result | Status / stream behavior |
| --- | --- |
| Success, help, version | 0; requested output on stdout |
| Syntax/value rejection by the CLI parser | 0, preserving the legacy convention; diagnostics on stderr and no side effects |
| Runtime/configuration-check failure | 1; diagnostics on stderr |
| Capture cancellation | 2 |
| Session-bus connection/loss | 3 |
| Daemon SIGINT / SIGTERM | 130 / 143 |

`cli_help.rs`, `config.rs`, the private `full.rs` and `daemon_exit.rs` tests provide
current execution evidence. GUI cancellation and raw-over-geometry precedence are
also asserted by the native desktop suite. Raw stdout remains PNG-only, including
when invalid configuration produces fallback diagnostics on stderr.

Shell completion validation runs with isolated English and Chinese preferences and covers:

- Bash live command contexts, all long-option scopes, values and filenames.
- Fish live contexts, booleans, version aliases and optional commands.
- Zsh option declarations against help, syntax, and real ZLE completion in a private
  PTY: command contexts, flag/value distinctions, escaped filenames and optional
  commands. Completed lines are captured by a widget and never executed. History
  and completion-cache writes are disabled; user shell configuration is not loaded.

The parser/scope tests, CLI/config integration and Bash/Fish/Zsh checks pass for both
all-feature and minimal builds. Private portal/signal tests and all-feature Clippy
also pass. Reproduction:

```sh
cargo test --locked -p flameshot --all-features --bin flameshot cli_tests
cargo test --locked -p flameshot --all-features --test cli_help --test config
FLAMESHOT_TEST_PRIVATE_BUS=1 dbus-run-session -- cargo test --locked -p flameshot --all-features --test full --test daemon_exit -- --ignored
python3 tests/rust/completions.py target/debug/flameshot /
```

Use the extracted shell-tool root instead of `/` where appropriate. Repeat parser,
CLI/config and completion checks with `--no-default-features` for the minimal build.
This closes the command/shell contract audit; translated UI coverage, installed
sandbox behavior, release packaging and hosted CI remain separate migration gates.
