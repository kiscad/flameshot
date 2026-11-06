# Build the local source archive with scripts/package-rust-source.sh first.
pkgname=flameshot-git
_pkgname=flameshot
_source_version=15.0.0
pkgver=r0
pkgrel=1
pkgdesc='Screenshot capture and annotation (Rust development build)'
arch=('i686' 'x86_64' 'aarch64' 'armv7h')
url='https://github.com/flameshot-org/flameshot'
license=('GPL-3.0-or-later')
depends=('glibc' 'gcc-libs' 'dbus' 'libx11' 'libxcursor' 'libxi' 'libxrandr'
         'libxkbcommon' 'libxkbcommon-x11' 'wayland' 'libglvnd'
         'vulkan-icd-loader' 'glib2' 'hicolor-icon-theme' 'ttf-dejavu')
makedepends=('cargo' 'pkgconf' 'desktop-file-utils')
checkdepends=('python')
optdepends=('xdg-desktop-portal: Wayland screenshot portal frontend'
            'xdg-desktop-portal-gtk: GTK portal backend'
            'xdg-desktop-portal-gnome: GNOME portal backend'
            'xdg-desktop-portal-kde: KDE portal backend'
            'xdg-desktop-portal-wlr: wlroots screenshot backend'
            'vulkan-swrast: software Vulkan rendering when no GPU driver is available'
            'mesa: OpenGL rendering implementation'
            'gnome-shell-extension-appindicator: GNOME tray icon support')
provides=("flameshot=${_source_version}")
conflicts=('flameshot')
source=("flameshot-${_source_version}-rust.tar.gz")
# This is a local, commit-specific source snapshot, not a remote release download.
sha256sums=('SKIP')

pkgver() {
    cat "${srcdir}/flameshot-${_source_version}/.git-version"
}

build() {
    cd "${srcdir}/flameshot-${_source_version}"
    export CARGO_INCREMENTAL=0
    cargo build --locked --offline --release -p flameshot --no-default-features --features imgur
}

check() {
    cd "${srcdir}/flameshot-${_source_version}"
    cargo test --locked --offline --workspace --no-default-features --features imgur
}

package() {
    cd "${srcdir}/flameshot-${_source_version}"
    DESTDIR="${pkgdir}" PREFIX=/usr \
        FLAMESHOT_BINARY="${CARGO_TARGET_DIR:-target}/release/flameshot" scripts/stage-rust.sh
    desktop-file-validate "${pkgdir}/usr/share/applications/org.flameshot.Flameshot.desktop"
    install -Dm644 README.md "${pkgdir}/usr/share/doc/flameshot/README.md"
    install -Dm644 docs/rust/BUILD.md "${pkgdir}/usr/share/doc/flameshot/BUILD.md"
}
