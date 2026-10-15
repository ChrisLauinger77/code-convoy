Source included with the CodeConvoy license audit candidate
==========================================================

option-ext-0.2.0.crate is the exact unmodified crates.io source archive whose
SHA-256 is recorded in Cargo.lock and THIRD-PARTY-INVENTORY.json. It contains
the MPL-2.0 source and license. Extract using tar -xzf or an archive manager.
There are no CodeConvoy patches to this crate. Rebuild CodeConvoy from the
matching candidate source using its locked Cargo dependencies.

AppImage-runtime-dd6cebed.tar.gz is the upstream type2-runtime source at
dd6cebedcbddde9c82f89b011e8e1d40b6e43868 (20251108). It includes build scripts,
the Makefile, and patches/libfuse/mount.c.diff. libfuse-3.15.0.tar.xz is the
exact library source downloaded by those scripts; squashfuse-0.5.2.tar.gz is
the exact squashfuse input. Their upstream licenses remain in the archives.
No CodeConvoy modifications are applied to these archives.

nsis-3.09-src.tar.bz2 is the official NSIS 3.09 source distribution, including
the LZMA compression module used in the Windows installer and its CPL-1.0
license/linking exception. The source is available directly in this package.
Read its COPYING and build instructions. CodeConvoy has not modified that
module; the exception permits linking separately licensed application code.

To inspect the upstream build recipe, read scripts/build-runtime.sh,
scripts/docker/Dockerfile, scripts/common/install-dependencies.sh and
src/runtime/Makefile. It patches libfuse and links it statically. The recipe's
Alpine packages are not version-pinned. A successful independent relink of the
shipped runtime has NOT been demonstrated, and this bundle is not represented
as a complete LGPL corresponding-source/relink kit. That is a release blocker.
Do not rely on the upstream tag URL alone as fulfillment of that obligation.

All source archive checksums and retrieval locations are in the accompanying
inventory. These files are shipped inside the existing packages; they are not
a new public release artifact or a replacement for the license terms.

rust-1.99.0-library-Cargo.lock records the standard library's separate dependency
versions/checksums, recovered from the verified Rust source distribution. Its
notices supplement COPYRIGHT-library.html where that report omits entries.

Additional AppImage source evidence
----------------------------------
The matching upstream runtime-x86_64.debug has the same GNU build ID and the
CRC32 required by the runtime's .gnu_debuglink. Its DWARF identifies musl 1.2.5
and libfuse 3.15.0. Its symbols locate runtime version functions reporting zstd
1.5.6, zlib 1.3.1 and mimalloc 2.1.7. See native_provenance in the inventory for
digests, offsets and references. These upstream source versions are included.
They do not identify the exact Alpine package revisions or downstream patches.

musl-1.2.5.tar.gz, zstd-1.5.6.tar.gz and zlib-1.3.1.tar.gz preserve upstream
archive bytes. mimalloc-2.1.7-source.tar.gz preserves every source and license
file byte but omits four unrelated prebuilt Windows executables from upstream's
bin directory. The inventory records the original digest and excluded paths;
tar ownership and times are normalized. This is a source-only repack, not an
upstream release archive or a modified mimalloc implementation.

gcc-14.2.0-crtstuff.c, gcc-14.2.0-COPYING3 and gcc-14.2.0-COPYING.RUNTIME
are unmodified upstream reference files. The matched runtime debug symbols
include GNU-style CRT startup functions, and its compiler comments include
Alpine GCC 14.2.0. These observations do not identify the exact selected CRT
objects or downstream patches. The reference source retains its copyright,
GPLv3-or-later notice and GCC Runtime Library Exception 3.1. This is not a
complete GCC source distribution or proof of the shipped object's provenance.
The exception's Target Code permission depends on its stated conditions;
establish those for the actual build before removing the provenance blocker.

LGPL relinking status and required verification
----------------------------------------------
The relevant LGPL 2.1 section 6(a) work is the standalone AppImage launcher
linked with libfuse, not the separate CodeConvoy executable in its filesystem.
The supplied launcher source is the work that uses the library; the library
archive plus patches/libfuse/mount.c.diff contain the known library change.
Read LGPL2.txt and LICENSE inside libfuse's archive, and the launcher LICENSE.
No written source offer is substituted for the included materials.

To complete verification, recover the original Alpine APK revisions/patches
and build inputs, prepare a clean x86-64 Linux build environment, and follow
the launcher's BUILD.md and scripts/build-runtime.sh. Its Dockerfile currently
downloads mutable packages, so running it alone does not prove correspondence.
Then change libfuse, rebuild/relink the launcher with the modified library and
use the existing appimagetool --runtime-file mechanism with the extracted
AppDir. Verify extraction, FUSE startup and normal exit of the rebuilt image.
Retain commands, input hashes and results. This modified-library relink has
NOT been performed successfully for this candidate; publication remains blocked.
Recipients may modify the runtime for their own use and reverse engineer it
to debug those modifications; CodeConvoy imposes no additional restrictions.

nsis-tauri-utils-0.2.1.tar.gz is the exact upstream tagged source with its MIT
and Apache notices. The release DLL matches upstream's published SHA-1. The
inventory and notices also cover 17 crate versions recovered from DLL strings.
The tag has no Cargo.lock, so neither this archive nor a new Cargo resolution
establishes the complete original dependency graph. Its embedded compiler is
Rust 1.71.1, not CodeConvoy's Rust 1.99.0. Remaining plugin source/license and
toolchain evidence gaps remain release blockers.

The outer ZIP has sorted portable names, 1980-01-01 timestamps, Unix regular
file mode 0644, fixed ZIP 2.0 metadata, no comments/extra fields, and no second
compression pass. Host file times, permissions and operating system do not
change its bytes. Final-package checks compare the actual ZIP bytes, including
metadata, with the audited bundle.
