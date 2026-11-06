#!/bin/bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Build a portable Cargo source archive with locked, vendored dependencies.
set -euo pipefail
if [ "$#" -ne 1 ]; then echo "Usage: $0 OUTPUT.tar.gz" >&2; exit 2; fi
package_output=$(realpath -m -- "$1")
package_version=$(sed -n 's/^version = "\([^"]*\)"/\1/p' Cargo.toml | head -n1)
[[ "$package_version" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.-]+)?(\+[0-9A-Za-z.-]+)?$ ]] || { echo 'Invalid workspace version' >&2; exit 1; }
package_epoch=${SOURCE_DATE_EPOCH:-$(git log -1 --format=%ct)}
[[ "$package_epoch" =~ ^[0-9]+$ ]] || { echo 'SOURCE_DATE_EPOCH must be numeric' >&2; exit 1; }
package_work=$(mktemp -d)
package_output_tmp=
cleanup() {
    rm -rf -- "$package_work"
    if [ -n "$package_output_tmp" ]; then rm -f -- "$package_output_tmp"; fi
}
trap cleanup EXIT
package_root="$package_work/flameshot-$package_version"
mkdir -p "$package_root"/{.cargo,docs,scripts,packaging,tests}
# Export only repository files; exclude local build artifacts and untracked data.
{
    git ls-files -z -- Cargo.toml Cargo.lock rust-toolchain.toml LICENSE README.md \
        crates data docs/rust tests/rust packaging/debian packaging/rpm \
        scripts/check-rust.sh scripts/stage-rust.sh scripts/package-rust-source.sh
    printf '%s\0' scripts/package-rust-source.sh
} | sort -zu | tar --null -T - -cf - | tar -xf - -C "$package_root"
(
    cd "$package_root"
    cargo vendor --locked --versioned-dirs vendor > .cargo/config.toml
    printf '\n[net]\noffline = true\n' >> .cargo/config.toml
)
mkdir -p "$(dirname "$package_output")"
package_output_tmp=$(mktemp "${package_output}.tmp.XXXXXX")
tar --sort=name --mtime="@$package_epoch" --owner=0 --group=0 --numeric-owner --mode='u=rwX,go=rX' \
    -C "$package_work" -cf - "flameshot-$package_version" | gzip -n > "$package_output_tmp"
mv -f -- "$package_output_tmp" "$package_output"
printf '%s\n' "$package_output"
