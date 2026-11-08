# Frozen Linux editor metadata

Reference commit: `f4f2e4e8` (the C++/Qt migration baseline).

`legacy-editor-tools.tsv` records the Type enum in `src/tools/capturetool.h` and
Linux toolbar priority in `src/widgets/capture/capturetoolbutton.cpp`, with Imgur
enabled. A `-` priority denotes an internal/non-toolbar value. ID 7 is deliberately
unused; Cancel (24) is a shortcut action, not a configurable toolbar button.
Priority gaps are preserved in the fixture; only relative order is significant.

`legacy-editor-shortcuts.tsv` records the SHORTCUT declarations in
`src/utils/confighandler.cpp`, choosing the Linux Delete binding instead of the
macOS Backspace branch. Empty values are unbound shortcuts, not missing records.
The set includes the pinned-window shortcut because it shares the configuration
namespace. Optional Imgur remains in the metadata even in builds where its action
is unavailable.

The rows were extracted from `git show f4f2e4e8:<path>`, independently of the Rust
implementation. Application unit tests compare the complete catalog and shortcut
map, plus drawing-tool enum mappings, against these frozen files. They do not claim
to verify rendered pixels, native input or the behavior behind each action; those
are covered by the editor acceptance matrix and native tests.
