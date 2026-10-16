# Third-party license audit — readiness PR 5

CodeConvoy's original code and artwork remain **MIT licensed**. This audit adds
notice/source delivery and omission checks; it does **not** certify complete
compliance or authorize a release. The unresolved obligations below block
publication. Application functionality, dependencies, tool pins, package formats
and the two-stage release architecture are unchanged.

## Inventory and evidence

[catalog.json](../third-party/catalog.json) inventories all **457 third-party
packages in Cargo.lock**, with exact versions, registry checksums, declared
license expressions, authors, source revisions and notice provenance. A
conservative non-dev dependency traversal for Linux x86-64, Windows x86-64,
macOS arm64 and macOS x86-64 reaches **383** crates, including build scripts and
procedural macros. `targets` records that closure; `application_targets`
separately records normal dependency edges. The other 74 entries are explicit
exclusions for these release targets. This does not imply every object survives
linking, or that every listed crate is in every package.

The collection contains **366 distinct notice texts**, deduplicated by
their content hashes in [texts.json](../third-party/texts.json). It retains
upstream copyright statements, notices, font licenses and protocol XML copyright
blocks. When a crate omits a license file, the evidence refers to its exact
`.cargo_vcs_info.json` upstream revision, rather than an unversioned default
branch. Notice text is decoded as UTF-8 (removing a BOM where present); source
archives retain their original bytes.

The inventory also identifies **52 assets, runtimes, packaging and supplemental
Rust library components**,
with unresolved versions expressly marked. Rust's separate standard-library
report covers **141 out-of-tree entries** plus in-tree exceptions; its complete
upstream HTML is included, with those component/version/license entries indexed
in the catalog. It was recovered from the checksum-verified Rust **1.99.0**
compiler distribution. That report conservatively includes tooling and targets
beyond CodeConvoy; the compiler itself is not shipped in the application.

Cross-checking the verified `rust-src` distribution exposed omissions in that
generated report: `compiler_builtins` and some versions in `library/Cargo.lock`
were absent. The catalog therefore also covers all **30 registry entries** in
that separate lockfile, with notices for 27 and explicit SGX/VEX/WASI exclusions
for the other three. It retains the in-tree compiler-builtins, libm, backtrace,
stdarch, portable-simd and LLVM unwind license files. Compiler-builtins 0.1.160
requires **MIT AND Apache-2.0 WITH LLVM-exception**, rather than an OR choice.
The complete original HTML is retained as evidence, not treated as an infallible
or exhaustive inventory. The library lockfile is included in the source ZIP.

| Component group | Distribution and conditions addressed |
| --- | --- |
| Rust crates, mostly MIT/Apache/BSD/ISC/Zlib/BSL/Unlicense | Preserve actual license/copyright/NOTICE texts. Retain OR alternatives; `self_cell` uses Apache rather than its GPL alternative. AND clauses are cumulative. |
| `epaint_default_fonts` 0.36.2 | Unmodified embedded Hack, Noto Emoji, Ubuntu Light and emoji-icon-font bytes; all four font notices included. Hack includes Bitstream-derived terms; OFL and Ubuntu font naming/reserved-name conditions remain applicable. |
| Unicode | Unicode-3.0 texts for 18 Unicode-licensed crates, the combined `unicode-ident` expression, and Rust standard-library Unicode exceptions. |
| `option-ext` 0.2.0 | MPL-2.0 notice and exact unmodified `.crate` source archive; checksum must equal Cargo.lock. |
| AppImage runtime 20251108, CodeConvoy build 1 | Same MIT launcher revision `dd6cebedcbddde9c82f89b011e8e1d40b6e43868`; static libraries rebuilt from pinned sources. Exact input manifest and recipient relinking recipe are distributed. See the [verification record](appimage-runtime-validation.md). |
| Compiler startup code | Source-built musl 1.2.5 CRT and checksum-identified GCC 14.2.0-r4 crtbeginS/crtendS objects. GCC source/Alpine recipe, copyright, GPLv3 and Runtime Library Exception 3.1 are included; the documented C compilation process supports the stated exception interpretation. |
| NSIS 3.09 | Installer/uninstaller stub, standard plugins, artwork and LZMA decoder; original COPYING and full official source archive included. LZMA's CPL-1.0 linking exception does not relicense CodeConvoy. |
| Windows replacement NSIS helpers | Original MIT shortcut helper and reduced MIT/Apache Tauri process/SemVer helper; locked source/build kit, dependency notices, binary pins and linker evidence. See the [validation record](nsis-helper-validation.md). |
| Windows CRT / SDK support code | Dynamic runtime DLLs are supplied by users; embedded Microsoft startup/support code and hosted-build conditions remain unresolved. |
| Original icons, AppRun and application assets | Project MIT. No new third-party artwork added. |
| Host libraries, Git and agent CLIs | Installed separately, not redistributed in these packages. Newly bundled ELF/PE/Mach-O code fails the inventory gate. |
| linuxdeploy, appimagetool, cargo-packager | Build tools; their executables are not shipped. The emitted AppImage runtime and NSIS/template code are separately covered. |

## MPL and source availability

`option-ext` is compiled as an unchanged library into a larger application. MPL
2.0 is file-level copyleft: its covered source remains MPL, while separately
licensed CodeConvoy files can remain MIT. The actual executable distribution
still requires making the covered source available and telling recipients where
to obtain it. Every package includes `option-ext-0.2.0.crate` in
`THIRD-PARTY-SOURCES.zip`, its full MPL text and an explicit source-access notice.
No expiring download or future written offer is relied upon. See Mozilla's
[FAQ, especially Q8 and Q11](https://www.mozilla.org/en-US/MPL/2.0/FAQ/) and
[MPL sections 3.1–3.4](https://www.mozilla.org/en-US/MPL/2.0/).

The same source ZIP contains the pinned AppImage runtime source, libfuse 3.15.0,
its upstream runtime patch/build recipe, squashfuse 0.5.2, musl 1.2.5, zstd 1.5.6,
zlib 1.3.1, mimalloc 2.1.7 source, nsis-tauri-utils 0.2.1 and official NSIS 3.09
source. Each archive has a recorded digest and origin. The AppImage kit now includes the pinned build environment, build scripts,
modification notices and a recipient patch example. Two clean outputs match;
stock and modified-library relinking and startup were demonstrated. See the
[AppImage record](appimage-runtime-validation.md) for scope and legal limits.

`codeconvoy-nsis-helpers-1.zip` supplies the original/reduced helper source,
Cargo.lock, exact source patches, custom template, notices and build recipe.
The SemVer and Windows binding source archives accompany it. The old ApplicationID
and nsis-tauri-utils prebuilt DLLs were absent from the byte-verified candidate
before their live inventory entries were removed. Historical evidence remains
in Git and the earlier audit record. This does not establish a grant for those
retired binaries; it establishes different source-backed replacement components.

## End-user access

The files are accessible offline, without launching CodeConvoy:

| Format | Notice/source directory |
| --- | --- |
| DEB | `/usr/share/doc/code-convoy/` |
| RPM | `/usr/share/licenses/code-convoy/` (`%license` files) |
| AppImage | `squashfs-root/usr/share/doc/code-convoy/` after `--appimage-extract`, or the same relative path in its FUSE mount |
| macOS DMG/app | `CodeConvoy.app/Contents/Resources/` (Finder: Show Package Contents) |
| Windows installer | Installation directory beside `CodeConvoy.exe` |
| Windows portable ZIP | Archive root beside `CodeConvoy.exe` |

Read `THIRD-PARTY-NOTICES.txt`, `THIRD-PARTY-INVENTORY.json` and
`RUST-STDLIB-LICENSES.html`; unpack `THIRD-PARTY-SOURCES.zip` with an ordinary
archive manager. All platforms receive the same conservative bundle. This adds
roughly 16 MB before package compression; source archives account for most of it.

## Unresolved release obligations

These are tracked in `catalog.json:release_blockers`; `publish.py` checks them
before creating even a draft release. Non-publishing candidate builds remain
available for inspection. Removing a blocker requires new evidence and review.

The [follow-up evidence record](license-blocker-evidence.md) documents the deeper
investigation, exact binary/debug identities, newly recovered sources/notices
and remaining requirements. The Windows CRT blocker remains. The `dispatch` notice blocker
is resolved under the documented interpretation of its explicit MIT declaration:
the bundle now preserves its original manifest/author metadata and reproduces
the standard MIT permission, condition and disclaimer, without inventing a
copyright notice. This is an evidence-based compliance interpretation submitted
for maintainer review, not authoritative legal approval. See
[section 3 of the evidence record](license-blocker-evidence.md#3-dispatch-020-resolved-under-the-documented-interpretation)
for sources, notice obligations and limits. The original plugin investigation
is historical; the replacement has a complete locked dependency inventory.

The two AppImage obligations are resolved for the replacement source-built
runtime under the documented provenance and LGPL-2.1 section 6(a) interpretation.
The original binary's missing provenance has not been recovered or waived.
Incomplete provenance was an internal evidence gap, not automatic proof of a
license violation. The replacement binds actual output to identified sources,
patches, CRT objects and build tools. Recipients receive the source/relink kit;
the modified library runs with the same AppDir through both FUSE and extraction.
Native Ubuntu 24.04, Debian 13 and Wayland desktop checks remain separate platform
validation gaps. Neither reproducibility nor the demonstration is legal approval.

The **NSIS source/license blocker** is resolved for the replacement DLLs under
the documented MIT/Apache notice-delivery interpretation. The source grants,
copyright notices, dependency graph, build inputs and identical clean-build
outputs are retained; the exact candidate payload contains the intended DLLs
and source/notice material. This does not waive missing rights/provenance for
the retired binaries. Native behavior evidence and its limitations are kept
separately in [NSIS helper validation](nsis-helper-validation.md); no legal
clearance or authorization to publish is inferred.

1. **Windows CRT.** Windows packaging now requires an application-bound
   `/VERBOSE:LIB` capture of the selected toolset/SDK libraries, hashes and
   archive members. Final installer/helper PE inspection and final package
   hashes are retained separately from the six candidate assets. The capture
   is evidence, not redistribution approval. Applicable static-code grants,
   hosted build entitlement, recipient conditions and any remaining installer
   runtime obligations still require confirmation. The replacement helpers now
   have their own x86 linker evidence. The current distribution policy uses
   dynamic CRT linkage for the application and Rust helper, with user-supplied
   runtime DLLs; the original AppId helper remains CRT-independent. Historical
   static-link evidence is retained as historical evidence. See the Windows
   section of [the evidence record](license-blocker-evidence.md#5-windows-static-crt-unresolved)
   for actual observations, Microsoft references and the repeatable checks.
   The application remains MIT; that license does not grant rights to Microsoft
   runtime code. The `windows-crt` publication guard remains active.
   The [post-PR-73 assessment](license-blocker-evidence.md#6-windows-crt-redistribution-assessment-after-pr-73)
   distinguishes the conditional SDK grant from the unconfirmed VC/hosted-build
   entitlement and recipient agreements. It covers both replacement helpers
   and the remaining NSIS binaries, and records the exact decisions needed
   before closure. Four of the five original blockers are resolved; Windows
   CRT is still the sole publication blocker. Dynamic linkage does not clear it.
   The current [dynamic CRT record](license-blocker-evidence.md#7-dynamic-crt-and-user-supplied-prerequisites)
   distinguishes user-supplied DLLs, embedded application/helper startup code,
   prebuilt NSIS inputs and hosted-toolchain entitlement.

Primary AppImage evidence: [runtime source/tag](https://github.com/AppImage/type2-runtime/tree/20251108),
[libfuse terms](https://github.com/libfuse/libfuse/blob/fuse-3.15.0/LICENSE),
[LGPL-2.1 text](https://github.com/libfuse/libfuse/blob/fuse-3.15.0/LGPL2.txt).
Installer evidence: [NSIS licenses and linking exception](https://nsis.sourceforge.io/License),
[ApplicationID upstream links](https://nsis.sourceforge.io/ApplicationID_plug-in),
[nsis-tauri-utils 0.2.1](https://github.com/tauri-apps/nsis-tauri-utils/tree/nsis_tauri_utils-v0.2.1).

## Updating the audit

1. Fetch locked sources and export `cargo metadata --locked --format-version 1`.
   `packaging/collect_licenses.py METADATA_JSON OUTPUT_JSON` collects candidate
   texts, including nested font/protocol notices. It is an evidence collector,
   not an approval tool. Inspect embedded code/assets and source headers too.
2. Export metadata with `--filter-platform` for each of the four target triples.
   Traverse from `resolve.root`, excluding dev edges; retain build/proc-macro
   inputs conservatively. Record normal-edge coverage separately. Review newly
   included crates, all AND obligations and the chosen OR alternatives.
3. Update the reviewed catalog and content-addressed text map. Recover missing
   texts from exact upstream source revisions, preserving origin URLs. For MPL
   source, use the original registry archive and match its Cargo.lock digest.
4. When changing Rust, recover its exact `COPYRIGHT-library.html`, MIT and Apache
   notices from a checksum-verified release archive. Compare the separate
   `library/Cargo.lock` and nested licenses in the matching verified `rust-src`
   archive; the generated report can omit them. Recheck installer/runtime
   native inputs when changing packaging tools or runner toolchains. Do not
   replace unknown identities with guesses or generic SPDX templates.
5. Run `python3 packaging/licenses.py check`, all packaging tests and required
   Rust checks. After merge, produce a fresh non-publishing candidate from `main`
   and inspect all six final formats. Update the verification record and resolve
   remaining blockers before running `python3 packaging/licenses.py release-ready`.

## Earlier candidate verification record

This records the candidate before the follow-up evidence and deterministic ZIP
checks. It is historical, not verification of the current head. PR checks and
interim artifact evidence are recorded in
[PR 68](https://github.com/ChrisLauinger77/code-convoy/pull/68). Per maintainer
direction, the next release build and final six-format inspection are deferred
until after merge into `main`; they are not claimed for this PR's final head.

Verification date: **2026-10-15**. Local 1.0.0 formatting, strict Clippy, locked
Rust tests (**602 passed, 10 ignored**), release build and **58 packaging tests**
passed. The separately enabled private-D-Bus regression also passed.
[CI 38757557996](https://github.com/ChrisLauinger77/code-convoy/actions/runs/38757557996)
passed on Linux, macOS and Windows.

The fresh non-publishing
[candidate 38757563994](https://github.com/ChrisLauinger77/code-convoy/actions/runs/38757563994)
passed validation, all three native build/package jobs and aggregation. Both CI
and candidate tested implementation commit
`9f0801ee01af24f05a5763913cb2cf82b727d8dc`; the subsequent documentation-only
commit records the evidence and does not represent another candidate.
Publication and external package-repository updates were skipped.

`candidate.verify_run` and `select_artifact` verified the successful first-attempt
manual run, repository, workflow, exact source and non-expired `release-assets`
artifact **11986990820**. All six downloaded files passed the original aggregate
`SHA256SUMS` through `release.verify_checksums`; the manifest was not regenerated.
Their bytes also matched the separately downloaded platform artifacts used for
independent inspection.

| Final artifact | Maximum required GLIBC | SHA-256 |
| --- | --- | --- |
| `code-convoy_1.0.0_amd64.deb` | 2.39 | `47a627145f035b032edb8f526e625956c4b9efae02a593dc8939682fb750d29c` |
| `code-convoy-1.0.0-1.x86_64.rpm` | 2.39 | `ad0a8e9b0a55748eb1b6dc78b3af4926de5eee2b9e1e42efd7bac6fb5c277d7b` |
| `CodeConvoy-1.0.0-x86_64.AppImage` | 2.39 | `8dd741bda5ef692e43fced4d0078fd1bd761da1eeed13a499e81178838730c12` |
| `CodeConvoy-1.0.0-windows-x86_64-setup.exe` | Not applicable | `a88f08ce4087ee8a30708601eab65a8a138ea482514a15dc1e2a14a6764aa725` |
| `CodeConvoy-1.0.0-windows-x86_64.zip` | Not applicable | `db0f162b776c8d30ba858fff6d40d38dd04a70ad9a08ae2e298f8f45d6bcda52` |
| `CodeConvoy-1.0.0-macos-universal.dmg` | Not applicable | `ea275701796d745752d9405ea49e77f48ccbf3678410e7e3f9b68bca120f8f80` |

Independent local inspection extracted all three Linux archives, the Windows
installer/ZIP and the final macOS DMG. Every package contains the same exact four
notice/source files. The AppImage runtime matches the pinned upstream binary,
apart from its documented MD5 field, and has no versioned GLIBC requirement.
No additional native library was found in the Linux payloads or macOS app.
The installer application matches the ZIP and all five embedded plugin hashes
match the audit. Windows' pre-existing project MIT file has CRLF line endings;
its normalized text matches the repository, while all new material is identical
byte for byte across platforms. The bundled source archives contain no ELF,
PE or Mach-O executable entries.

The native macOS job verified both architectures and signatures in the mounted
final DMG; local extraction independently verified its resources and 1.0.0
metadata. Windows verified installed notice bytes, per-user installation,
startup, Start Menu identity and uninstall with user-data preservation. Linux
passed APT installation/startup/removal on Ubuntu 24.04 and Debian 13, DNF on
Fedora 44, AppImage extraction, the hosted Ubuntu FUSE check and disposable
Git/process probes. These runtime checks and their limitations are detailed in
the [readiness report](release-readiness.md#final-verification).

At that earlier candidate revision, all five obligations blocked
`licenses.py release-ready` and publication. The current unresolved-obligations
section above records the remaining `windows-crt` blocker. No tag or public
release was created by that verification.
