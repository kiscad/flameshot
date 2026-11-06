#!/bin/sh
# SPDX-License-Identifier: GPL-3.0-or-later
set -eu
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
FLAMESHOT_TEST_PRIVATE_BUS=1 dbus-run-session -- cargo test --locked -p flameshot-platform --test portal -- --ignored
FLAMESHOT_TEST_PRIVATE_BUS=1 dbus-run-session -- cargo test --locked -p flameshot --test full -- --ignored
FLAMESHOT_TEST_PRIVATE_BUS=1 dbus-run-session -- cargo test --locked -p flameshot --test daemon -- --ignored
FLAMESHOT_TEST_PRIVATE_BUS=1 dbus-run-session -- cargo test --locked -p flameshot --test notifications -- --ignored
FLAMESHOT_TEST_PRIVATE_BUS=1 dbus-run-session -- cargo test --locked -p flameshot-platform --test notifications -- --ignored
cargo clippy --locked --workspace --all-features --all-targets -- -D warnings
cargo test --locked --workspace --all-features
FLAMESHOT_TEST_PRIVATE_BUS=1 cargo test --locked -p flameshot --test daemon_exit --all-features -- --ignored
cargo check --locked --workspace --no-default-features

python3 tests/rust/completions.py "${CARGO_TARGET_DIR:-target}/debug/flameshot"
