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

AppImage source/relink build instructions are in runtime-README.txt. The
standalone build-runtime.py and build-runtime.sh use runtime-inputs.json to
reconstruct an offline checksum-pinned Alpine build filesystem, build all static
libraries, and relink the original upstream runtime. libfuse-notice.patch adds
a dated modification notice; libfuse-demo.patch is a recipient modification
example and is never applied to the stock runtime. Read the applicable notices
and LGPL2.txt inside the libfuse archive. Recipients may modify the combined
runtime for their own use and reverse engineer it to debug those modifications.
The separate CodeConvoy executable and its MIT license are unchanged.

All source archive checksums and retrieval locations are in the accompanying
inventory. These files are shipped inside the existing packages; they are not
a new public release artifact or a replacement for the license terms.

rust-1.99.0-library-Cargo.lock records the standard library's separate dependency
versions/checksums, recovered from the verified Rust source distribution. Its
notices supplement COPYRIGHT-library.html where that report omits entries.

AppImage compiler startup and system-header provenance
-----------------------------------------------------
Source-built musl supplies rcrt1.o, crti.o and crtn.o. The pinned Alpine GCC
14.2.0-r4 package supplies crtbeginS.o and crtendS.o; exact object hashes are
in runtime-inputs.json. gcc-14.2.0-alpine-r4.tar.gz retains the Alpine source
recipe and patches at commit 1a03a2a9c9e77f1a07d48b6f6805e72c8c63c03f.
gcc-14.2.0-crtstuff.c, COPYING3 and COPYING.RUNTIME retain the upstream source
and terms. Compiler packages are fetched by exact filename and checksum, not
resolved afresh. The full GCC source URL and SHA-512 are recorded in the input
manifest for further inspection. No libgcc, libgcc_eh or libssp_nonshared archive
member is selected into the reviewed launcher; a new selection fails the build.
linux-uapi-6.6-r1.tar.gz contains every exact installed Linux UAPI header from
the pinned package, including inline byte-order code, its Alpine recipe and
GPLv2/Linux-syscall-note terms. No kernel binary is bundled.
The Runtime Library Exception assessment concerns C compilation using GCC
without plugins or LTO. It is documented evidence and interpretation, not legal
approval. See the repository's AppImage validation record for results and gaps.

codeconvoy-nsis-helpers-1.zip contains the original shortcut identity helper,
the reduced Tauri process/version helper, exact Cargo.lock, recorded source patches,
template, license notices and build recipe. Unpack it into an empty directory;
read third-party/nsis/README.txt. The separate semver-1.0.18.crate,
windows-sys-0.48.0.crate and windows-targets-0.48.5.crate archives are the exact
checksum-verified dependency sources. Place them in third-party/sources when
reconstructing the kit outside the repository. The builder applies the one
recorded windows-sys stable-compiler patch, verifying both source hashes.
nsis-tauri-utils-0.2.1.tar.gz retains the upstream tagged source and MIT/Apache
notices for the process/version/ABI modules. Process and version source remain
unchanged; the kit records the stack safety patch and original/modified hashes.
Its original prebuilt
DLL and the old ApplicationID DLL are not distributed. The new source build
uses Rust 1.99.0 and separately captures x86 CRT inputs. Windows CRT publication
obligations remain unresolved; no Microsoft build tools or license grants are
supplied by this source kit.

The outer ZIP has sorted portable names, 1980-01-01 timestamps, Unix regular
file mode 0644, fixed ZIP 2.0 metadata, no comments/extra fields, and no second
compression pass. Host file times, permissions and operating system do not
change its bytes. Final-package checks compare the actual ZIP bytes, including
metadata, with the audited bundle.
