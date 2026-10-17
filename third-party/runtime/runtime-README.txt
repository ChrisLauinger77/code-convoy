CodeConvoy: official AppImage runtime sources and recipient relinking
==================================================================

The release embeds the unmodified official x86_64 Type 2 runtime 20251108,
revision dd6cebedcbddde9c82f89b011e8e1d40b6e43868, SHA-256
2fca8b443c92510f1483a883f60061ad09b46b978b2631c807cd873a47ec260d.
The separate CodeConvoy application retains its MIT license. The launcher
statically links LGPL-2.1 libfuse. Recipients may modify the combined runtime
for their own use and reverse engineer it to debug those modifications.
Read LICENSE, LGPL2.txt and GPL2.txt in the supplied libfuse archive.

THIRD-PARTY-SOURCES.zip includes the exact runtime source, upstream build
scripts and mount.c.diff, libfuse 3.15.0, squashfuse 0.5.2, musl 1.2.5,
zstd 1.5.6, zlib 1.3.1 and mimalloc 2.1.7 sources with their original notices.
The mimalloc source-only repack excludes prebuilt Windows executables.
All archive checksums and origins are in THIRD-PARTY-INVENTORY.json.
GNU CRT reference source, GPLv3 and Runtime Library Exception 3.1 and Linux
UAPI reference sources/notices are retained conservatively. They do not imply
recovery of every original Alpine package revision or byte-identical output.

Use a disposable x86_64 Alpine 3.21 VM/container with about 2 GB free space,
network access for ordinary Alpine build tools, and a shell as root *inside
that build environment*. Do not run its package installation on your host.
The upstream scripts/docker/Dockerfile and BUILD.md identify the environment.
For example, an existing Docker installation can provide an Alpine 3.21 shell
with the extracted ZIP mounted read-only at /sources. This is an upstream
build recipe for recipients, not CodeConvoy's release build infrastructure.
The validation used the same Alpine userspace in a disposable user namespace;
Docker itself was unavailable. libfuse's installation expects /dev/fuse to
exist (or permission to create that device); expose that host device to the
build environment if its container policy prohibits mknod.

Inside the disposable environment, with the extracted ZIP at /sources:

  apk add --no-cache bash alpine-sdk util-linux strace file autoconf automake libtool xz eudev-dev gettext-dev linux-headers meson zstd-dev zstd-static zlib-dev zlib-static clang musl-dev mimalloc-dev
  mkdir /work
  cd /work
  tar xf /sources/AppImage-runtime-dd6cebed.tar.gz
  cd type2-runtime-dd6cebedcbddde9c82f89b011e8e1d40b6e43868
  printf '%s\n' 'https://github.com/AppImage/type2-runtime/commit/dd6cebe-recipient-demo' > src/runtime/version

Use the supplied library archives instead of the two upstream downloads.
Keep the upstream checksum checks and apply its mount patch unchanged. Add
our source-only dated annotation and the optional observable LGPL demo:

  sed -i '/^wget .*libfuse/c\cp /sources/libfuse-3.15.0.tar.xz fuse-3.15.0.tar.xz' scripts/common/install-dependencies.sh
  sed -i '/^wget .*squashfuse/c\cp /sources/squashfuse-0.5.2.tar.gz 0.5.2.tar.gz' scripts/common/install-dependencies.sh
  sed -i '/^patch -p1/a\patch -p1 < /sources/libfuse-notice.patch\npatch -p1 < /sources/libfuse-demo.patch' scripts/common/install-dependencies.sh
  bash scripts/common/install-dependencies.sh
  bash scripts/build-runtime.sh
  ./out/runtime-x86_64 --appimage-version

Expect "CodeConvoy modified libfuse relink demonstration" on stderr. The demo
changes libfuse's initialization function, not the runtime launcher source.
To make your own library modification, replace libfuse-demo.patch with your
patch, preserving notices and documenting your change and date. The supplied
notice patch annotates the upstream mount modification in the recipient source;
it does not claim CodeConvoy patched the official distributed runtime.

The unchanged upstream Makefile and build-runtime.sh compile and statically
link the replacement using ordinary Alpine tools/libraries. Supporting Alpine
package revisions can differ from those used for the official binary (the
validation used zlib 1.3.2 instead of 1.3.1). Matching the official SHA-256 is
neither expected nor required for recipient replacements. The supplied original
supporting sources remain available for inspection/modification. No private
compiler, APK inventory, source downloader or reproducible-build system is needed.

Copy out out/runtime-x86_64. On your graphical x86_64 Linux machine, obtain
appimagetool 1.9.1 from
https://github.com/AppImage/appimagetool/releases/download/1.9.1/appimagetool-x86_64.AppImage
and verify SHA-256 ed4ce84f0d9caff66f50bcca6ff6f35aae54ce8135408b3fa33abfc3cb384eb0
before execution. In a fresh directory, with the existing CodeConvoy AppImage:

  chmod +x appimagetool-x86_64.AppImage runtime-x86_64
  ./CodeConvoy-1.0.0-x86_64.AppImage --appimage-extract
  env -u VERSION ARCH=x86_64 ./appimagetool-x86_64.AppImage --appimage-extract-and-run --runtime-file ./runtime-x86_64 squashfs-root ./modified.AppImage
  env -u APPIMAGE_EXTRACT_AND_RUN ./modified.AppImage
  APPIMAGE_EXTRACT_AND_RUN=1 ./modified.AppImage

Close each application normally before starting the next. Both launches must
print the modified-libfuse marker. Normal FUSE mode needs /dev/fuse and the
system fusermount3/fusermount helper; extraction does not. The same AppDir and
application executable are reused; CodeConvoy does not need rebuilding.

Known upstream limitation: extraction-and-run retains appimage_extracted_*
under TMPDIR after exit, in both the published custom and official runtimes.
Repeated identical images reuse that directory; distinct images can accumulate
copies. FUSE unmount/mountpoint cleanup is unaffected in the verified cases.
Use a private temporary directory for testing and remove only your own test
files after all processes exit. No automatic cleanup workaround is shipped.

Maintenance is limited to reviewing new official pins, source/license changes,
recipient instructions and packaging/compatibility tests. The dated validation
record is docs/appimage-upstream-runtime-validation.md in the matching CodeConvoy
source. The demonstration establishes practical relinking; it is not legal
approval or a promise to reproduce the original compiler's output byte for byte.
