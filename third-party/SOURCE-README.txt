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
