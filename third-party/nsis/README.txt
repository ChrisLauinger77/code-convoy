CodeConvoy NSIS helper source kit, build 1
========================================

Scope: CodeConvoyAppId.dll exports only Set. codeconvoy_utils.dll exports only
FindProcess, KillProcess and SemverCompare. Both are x86 Unicode NSIS plugins,
even though the installed CodeConvoy application is x86-64.

appid.cpp is original CodeConvoy code, MIT licensed; see LICENSE-CodeConvoy.
It loads the existing shortcut through IShellLinkW/IPersistFile, assigns
PKEY_AppUserModel_ID through IPropertyStore, commits and saves the shortcut.
It has no DLL entry point, C/C++ runtime, account registration, pinning,
notification activation or shortcut deletion functionality. Errors return 1,
success 0; the template consumes the result. The application identity stays
io.github.chrislauinger77.code-convoy. Application startup is unchanged.
API references:
https://learn.microsoft.com/en-us/windows/win32/shell/appids
https://learn.microsoft.com/en-us/windows/win32/properties/props-system-appusermodel-id
https://learn.microsoft.com/en-us/windows/win32/shell/links

The crates/nsis-process and nsis-semvercompare runtime sources are byte-for-byte
upstream nsis-tauri-utils 0.2.1 at revision
c3a4447060a260c5e4e09d94284948c4f864da02. pluginapi derives from that same source;
pluginapi-stack-safety.patch records the CodeConvoy 2026-10-16 modification:
validate local stack pointers and GlobalAlloc, use the correct UTF-16 element
size and bounded copies, and release a consumed node on invalid Unicode too.
Native unit tests cover null/empty stacks, invalid capacities, truncation,
Unicode and invalid-surrogate ownership. inputs.json records pre/post hashes.
The host still owns pointer validity and must call synchronously on one thread.
See LICENSE_MIT and LICENSE_APACHE-2.0; the original license terms are retained.
The original source archive is included separately. The modified workspace
keeps only these crates and a facade reexporting their functions. The unused
downloader and its HTTP/TLS stack are excluded. Manifests pin semver 1.0.18 and
windows-sys 0.48.0 exactly; Cargo.lock pins the full graph. Runtime process and
version semantics are intentionally unchanged, including upstream invalid
version ordering, build-metadata ordering, case-insensitive process names,
host-PID exclusion and return codes. No additional process framework is added.

dependencies.json inventories every registry lock entry. The three included
source crates and their actual notices are distributed. Architecture-specific
windows_* import archives remain locked but are not selected into the DLL:
the upstream windows-targets windows_raw_dylib configuration lets rustc generate
the imports. The linker gate rejects windows.0.* prebuilt import libraries.
windows-sys 0.48.0 still marks raw_dylib as an unstable compiler feature. The
single recorded windows-sys-stable-raw-dylib.patch removes that obsolete feature
gate (raw_dylib stabilized in Rust 1.71). The builder recreates a local path
dependency from the original checksum-verified crate and verifies the exact
before/after source hashes. Cargo.lock records the path replacement; the
inventory retains the original crate checksum and upstream revision. No API,
ABI or generated binding is changed. No nightly compiler or bootstrap is used.
Rust 1.99.0 library notices/lockfile are supplied by the existing distribution
inventory; they apply to this helper too. windows-sys/windows-targets and SemVer
are MIT OR Apache-2.0; both sets of notices are retained.

Build on Windows with Rust 1.99.0 (i686-pc-windows-msvc target), installed MSVC
14.51.36231 and Windows SDK 10.0.26100.0; inputs.json pins these versions.
From the repository root:
  rustup target add --toolchain 1.99.0 i686-pc-windows-msvc
  python packaging/nsis_helpers.py build
  python packaging/nsis_helpers.py verify
The source ZIP recreates the same third-party/nsis and packaging paths.
The builder selects the x86 MSVC environment explicitly, uses Cargo --locked,
static CRT only for the Rust helper, /NODEFAULTLIB /NOENTRY for the shortcut
helper, and /Brepro for both. It builds into independent empty output trees.
provenance.json binds source file hashes, locked packages, commands, Rust flags,
compiler/linker hashes, SDK versions, selected archive hashes/members, maps,
imports/exports, PE hashes and both clean builds. Unequal outputs fail.
Do not infer cross-toolchain reproducibility from two matching builds on one
runner. No compiler, SDK or Microsoft CRT license grant is supplied by this kit.

The Rust helper adds x86 static libcmt/libvcruntime/libucrt inputs to the existing
Windows CRT evidence. The shortcut helper uses SDK import/UUID libraries but
no CRT. Microsoft redistribution obligations remain separately unresolved.
CodeConvoy.exe's existing x64 CRT selection is unchanged. A source build alone
does not authorize Microsoft runtime redistribution or publication.

packaging/nsis/installer.nsi derives from cargo-packager 0.11.8 under its
preserved MIT/Apache terms. UPSTREAM.json records the original template hash.
Changes only select the uniquely named replacement DLLs from an independent
directory and check the shortcut helper result. cargo-packager still downloads
its legacy helper cache; those DLLs are not referenced by the custom template.
Never replace cache files: cargo-packager verifies/re-downloads the old helper.
Final extraction must reject old or unexpected plugins and compare all pinned
DLL hashes. Native tests use only a disposable Windows Actions account.
