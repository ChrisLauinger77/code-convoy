# Third-party licenses and maintenance policy

CodeConvoy's original code and artwork remain **MIT licensed**. This audit adds
notice/source delivery and omission checks; it does **not** certify complete
compliance or authorize a release. Any catalogued unresolved obligation blocks
publication; the Windows interpretation risk has a separate maintainer decision. Application functionality, dependencies, tool pins, package formats
and the two-stage release architecture are unchanged.

## Inventory and evidence

[catalog.json](../third-party/catalog.json) is the reviewed release snapshot,
not a lockstep dependency database for ordinary pull requests. The audit below
records **491 third-party packages**, with exact versions, registry checksums, declared
license expressions, authors, source revisions and notice provenance. A
conservative non-dev dependency traversal for Linux x86-64, Windows x86-64,
macOS arm64 and macOS x86-64 reaches **416** crates, including build scripts and
procedural macros. `targets` records that closure; `application_targets`
separately records normal dependency edges. The other 75 entries are explicit
exclusions for these release targets. This does not imply every object survives
linking, or that every listed crate is in every package. The
[1.3.1 release refresh](release-1.3.1-license-validation.md) records the 10
added and 8 removed crate versions since 1.3.0. The
[1.3.0 release refresh](release-1.3.0-license-validation.md) records the 56
added and 24 removed crate versions and refreshed target coverage; the earlier
[lock-file maintenance review](lockfile-maintenance-license-validation.md)
records the 1.2.0 refresh.

The collection contains **402 distinct notice texts**, deduplicated by
their content hashes in [texts.json](../third-party/texts.json). It retains
upstream copyright statements, notices, font licenses and protocol XML copyright
blocks. When a crate omits a license file, the evidence refers to its exact
`.cargo_vcs_info.json` upstream revision, rather than an unversioned default
branch. Notice text is decoded as UTF-8 (removing a BOM where present); source
archives retain their original bytes.

The inventory also contains **48 component records** for embedded assets,
runtimes, packaging and supplemental Rust libraries,
with unresolved versions expressly marked. Rust's separate standard-library
report covers **141 out-of-tree entries** plus in-tree exceptions; its complete
upstream HTML is included, with those component/version/license entries indexed
in the catalog. It was recovered from the checksum-verified Rust **1.99.1**
compiler distribution. That report conservatively includes tooling and targets
beyond CodeConvoy; the compiler itself is not shipped in the application. The
[1.99.1 review](rust-1.99.1-license-validation.md) records the verified archives
and unchanged notice bytes.

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
| `epaint_default_fonts` 0.37.0 | Unmodified Hack, Noto Emoji, Ubuntu Light and emoji-icon-font bytes, plus the new egui icon subset; font notices and the subset attribution are included. Hack includes Bitstream-derived terms; OFL and Ubuntu font naming/reserved-name conditions remain applicable. |
| Unicode | Unicode-3.0 texts for 18 Unicode-licensed crates, the combined `unicode-ident` expression, and Rust standard-library Unicode exceptions. |
| `option-ext` 0.2.0 | MPL-2.0 notice and exact unmodified `.crate` source archive; checksum must equal Cargo.lock. |
| Official AppImage runtime 20251108 | Checksum-pinned upstream MIT launcher at `dd6cebedcbddde9c82f89b011e8e1d40b6e43868`; sources, applicable patches/notices and upstream recipient build/relink instructions are distributed. See the [verification record](appimage-upstream-runtime-validation.md). |
| Compiler startup code | musl 1.2.5 and observed GNU CRT code; GCC 14.2.0 reference source, copyright, GPLv3 and Runtime Library Exception 3.1 remain supplied. Upstream C/clang build evidence supports the eligible-compilation interpretation; exact original APK/object hashes are not claimed. |
| NSIS 3.09 | Installer/uninstaller stub, standard plugins, artwork and LZMA decoder; original COPYING and full official source archive included. LZMA's CPL-1.0 linking exception does not relicense CodeConvoy. |
| Windows installer operations | Native NSIS WordFunc/System/COM support and Windows APIs. Custom and legacy AppID/Tauri helpers are absent; no separate helper dependencies or source kit. See [installer validation](windows-installer-validation.md). |
| Windows CRT / SDK support code | Original Windows-specific Microsoft Distributable Code notice included in the aggregate; runtime DLLs are supplied by users. Grant interpretation and recipient-agreement effectiveness remain uncertain under the maintainer-accepted 1.0.0 risk decision; conditions and operative terms remain applicable. |
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
zlib 1.3.1, mimalloc 2.1.7 source and official NSIS 3.09
source. Each archive has a recorded digest and origin. The AppImage kit includes
upstream build scripts, source annotations and a recipient patch example.
Modified-library relinking from the extracted kit and same-AppDir startup were
demonstrated with ordinary Alpine build tools. See the
[AppImage record](appimage-upstream-runtime-validation.md) for scope and legal limits.

The custom NSIS helpers, their dependency archives and source/build kit are
removed from the current payload and inventory. The released 1.0.0 sources and
native evidence remain available in the `v1.0.0` Git tag and published release.
NSIS, cargo-packager/template and Microsoft notices remain required. Removing
binaries does not establish licensing clearance for remaining components.

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

The [Microsoft Distributable Code notice](../third-party/MICROSOFT-DISTRIBUTABLE-CODE.txt)
is reproduced verbatim inside `THIRD-PARTY-NOTICES.txt`, under its Windows heading.
Both NSIS and the portable ZIP use this existing file; their validators reject
omission or alteration. Its Microsoft-specific scope does not change CodeConvoy's
MIT license. The [proposal and sufficiency review](license-blocker-evidence.md#8-proposed-microsoft-distributable-code-notice)
records the Microsoft references, the comparison with CPython/yt-dlp and why
notice delivery alone does not discharge the recipient/distributor requirements.
The [maintainer decision](windows-licensing-decision.md#implemented-maintainer-risk-decision--2026-10-16)
adds the proposed operative terms to this same notice and records the remaining
interpretation risk; it does not equate delivery with agreement or add a click-through.

## Publication obligations and accepted risk

These are tracked in `catalog.json:release_blockers`; `publish.py` checks them
before creating even a draft release. Non-publishing candidate builds remain
available for inspection. Known unmet obligations remain blockers. An explicit
maintainer policy decision may accept an interpretation risk without declaring
legal compliance; `accepted_licensing_risks` records that distinct disposition.

The [follow-up evidence record](license-blocker-evidence.md) documents the deeper
investigation, exact binary/debug identities, newly recovered sources/notices
and remaining requirements. The Windows CRT interpretation risk is accepted
under the existing [component- and distribution-bound decision](windows-licensing-decision.md#component-and-distribution-decision--2026-10-18).
Application version changes alone require no new acceptance or full Microsoft
audit. Material MSVC/SDK, linked-component, NSIS, distribution or terms changes
require targeted reassessment; all objective checks and residual uncertainty
remain. The `dispatch` notice blocker
is resolved under the documented interpretation of its explicit MIT declaration:
the bundle now preserves its original manifest/author metadata and reproduces
the standard MIT permission, condition and disclaimer, without inventing a
copyright notice. This is an evidence-based compliance interpretation submitted
for maintainer review, not authoritative legal approval. See
[section 3 of the evidence record](license-blocker-evidence.md#3-dispatch-020-resolved-under-the-documented-interpretation)
for sources, notice obligations and limits. The original plugin investigation
is historical; the replacement has a complete locked dependency inventory.

The official AppImage runtime now has a usable recipient source/relink kit under
the documented LGPL-2.1 section 6(a) interpretation. Source/version identities,
upstream build scripts, applicable patches/notices and a modified-libfuse runtime
launching the same AppDir in both modes are recorded in the
[official-runtime validation](appimage-upstream-runtime-validation.md). Exact
historical APK/CRT object reconstruction and byte-identical output are not
requirements of this source-based approach. GNU CRT reference source and the
eligible-compilation interpretation remain documented; no license is waived.
Ubuntu 24.04/Debian 13 userspaces and native Forky GNOME Wayland/XWayland were
checked separately. Native stable-distribution desktops and automated native
Wayland normal close remain explicit test limits. The demonstration is not legal
approval. The pre-existing extraction cleanup defect is a separate approved
behavioral limitation, not a licensing-risk waiver.

The 1.0.0 helper source/license assessment is historical; its evidence remains
in [the tagged validation record](https://github.com/ChrisLauinger77/code-convoy/blob/v1.0.0/docs/nsis-helper-validation.md).
The current installer ships neither the custom helpers nor the retired upstream
AppID/Tauri binaries. Existing NSIS notices, sources, Microsoft terms and final
payload checks remain in force; no new licensing clearance is inferred.

1. **Windows CRT.** Windows packaging now requires an application-bound
   `/VERBOSE:LIB` capture of the selected toolset/SDK libraries, hashes and
   archive members. Final installer PE inspection and final package
   hashes are retained separately from the six candidate assets. The capture
   is evidence, not redistribution approval. The current distribution policy uses
   dynamic CRT linkage for the application with user-supplied x64 runtime DLLs.
   Custom helpers and their separate linker evidence are removed. Historical
   static-link evidence is retained as historical evidence. See the Windows
   section of [the evidence record](license-blocker-evidence.md#5-windows-static-crt-unresolved)
   for actual observations, Microsoft references and the repeatable checks.
   The application remains MIT; that license does not grant rights to Microsoft
   runtime code. The [implemented maintainer decision](windows-licensing-decision.md#implemented-maintainer-risk-decision--2026-10-16)
   records `windows-crt` as an accepted interpretation risk, with Microsoft-only
   operative recipient/distributor terms and pre-use notice. Permission and
   agreement effectiveness are not conclusively established. The prior demand
   for an express grant covering every VC startup object is no longer an
   absolute internal blocker. Known mandatory conditions, distributor duties,
   notice/source integrity and final-package checks remain enforced. This is
   the sole accepted licensing risk; it does not waive another obligation.
   The current [dynamic CRT record](license-blocker-evidence.md#7-dynamic-crt-and-user-supplied-prerequisites)
   distinguishes user-supplied DLLs, embedded application startup code,
   prebuilt NSIS inputs and hosted-toolchain entitlement.

Primary AppImage evidence: [runtime source/tag](https://github.com/AppImage/type2-runtime/tree/20251108),
[libfuse terms](https://github.com/libfuse/libfuse/blob/fuse-3.15.0/LICENSE),
[LGPL-2.1 text](https://github.com/libfuse/libfuse/blob/fuse-3.15.0/LGPL2.txt).
Installer evidence: [NSIS licenses and linking exception](https://nsis.sourceforge.io/License),
[ApplicationID upstream links](https://nsis.sourceforge.io/ApplicationID_plug-in),
[nsis-tauri-utils 0.2.1](https://github.com/tauri-apps/nsis-tauri-utils/tree/nsis_tauri_utils-v0.2.1).

## Ordinary CI and release validation

Routine Renovate updates do **not** require regenerating or manually reviewing
the complete 1.0.0 audit. The former unconditional Cargo.lock equality and
dependency/feature fingerprint checks made ordinary updates fail even when no
license obligation changed. These synchronization checks now run only when
preparing release materials or checking publication readiness.

| Ordinary pull-request CI | Final release validation |
| --- | --- |
| Formatting, strict Clippy, Rust tests, platform/process safety tests, release compilation and packaging regressions remain enabled. | The same checks plus existing native package/startup checks remain enabled. |
| `licenses.py check-dependencies` reads current `cargo metadata --locked --all-features --format-version 1`, without a platform filter, once on Linux. It checks dependencies for every target, including build/dev inputs, rather than trusting old catalog versions or target exclusions. | `prepare` and `release-ready` require the catalog to match locked crate versions, registry sources/checksums and dependency/feature coverage. Refresh affected entries once during release preparation. |
| Known notice-only license choices pass without catalog edits. The policy supports MIT, Apache-2.0 (including its LLVM exception), 0BSD, BSD-2/3-Clause, ISC, Zlib, BSL-1.0, Unlicense and Unicode-3.0. AND obligations accumulate; OR selects a supported alternative (including Apache for `self_cell`). | Actual notices are retained, including nested font/protocol copyrights and existing upstream-recovered texts; a supported SPDX declaration alone is not notice delivery. Use the existing collector to update changed entries and texts. |
| New/changed copyleft, restricted, custom, unknown or license-file-only declarations fail with the affected crate named. Existing nonstandard reviews, currently MPL and fonts, carry forward only at the same source/version/checksum and terms. Git/path dependencies require a targeted source review. | Exact MPL sources, AppImage relinking inputs, NSIS sources, Rust library notices and Microsoft terms remain checksum-bound and delivered. Missing/altered materials and unexpected bundled ELF/PE/Mach-O binaries still fail final extraction checks. |
| Retained evidence integrity and relevant release-tool/runtime pin checks remain covered by packaging tests. No manual complete-inventory refresh is needed for ordinary dependency version changes. | Compiler/runtime identity, GLIBC compatibility, original candidate checksums and provenance, all six assets and the two-stage candidate/publish workflow remain enforced. Publication still refuses recorded obligations. |

This is an automated declaration policy and omission check, not a legal
compliance certification or an exhaustive scan of source contents. Normal CI
does not download/reconstruct the historical native audit. The existing
standard-library Python scripts and Cargo metadata are used; no license
management framework or new dependency is introduced.

## When to review provenance

Review the **affected component**, not the whole historical audit, when release
Rust/compiler libraries, cargo-packager/NSIS templates or plugins, AppImage
build tools/runtime, bundled native code/assets, retained source/relink recipes,
governing terms or redistribution arrangements change. Contrary facts and new
obligations also require review. Ordinary crate version bumps using the supported
license choices need current notices at release time, not a repeat of unrelated
native provenance investigations. CI runner/Python/action updates alone do not
invalidate the native audit; changes to the actual release SDK/toolchain or
licensing channel still need the relevant compatibility/provenance review.

## Refreshing release materials

For unchanged Windows components, reuse the final review and retain automated
notice, source and binary/package checks. Revisit affected licensing questions
when toolchain/components, the runner licensing channel, governing terms,
bundled Microsoft payloads or recipient/distribution arrangements change, or
when a concrete contrary fact appears. Do not repeat the historical audit or
require external confirmation for every application release. The following
collection steps apply to changed inputs during release preparation; they are
not a requirement to reopen unchanged dependency decisions or edit the snapshot
in every dependency PR. Keep existing evidence and update affected entries only.

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
5. Refresh the release snapshot's dependency-input fingerprint after checking
   target coverage, using the existing `dependency_inputs`/SHA-256 calculation
   in `licenses.py`. This is release inventory maintenance, not a new native
   audit. Run `python3 packaging/licenses.py check-dependencies`,
   `python3 packaging/licenses.py check`, all packaging tests and required
   Rust checks. After merge, produce a fresh non-publishing candidate from `main`
   and inspect all six final formats. Update the verification record and resolve
   remaining blockers before running `python3 packaging/licenses.py release-ready`.

On 2026-10-17, Cursor PR 2's Windows cleanup fix explicitly declared `Win32_Foundation`,
`Win32_Security` and `Win32_System_JobObjects` on the existing `windows` 0.62
dependency. These APIs were already enabled by locked process-wrap 10.0.1.
Before/after `cargo metadata --locked --filter-platform` exports for all four
release targets showed identical non-root dependency versions, features and
edges. No crate, source, native payload or notice coverage changed. The catalog's
manifest fingerprint was updated for the explicit declarations. Under the
current policy that fingerprint is checked only during release preparation;
ordinary CI verifies current license declarations and the retained evidence.

On 2026-10-19, the explicit About update check added `ureq` 3.4.2 (no default
features, `rustls-no-provider`), `rustls` 0.23.46 (`ring` only),
`rustls-native-certs` 0.8.5 and a direct declaration of the already locked
`semver` 1.0.28. The lockfile gained 19 crates: `ureq`, `ureq-proto`, `http`,
`httparse`, `utf8-zero`, `rustls`, `rustls-pki-types`, `rustls-webpki`, `ring`,
`untrusted`, `subtle`, `zeroize`, `getrandom` 0.2.17, `rustls-native-certs`,
`openssl-probe` (non-macOS Unix), `security-framework`/`-sys` and
`core-foundation` 0.10.1 (macOS), and `schannel` (Windows). Their declarations
are MIT/Apache-2.0/ISC/BSD-3-Clause choices; `ring` is **Apache-2.0 AND ISC**.
`licenses.py check-dependencies` accepts all of them, so no catalog entry was
edited under the ordinary-PR policy. `ring` statically compiles bundled C and
assembly derived from BoringSSL and other upstreams; the next release refresh
must collect its `LICENSE`, `LICENSE-BoringSSL`, `LICENSE-other-bits` and nested
`third_party` notices with the other crate texts. No ELF/PE/Mach-O payload or
Linux system library is added; macOS Security.framework and Windows certificate
store APIs are operating-system components. ureq's default TLS features were
avoided because they select `webpki-roots` (CDLA-Permissive-2.0), outside the
notice-only policy; `rustls-platform-verifier` would similarly pull
`webpki-root-certs` for wasm targets, which the all-target check reports.

On 2026-10-19, eframe PR #92 upgrades the font package from 0.36.2 to
0.37.0. Its registry archive SHA-256 is
`cfc1cc505827f4c23eedc297975a0fd038268d41293f389dd8ff3b574aefab19`,
verified against Cargo.lock; its recorded upstream revision is
`7c0b40402271e8d40cda69f87cf3c2617c2e4382`. The four existing TTF files
and their four notice files are byte-identical to 0.36.2. The added
`egui-icons.ttf` is an upstream five-glyph subset of emoji-icon-font under
its existing MIT terms; its SHA-256 is
`122806555c73df7cef55aa1d945289ae27fbc4dbbd31abbb4204ee0cda829cc8`.
The exact `fonts/egui-icons.txt` attribution/regeneration note and original
MIT notice are retained for that asset. The focused catalog update changes
only this crate and its font components, and adds one content-addressed text;
other crate records remain the reviewed release snapshot. The notice collector
now retains the subset's attribution file. The direct egui declaration enables
`monochrome_emoji_fonts` to preserve the previous embedded-font coverage with
eframe defaults disabled; the native clipboard feature is also explicit.
Release-wide inventory refresh and final-package verification remain required
under the existing release policy.

On 2026-10-20, the 1.3.0 release preparation refreshed the snapshot for these
changes and zbus 6. `ring`'s license files, its nested once_cell and fiat
notices, and the ISC header blocks of its Intel- and Google-copyrighted C files
are retained. AccessKit's `LICENSE.chromium`, which its Chromium-derived files
reference, is now retained as well. See the
[1.3.0 release refresh](release-1.3.0-license-validation.md). Fresh candidate
packaging and final-package verification remain required.

On 2026-10-22, the 1.3.1 release preparation refreshed the snapshot for routine
lock-file and dependency updates: tokio 1.54.1, ureq 3.5.0, ksni 0.3.7 and
related crates. ksni now brings `arc-swap` and `crossbeam-queue` to Linux, and
`rustversion` becomes a Linux build-only input. See the
[1.3.1 release refresh](release-1.3.1-license-validation.md).

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
section above records the subsequent `windows-crt` risk decision. No tag or public
release was created by that verification.
