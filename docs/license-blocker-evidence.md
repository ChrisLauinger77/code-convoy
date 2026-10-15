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

### Capture and final-package verification

`packaging/build.py` retains `+crt-static`, the existing Rust version and release
flags. On Windows it selects the installed stable x64 MSVC developer environment
through `vswhere` / `vcvarsall`, then wraps only the application's final link with
`/VERBOSE:LIB`, full `/VERBOSE` detail and `/MAP`. It does not install or update
MSVC or the SDK. Microsoft's
[linker diagnostic documentation](https://learn.microsoft.com/en-us/cpp/build/reference/verbose-print-progress-messages?view=msvc-170)
describes library paths and selected member identities. Merely searching a
library or finding a symbol is not counted as loading an archive member. When
MSVC prints only a basename on `Loaded`, it must match the immediately active
absolute `Searching` path. Missing, mismatched or ended search context fails;
there is no fallback lookup through installed libraries.

The capture records the VS instance/edition, toolset, SDK, linker banner/hash,
Rust/Cargo versions, source revision, runner identity, linker arguments/response
files, a retained-symbol map, selected archive paths/hashes and member names. Archive hashing happens
inside the wrapper, before Rust removes temporary import libraries. These are
binary input identities, not recovered Microsoft source or a claim that every
selected member survives linker garbage collection. No Microsoft `.lib`/`.obj`
files are copied into the evidence or source bundle.

`provenance.json` binds the capture to the application SHA-256 and build inputs.
Missing, failed, stale or altered capture fails Windows packaging. The existing
final installer extraction and ZIP byte comparisons remain mandatory. Native
`dumpbin /headers /imports` inspection records each installer/helper separately;
`packages.json` binds both final package hashes to the application and inspection
evidence. An absent CRT DLL import does not establish which static CRT was used.
All reports deliberately retain `license_assessment: unresolved`.

The first-stage release workflow retains `windows-crt-evidence` separately from
`packages-windows`; aggregation still consumes exactly the existing six formats.
The existing CI workflow has an opt-in `windows_crt_audit` input, using the same
Rust/packager pins for a Windows packaging/install/uninstall diagnostic. This is
not a release candidate. Release candidates continue to be built from `main`
after merge. CI artifacts expire: retain the reviewed evidence with the release
records before expiry. The reviewed diagnostic below is also retained in the
repository, with its archive hash enforced by the existing catalog checker.

### Verified native diagnostic, 2026-10-15

[CI run 38785562437](https://github.com/ChrisLauinger77/code-convoy/actions/runs/38785562437)
built source `acb9ff6646d07b18e7ce75ae3435652db0d2180f`. This is the final
implementation revision; subsequent documentation/catalog evidence records do
not describe a rebuilt release candidate. The runner was
`win25-vs2026/20261007.264.1`, with these actual selected tools:

| Tool | Observed identity |
| --- | --- |
| Visual Studio | Enterprise, installation `18.10.12224.181`, non-prerelease |
| VC toolset | `14.51.36231` |
| MSVC linker | `14.51.36260.0` |
| Windows SDK | `10.0.26100.0` |
| Rust / Cargo | `1.99.0` / `1.99.0`; LLVM `23.1.1` |
| cargo-packager | Existing pinned `0.11.8` |

The SDK identity above is the selected directory version. Its library hashes
identify the exact audited bytes; the capture does not infer an SDK installer
servicing revision from that directory name.

The trace records 17 archives with loaded members. Its three selected CRT
archives are below. Counts are distinct member **names**, not a claim that every
loaded object survives optimization. The map independently contains retained
symbols from all three libraries.

| Selected archive | Distinct loaded member names | SHA-256 |
| --- | ---: | --- |
| `libcmt.lib` | 39 | `d4cc4c224455825dec1669f9ad6a9872a4f5b37ed1daae1fd0b9044d7650d908` |
| `libvcruntime.lib` | 26 | `f0a898588ecb26fe4c7530b2495a6fe5e768856309a33391702a3cbcb72b0589` |
| `libucrt.lib` | 243 | `ea2ffa5d9b1ebc3fa76f34c364471750db8951b341dbccbc1bef628ef1a20e39` |

The first two are under
`C:\Program Files\Microsoft Visual Studio\18\Enterprise\VC\Tools\MSVC\14.51.36231\lib\x64`;
the third is under
`C:\Program Files (x86)\Windows Kits\10\lib\10.0.26100.0\ucrt\x64`.
Examples of selected members include `exe_main.obj` / `chkstk.obj`,
`memcpy.obj` / `frame.obj`, and `strnlen.obj` / `round.obj`, respectively.
The remaining Loaded records identify SDK `kernel32.lib`, `ntdll.lib`,
`user32.lib`, the `windows_x86_64_msvc` import library, Rust compiler-builtins,
and generated import archives. Direct object/rlib inputs are additionally
recorded in the original response file; the 17 archives are not a count of all
Rust dependencies.

[The retained evidence archive](../third-party/evidence/windows-crt-38785562437.zip)
contains the complete trace, symbol map, original response file, selected
archive/member identities, toolchain/provenance JSON, eight PE inspections and
final package manifest. All 17 entries preserve the downloaded bytes; the
archive contains no application binaries or Microsoft `.lib`/`.obj` files.
It is repository audit evidence, not a new release format or an addition to the
end-user source bundle. Its SHA-256 is
`e2224cd608658761f567b73d1d6b0f40bce971abec951e6ce678faf575011031`.

| Final diagnostic artifact | SHA-256 |
| --- | --- |
| `CodeConvoy.exe` in both packages | `a44e9de355a0701eeb812563fa6a6f50aba5796a4bea713c78a97a0c457da15b` |
| `CodeConvoy-1.0.0-windows-x86_64-setup.exe` | `c91332c8622bf892feba8d0838861fc80e4cf07aec1954731d226fce5dd3a831` |
| `CodeConvoy-1.0.0-windows-x86_64.zip` | `a969c1cb11f9a0021c0bfc3fa1ff1744a43c77d2142098aac35956d4f0462fe1` |

Independent local extraction verified both downloaded final packages against
that exact source revision: application bytes, all notice/source material,
pinned plugins, every PE hash and raw inspection hash, and the provenance chain
passed. The reference checkout reproduced the native CRLF bytes recorded for
the four build inputs and project `LICENSE`; no production check was relaxed
to accommodate Linux line endings.

### Prebuilt helper scope

These helpers are not rebuilt by the application link. The pinned DLLs already
in the audit have no VC/UCRT DLL imports. Their PE linker fields are 14.0 for
ApplicationID, 14.36 for nsis-tauri-utils and 6.0 for StartMenu/System/nsDialogs.
These fields do not establish a complete compiler version or static-library
identity. The fresh native audit independently records their hashes, PE headers
and imports; plugin hashes must still match the existing inventory.

The already bundled `nsis-3.09-src.tar.bz2` supplies additional CRT-specific
context: `SCons/Config/ms:152` disables default libraries for the stub, and
`SConstruct:707` defaults plugin builds to `nodeflib=True`. StartMenu and
nsDialogs retain that default; `Contrib/System/SConscript` explicitly sets it
false. The GNU stub configuration likewise uses `-nostdlib`. This is evidence
against assuming that every NSIS binary necessarily embeds Microsoft's CRT,
but it is not an exact-binary build trace. The build-only `makensis` compiler's
runtime settings must not be confused with those of the redistributed stub.
No NSIS source, plugin, license or tool version is changed here.

The final native inspection covered the setup stub, application, uninstaller
and all five extracted plugins. The ApplicationID and nsis-tauri-utils hashes
match the existing inventory; none of the eight inspected PEs imports a VC/UCRT
runtime DLL. The full per-binary imports and hashes are preserved in
`packaged-binaries.json`. This does not settle the helpers' static runtime
provenance, including the System plugin whose source permits default libraries.

### Redistribution assessment

Confirmed reference terms and guidance:

- Microsoft [CRT documentation](https://learn.microsoft.com/en-us/cpp/c-runtime-library/crt-library-features?view=msvc-170)
  distinguishes SDK `libucrt.lib`, VC `libvcruntime.lib` and static startup
  `libcmt.lib`. Debug variants are not redistributable. Even dynamic CRT builds
  retain statically linked startup code; changing linking is not a license fix.
- Microsoft [licensing guidance](https://www.microsoft.com/licensing/guidance/Visual-Studio)
  permits distribution of linked output for `.lib` code identified as
  distributable, rather than redistribution of the library itself. This supports
  retaining static linking, conditional on identifying the applicable grant.
- [VS 2026 Enterprise/Professional terms](https://visualstudio.microsoft.com/license-terms/vs2026-ga-pro-enterprise/),
  under “Distributable Code”, refer to the
  [REDIST list](https://learn.microsoft.com/en-us/visualstudio/releases/2026/redistribution).
  They require significant application functionality, protective recipient and
  distributor terms, and indemnification subject to the stated exception. They
  restrict unreleased components, misleading trademark use and imposing an
  excluded source license on Microsoft code. Trial/Insiders restrictions and
  separate component terms also matter. The original downloaded terms' SHA-256
  remains recorded in the catalog.
- The official [SDK 26100 package](https://www.nuget.org/packages/Microsoft.Windows.SDK.CPP/10.0.26100.1)
  links to [Microsoft's SDK license](https://aka.ms/WinSDKLicenseURL), still titled
  Windows 10 SDK. The retrieved RTF SHA-256 is
  `dd07eb178e00c6bba4148457fc00ff77cd4887eb521d504186fe59c9ec8bbe62`.
  Its distributable-code terms require linked `.lib` output, preserved notices,
  an application copyright notice, protective recipient terms and distribution
  on Microsoft operating systems, as well as significant application
  functionality and indemnification; other terms accompanying components control.
  Crucially, the referenced [SDK REDIST list](https://learn.microsoft.com/en-us/legal/windows-sdk/redist#static-lib-files)
  explicitly covers static `.lib` files in publicly released SDK `um` and
  `ucrt` directories, including x64. This establishes published static-library
  coverage for those SDK paths, subject to the conditions; it does not cover
  the separate VC toolset directory or prove fulfilment of recipient terms.
- The [redistribution guidance](https://learn.microsoft.com/en-us/cpp/windows/redistributing-visual-cpp-files)
  directs distributors to the applicable license. The
  [V14 runtime EULA](https://visualstudio.microsoft.com/license-terms/vs2026-ga-visualcpp-v14-redist-runtime/)
  is not itself proof of a distributor's grant. An installed Enterprise instance
  does not prove a trial restriction applies, nor establish the maintainer's
  entitlement or the hosted service's applicable agreement.

The current packages preserve the project's copyright/license file and existing
third-party notices. This investigation has not established that shipping the
complete VS or SDK agreement is mandatory, or that doing so would satisfy the
recipient-agreement requirement. The needed recipient wording and notice
placement must be assessed under the confirmed applicable terms; no Microsoft
runtime code is relabeled as MIT.

**Still needed before resolving this blocker:**

1. Confirm the applicable grant for the selected VC toolset libraries and the
   licensing basis for hosted builds. SDK static-library path coverage is now
   documented, but confirm that no differing in-installation component terms
   override the public SDK terms. The VS DLL REDIST list alone does not settle
   static-library coverage for `libcmt.lib` / `libvcruntime.lib`.
2. Have the maintainer confirm the applicable recipient/distributor conditions
   and how both NSIS and portable ZIP distribution satisfy them. Determine any
   required notice/terms text from that confirmed basis. A notice file alone
   cannot be assumed to satisfy an agreement requirement. No new EULA, assent
   screen or restriction on CodeConvoy's MIT source is invented here.
3. Establish CRT provenance and redistribution coverage for prebuilt installer
   binaries separately. In particular, the pinned `nsis-tauri-utils` DLL was
   built with Rust 1.71.1 and static CRT settings; the application's new trace
   does not cover it. This overlaps the existing plugin blocker; no plugin is
   replaced or otherwise re-audited in this task.

These are unresolved applicability/fulfilment questions, not a finding that
static linking or MIT-licensed CodeConvoy source is prohibited. The Windows CRT
publication guard stays active. The other four publication blockers are unchanged.

### Repeat after a runner or toolchain update

Run the opt-in Windows CI diagnostic, then compare actual selected archive
versions/hashes/member lists, linker/SDK identities and helper hashes/imports
with the reviewed record. Recheck grants/conditions for changed inputs, including
security servicing of a statically linked runtime. After merge, capture and
retain new evidence from the ordinary `main` candidate and verify final package
hashes again. A green capture is technical evidence only, never automatic legal
approval. Keep the existing toolchain pins and publication guards in place.

### Validation for the CRT capture change

Local `cargo fmt --check`, strict all-target/all-feature Clippy, all-feature
Rust tests (602 passed, 10 ignored), and `cargo build --release` passed.
All 78 packaging tests pass, including malformed/stale capture, temporary
archive lifetime, active search-path binding, final-package hash binding and
inspection failure propagation. The license inventory check passes with 457
locked crates and five unresolved obligations. `release-ready` still rejects
all five blocker IDs, including `windows-crt`.

The native diagnostic passed release compilation, linker capture, final NSIS
and ZIP inspection, and the existing Windows installation smoke test:
executable metadata, per-user registration, Start Menu shortcut, identical
installed/portable executables, installed notice/source bytes, uninstall and
user-data preservation. Independent downloaded-artifact verification also
passed as described above. These checks do not establish interactive desktop,
file-dialog or desktop cancellation behavior.

The linked CI run passed on Linux, macOS and Windows. Windows also passed the
ignored software-adapter/egui startup check; this is not an interactive desktop
test. The Windows release build ran inside the audited packaging step, so CI
skipped only its redundant final release-build invocation. No first-stage
release candidate was dispatched from this branch. After merge,
the usual `main` candidate must carry its own fresh provenance and final
package hashes; this diagnostic is not a substitute for that release record.

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
