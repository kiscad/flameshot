#!/bin/sh
# SPDX-License-Identifier: GPL-3.0-or-later
set -eu
check_completions() {
    for check_locale in en zh_CN; do
        if [ -n "${FLAMESHOT_COMPLETION_TOOL_ROOT:-}" ]; then
            FLAMESHOT_TEST_UI_LANGUAGE="$check_locale" python3 tests/rust/completions.py "${CARGO_TARGET_DIR:-target}/debug/flameshot" "$FLAMESHOT_COMPLETION_TOOL_ROOT"
        else
            FLAMESHOT_TEST_UI_LANGUAGE="$check_locale" python3 tests/rust/completions.py "${CARGO_TARGET_DIR:-target}/debug/flameshot"
        fi
    done
}
cargo fmt --all -- --check
python3 tests/rust/translation_lookups.py
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
FLAMESHOT_TEST_PRIVATE_BUS=1 cargo test --locked -p flameshot --test daemon_exit -- --ignored
FLAMESHOT_TEST_PRIVATE_BUS=1 dbus-run-session -- cargo test --locked -p flameshot-platform --test portal -- --ignored
FLAMESHOT_TEST_PRIVATE_BUS=1 dbus-run-session -- cargo test --locked -p flameshot --test portal_open -- --ignored
FLAMESHOT_TEST_PRIVATE_BUS=1 dbus-run-session -- cargo test --locked -p flameshot --test full -- --ignored
FLAMESHOT_TEST_PRIVATE_BUS=1 dbus-run-session -- cargo test --locked -p flameshot --test daemon -- --ignored
FLAMESHOT_TEST_PRIVATE_BUS=1 dbus-run-session -- cargo test --locked -p flameshot --test notifications -- --ignored
FLAMESHOT_TEST_PRIVATE_BUS=1 dbus-run-session -- cargo test --locked -p flameshot-platform --test notifications -- --ignored
cargo clippy --locked --workspace --all-features --all-targets -- -D warnings
cargo test --locked --workspace --all-features
FLAMESHOT_TEST_PRIVATE_BUS=1 cargo test --locked -p flameshot --test daemon_exit --all-features -- --ignored
check_completions
cargo test --locked --workspace --no-default-features
FLAMESHOT_TEST_PRIVATE_BUS=1 cargo test --locked -p flameshot --test daemon_exit --no-default-features -- --ignored
FLAMESHOT_TEST_PRIVATE_BUS=1 dbus-run-session -- cargo test --locked -p flameshot --test daemon --no-default-features -- --ignored

check_completions
