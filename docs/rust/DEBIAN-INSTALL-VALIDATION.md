# Debian package installation on Ubuntu Base

## Refreshed package with patched X11 wheel input

Validated source: `r2493.18f5492a`. Package:
`target/debian-wheel-9te5bn27/flameshot_15.0.0-1_amd64.deb`.

- Package SHA-256: `f8efbf44917805cbead5780a22dc8c85b3c1407f000dd95ca239f32d5e1f59b1`.
- Installed executable SHA-256: `31e3647bd227a30c48fe15367915327271bfd59aee1428840a82d8c4aa01550f`.

The fresh source archive built through `dpkg-buildpackage -b -us -uc -d`, including
the recipe's Rust workspace tests. The same extracted debhelper tools and external
Rust toolchain described below were used; this was not a containerized compiler
build. The release compilation completed in 109 seconds. Cleanup preserved the
patched dependency's original manifest and source. The package now installs the
winit license and patch provenance alongside the application license.

A new disposable container used the cached `flameshot-test-installed:review` image,
with `--init --network none` and no mounts. Its previous application package was
purged before installing the new package. Runtime dependencies were already
cached; the new and previous package dependency lists are identical. Purging and
reinstalling the new package succeeded, retained the user INI byte-for-byte and
passed `dpkg -V`. The installed license, patch notice and D-Bus XML match the
source resources. Imgur/history is present, update checking is absent, and the
executable has no missing dynamic libraries or Qt linkage.

The installed executable passes the Chinese-window and pin-save suites, including
consecutive wheel clicks, reverse scrolling, rotated output pixels, save retry,
overwrite confirmation and clipboard ownership after close. The full installed
X11 suite also passes: full/screen capture, selection/edit/export, cancellation,
save retry, monitor rejection and daemon/clipboard lifetime. Artifacts were copied
to `target/debian-wheel-artifacts`; the test container was stopped and removed.

Local evidence: `target/debian-wheel-build-validation.json`,
`target/debian-wheel-install-validation.json` and `target/debian-wheel-*.log`.
This refresh does not rerun the historical Qt-to-Rust upgrade below or establish
other distributions, architectures, Wayland package installation or sandbox builds.

## Earlier installation baseline

Validated source snapshot: `r2446.4bc3a3a3`.
Package: `target/deb-install-review-14082kir/flameshot_15.0.0-1_amd64.deb`.
Package SHA-256:

```text
71efd8740229878b13739381a726252a1ae24276550bd2a97bec4d3e3b8ca2d6
```

Installed executable SHA-256:

```text
c03756f9c95cedbea2f419d748b361d06a65641a55b453cab6f388a89e310149
```

The package was built from a fresh vendored archive through packaging/debian/rules,
including Cargo tests. As in earlier package builds, extracted debhelper tools and
the installed Rust toolchain were supplied outside dpkg's build-dependency database,
so dpkg-buildpackage used -d. This was not a containerized compiler/toolchain build.
The recipe enabled Imgur and disabled the application update checker; the installed
CLI exposes history and does not expose updates.

The installation target was a new Docker filesystem imported from the signed
[Ubuntu Base 24.04.5 amd64 archive](https://cdimage.ubuntu.com/ubuntu-base/releases/24.04/release/ubuntu-base-24.04.5-base-amd64.tar.gz).
Its SHA-256 was `e77b6f10c2590cef872b33ee9f635a0e3fd1f57fb074c0e52b5c7f56147a0c86`;
SHA256SUMS.gpg verified using the installed Ubuntu archive keyring and Ubuntu CD
Image Automatic Signing Key (2012). Docker Hub/cache pulls were unavailable, so the
verified root archive was imported locally instead. No host directories, display,
D-Bus socket or Docker socket were mounted into the test container.

APT installed the local package with --no-install-recommends using its declared
runtime dependencies. Version/configuration commands passed and no Qt 5/6 runtime
packages were present. Ubuntu Base excludes manuals/documentation by default; that
policy was disabled inside the container and the package reinstalled before full
payload verification. `dpkg -V flameshot` then produced no output.

After installing Xvfb/xdotool/xclip/Pillow test utilities, the installed /usr/bin
executable passed the complete isolated X11 desktop suite. The container used
Docker --init: a plain sleep PID 1 had retained zombie descendants and invalidated
the ownership test. The suite also needed its language-menu coordinates adjusted
for the distro build's absent update-check checkbox; no production behavior changed.

APT purge removed the executable, desktop entry and D-Bus service. A pre-existing
user INI (including an unknown section) remained byte-for-byte unchanged. Reinstall
restored a verifiable payload and accepted that same configuration. The test
container was stopped/removed; task-specific base/installed images remain cached.
A machine-readable report is at target/debian-install-validation.json, with desktop
artifacts under target/debian-install-artifacts.

This verifies this Ubuntu/amd64 package installation and X11 runtime. It does not
establish all Debian releases, other architectures, containerized source builds,
Wayland package installation, Flatpak/Snap confinement or hosted CI. The inherited
changelog still produces its previously recorded version-order warning for the old
15.0.rc2 entry. Later production changes require refreshed package evidence.

## Upgrade from the packaged Qt implementation

A separate isolated container installed Ubuntu's `12.1.0-2build2` package. That
executable identified itself as Flameshot 12.1.0 compiled with Qt 5.15.12. Its actual
CLI wrote a filename pattern and main/contrast colors into an isolated user INI
that also contained an unknown External section. The generated file's SHA-256 was
`8328b5f741aac733e1ef443a9531aea665bb55a9bb2dab467dfd929320c77668`.

APT then upgraded it to the local Rust `15.0.0-1` package. The INI bytes were unchanged
and the Rust config checker accepted them. Updating the filename with the Rust CLI
changed only that setting's line, preserving all remaining Qt-generated fields and
the unknown section. Package checksum verification stayed clean; the new executable
had no Qt linkage and the new package owned no QM catalogs. Old Qt dependency
packages were not forcibly removed.

This validates the tested Qt 5 package/configuration path, not every historical Qt
schema or an in-place upgrade while a legacy GUI is running. The container was
stopped and removed. Local evidence is in target/qt-to-rust-upgrade-validation.json
and target/qt-upgrade-config-before.ini / target/qt-upgrade-config-after.ini.
