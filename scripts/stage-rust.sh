#!/bin/sh
# SPDX-License-Identifier: GPL-3.0-or-later
# Assemble a package payload without modifying a live installation.
set -eu
: "${DESTDIR:?Set DESTDIR to a package staging directory}"
package_prefix=${PREFIX:-/usr}
package_binary=${FLAMESHOT_BINARY:-target/release/flameshot}
case "$package_prefix" in /*) ;; *) echo 'PREFIX must be absolute' >&2; exit 1;; esac
[ -x "$package_binary" ] || { echo 'Build the Rust release executable first' >&2; exit 1; }
package_root="$DESTDIR$package_prefix"
install -Dm755 "$package_binary" "$package_root/bin/flameshot"
install -d "$package_root/share/applications" "$package_root/share/dbus-1/services" "$package_root/share/metainfo"
sed 's|@LAUNCHER_EXECUTABLE@|flameshot|g' data/desktopEntry/package/org.flameshot.Flameshot.desktop > "$package_root/share/applications/org.flameshot.Flameshot.desktop"
package_exec=$(printf '%s' "$package_prefix/bin/flameshot" | sed 's/[\\"]/\\&/g')
printf '[D-BUS Service]\nName=org.flameshot.Flameshot\nExec="%s"\n' "$package_exec" > "$package_root/share/dbus-1/services/org.flameshot.Flameshot.service"
# Catalogs are embedded by the Rust build script, not installed as Qt QM files.
sed '/<translation type="qt">flameshot<\/translation>/d' data/appdata/org.flameshot.Flameshot.metainfo.xml > "$package_root/share/metainfo/org.flameshot.Flameshot.metainfo.xml"
install -Dm644 data/img/app/org.flameshot.Flameshot.svg "$package_root/share/icons/hicolor/scalable/apps/org.flameshot.Flameshot.svg"
install -Dm644 data/img/app/org.flameshot.Flameshot.png "$package_root/share/icons/hicolor/128x128/apps/org.flameshot.Flameshot.png"
install -Dm644 LICENSE "$package_root/share/licenses/flameshot/LICENSE"
install -Dm644 data/img/app/flameshotLogoLicense.txt "$package_root/share/licenses/flameshot/logo-license.txt"
install -Dm644 data/img/material/LICENSE.txt "$package_root/share/licenses/flameshot/material-icons-license.txt"
chmod 644 "$package_root/share/applications/org.flameshot.Flameshot.desktop" \
  "$package_root/share/dbus-1/services/org.flameshot.Flameshot.service" \
  "$package_root/share/metainfo/org.flameshot.Flameshot.metainfo.xml"

install -Dm644 data/dbus/org.flameshot.Flameshot.xml "$package_root/share/dbus-1/interfaces/org.flameshot.Flameshot.xml"

install -Dm644 data/shell-completion/flameshot.bash "$package_root/share/bash-completion/completions/flameshot"
install -Dm644 data/shell-completion/flameshot.zsh "$package_root/share/zsh/site-functions/_flameshot"
install -Dm644 data/shell-completion/flameshot.fish "$package_root/share/fish/vendor_completions.d/flameshot.fish"

install -Dm644 data/man/man1/flameshot.1 "$package_root/share/man/man1/flameshot.1"
