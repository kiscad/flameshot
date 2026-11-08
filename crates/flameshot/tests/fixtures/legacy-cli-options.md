# Legacy Linux CLI option scopes

Transcribed from option declarations and `AddOptions` relationships in baseline
`f4f2e4e8:src/main.cpp`, plus `addHelpOption`/`addVersionOption` in the command parser.
The columns are command (`root` means no subcommand), long name, primary short name
(`-` means none), and whether the option takes a value (`1` or `0`).

Baseline source SHA-256:

- `src/main.cpp`: `e28d0f9b8d1d021414ca564178956f746f148a93523fc157482c07786b772cd2`
- `src/cli/commandlineparser.cpp`: `da5a744bc8ea7d42e9f74928f99a75451852930dc9515fa62703272affd98d66`

The scope test compares these declarations with the built Rust argument schema,
including implicit help flags. Additional Rust commands (help, optional history and
updates), the -V version alias and hidden internal commands are covered separately.
This fixture is a syntax contract, not a reference for translated help prose.
