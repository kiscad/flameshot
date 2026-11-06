# Build Source0 with scripts/package-rust-source.sh; it contains locked vendor sources.
Name: flameshot
Version: 15.0.0
Release: 1%{?dist}
Summary: Screenshot capture and annotation for Linux
License: GPL-3.0-or-later
URL: https://github.com/flameshot-org/flameshot
Source0: %{name}-%{version}.tar.gz

BuildRequires: cargo >= 1.99
BuildRequires: rust >= 1.99
BuildRequires: gcc
BuildRequires: pkgconfig(xkbcommon)
BuildRequires: pkgconfig(wayland-client)
BuildRequires: desktop-file-utils
BuildRequires: /usr/bin/python3
BuildRequires: /usr/bin/gio
%if 0%{?suse_version}
BuildRequires: dejavu-fonts
Requires: dejavu-fonts
Recommends: libvulkan_lvp
%else
BuildRequires: dejavu-sans-fonts
Requires: dejavu-sans-fonts
Recommends: mesa-vulkan-drivers
%endif

# These libraries are loaded dynamically and are invisible to ELF dependency scans.
%if 0%{?__isa_bits} == 64
%global library_suffix ()(64bit)
%else
%global library_suffix %{nil}
%endif
Requires: libX11.so.6%{library_suffix}
Requires: libX11-xcb.so.1%{library_suffix}
Requires: libXcursor.so.1%{library_suffix}
Requires: libXi.so.6%{library_suffix}
Requires: libXrandr.so.2%{library_suffix}
Requires: libxkbcommon.so.0%{library_suffix}
Requires: libxkbcommon-x11.so.0%{library_suffix}
Requires: libwayland-client.so.0%{library_suffix}
Requires: libwayland-egl.so.1%{library_suffix}
Requires: libEGL.so.1%{library_suffix}
Requires: libGL.so.1%{library_suffix}
Requires: libvulkan.so.1%{library_suffix}
Requires: /usr/bin/gio
Requires: /usr/bin/dbus-run-session
Requires: hicolor-icon-theme
Recommends: xdg-desktop-portal
Recommends: (xdg-desktop-portal-gnome if gnome-shell)
Recommends: (xdg-desktop-portal-kde if plasma-workspace-wayland)
Recommends: (xdg-desktop-portal-wlr if wlroots)

%description
Flameshot provides screenshot capture, annotation, pinned images, and graphical
and command-line interfaces. This Rust build supports X11 and screenshot portals.
Imgur integration is included; distribution packages disable application update
checking.

%prep
%autosetup -p1

%build
export CARGO_INCREMENTAL=0
cargo build --locked --offline --release -p flameshot --no-default-features --features imgur

%install
DESTDIR="%{buildroot}" PREFIX="%{_prefix}" FLAMESHOT_BINARY="${CARGO_TARGET_DIR:-target}/release/flameshot" scripts/stage-rust.sh

%check
cargo test --locked --offline --workspace --no-default-features --features imgur
desktop-file-validate %{buildroot}%{_datadir}/applications/org.flameshot.Flameshot.desktop

%files
%doc README.md docs/rust/BUILD.md
%license %{_datadir}/licenses/%{name}
%{_bindir}/flameshot
%{_datadir}/applications/org.flameshot.Flameshot.desktop
%{_datadir}/metainfo/org.flameshot.Flameshot.metainfo.xml
%{_datadir}/dbus-1/interfaces/org.flameshot.Flameshot.xml
%{_datadir}/dbus-1/services/org.flameshot.Flameshot.service
%{_datadir}/icons/hicolor/128x128/apps/org.flameshot.Flameshot.png
%{_datadir}/icons/hicolor/scalable/apps/org.flameshot.Flameshot.svg

%{_datadir}/bash-completion/completions/flameshot
%{_datadir}/zsh/site-functions/_flameshot
%{_datadir}/fish/vendor_completions.d/flameshot.fish

%changelog
* Sun Nov 01 2026 Jeremy Borgman <borgman.jeremy@pm.me> - 15.0.0
- Updated for v15.0.0 release

* Fri Oct 30 2026 Jeremy Borgman <borgman.jeremy@pm.me> - 15.0.rc2
- Beta for 15 release

* Sat Sep 12 2026 Jeremy Borgman <borgman.jeremy@pm.me> - 15.0.rc1
- Beta for 15 release

* Mon May 18 2026 Jeremy Borgman <borgman.jeremy@pm.me> - 14.0.rc2
- Beta for 14 release

* Mon Mar 02 2026 Jeremy Borgman <borgman.jeremy@pm.me> - 14.0.rc1
- Beta for 14 release

* Tue Oct 28 2025 Jeremy Borgman <borgman.jeremy@pm.me> - 13.3.0
- Updated for v13.3.0 release

* Fri Oct 24 2025 Jeremy Borgman <borgman.jeremy@pm.me> - 13.2.0
- Updated for v13.2.0 release

* Sat Aug 16 2025 Elliott Tallis <tallis.elliott@gmail.com> - 13.1.0-2
- Minor spec file tweaks

* Fri Aug 15 2025 Jeremy Borgman <borgman.jeremy@pm.me> - 13.1.0
- Update for v13.1.0 release

* Wed Aug 06 2025 Jeremy Borgman <borgman.jeremy@pm.me> - 13.0.1
- Update for v13.0.1 release

* Sun Aug 03 2025 Jeremy Borgman <borgman.jeremy@pm.me> - 13.0.0
- Update for v13 release

* Sun Jul 27 2025 Jeremy Borgman <borgman.jeremy@pm.me> - 13.0.rc2
- Beta for 13 release.

* Sat Jul 12 2025 Jeremy Borgman <borgman.jeremy@pm.me> - 13.0.rc1
- Beta for 13 release.

* Sun Jul 03 2022 Jeremy Borgman <borgman.jeremy@pm.me> - 12.1.0-1
- Update for 12.1 release.

* Tue Jun 21 2022 Jeremy Borgman <borgman.jeremy@pm.me> - 12.0.0-1
- Update for 12.0 release.

* Fri Jan 14 2022 Jeremy Borgman <borgman.jeremy@pm.me> - 11.0.0-1
- Update for 11.0 release.

* Sun Aug 29 2021 Zetao Yang <vitzys@outlook.com> - 0.10.1-2
- Minor SPEC fixes.

* Sun Jul 25 2021 Jeremy Borgman <borgman.jeremy@pm.me> - 0.10.1-1
- Updated for flameshot 0.10.1

* Mon May 17 2021 Jeremy Borgman <borgman.jeremy@pm.me> - 0.10.0-1
- Updated for flameshot 0.10.0

* Sat Feb 27 2021 Jeremy Borgman <borgman.jeremy@pm.me> - 0.9.0-1
- Updated for flameshot 0.9.0

* Wed Oct 14 2020 Jeremy Borgman <borgman.jeremy@pm.me> - 0.8.5-1
- Updated for flameshot 0.8.5

* Sat Oct 10 2020 Jeremy Borgman <borgman.jeremy@pm.me> - 0.8.4-1
- Updated for flameshot 0.8.4

* Sat Sep 19 2020 Jeremy Borgman <borgman.jeremy@pm.me> - 0.8.3-1
- Updated for flameshot 0.8.3

* Mon Sep 07 2020 Zetao Yang <vitzys@outlook.com> - 0.8.0-1
- Updated for flameshot 0.8.0
- More details, please see https://flameshot.org/changelog/#v080

* Sat Aug 18 2018 Zetao Yang <vitzys@outlook.com> - 0.6.0-1
- Updated for flameshot 0.6.0
- More details, please see https://flameshot.org/changelog/#v060

* Tue Jan 09 2018 Zetao Yang <vitzys@outlook.com> - 0.5.0-1
- Initial package for flameshot 0.5.0
- More details, please see https://flameshot.org/changelog/#v050
