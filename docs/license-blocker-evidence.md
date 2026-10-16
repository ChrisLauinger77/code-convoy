# PR 68 — native license evidence follow-up

Audit date: 2026-10-15; dispatch follow-up: 2026-10-16.
**Historical follow-up:** the AppImage sections below describe the original
upstream binary. The replacement runtime, relinking demonstration and current
disposition are recorded in [AppImage runtime validation](appimage-runtime-validation.md).
The NSIS plugin section is also historical: the replacement and its source,
license and binary evidence are in [NSIS helper validation](nsis-helper-validation.md).
The Windows CRT obligation remains unresolved; the dispatch resolution is unchanged.
The current risk classification is in [section 9](#9-final-risk-based-windows-review).
The [maintainer release decision and licensing assessment](windows-licensing-decision.md)
records the decision to proceed, the unchanged publication blocker, the precise
grant mapping and draft recipient terms for its two open questions.
It supersedes the broader Windows closure requirements in sections 5–8;
their binary evidence and historical conclusions remain recorded.
Publication remains blocked. This record distinguishes
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

## 3. dispatch 0.2.0: resolved under the documented interpretation

This 2026-10-16 follow-up examines only `dispatch`; it does not repeat the full
audit or change the other four blockers. The proposed notice and inventory
change is ready for maintainer review. It is **not authoritative legal approval**.

### Technical evidence

- The original [published crate](https://crates.io/api/v1/crates/dispatch/0.2.0/download)
  has SHA-256
  `bd0c93bb4b0c6d9b77f4435b0ae98c24d17f1c45b2ff844c6151a07256ca923b`,
  matching `Cargo.lock` and the catalog. Both its normalized `Cargo.toml` and
  original `Cargo.toml.orig` explicitly declare `license = "MIT"` and
  `authors = ["Steven Sheldon"]`; neither specifies `license-file`.
  Its `.cargo_vcs_info.json` pins
  `82d6c7a5b75dc0c71c3f46f87bb6c16a476f7748`.
- The [crates.io version metadata](https://crates.io/api/v1/crates/dispatch/0.2.0)
  independently reports MIT, the same checksum, publication on 2020-01-19 and
  publisher `SSheldon` (Steven Sheldon); the version is not yanked. This
  corroborates the publisher's declaration, not an independent legal review.
- The exact [upstream manifest](https://github.com/SSheldon/rust-dispatch/blob/82d6c7a5b75dc0c71c3f46f87bb6c16a476f7748/Cargo.toml)
  is byte-identical to `Cargo.toml.orig`. All six `src/*.rs` files,
  `examples/main.rs` and `README.md` in the crate are byte-identical to that
  revision. Inspection of every file in the crate and pinned source, including
  source headers and the excluded iOS test/build scripts, found no copyright
  notice, license text, conflicting license or extra attribution requirement.
- The [upstream tag list](https://api.github.com/repos/SSheldon/rust-dispatch/tags?per_page=100)
  contains `0.2.0`, `0.1.4`, `0.1.3`, `0.1.2`, `0.1.1`, `0.1.0`, `0.0.3`,
  `0.0.2` and `0.0.1`. Tag `0.2.0` points to the exact pinned revision; no
  separate GitHub release is listed by the
  [releases API](https://api.github.com/repos/SSheldon/rust-dispatch/releases?per_page=100).
  The [repository metadata](https://api.github.com/repos/SSheldon/rust-dispatch)
  has `license: null`, so GitHub's license detection supplies no additional
  evidence. It does not negate the explicit upstream/package MIT declaration.
- The [missing-license issue #18](https://github.com/SSheldon/rust-dispatch/issues/18)
  still has zero comments and was closed by its reporter, Jake-Shadle, on
  2024-05-16. Its closure supplies neither a new grant nor a copyright notice.
  No upstream message has been sent.

### Notice obligations and interpretation

Cargo's [manifest specification](https://doc.rust-lang.org/cargo/reference/manifest.html#the-license-and-license-file-fields)
defines `license` as the license under which the package is released, using
SPDX identifiers; `license-file` is a separate way to supply terms. Consequently
the MIT field is affirmative publisher-supplied licensing evidence, rather than
an inferred license from a similar project. The [standard MIT terms](https://opensource.org/license/mit)
permit use, copying, modification, merging, publication, distribution,
sublicensing and sale subject to preserving the copyright and permission
notices in copies or substantial portions. The complete permission, condition
and warranty/liability disclaimer should accompany redistribution. MIT does
not prescribe a file named `LICENSE` or require source delivery.

The interpretation used for this change is that the explicit, unqualified MIT
designation identifies those standard terms by reference. Preserve every notice
actually supplied, retain the published author attribution and reproduce the
identified terms. No copyright statement was found to copy; the authors field
is not proof of sole copyright ownership and must not be converted into a
fabricated copyright line or year. Reproducing the standard terms is notice
delivery based on upstream's declaration, not CodeConvoy issuing a new grant.

On that interpretation, the evidence is sufficient to retain and redistribute
the unchanged crate with the supplemented notices. The earlier demand for a
separate rights-holder-issued notice was a conservative internal evidence
requirement, not a demonstrated lack of permission or proof of noncompliance
from the missing file alone. There was, however, a concrete delivery gap: the
old `dispatch` catalog entry had **no referenced notice text**. Generated notices
included only its name/version, MIT label, author, targets and unresolved marker;
other crates' MIT texts were not a substitute for an explicit dispatch notice.

This remains a documented compliance interpretation, not a legal opinion or a
guarantee of title to every contribution. A completed upstream copyright notice
and separate maintainer confirmation remain unavailable. If the maintainer's
policy requires either, seek upstream confirmation or qualified legal review
before accepting this resolution; do not invent the missing statement. No
evidence of a contrary grant or omitted existing notice was found in the
inspected release material.

### Distribution change and maintenance

The `dispatch` entry now references a content-addressed notice in `texts.json`.
It retains the original manifest verbatim, the published author attribution,
crate checksum and pinned revision, followed by the standard MIT permission,
condition and disclaimer. The latter are reproduced from
[SPDX license-list-data v3.26.0, text/MIT.txt](https://raw.githubusercontent.com/spdx/license-list-data/v3.26.0/text/MIT.txt)
(reference SHA-256
`6d7c9e9d7d12c8d3753bdb24417b84819ba4d6c2f025c37948113d86dc81eef9`).
Only its unfilled template copyright line and generic heading are replaced by
an explanation and dispatch-specific heading; the terms themselves are
unchanged. The notice explicitly distinguishes reproduction from an upstream
license file. No copyright owner or year is invented.

The existing bundle generator delivers the notice and its provenance through
`THIRD-PARTY-NOTICES.txt` and `THIRD-PARTY-INVENTORY.json` to all package formats.
Only `dispatch-notice` is removed from the publication blockers and the crate's
unresolved marker; AppImage provenance, LGPL relinking, NSIS plugins and Windows
CRT entries retain their exact content and enforcement. Overall publication is
still blocked. CodeConvoy's MIT license and the two-stage workflow are unchanged.

No dependency or application/packaging code changes are necessary. The
conditional `objc2-foundation`/`dispatch2` patch is therefore not pursued;
main-queue, return-value, panic, event-loop and accessibility behavior are
unchanged. Future version/checksum/source changes must re-evaluate this
determination through the existing locked-inventory checks. Preserve any
authoritative upstream notices subsequently recovered, and keep this
publisher-declaration interpretation specific to the reviewed release.

### Validation

Local validation on macOS arm64 with Rust/Cargo 1.99.0:

| Check | Result |
| --- | --- |
| `python3 packaging/licenses.py check` | Passed: 457 locked crates, 362 notice texts, four remaining release obligations. |
| `python3 packaging/licenses.py prepare` and `python3 packaging/licenses.py verify target/packaging/licenses` | Passed for the generated bundle directory; not a new final-package inspection. |
| Scoped bundle/evidence comparison against `HEAD` | Passed: dispatch attribution, original manifest, provenance and exact reproduced MIT terms reach the generated notices/inventory; no invented copyright line. All other crates, pre-existing notice texts, components, sources and four blocker details are unchanged. |
| `python3 packaging/licenses.py release-ready` | Expected refusal (exit 1) listing exactly `appimage-versions`, `appimage-relink`, `nsis-plugins`, `windows-crt`. |
| `python3 -m unittest discover -s packaging -p 'test_*.py'` | Passed: 78 tests run, 77 passed, one Linux-only native ELF/DEB fixture skipped. |
| `cargo fmt --check` | Passed. |
| `cargo clippy --all-targets --all-features -- -D warnings` | Passed. |
| `cargo test --all-features` | Passed: 625 passed, nine ignored. The initial sandbox run could not bind the existing temporary Unix-socket fixture (`Operation not permitted`); the full rerun outside that sandbox passed. Counts exclude nested subprocess fixture summaries. |
| `cargo build --release` | Passed for the native arm64 target. |
| `git diff --check` | Passed. |

No new candidate or final six-format package extraction was performed. No native
interactive macOS, event-loop, accessibility or x86-64 compilation/runtime check
is claimed; no executable behavior or dependency changed. Packaging fixture
tests are not evidence of interactive platform behavior. The earlier candidate
records retain their original scope. Stop here for maintainer approval; the
other licensing tasks remain outside this change.

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

## 6. Windows CRT redistribution assessment after PR 73

Assessment date: **2026-10-16**, after PR 73 merged as
`d96c94e789afae9611df9229d0c22e4b83cd2b6b`. **Keep `windows-crt` active.**
The public terms establish conditional SDK coverage, but do not establish the
complete grant and recipient arrangements for this distribution. This is an
evidence gap, not a determination that static linking is prohibited. The four
other licensing decisions remain resolved. No application, linking, installer,
notice-bundle, inventory or publication implementation is changed by this
assessment.

### Authoritative terms and the remaining decision

The official VS and SDK downloads were retrieved again; their SHA-256 values
match the previously recorded documents. No toolchain discovery or dependency
audit was repeated.

| Reference | Finding and limit |
| --- | --- |
| [VS 2026 Enterprise/Professional terms](https://visualstudio.microsoft.com/license-terms/vs2026-ga-pro-enterprise/), updated October 1, 2025, sections II.6 and II.7 | The distribution grant depends on the Distributable List. Component-specific terms can control. Full-use entitlement and the relevant licensing channel must be established; the installed edition does not establish either. |
| [VS 2026 Distributable List](https://learn.microsoft.com/en-us/visualstudio/releases/2026/redistribution) and [Microsoft licensing guidance](https://www.microsoft.com/licensing/guidance/Visual-Studio) | The list names runtime merge modules and `VC\redist`, but does not expressly identify the observed `VC\Tools\MSVC\14.51.36231\lib` static libraries. Guidance allows linked output from **identified distributable** `.lib` code; it does not itself identify these two libraries. Whether the listed runtime code also grants these static forms needs confirmation. |
| [Windows SDK license](https://aka.ms/WinSDKLicenseURL), section 2(a), and [SDK REDIST static LIB list](https://learn.microsoft.com/en-us/legal/windows-sdk/redist#static-lib-files) | Explicit conditional coverage of linked output from public SDK `um` and `ucrt` paths includes the observed x86 and x64 inputs. Confirm that the installed components carry no overriding terms. This list does not cover the separate VC toolset paths. |
| [Visual C++ V14 Redistributable and Runtime terms](https://visualstudio.microsoft.com/license-terms/vs2026-ga-visualcpp-v14-redist-runtime/), updated October 1, 2025 | These end-user terms allow installation/use but expressly restrict redistribution of the runtime/code. They are **not** a substitute distributor grant. This does not negate a grant under an applicable development-tool agreement. |
| [GitHub Additional Products and Features terms, Actions](https://docs.github.com/en/site-policy/github-terms/github-terms-for-additional-products-and-features#actions), effective August 27, 2026 | They authorize development/testing use of Actions, together with the customer's main agreement. The reviewed text does not expressly establish the customer's VS Enterprise static-runtime redistribution entitlement. Its authorized-developer provision concerns Apple software; it cannot be extended to Microsoft code. |

The [VS license directory](https://visualstudio.microsoft.com/license-terms/)
also distinguishes subscription Product Terms from the published EULAs. Neither
a subscription agreement nor a Microsoft/GitHub confirmation covering this
distributor is in the project evidence. The
[hosted-runner documentation](https://docs.github.com/en/actions/reference/runners/github-hosted-runners)
documents supplied tools, not the missing redistribution grant. Public availability,
runner activation, an Enterprise label, and absence of CRT DLL imports are not
substitutes. No conclusion that the runner is an unlicensed trial is drawn.
Further searches for the same public lists will not resolve this contractual gap.

| Downloaded reference document | SHA-256 |
| --- | --- |
| `Visual_Studio_2026-License-Enterprise_Professional_ENU.docx` | `dd2ab92a7c2bb90dd509e16572707a34b13bc0bc09b9199d115f25594ac39053` |
| `sdk_license.rtf` | `dd07eb178e00c6bba4148457fc00ff77cd4887eb521d504186fe59c9ec8bbe62` |
| `Visual-C-V14-License-Redistributable_and_Runtime_ENU.docx` | `08651651a7602fc7c0e2763de0fde1ff9f868df2780597cd1775ee9d6441c783` |

### Conditions versus the existing packages

The VS distribution requirements include substantial application functionality,
protective distributor/end-user agreements and indemnification (with its stated
exception); restrictions include unreleased components, misleading Microsoft
branding and imposing an Excluded License on Microsoft source. The SDK also
requires linked output rather than library delivery, preserved notices, an
application copyright notice and Microsoft-platform distribution; its
indemnification wording differs. These are conditional obligations under the
respective agreements, not a proposed new license for CodeConvoy.

CodeConvoy supplies substantial independent functionality. The recorded builds
use release CRT inputs; the Windows formats contain executables rather than
Microsoft `.lib`/`.obj` files and target Windows. Existing project copyright,
third-party notices and the distinction between MIT source and Microsoft code
are present. These observations support some conditions; they do not establish
all contractual obligations or completeness of Microsoft-specific notices.

The installer currently displays the project `LICENSE` through `licenseFile`
and `MUI_PAGE_LICENSE`; the ZIP supplies `LICENSE.txt` and the four existing
third-party files. Neither establishes agreement to protective Microsoft-code
terms. In particular, silent installation and unpacking a portable ZIP cannot
be assumed to supply assent. No particular click-through design is mandated by
this assessment. A bare attribution, a copied Microsoft EULA, or MIT's warranty
disclaimer cannot be declared sufficient. Recipient/distributor wording and
the mechanism for both formats require approval under the confirmed grant;
indemnification is a distributor obligation, not something a packaging test
can discharge. Consequently no speculative EULA or notice text was added.

### Application and installer coverage

The recorded selected paths, not inferred libraries, determine this scope.
The application evidence in section 5 and the independent
[PR 73 helper evidence](nsis-helper-validation.md#validation-and-remaining-checks)
use VS Enterprise **18.10.12224.181**, VC **14.51.36231**, LINK
**14.51.36260.0**, SDK **10.0.26100.0**, and Rust **1.99.0**.

| Distributed code | Actual runtime evidence and assessment |
| --- | --- |
| `CodeConvoy.exe`, identical in setup and portable ZIP | Captured x64 `libcmt.lib` and `libvcruntime.lib` under the VS toolset, and `libucrt.lib` under SDK `ucrt\x64`; exact paths/hashes in section 5. VC grant and recipient conditions remain open. |
| `codeconvoy_utils.dll`, installer only | Separate PR 73 trace selects the same three release library names for **x86**. This is covered by the same unresolved VC/SDK decision, not by the helper's MIT/Apache source grants. |
| `CodeConvoyAppId.dll`, installer only | `/NODEFAULTLIB /NOENTRY`; selected SDK `um\x86\kernel32.lib`, `ole32.lib`, and `uuid.lib`. No CRT selected. SDK support-code conditions still apply. |
| NSIS setup stub, generated uninstaller, `StartMenu.dll`, `nsDialogs.dll`, `System.dll` | Existing exact binary hashes/import inspections and NSIS source/license decision are retained. No VC/UCRT DLL imports were observed. Original build inputs are not established by those imports; section 5 records the source's default-library settings and the System exception. No new claim of CRT absence or Microsoft permission is made for these prebuilt components. |
| Portable ZIP | Only the application is executable; no installer helpers. Existing byte/notice/source checks cover its six root entries. |

The retained `nsis-build-evidence.zip` gives the Rust helper's exact x86 inputs:

| Directory under the recorded installation | Library | SHA-256 |
| --- | --- | --- |
| `VC\Tools\MSVC\14.51.36231\lib\x86` | `libcmt.lib` | `e5350669a2e6611019dcb0ee37fb577e8783d808e4d15317972d8a2e9fb76ed5` |
| Same VC directory | `libvcruntime.lib` | `699f86b2144f39962eec5c39a29fd5049a94e8552a44538f101fa5dccbb43217` |
| `Windows Kits\10\lib\10.0.26100.0\ucrt\x86` | `libucrt.lib` | `5a17862298fc04332aae61315014d838f5a7c6a05c9e643666939d91b533f0cb` |

### Exact decision needed to close the blocker

Obtain Microsoft/GitHub licensing confirmation or a supported legal review of
the applicable agreement that:

1. Identifies the grant for linked `libcmt`/`libvcruntime` output from these
   x64/x86 VC toolset paths and establishes this distributor's entitlement for
   the hosted builds, including any controlling component terms.
2. Specifies the required Microsoft-code notices, recipient/distributor terms
   and a sufficient delivery/agreement method for both installer and portable
   ZIP, with the distributor's obligations accepted separately.
3. Determines whether the remaining prebuilt NSIS components need further
   Microsoft-runtime provenance or permission evidence. Their existing NSIS
   source/license resolution is not reopened or treated as a Microsoft grant.

This is the stop point for the licensing assessment. Rebuilding, changing
static linking, adding an acknowledgment file, or clearing an inventory flag
would not supply these determinations. `licenses.py release-ready` must continue
to reject **only `windows-crt`**; the publisher must continue to call that gate
before creating a release.

### Validation of this assessment

Local `cargo fmt --check`, strict all-target/all-feature Clippy,
`cargo test --all-features` (**621 passed, 10 ignored**), `cargo build --release`,
all **101 packaging tests**, and `licenses.py check` passed. The first Rust run
encountered the already documented read-only `/tmp/.git` environment marker;
the complete rerun with `TMPDIR=/var/tmp` passed without code changes.
`licenses.py release-ready` rejected exactly `windows-crt`, as intended.
The existing publisher regression verifies rejection before even draft release
creation. Neither the live publisher nor a release workflow was invoked.

[Non-publishing CI diagnostic 38906991244](https://github.com/ChrisLauinger77/code-convoy/actions/runs/38906991244)
was dispatched against merged `main` at
`d96c94e789afae9611df9229d0c22e4b83cd2b6b`, with `windows_crt_audit=true`.
All three CI jobs passed, including the Linux private-D-Bus and Windows
software-graphics startup checks. The Windows packaging step passed default and
Unicode-path installation/removal and native helper tests. Its audited release
build ran in that step; only the redundant final release-build command was
skipped. The separate helpers-only job was not requested; the packaging step
built and checked both helpers. The two local documentation edits do not change
the build inputs, inventory or bundled notices; this diagnostic is not a
publication candidate.

Downloaded `windows-crt-audit` artifact **12049765094** identifies that exact
source and run. Independent local extraction passed the existing installer/ZIP
byte checks, source/notice delivery, all eight PE hashes and inspection hashes,
both helpers' two-build evidence, application link capture and final package
manifest. Only checkout line endings matching the captured input hashes were
reconstructed in a disposable source snapshot; production checks were not
relaxed. The actual VS/toolset/SDK/linker identities and the application's three
CRT paths/hashes match section 5 exactly. Both helper binaries match PR 73, and
the source-bundle hash is unchanged. This reuses the existing provenance
machinery and reviewed baseline rather than introducing another audit pipeline.

| Verified diagnostic output | SHA-256 |
| --- | --- |
| `CodeConvoy-1.0.0-windows-x86_64-setup.exe` | `212cbb78467273a3b8025b6f254cd66b11962d15e562443bcffae03c59a2996f` |
| `CodeConvoy-1.0.0-windows-x86_64.zip` | `55cc9ab47675706c1676d1b93c993665456e6402dd3622cbfe8aa3cec90887cc` |
| `CodeConvoy.exe` in both packages | `819b59fd1a65b3f4daf783d48779cf9d22c2e1f0b16a012a807128d69b0c7364` |
| `THIRD-PARTY-SOURCES.zip` in both packages | `b253fcf0f891ff1bb20b1240b72e80c8a09a3ec760bc1d11c9ea9d4f9c603f31` |

The installer behavior and limitations remain those documented in
[NSIS helper validation](nsis-helper-validation.md#validation-and-remaining-checks):
no interactive desktop certification, real historical-installer upgrade claim,
or uninstaller final self-deletion check is inferred. The technical checks do
not alter the licensing outcome above.

## Historical PR 68 record: reproducible source ZIP and gates

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
At that earlier revision, all five blocker IDs were enforced by
`licenses.py release-ready` and the publisher. Section 6 records the current
single-blocker state. That change introduced no new package formats, channels
or runtime substitutions.

## 7. Dynamic CRT and user-supplied prerequisites

The 1.0.0 distribution policy now uses dynamic CRT linkage for CodeConvoy.exe
(x64) and codeconvoy_utils.dll (x86). Sections 5–6 retain the preceding static
build evidence and legal assessment; they do not describe this new linkage.
The publication blocker **windows-crt remains active**.

The implementation supplies no separate Microsoft runtime DLL, redistributable
installer, downloader or additional portable launcher. Users install Microsoft's
v14 runtime independently from its
[official download page](https://learn.microsoft.com/en-us/cpp/windows/latest-supported-vc-redist).
The reviewed VCRUNTIME140.dll file-version floor is 14.51.36247.0 for both
architectures, established from the native system files that run the application
and helper built with MSVC 14.51.36231. The linker has a separate component
version, 14.51.36260.0; it is not the DLL file version or Redistributable package
version. This is a tested deployment floor, not a claim to have measured the
oldest DLL exporting each required symbol.
Windows supplies the UCRT; its checked floor is 10.0.10240.0, with Windows updates
required for supported Windows 10/11 systems. The existing product support matrix
is not expanded by this CRT change.

| Category | Current scope and obligation |
| --- | --- |
| User-supplied runtime DLLs | VCRUNTIME140.dll and Windows UCRT interfaces are external dependencies. CodeConvoy does not redistribute their DLLs or the Microsoft installer. Users obtain them independently; no Microsoft distributor grant is inferred from download availability. |
| Code embedded in CodeConvoy.exe and the Rust utility | Dynamic linkage still selects static startup/support objects from msvcrt.lib, alongside vcruntime.lib/ucrt.lib import libraries. The old libcmt/libvcruntime/libucrt implementation archives are rejected. The applicable VC object-code grant, SDK/component terms, notices and recipient/distributor obligations still need a qualified determination. |
| CodeConvoyAppId.dll | Unchanged CRT-independent helper; kernel32/ole32 SDK imports and UUID data remain. SDK conditions are not removed by the Rust linkage change. |
| Other NSIS binaries | Setup stub, uninstaller, StartMenu.dll, nsDialogs.dll and System.dll remain the recorded prebuilt components. Their absence of VC/UCRT DLL imports does not establish the absence, identities or licensing of embedded Microsoft code. No rebuild or renewed NSIS source-license audit was performed. |
| Build environment | Hosted VS Enterprise/MSVC/SDK entitlement remains a separate question. Dynamic linkage, a working compiler and a successful CI run do not establish this distributor's contractual grant. |

Microsoft explicitly identifies msvcrt.lib as static startup code for the dynamic
CRT in its [CRT library documentation](https://learn.microsoft.com/en-us/cpp/c-runtime-library/crt-library-features).
The existing [VS terms](https://visualstudio.microsoft.com/license-terms/vs2026-ga-pro-enterprise/)
and [SDK REDIST list](https://learn.microsoft.com/en-us/legal/windows-sdk/redist#static-lib-files)
remain the references for embedded code. The Microsoft runtime end-user license
is not a replacement distributor grant. The precise recipient agreement and
notice mechanism for embedded code, including ZIP and silent installation, is
still unresolved; no speculative EULA or change to CodeConvoy's MIT license is
introduced.

The installer checks actual system DLL file versions and PE architecture, using
SysWOW64 for x86 and Sysnative for x64 from the 32-bit NSIS host. It also performs
a system-only x86 load check. Registry version claims are not used, so stale keys
cannot override missing/old files. The x86 host does not execute x64 DLLs as a
loadability probe. Failures identify the affected architecture, provide the
Microsoft link and stop with status 1603; silent/passive modes do not block on a
dialog. Checks precede helper use and repeat before maintenance/process calls.
Uninstall requires x86 only and never bypasses process detection. The portable
ZIP requires x64; missing imports can prevent startup before Rust main and the
application's own error dialog.

Exact import lists, retained build evidence, final hashes and native test results
are recorded with the [dynamic migration](nsis-helper-validation.md#dynamic-crt-migration-and-prerequisite-validation).
These are technical evidence, not permission to clear windows-crt or publish.

## 8. Proposed Microsoft Distributable Code notice

Prepared on 2026-10-16 against merged PR 74 (`ca5ade2`). This section adds a
notice-delivery proposal to section 7; it does not replace its binary evidence
or close the **windows-crt** blocker. The absence of a proposed notice described
in the earlier assessments is historical. No completed dependency or native
binary audit was repeated.

The original [Windows notice](../third-party/MICROSOFT-DISTRIBUTABLE-CODE.txt)
distinguishes CodeConvoy's MIT code, embedded Microsoft startup/SDK code,
separately installed runtime DLLs and the remaining prebuilt NSIS uncertainties.
It identifies the observed application/helper inputs rather than borrowing a
claim that every EXE/DLL contains the same Microsoft code. Its conditional use
of Distributable Code does not declare an unverified input redistributable.

### Microsoft references and applicability

The official VS and SDK documents were downloaded again. Their digests match
section 6, so this proposal does not silently substitute a newer agreement:

| Reference | Role in this proposal |
| --- | --- |
| [VS 2026 Enterprise/Professional terms](https://visualstudio.microsoft.com/license-terms/vs2026-ga-pro-enterprise/), [original DOCX](https://visualstudio.microsoft.com/wp-content/uploads/2025/10/Visual_Studio_2026-License-Enterprise_Professional_ENU.docx), II.6–II.7 | Conditional distributor obligations, component precedence and downstream protection. SHA-256 `dd2ab92a7c2bb90dd509e16572707a34b13bc0bc09b9199d115f25594ac39053`. These are the reviewed reference terms, not proof that this is the distributor's controlling licensing channel. |
| [VS 2026 Distributable List](https://learn.microsoft.com/en-us/visualstudio/releases/2026/redistribution#visual-c-runtime-files) | Identifies release runtime files and exclusions; still does not expressly name the observed toolset `lib` paths. |
| [Microsoft licensing guidance](https://www.microsoft.com/licensing/guidance/Visual-Studio#how-certain-software-can-be-distributed-to-others-within-your-applications) | Distinguishes licensing channels and describes linked output from identified distributable libraries. Guidance does not itself identify our `msvcrt.lib` inputs as covered. |
| [Windows SDK license](https://aka.ms/WinSDKLicenseURL), [resolved RTF](https://download.microsoft.com/download/0/F/F/0FF2B061-47DD-4F55-89B6-FD1D8C44F14D/sdk_license.rtf), 2(a)(i)–(iii) | SDK-specific conditions summarized in the notice. SHA-256 `dd07eb178e00c6bba4148457fc00ff77cd4887eb521d504186fe59c9ec8bbe62`. Its older document title is retained, not treated as evidence of a different binary SDK version. |
| [SDK REDIST static LIB list](https://learn.microsoft.com/en-us/legal/windows-sdk/redist#static-lib-files) | Conditional coverage of the observed public SDK `um` and `ucrt` directory families. This does not extend that list to the separate MSVC toolset directory. |
| [CRT library documentation](https://learn.microsoft.com/en-us/cpp/c-runtime-library/crt-library-features) | Explains why dynamic linkage still embeds `msvcrt.lib` startup code. Actual selected members/hashes remain those recorded in the dynamic-build evidence. |
| [V14 runtime end-user terms](https://visualstudio.microsoft.com/license-terms/vs2026-ga-visualcpp-v14-redist-runtime/) | A separate agreement for the external runtime, not the distributor grant for CodeConvoy's embedded code. It is not copied into the packages as a substitute grant. |

The notice distinguishes the SDK's operating-system restriction from broader
wording in the comparison projects. It also keeps the VS indemnification
exception separate from the SDK clause. An ordinary recipient is not made the
distributor's indemnitor. The Excluded License summary concerns Microsoft's
source; it does not claim that independent MIT code becomes restricted.

### CPython and yt-dlp comparison

The following upstream sources were inspected at fixed revisions on 2026-10-16:

| Project/source | Observed approach and what we adopt |
| --- | --- |
| CPython [`PC/crtlicense.txt`](https://github.com/python/cpython/blob/d3ada4509ea9f4d5c7a7a28566049c8891ee6d11/PC/crtlicense.txt) | A dedicated Windows notice separates Microsoft code from Python's own license, describes incorporated code and alerts redistributors to downstream conditions. We adopt that separation and notice structure, not its Python-specific coverage or its assertion that redistribution complies. |
| CPython [`PC/layout/main.py`](https://github.com/python/cpython/blob/d3ada4509ea9f4d5c7a7a28566049c8891ee6d11/PC/layout/main.py) | The Windows layout ships the build's `LICENSE.txt`. This is a file-delivery comparison, not a legal opinion about every CPython installer or recipient. |
| yt-dlp [`THIRD_PARTY_LICENSES.txt`](https://github.com/yt-dlp/yt-dlp/blob/897341241f2bfb2439fe0367f4c7c9ccd29a3828/THIRD_PARTY_LICENSES.txt) and [generator](https://github.com/yt-dlp/yt-dlp/blob/897341241f2bfb2439fe0367f4c7c9ccd29a3828/devscripts/generate_third_party_licenses.py) | The generator imports CPython's CRT notice into the aggregate third-party text and labels it Windows-only. Its introduction separately identifies yt-dlp's own Unlicense. We likewise use the existing aggregate notice with an explicit Windows scope. |

The two projects are not independent Microsoft permissions: yt-dlp directly
reuses CPython's text. Their notices do not identify CodeConvoy's toolset,
hosted-build agreement or NSIS inputs. Neither is authority that Microsoft
accepts passive notice delivery for this distributor. Our text is original;
no Python-specific grant, platform wording, `.pyd` claim or blanket compliance
assertion was copied. No click-through requirement can be inferred from these
files, and their presence does not prove that no other agreement process exists.

### Delivery and whether the notice is sufficient

The canonical notice is registered on the existing Microsoft component in
`catalog.json`, with its complete UTF-8 text in the content-addressed `texts.json`
map. Its source bytes are pinned in the catalog's existing `files` collection.
`licenses.py` verifies that all three bindings agree, even while the component
remains unresolved. The ordinary renderer includes it verbatim in
`THIRD-PARTY-NOTICES.txt`; no new packaged filename or format is introduced.

The NSIS resource configuration delivers that aggregate file beside
`CodeConvoy.exe`, and the portable ZIP places it at its root. Both existing
final-artifact validators compare the complete generated bytes. As before,
all platforms receive the conservative aggregate, but this notice applies only
to its identified Windows components. Source archives and unrelated notices
are unchanged. The project `LICENSE`, installer license page, silent/passive
operation and runtime prerequisites are unchanged.

**Assessment: the proposal supplies an attributed, component-scoped notice and
reliable delivery, but it does not by itself satisfy all recipient/distributor
requirements.** In particular:

- Communicating restrictions is not evidence that a downstream distributor or
  external end user agreed to them. The current MIT license page covers a
  different grant, and merely reading an installed file, extracting a ZIP or
  completing a silent installation does not establish assent.
- The summarized conditions have not been established as a complete protective
  sublicense. Review must identify which provisions of the controlling
  agreement need to protect Microsoft code downstream, including applicable
  use restrictions, disclaimers, liability protections and statutory exceptions.
  Importing all IDE/SDK development terms indiscriminately into an end-user
  agreement would also be unjustified.
- The reviewed distribution clauses require agreement, but do not prescribe a
  checkbox or an installer-specific mechanism. No click-through is added. A
  supported determination must address ordinary installation, unattended
  deployment, portable delivery and further redistribution; that may be an
  appropriate existing agreement rather than an installer change.

This is an evidence-based proposal for maintainer/legal review, not a Microsoft
approval or a final opinion on enforceability. No statement here accepts the
distributor's contractual undertakings on the maintainer's behalf.

### Exact remaining questions before blocker removal

1. **Grant and governing agreement:** identify the controlling agreement and
   authorized distributor for the recorded hosted VS Enterprise build, and
   establish the grant for the selected x64/x86 `msvcrt.lib` startup/support
   output and other VC inputs. Confirm whether the installed components carry
   overriding terms. The SDK list provides conditional coverage for its own
   recorded paths; neither a DLL REDIST entry nor this notice proves VC library
   coverage. This is an unresolved evidence question, not a finding of an
   unlicensed build or a demand to buy a separate license without that review.
2. **Recipient protection and assent:** determine the sufficient protective
   wording and supported agreement/delivery method for both formats and
   downstream redistribution. The notice intentionally does not assert that
   this requirement has been discharged.
3. **Distributor undertakings:** establish that the actual distributor can and
   does comply with the applicable contractual obligations, including the
   separate indemnification commitment. Package presence tests cannot do this.
4. **Remaining installer inputs:** establish whether the recorded prebuilt
   NSIS setup/uninstaller/plugins incorporate Microsoft code needing additional
   rights or notices and, if so, identify them. Their already resolved NSIS
   source/license status is not reopened and supplies no Microsoft grant.

These are the reasons `windows-crt` stays active. No missing notice-delivery
mechanism is being used as a proxy for those separate decisions. New terms or
an authoritative interpretation could resolve more than one question together;
no new binary audit is requested merely to repeat existing import/link evidence.

### Validation of the notice change

Local validation passed on Linux using Rust 1.96.1:

| Check | Result |
| --- | --- |
| `cargo fmt --check` | Passed |
| `cargo clippy --all-targets --all-features -- -D warnings` | Passed |
| `cargo test --all-features` | 621 passed; 10 intentionally ignored |
| `cargo build --release` | Passed |
| `python3 -m unittest discover -s packaging -p 'test_*.py'` | 109 passed |
| `python3 packaging/licenses.py check` | 457 locked crates; one unresolved obligation |
| Publication guard | Rejected solely for `windows-crt`, as intended |
| `git diff --check` | Passed |

Packaging regressions exercise actual notice generation/resource configuration,
portable ZIP byte verification and the final NSIS verifier with a controlled
extraction fixture. They reject deleting only the Microsoft section or changing
its attribution while leaving all other notice bytes intact. Catalog regressions
reject lost component/text/source bindings and source-only hash replacement.
The resource test supplies a compiler-version fixture and mocks package creation;
the installer test supplies fixture executables/plugins. These tests verify
delivery logic, not a new native Windows build or a recipient's legal agreement.

The original source ZIP remains byte-identical
(`eb61566a9f3e26f7c5d29482cabb22828a86cd6b985480e48faf196e77ba7b8f`),
all 366 earlier notice texts are unchanged, and existing native provenance and
the `windows-crt` blocker entry are unchanged. Rust tests use `/var/tmp` for
temporary repositories to avoid the previously recorded `/tmp/.git` marker.
No CRT, installer, dependency, workflow or package-format changes were made.

Native Windows packages were not rebuilt for this notice-only change, and
cross-platform CI was not dispatched. The earlier dynamic-CRT packages do not
contain the new notice and are not claimed as validation of its delivery.
The release compiler pin remains 1.99.0; the local Rust checks are source checks,
not release-toolchain evidence. No commit, push, merge, tag or release was made
for this proposal.

## 9. Final risk-based Windows review

Reviewed on 2026-10-16 against `56dfe55` (the committed notice implementation).
**Recommendation for maintainer approval: retain `windows-crt`, narrowed to
the two material decisions below.** The earlier requirement for external
confirmation of every uncertainty was an internal assurance policy. It is not
itself a Microsoft license condition. This review is a documented compliance
assessment for a maintainer decision, not authoritative legal advice or a
finding of infringement. No individual Microsoft approval letter or mandatory
external legal opinion is required by the cited distribution clauses.

### Scope and classification

The current payload is the dynamic-CRT application and utility helper, the
CRT-independent AppId helper, and the unchanged recorded NSIS binaries. Users
supply the runtime DLLs; neither format bundles a runtime DLL or redistributable
installer. Selected `msvcrt.lib` startup objects and SDK support/import/UUID
objects still occur in linked output. Sections 7–8 and the existing native
validation record supply these facts; no binary investigation was repeated.

The terms and their document digests in section 8 remain the references for
this review. A **satisfied obligation** below is limited to the stated element;
it does not imply that every condition of the grant is satisfied.

| Previously raised concern | Classification | Evidence and disposition |
| --- | --- | --- |
| Permission for the linked VC startup/support output and controlling component agreement | **Unresolved material obligation (G1)** | VS II.7(a)(i) ties object-code permission to the distributable list; II.6, “Licenses for Other Components / Microsoft Platforms,” gives component terms precedence. The selected VC inputs are known, but the record has not mapped their linked output to the controlling grant. Missing literal `msvcrt.lib` paths in the public list is not proof of prohibition. A supported interpretation covering this class of output is sufficient; a separate permission for each archive member is not required. |
| SDK library coverage and linked-output form | **Satisfied obligation**, conditional grant | SDK 2(a)(i)–(ii) and the SDK REDIST static-LIB entry cover the recorded public `um`/`ucrt` directory families and linked output. No SDK library files are shipped separately. The directory grant is sufficient evidence for those inputs; more historical member-level licensing reconstruction is unnecessary. Recipient conditions remain G2. |
| Hosted-build entitlement | **Confirmed obligation** to use an authorized toolchain; a separate approval certificate is an **internal assurance requirement** | The recorded non-prerelease Enterprise installation on GitHub's standard hosted image supports ordinary authorized build use. [GitHub's hosted-runner documentation](https://docs.github.com/en/actions/reference/runners/github-hosted-runners) and [Actions terms](https://docs.github.com/en/site-policy/github-terms/github-terms-for-additional-products-and-features#actions) describe that service. They do not themselves specify the VC output grant. Treat that mapping within G1, not as a separate demand to purchase a subscription or obtain a Microsoft/GitHub letter. No evidence here identifies trial or unauthorized use; `isPrerelease: false` alone is not proof of a full-use license. |
| Recipient/downstream protection and agreement | **Unresolved material obligation (G2)** | VS II.7(b)(ii) and SDK 2(a)(ii) require protective recipient terms and agreement. Our notice expressly remains a summary and disclaims establishing a complete protective set or recipient arrangement. Its verified delivery establishes communication, not the missing contractual determination. Neither clause specifies a checkbox, signatures or per-install acceptance logs. |
| Distributor defense/indemnity and continuing conduct | **Confirmed obligation**, not a separate evidence blocker | VS II.7(b)(iii) and SDK 2(a)(ii) place undertakings on the distributing licensee. These are continuing responsibilities under the applicable agreement, not a requirement to obtain an indemnity certificate, prove financial capacity or make ordinary end users indemnify Microsoft. The maintainer must account for them when adopting the distribution basis; automated tests cannot discharge them. The VS sole-Distributable-Code exception must not be imported into the SDK clause. |
| Notice preservation, distinct MIT grant, substantive application and Windows-only distribution | **Satisfied obligation**, for observed materials | The canonical Microsoft notice and existing license material are delivered by the shared notice infrastructure; byte checks cover both formats. CodeConvoy supplies substantial independent functionality, preserves its own copyright/license, and does not present Microsoft code as MIT. No Microsoft source is relicensed, and these native payloads target Windows. The current release toolchain evidence supports the non-preview condition. This is not a guarantee about future downstream conduct. |
| Separate VC runtime redistribution rights | **Satisfied obligation by avoiding that distribution** | No DLLs or redistributable installer are bundled. The separate runtime end-user agreement does not supply the grant for embedded startup code and need not be substituted for it. Runtime prerequisites remain a technical requirement. |
| Complete historical CRT provenance for unchanged official NSIS binaries | **Internal assurance requirement**, absent a specific contrary fact | Existing source/license, binary identity and package evidence remain usable. Unknown historical inputs are a recorded residual uncertainty, not affirmative evidence of an unmet Microsoft condition. Reconstructing every input or proving total absence of Microsoft code is no longer a standalone closure requirement. A newly identified component with incompatible/missing rights would trigger a targeted review. This does not assert that NSIS's own license grants rights to Microsoft code. |

### What the comparison projects support

The pinned CPython and yt-dlp sources in section 8 support Windows-specific
conditions, downstream redistribution guidance and separation from an
application's open-source license. CPython makes an affirmative compliance
statement subject to conditions; yt-dlp imports that notice. Their approach is
useful drafting precedent, but yt-dlp is not independent evidence of a Microsoft
grant and neither notice identifies CodeConvoy's governing agreement.

Firefox's [license-page source](https://github.com/mozilla/gecko-dev/blob/master/toolkit/content/license.html#L5713-L5753),
inspected on 2026-10-16, places conditional Microsoft use/distribution terms in
the Windows-only proprietary-components section. Its listed runtime DLLs differ
from CodeConvoy's embedded startup output. Mozilla's [binary-components policy](https://www.mozilla.org/en-US/foundation/licensing/binary-components/)
explains the separation from open-source licensing. Firefox also has
[Terms of Use](https://www.mozilla.org/en-US/about/legal/terms/firefox/)
(effective 2026-09-16) addressing executable-code use; its notice is not evidence
that Firefox operates without any agreement.

These sources support a component-scoped conditional notice without imposing
Microsoft terms on independent MIT source. They do not demonstrate a requirement
for a separate Microsoft click-through. Equally, they cannot establish that
CodeConvoy's current explicitly informational text supplies the required
agreement. A notice-based arrangement is a possible solution to G2, subject to
an actual determination of its protective wording and delivery; absence of
recorded clicks is not itself a defect.

### Two bounded decisions before publication

1. **G1 — applicable VC output grant (legal/contractual, not missing binary
   evidence).** Identify the agreement covering the recorded hosted build and
   cite the provision or supported interpretation permitting its ordinary
   linked VC startup/support output. [Microsoft's redistribution guidance](https://www.microsoft.com/licensing/guidance/Visual-Studio#how-certain-software-can-be-distributed-to-others-within-your-applications)
   supports linked output from identified distributable libraries, but does not
   independently identify these VC inputs. The smallest next action is a short
   grant mapping using the existing terms and, if needed, only the accompanying
   VC component license/additional-license record. There is no need for another
   linker audit, exact-member permission letter or blanket Microsoft approval.
   A maintainer can adopt a supported reading; merely declaring that risk is
   acceptable does not create missing rights. If the component record still
   leaves this specific question ambiguous, seek a focused interpretation of
   it rather than restarting the audit.
2. **G2 — recipient terms and agreement (contractual).** Establish the operative
   Microsoft-component conditions and how recipients agree for installer,
   silent deployment, ZIP and further redistribution. The minimum next action
   is a bounded review/adoption of that component-only wording and its delivery
   basis, considering the existing notice-based precedents. This may use a
   notice-based or existing agreement; it does not require an installer feature.
   Current MIT text and the summary's disclaimer do not establish that basis.
   No change to CodeConvoy's MIT grant or speculative click-through is proposed.

The active catalog detail now names G1 and G2 only. The component's unresolved
marker, `release-ready` enforcement and all objective checks remain unchanged.
The SDK grant, historical NSIS uncertainty and an extra indemnity confirmation
are not additional independent blockers. Approval of this review adopts that
narrower policy; it does not by itself answer G1/G2 or authorize publication.

### Maintenance and validation boundary

Reuse this assessment for unchanged components. Continue automated notice,
source-bundle, binary identity, prerequisite and final-package verification.
Review only affected questions when the MSVC/SDK or runner licensing channel
changes, governing terms change, NSIS/helper inputs change, a new Microsoft
payload is bundled, or recipient terms/distribution methods change. An unchanged
application release does not require another historical audit or external
confirmation. A concrete contradictory licensing fact also triggers review.

The Microsoft notice, its text-map entry and hash remain unchanged. No CRT,
helper, installer behavior, dependency, AppImage, workflow or package-format
change is part of this review. The aggregate notice's blocker description and
generated inventory reflect the narrower assessment; all full notice texts
remain unchanged.

Validation of this review on Linux with Rust 1.96.1:

| Check | Result |
| --- | --- |
| `cargo fmt --check` | Passed |
| `cargo clippy --all-targets --all-features -- -D warnings` | Passed |
| `cargo test --all-features` | 621 passed; 10 intentionally ignored (nested child invocations not double-counted) |
| `cargo build --release` | Passed |
| `python3 -m unittest discover -s packaging -p 'test_*.py'` | 109 passed |
| `python3 packaging/licenses.py check` | Passed: 457 locked crates; one unresolved release obligation |
| `python3 packaging/licenses.py release-ready` | Failed as expected, exit 1, solely `windows-crt` |
| `git diff --check` | Passed |
| Material preservation | Canonical Microsoft notice occurs exactly once, byte-identical, in the generated aggregate; all 367 texts unchanged; source ZIP remains `eb61566a9f3e26f7c5d29482cabb22828a86cd6b985480e48faf196e77ba7b8f` |

Only the blocker description and current Windows limitation text changed in the
catalog. Native provenance, pinned hashes, component unresolved marker and all
verification code/tests are unchanged. Rust test repositories again used
`/var/tmp` because of the known `/tmp/.git` marker.

**Technical candidate readiness:** the source and packaging checks support
proceeding to a non-publishing 1.0.0 candidate validation after maintainer
approval. The two-stage workflow permits that validation with the publication
blocker active. This does not certify a final candidate: the earlier
[dynamic-CRT CI run](https://github.com/ChrisLauinger77/code-convoy/actions/runs/38923230998)
predates the Microsoft notice. A candidate from the final reviewed commit must
still verify that notice and updated inventory in actual packages using the
pinned Rust 1.99.0 toolchain. Local Rust 1.96.1 checks are not that evidence.
Existing manual Windows 10/11 desktop validation limitations remain recorded;
no platform test, helper rebuild or workflow dispatch was repeated here.
Publication readiness remains **no**, because G1/G2 are unresolved.

Work stops at this review. No commit, push, tag, merge, publication or external
licensing enquiry was performed. Maintainer approval is pending.
