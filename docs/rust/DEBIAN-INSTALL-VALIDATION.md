# Debian package installation on Ubuntu Base

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
