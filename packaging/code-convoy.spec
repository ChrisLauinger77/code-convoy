Name: code-convoy
Version: %{app_version}
Release: 1
Summary: Local desktop task runner for coding-agent CLIs
License: MIT
URL: https://github.com/ChrisLauinger77/code-convoy
BuildArch: x86_64
Requires: git, xdg-utils, xdg-desktop-portal
Requires: libX11, libXcursor, libXi, libXrandr, libxkbcommon, libxkbcommon-x11, wayland-libs
Requires: mesa-libEGL, mesa-libGL, dbus-libs, fontconfig
# rpmbuild additionally derives ELF/glibc/libgcc requirements from the binary.
%global debug_package %{nil}
%global __os_install_post %{nil}

%description
Run one task across multiple local Git repositories with Codex CLI,
GitHub Copilot CLI, OpenCode, or Claude Code. Agent CLIs are installed and
authenticated separately. CodeConvoy does not manage credentials.

%install
mkdir -p %{buildroot}/usr/bin %{buildroot}/usr/share/applications
mkdir -p %{buildroot}/usr/share/icons/hicolor/256x256/apps
mkdir -p %{buildroot}/usr/share/licenses/code-convoy
install -m755 %{project_root}/target/release/codeconvoy %{buildroot}/usr/bin/codeconvoy
install -m644 %{project_root}/packaging/codeconvoy.desktop %{buildroot}/usr/share/applications/codeconvoy.desktop
install -m644 %{project_root}/assets/codeconvoy-256.png %{buildroot}/usr/share/icons/hicolor/256x256/apps/codeconvoy.png
install -m644 %{project_root}/LICENSE %{buildroot}/usr/share/licenses/code-convoy/LICENSE

%files
/usr/bin/codeconvoy
/usr/share/applications/codeconvoy.desktop
/usr/share/icons/hicolor/256x256/apps/codeconvoy.png
%license /usr/share/licenses/code-convoy/LICENSE
