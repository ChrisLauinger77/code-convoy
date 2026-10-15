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

The initial collection contains **327 distinct notice texts**, deduplicated by
their content hashes in [texts.json](../third-party/texts.json). It retains
upstream copyright statements, notices, font licenses and protocol XML copyright
blocks. When a crate omits a license file, the evidence refers to its exact
`.cargo_vcs_info.json` upstream revision, rather than an unversioned default
branch. Notice text is decoded as UTF-8 (removing a BOM where present); source
archives retain their original bytes.

The inventory also identifies **19 assets, runtimes and packaging components**,
with unresolved versions expressly marked. Rust's separate standard-library
report covers **141 out-of-tree entries** plus in-tree exceptions; its complete
upstream HTML is included, with those component/version/license entries indexed
in the catalog. It was recovered from the checksum-verified Rust **1.99.0**
compiler distribution. That report conservatively includes tooling and targets
beyond CodeConvoy; the compiler itself is not shipped in the application.

| Component group | Distribution and conditions addressed |
| --- | --- |
| Rust crates, mostly MIT/Apache/BSD/ISC/Zlib/BSL/Unlicense | Preserve actual license/copyright/NOTICE texts. Retain OR alternatives; `self_cell` uses Apache rather than its GPL alternative. AND clauses are cumulative. |
| `epaint_default_fonts` 0.36.2 | Unmodified embedded Hack, Noto Emoji, Ubuntu Light and emoji-icon-font bytes; all four font notices included. Hack includes Bitstream-derived terms; OFL and Ubuntu font naming/reserved-name conditions remain applicable. |
| Unicode | Unicode-3.0 texts for 18 Unicode-licensed crates, the combined `unicode-ident` expression, and Rust standard-library Unicode exceptions. |
| `option-ext` 0.2.0 | MPL-2.0 notice and exact unmodified `.crate` source archive; checksum must equal Cargo.lock. |
| AppImage runtime 20251108 | MIT launcher at `dd6cebedcbddde9c82f89b011e8e1d40b6e43868`; static libfuse, squashfuse, musl, zstd, zlib and mimalloc. Source/version gaps remain below. |
| NSIS 3.09 | Installer/uninstaller stub, standard plugins, artwork and LZMA decoder; original COPYING and full official source archive included. LZMA's CPL-1.0 linking exception does not relicense CodeConvoy. |
| Windows additional plugins and static CRT | Identified separately; not assumed to inherit the project's MIT license. Outstanding evidence below. |
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
its upstream runtime patch/build recipe, squashfuse 0.5.2 and official NSIS 3.09
source. Each archive has a recorded digest and origin. The available AppImage
source is **not claimed to be a complete LGPL relinking kit**.

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
roughly 9 MB before package compression; source archives account for most of it.

## Unresolved release obligations

These are tracked in `catalog.json:release_blockers`; `publish.py` checks them
before creating even a draft release. Non-publishing candidate builds remain
available for inspection. Removing a blocker requires new evidence and review.

1. **AppImage static-library versions.** The exact runtime recipe uses mutable
   Alpine 3.21 packages for musl, zstd, zlib and mimalloc. Its 2025 build log is
   expired (GitHub returns 410). The included reference notices are labelled
   with reference versions, not asserted to identify the shipped binaries.
   Obtain exact build provenance and matching notices. The upstream LICENSE
   lists several dependencies but omits mimalloc, which its Makefile links.
2. **AppImage LGPL relinking.** The runtime statically links patched libfuse.
   LGPL-2.1 section 6 requires more than adding a license text. Verify a complete
   corresponding source/build or object/relink set for the shipped runtime,
   including the necessary build inputs, and demonstrate relinking. Current
   archives preserve the known source/patch, but mutable dependencies and absent
   build evidence prevent claiming this obligation is fully satisfied.
3. **`dispatch` 0.2.0 notice.** Its crate and pinned upstream revision declare
   MIT and identify Steven Sheldon, but contain no copyright/license file.
   Obtain authoritative notice evidence; no copyright wording has been invented.
4. **NSIS plugins.** The prebuilt ApplicationID archive contains only DLL/PDB
   files. The linked source identifies Mike Anchor/Mozilla/Connectify
   contributions without a clear license grant. Establish the exact binary's
   source/license. The `nsis-tauri-utils` 0.2.1 root MIT/Apache texts are included,
   but its prebuilt DLL's transitive dependencies and build toolchain remain to
   be established; the source tag has no Cargo.lock. Plugin hashes prevent an
   unnoticed binary substitution during future candidate builds.
5. **Windows CRT.** Record the actual hosted toolset/SDK runtime versions and
   review applicable Microsoft redistribution rights for statically linked
   runtime code. The application MIT license does not grant those rights.
   Microsoft's [redistribution guidance](https://learn.microsoft.com/en-us/cpp/windows/redistributing-visual-cpp-files)
   points distributors to the applicable Visual Studio/SDK license terms.

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
   notices from a checksum-verified release archive. Recheck installer/runtime
   native inputs when changing packaging tools or runner toolchains. Do not
   replace unknown identities with guesses or generic SPDX templates.
5. Run `python3 packaging/licenses.py check`, all packaging tests and required
   Rust checks. Produce a fresh non-publishing candidate and inspect all six
   final formats. Update the verification record and resolve remaining blockers
   before running `python3 packaging/licenses.py release-ready`.

## Verification record

Local required Rust checks and the packaging regression suite have passed during
implementation. Final PR-head validation and fresh candidate artifact evidence
will be recorded here after the package build completes. Older candidate bytes
were used only to inspect installer layout, not to claim notice delivery by this
change. No tag or public release has been created.
