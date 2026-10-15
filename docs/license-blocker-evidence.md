# PR 68 — native license evidence follow-up

Audit date: 2026-10-15. **Publication remains blocked.** This record distinguishes
recovered evidence from missing evidence; it is not a legal clearance. The
project's MIT license is unchanged. PR checks, interim artifact evidence and the
exact tested revisions are recorded in [PR 68](https://github.com/ChrisLauinger77/code-convoy/pull/68).
Per maintainer direction, the next release candidate and final six-format
verification will run from `main` after merge; no further branch release build
is part of this handoff.

## 1. AppImage static-library provenance: partially resolved

The shipped launcher is the checksum-pinned upstream `20251108` x86-64 runtime,
source revision `dd6cebedcbddde9c82f89b011e8e1d40b6e43868`. The corresponding
[debug asset](https://github.com/AppImage/type2-runtime/releases/download/20251108/runtime-x86_64.debug)
has SHA-256 `1f3c4a8439e344d74b61bfd0610b89e8499453791bbc3c5b4ae38c181fe54942`.
Both files have GNU build ID `a87aaf5da1bf2ef30becedd1aa58e217818fb503`;
the debug asset's CRC32 `dd5771c8` matches the launcher's `.gnu_debuglink`.
This ties the symbols to the actual redistributed runtime, rather than merely
to a similarly named release. Reproduce with `readelf -n`,
`readelf -x .gnu_debuglink runtime` and the debug file's CRC32.

`readelf --debug-dump=info runtime-x86_64.debug` and `nm` identify the following;
`objdump -d` of the **runtime** confirms the version-function bytes:

| Library | Recovered upstream identity | Evidence |
| --- | --- | --- |
| musl | 1.2.5 | DWARF compilation directory `/home/buildozer/aports/main/musl/src/musl-1.2.5` |
| libfuse | 3.15.0 | DWARF directory `/tmp/tmp.mOkBIM/fuse-3.15.0/build`; pinned source in upstream build recipe |
| squashfuse | 0.5.2 | Exact source URL/digest in upstream build recipe |
| zstd | 1.5.6 | `ZSTD_versionString` at `0x32340` returns string at `0x899f2` |
| zlib | 1.3.1 | `zlibVersion` at `0x3bbc0` returns string at `0x89b70` |
| mimalloc | 2.1.7 | `mi_version` at `0x63340`: `b8 d9 00 00 00 c3`, returning 217 |

The [mimalloc 2.1.7 header](https://github.com/microsoft/mimalloc/blob/v2.1.7/include/mimalloc.h)
defines `MI_MALLOC_VERSION` as 217. Its actual matching MIT notice replaces the
previous unverified 2.1.8 reference. Matching musl, zstd, zlib and mimalloc source
material is now bundled. The mimalloc source repack excludes four upstream
prebuilt Windows binaries, preserving every remaining file's bytes and all
licenses; its original archive hash, transformation and exclusions are recorded.
No unrelated native executables are added through the source bundle.

The same matched symbols also expose GNU-style compiler startup code:
`deregister_tm_clones` (`0x2b130`), `register_tm_clones` (`0x2b160`),
`__do_global_dtors_aux` (`0x2b1a0`) and `frame_dummy` (`0x2b200`). The runtime's
`.comment` contains Alpine GCC 14.2.0 and clang 19.1.4 strings. This is evidence
of an additional CRT input, not proof of the exact selected object or patch set.
The unmodified upstream [GCC 14.2.0 reference source](https://github.com/gcc-mirror/gcc/blob/releases/gcc-14.2.0/libgcc/crtstuff.c),
its copyright, GPLv3 text and [Runtime Library Exception 3.1](https://github.com/gcc-mirror/gcc/blob/releases/gcc-14.2.0/COPYING.RUNTIME)
are now included conservatively. The exception permits certain Target Code
combinations with Independent Modules under their own terms when its Eligible
Compilation Process conditions hold. It does not automatically relicense
CodeConvoy, nor establish that these exact shipped objects meet those conditions.
The reference files are not a complete GCC source or relinking kit.

**Still blocking:** base versions do not establish exact Alpine APK revisions,
downstream patches, selected CRT objects or the original image digest. The
[Docker recipe](https://github.com/AppImage/type2-runtime/blob/dd6cebedcbddde9c82f89b011e8e1d40b6e43868/scripts/docker/Dockerfile)
uses mutable `alpine:3.21` and unpinned packages; original build logs have expired.
Do not infer the selected revisions from today's APK index or build date.
`native_provenance` records this limit, and the `appimage-versions` gate remains.

## 2. LGPL relinking: unresolved

The actual LGPL-covered combination is the standalone launcher statically linked
with patched libfuse, separate from the CodeConvoy executable in SquashFS.
[libfuse's license](https://github.com/libfuse/libfuse/blob/fuse-3.15.0/LICENSE)
and [LGPL-2.1 section 6](https://github.com/libfuse/libfuse/blob/fuse-3.15.0/LGPL2.txt)
require the applicable library source/changes and a usable way to modify and
relink this combination. Section 6(a) permits the work using the library in
source and/or object form; it does **not** invariably demand proprietary
application object files or relicensing CodeConvoy. Complete source can suffice.

The bundle contains libfuse 3.15.0, the launcher's exact source/Makefile and
`mount.c.diff`, plus the recovered libraries' upstream sources. The prominent
LGPL notice, full terms and permission to modify/reverse-engineer for debugging
are explicit. `SOURCE-README.txt` identifies the rebuild/relink procedure and
remaining prerequisites. No three-year written offer is made, and static
linking does not qualify as section 6(b)'s shared-library mechanism.

**Still blocking:** exact downstream patches/inputs and a demonstrated relink
with modified libfuse. A successful build against newly resolved Alpine packages
would not by itself establish corresponding source for the shipped runtime.
No successful independent relink is claimed. Recover the original build inputs
or obtain a complete upstream corresponding-source/relink kit, then verify the
modified runtime with the same extracted AppDir. A replacement runtime would
need a separate review of its provenance and behavior; none is substituted here.

## 3. dispatch 0.2.0: unresolved

The exact crate and source revision
[`82d6c7a5b75dc0c71c3f46f87bb6c16a476f7748`](https://github.com/SSheldon/rust-dispatch/tree/82d6c7a5b75dc0c71c3f46f87bb6c16a476f7748)
declare MIT and identify Steven Sheldon. Neither contains a license text.
The upstream [missing-license issue #18](https://github.com/SSheldon/rust-dispatch/issues/18)
has **zero comments** and was closed by its reporter, Jake-Shadle, on 2024-05-16.
Its closure supplies neither a maintainer grant nor a missing copyright notice.
The repository history/root was also checked; no authoritative replacement text
was recovered. Existing author/license metadata remains in the inventory.

**Still blocking:** an authoritative notice from the rights holder or a supported
license determination for the actual code. Do not copy a different project's MIT
copyright, invent a year, or treat closing an issue as granting permission.
No upstream message has been sent and no dependency was replaced.

## 4. Installer plugins: partially resolved

### ApplicationID

The final installer DLL remains pinned to SHA-256
`f6851dcbf0a39edecd8a46564bc455e5273736c3dbcb02b954c201c79ccdf117`.
The [NSIS wiki](https://nsis.sourceforge.io/ApplicationID_plug-in) links the
Connectify fork and the original author's source. The original source URL now
returns a small HTML landing page, not a source archive. The updated archive
contains DLL/PDB files, without license material.

The exact [fork source](https://github.com/connectiblutz/NSIS-ApplicationID/blob/ad7e5084c69342d8f9fa7c66c6a135ca04e3c284/Set.cpp)
attributes original code to Mike Anchor (2009), Mozilla contributions and
Connectify changes. Those original copyright headers are now preserved in the
notices. They contain no clear redistribution license grant. The official NSIS
license for its own bundled plugins does not establish the license of this
separately downloaded plugin, and Mozilla authorship alone does not establish MPL.

**Still blocking:** source correspondence for the pinned DLL and an authoritative
license grant covering all contributions. Attribution alone is insufficient.

### nsis-tauri-utils 0.2.1

The actual DLL SHA-256 is
`0eed48313a7f904d7cc1977b70000ab3f11f18cadc8e6a69b807d288ca71f9db`;
its SHA-1 `53a7cfaeb6a4a9653d6d5fbff02a3c3b8720130a` matches the
[upstream release's published digest](https://github.com/tauri-apps/nsis-tauri-utils/releases/tag/nsis_tauri_utils-v0.2.1).
The source tag resolves to `c3a4447060a260c5e4e09d94284948c4f864da02`;
the release notes cite fix `92f92648b50fd298590570f43ed00de089609536`.
The exact tagged source and root MIT/Apache terms are included in the source ZIP.

Embedded registry paths identify 17 crate versions. Their exact crate archives
were inspected; their versions, SHA-256 values, license expressions and nested
notices are now separately inventoried. They include ureq 2.7.1, rustls 0.21.6,
ring 0.16.20, rustls-webpki 0.101.3 and semver 1.0.18. ring carries ISC, OpenSSL,
SSLeay and MIT terms, including required acknowledgements now reproduced in the
notice introduction. It must not be represented as simply MIT/Apache.

Embedded Rust source revision `eb26296b556cef10fb713a38f3d16b9886080f26` identifies
[Rust 1.71.1](https://github.com/rust-lang/rust/blob/eb26296b556cef10fb713a38f3d16b9886080f26/src/version).
The source's `.cargo/config.toml` enables static CRT for i686 MSVC. Its workflow
uses an unpinned Windows environment, and its tag contains **no Cargo.lock**.

**Still blocking:** strings are only a lower bound on included code. The original
resolved graph, remaining dependencies, old Rust standard-library/native runtime
notices and toolchain provenance remain incomplete. A current Cargo resolution
cannot establish what that old DLL contains. The `nsis-plugins` gate remains.

## 5. Windows static CRT: unresolved

The application uses `+crt-static`. The earlier successful candidate's actual
runner was `windows-2025-vs2026`, image `20260925.250.1`, as recorded by GitHub;
assuming Visual Studio 2022 because the OS label says Windows 2025 is incorrect.
The [image manifest](https://github.com/actions/runner-images/blob/win25-vs2026/20260925.250/images/windows/Windows2025-VS2026-Readme.md)
identifies installed software, not which library objects Rust's linker selected.

The applicable edition/component terms must be established. Microsoft's
[VS 2026 Enterprise/Professional terms](https://visualstudio.microsoft.com/license-terms/vs2026-ga-pro-enterprise/)
refer distribution to its [REDIST list](https://learn.microsoft.com/en-us/visualstudio/releases/2026/redistribution)
and impose conditions; the [V14 runtime EULA](https://visualstudio.microsoft.com/license-terms/vs2026-ga-visualcpp-v14-redist-runtime/)
is an end-user license and is not by itself a distributor's grant. The downloaded
Enterprise terms' original DOCX SHA-256 is recorded in the inventory. Microsoft's
[licensing guidance](https://www.microsoft.com/licensing/guidance/Visual-Studio)
discusses linked library code, but that general guidance alone does not identify
this build's libraries or establish this distributor's applicable rights.

**Still blocking:** record selected release CRT/UCRT/SDK library versions and
hashes from the actual link, establish the applicable hosted toolset/SDK grant
and distribution conditions, and separately cover the older prebuilt plugin
CRTs. Do not ship Microsoft source or proprietary object files on the assumption
that CodeConvoy's MIT license grants permission. No application license or
installer terms are changed to manufacture a compliance claim.

## Reproducible source ZIP and gates

The outer ZIP explicitly fixes ASCII name ordering, DOS timestamp, Unix creator,
regular-file permissions, ZIP version fields, flags, comments and extra fields.
Stored entries avoid platform/zlib compression differences. Input bytes are
read as bytes; filesystem modes and times never enter the ZIP. Duplicate and
case-colliding/nonportable names fail. A binary/CRLF fixture has a fixed SHA-256
on every test host; another test changes inventory order and filesystem metadata.
A final-payload test rejects a ZIP with identical extracted bytes but different
archive metadata. Existing final-package byte comparisons remain in place for
all six formats, as do checksum, provenance and native-payload gates.

Inventory checks now tie the native evidence to the pinned runtime/plugin hashes
and reject removal of recovered AppImage sources, CRT reference source/terms
or observed plugin notices.
All five blocker IDs remain enforced by `licenses.py release-ready` and the
publisher. There are no new package formats, channels or runtime substitutions.
