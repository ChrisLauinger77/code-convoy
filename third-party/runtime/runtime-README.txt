CodeConvoy AppImage runtime source and relinking kit
==================================================

This kit covers the separate AppImage launcher, which statically uses libfuse
3.15.0 under LGPL-2.1. CodeConvoy's separate executable and MIT license are
unchanged. Build scripts are MIT licensed; see runtime-build-LICENSE.txt.
Recipients may modify this runtime for their own use and reverse
engineer it to debug those modifications. Preserve the supplied license and
copyright notices. No written source offer or shared-library alternative is
being substituted for these materials. Compilation is not legal approval.

Extract THIRD-PARTY-SOURCES.zip into a new directory. All seven source archives,
the exact upstream libfuse patch (inside the runtime source), our dated source
modification notice, build scripts, and the build-input manifest are included.
The archives retain original source and licenses; mimalloc's documented repack
omits only prebuilt Windows executables. Read libfuse's LICENSE and LGPL2.txt.
The linux-uapi-6.6-r1 archive retains the exact kernel headers used for inline
byte-order code and mmap flags, the Alpine recipe, GPLv2 and Linux-syscall-note.
These system headers do not add a runtime shared-library dependency.
The supplied GCC crtstuff.c, COPYING3 and COPYING.RUNTIME cover compiler startup
code. gcc-14.2.0-alpine-r4.tar.gz retains the exact Alpine recipe and patches.

Prerequisites: x86-64 Linux, Python 3.11 or newer, bubblewrap, enabled unprivileged
user namespaces, about 2 GB of disk space per clean build, and network access
for the initial tool download. Python 3.12 or newer is recommended (tar filters).
On Ubuntu/Debian, the host build-only packages are python3 and bubblewrap.
No compiler or library is installed on the host by this recipe. It does not
need Docker or administrative privileges. A host policy blocking namespaces
must be addressed explicitly; never remove build isolation to work around it.

From the extracted kit directory:

  python3 build-runtime.py verify
  python3 build-runtime.py fetch --cache /absolute/path/runtime-input-cache
  python3 build-runtime.py build --cache /absolute/path/runtime-input-cache --work /absolute/path/build-one
  python3 build-runtime.py build --cache /absolute/path/runtime-input-cache --work /absolute/path/build-two
  cmp /absolute/path/build-one/out/runtime /absolute/path/build-two/out/runtime

Work directories must not exist. They are retained for inspection. The input
cache contains only checksum-verified upstream files. Fetch is the only network
operation; each build reconstructs a fresh checksum-pinned Alpine 3.21.3 root
filesystem, installs the exact listed APKs offline, clears the environment,
and builds without network access. No apk update, latest resolution, or mutable
container tag is used. If a pinned URL disappears, obtain the same bytes from
a trusted mirror/cache; do not substitute today's package or loosen a hash.

runtime-inputs.json records all versions, source/archive/patch digests, the
rootfs digest and the complete build-tool package closure with Alpine source
commits. Those external compiler/tool packages are system build prerequisites,
not shipped application code. Source-built musl supplies libc and rcrt1.o,
crti.o, crtn.o; GCC's pinned APK supplies crtbeginS.o and crtendS.o. The linker
map must not select libgcc, libgcc_eh or libssp_nonshared archive members.
The GCC Runtime Library Exception assessment relies on compilation from C by
GCC, without proprietary plugins or intermediate-code processing; see the
license itself. It is a documented interpretation, not a guarantee of title.

The upstream launcher source is unchanged except its build-time version label.
The libfuse code change is the upstream FUSERMOUNT_PROG patch; our second patch
adds the dated modification notice required by LGPL-2.1 section 2(a). Helpers,
examples and tests are not installed into the runtime. Squashfuse explicitly
supports zstd and gzip, matching the existing AppImage; xz, lzo and lz4 are
disabled. No host compression library can enter the static link implicitly.
GCC emits an existing upstream sprintf warning; the recipe keeps that specific
warning visible without making it an error. It does not change the function.

The build fixes SOURCE_DATE_EPOCH, locale, timezone, build paths and compiler
prefix maps. Static archives use deterministic GNU ar ordering. It targets
generic x86-64 and strips the runtime after linking, then writes Type 2 magic.
out/audit.json records tools, every link input, selected archive members and
output hashes. out/runtime.map and build.log retain the link/build evidence.
Two matching outputs prove repeatability for those builds, not license approval
or independence from all possible host kernels/hardware.

Modify and relink libfuse
------------------------
The included libfuse-demo.patch adds an observable marker to the library's initialization
function, keeping its license and a dated modification notice. It is never
applied to the stock release runtime. Demonstrate a recipient modification:

  python3 build-runtime.py build --cache /absolute/path/runtime-input-cache --work /absolute/path/build-modified --recipient-patch libfuse-demo.patch
  /absolute/path/build-modified/out/runtime --appimage-help

Expect "CodeConvoy modified libfuse relink demonstration" on stderr.
You may instead supply your own patch relative to the libfuse source root.
It is applied after the two documented patches. Source archives remain intact;
only the fresh build tree changes. The audit records your patch checksum.
The builder intentionally allows modified output; CodeConvoy release packaging
separately rejects runtimes that do not match its reviewed stock output digest.

Reuse the existing AppDir without rebuilding CodeConvoy:

  ./CodeConvoy-1.0.0-x86_64.AppImage --appimage-extract
  ARCH=x86_64 ./appimagetool --appimage-extract-and-run --runtime-file /absolute/path/build-modified/out/runtime squashfs-root ./modified.AppImage
  ./modified.AppImage --appimage-extract
  APPIMAGE_EXTRACT_AND_RUN=1 ./modified.AppImage
  env -u APPIMAGE_EXTRACT_AND_RUN ./modified.AppImage

appimagetool's exact version, URL and checksum are in runtime-inputs.json.
Verify the checksum before executing it. Normal FUSE startup needs /dev/fuse,
an installed fusermount3 or fusermount helper and a graphical desktop; extraction
fallback does not exercise FUSE. Close the application normally and confirm its
process exits in each mode. Wayland/XWayland behavior belongs to the unchanged
AppRun and application, not to the statically linked launcher. Test native
Ubuntu 24.04 and Debian 13 separately; containers alone do not establish native
desktop compatibility. Retain failures and unavailable checks explicitly.

Maintenance
-----------
Any input/patch/compiler change requires a fresh provenance and notice review,
two clean builds, modified-library relinking, final-image byte/ABI/material
verification and platform checks. Update hashes only after inspecting the new
inputs. Never infer resolution of a publication blocker from a successful build.
