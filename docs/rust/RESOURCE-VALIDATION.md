# Resource and translation lookup validation

The resource contract covers the retained branding assets, all existing translation
catalogs, lookup routing and locale/source fallback. Desktop font availability and
compositor-specific rendering remain separate acceptance work.

## Preserved inputs and packaging

A comparison with baseline `f4f2e4e8` confirms the same 49 TS filenames and no removed
active context/source keys. The application PNG and scalable SVG are byte-for-byte
unchanged. Local comparison evidence is in `target/resource-baseline-audit.json`.
`app_ui::viewport` embeds the PNG for native windows; the tray derives its ARGB fallback
pixmap from the same image. Staging now compares both installed icon files directly
with their source bytes, alongside existing metadata, license and permission checks.

## Rust catalog behavior

`flameshot-core/build.rs` reads TS files and generates the embedded catalogs without
Qt tools. Unfinished, obsolete, vanished or empty translations do not replace source
text. `Translator` tests cover locale normalization, region/language fallback, unknown
locales, context distinctions and parameter preservation. Configured language selection
is independent of unrelated invalid preferences, and restart notices use the language
actually cached by the session.

## Lookup audit

`tests/rust/translation_lookups.py` checks ordinary quoted calls, local wrappers,
diagnostic template declarations, D-Bus menu labels and the configured button catalog.
Finite tool/annotation names, alignment labels, geometry choices, delay units and
repair/retry branches now use explicit calls. The button catalog declares the same
context used by its runtime translation helper. The current audit covers 392 call
sites/catalog labels and verifies parameter markers for all finished used templates.
Two Tamil monitor labels with broken percent markers were corrected during this audit.

The remaining variable lookup sites are forwarding helpers, declared diagnostic
presentations, the checked button catalog and best-effort status translation. User data
(paths, application/monitor names, locale identifiers, values), technical backend details,
branding/license names and mathematical/key symbols remain data rather than translated
message identifiers. The scanner is intentionally not a general Rust dataflow analyzer;
raw-string or newly introduced dynamic lookup forms require explicit review.

## Execution evidence

The complete local Rust gate passes. Native regression evidence includes the full X11
suite, text formatting, configuration recovery and simplified/traditional Chinese
windows. After the final branch refactoring, the rebuilt native locale and global
arrow/counter/layer-rendering fixtures also pass. Catalog tests and Clippy pass after
the final Tamil marker correction. Staging verifies both ordinary and spaced prefixes.

This closes the catalog/icon/lookup audit. It does not assert complete human translation
of unfinished entries, rendering on every font installation, installed sandbox policy,
hosted CI or refreshed release/package artifacts.
