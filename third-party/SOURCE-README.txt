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

AppImage source/relink instructions are in runtime-README.txt. They use the
supplied archives and original upstream build scripts with ordinary Alpine
build tools. libfuse-notice.patch annotates the upstream mount modification
in recipient source; libfuse-demo.patch is an observable LGPL modification
example. Neither is applied to the official release runtime. Recipients may
modify the combined runtime for their own use and reverse engineer it to debug
those modifications. The separate CodeConvoy executable remains MIT licensed.
No private builder, compiler or Alpine package inventory is required.

All source archive checksums and retrieval locations are in the accompanying
inventory. These files are shipped inside the existing packages; they are not
a new public release artifact or a replacement for the license terms.

rust-1.99.0-library-Cargo.lock records the standard library's separate dependency
versions/checksums, recovered from the verified Rust source distribution. Its
notices supplement COPYRIGHT-library.html where that report omits entries.

AppImage compiler startup and system-header provenance
-----------------------------------------------------
The official runtime's matching debug asset and compiler comments identify
musl 1.2.5, GCC 14.2.0 and Alpine clang 19.1.4. gcc-14.2.0-crtstuff.c, COPYING3
and COPYING.RUNTIME retain reference source and applicable terms conservatively.
linux-uapi-6.6-r1.tar.gz retains reference Linux headers and GPLv2/Linux-syscall-note
terms. No kernel binary is bundled. Exact original APK revisions and CRT object
hashes are not claimed. Upstream's C/clang build and the successful recipient
relink procedure are documented in docs/appimage-upstream-runtime-validation.md.
The eligible-compilation interpretation of the GCC exception does not require
byte-identical output or proprietary intermediate-code processing. This is
source/build evidence and a license interpretation, not legal approval.

The Windows installer uses standard NSIS instructions, bundled headers and
plugins, and Windows system APIs for version comparison, process handling and
shortcut identity. Their NSIS source and license terms are retained in
nsis-3.09-src.tar.bz2. No custom helper DLLs, helper dependency archives or
helper source kit are included or required. Rebuild the installer from the
matching CodeConvoy source using packaging/nsis/installer.nsi,
packaging/windows-installer.nsh and the instructions in docs/releasing.md.
Historical custom-helper source and reconstruction instructions belong to the
published 1.0.0 release and its Git tag:
https://github.com/ChrisLauinger77/code-convoy/tree/v1.0.0/third-party
The current Microsoft notices and version-bound licensing-risk decision remain
in THIRD-PARTY-NOTICES.txt and THIRD-PARTY-INVENTORY.json. This source bundle
does not supply Microsoft build tools or additional license grants.

The outer ZIP has sorted portable names, 1980-01-01 timestamps, Unix regular
file mode 0644, fixed ZIP 2.0 metadata, no comments/extra fields, and no second
compression pass. Host file times, permissions and operating system do not
change its bytes. Final-package checks compare the actual ZIP bytes, including
metadata, with the audited bundle.
