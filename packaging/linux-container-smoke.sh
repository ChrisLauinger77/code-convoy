#!/bin/bash
# Only run inside a disposable container. Install the exact candidate with the
# distribution's ordinary dependency resolver; never force missing dependencies.
set -euo pipefail
if ! test -f /.dockerenv; then
  echo 'Run this installation/removal test only inside its disposable Docker container.' >&2
  exit 2
fi
cd /candidate
case "$1" in
  ubuntu|debian)
    export DEBIAN_FRONTEND=noninteractive
    apt-get update
    apt-get install -y ./dist/*.deb
    apt-get install -y python3 xvfb xauth openbox wmctrl dbus-x11 procps
    dpkg-query -W code-convoy libc6
    ;;
  fedora)
    dnf install -y ./dist/*.rpm
    dnf install -y python3 xorg-x11-server-Xvfb xorg-x11-xauth openbox wmctrl dbus-daemon procps-ng
    rpm -q code-convoy glibc
    rpm -q --requires code-convoy
    ;;
  *) exit 2 ;;
esac
git --version
sha256sum dist/*.deb dist/*.rpm dist/*.AppImage
xvfb-run -a dbus-run-session -- python3 packaging/linux_runtime.py /usr/bin/codeconvoy
xvfb-run -a dbus-run-session -- python3 packaging/linux_runtime.py dist/*.AppImage --mode extract
export PATH="/candidate/target/packaging/linux/host-bin:$PATH"
export CODECONVOY_EXPECT_PATH="$PATH"
export CODECONVOY_EXPECT_LD_LIBRARY_PATH="${LD_LIBRARY_PATH-}"
APPIMAGE_EXTRACT_AND_RUN=1 /candidate/target/packaging/linux/probe.AppImage
case "$1" in
  ubuntu|debian) apt-get remove -y code-convoy ;;
  fedora) dnf remove -y code-convoy ;;
esac
test ! -e /usr/bin/codeconvoy
echo "PASS disposable $1 package install/start/normal-close/remove and process/Git probe; native desktop checks remain separate."
