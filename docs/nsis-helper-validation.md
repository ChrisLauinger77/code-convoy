# NSIS helper replacement — CodeConvoy 1.0.0

Status: replacement source/notices and the exact candidate payload have been
verified; `nsis-plugins` is removed from the inventory. `windows-crt` remains a
publication blocker. Native Unicode-path regression verification is still in
progress, so this change is not yet ready for merge. No release or tag is
authorized by this record. AppImage/LGPL and dispatch resolutions are unchanged.

## Required functionality and implementation

cargo-packager **0.11.8** calls only these functions for CodeConvoy:

| Original helper | Used functionality | Replacement |
| --- | --- | --- |
| ApplicationID | `Set` on desktop and Start Menu `.lnk` files | Original `CodeConvoyAppId.dll`, one export, project MIT |
| nsis-tauri-utils 0.2.1 | `FindProcess`, `KillProcess`, `SemverCompare` | `codeconvoy_utils.dll`, reduced upstream facade, MIT OR Apache-2.0 |

The new shortcut helper uses documented
[ShellLink/property-store APIs](https://learn.microsoft.com/en-us/windows/win32/shell/appids)
to write `System.AppUserModel.ID` and save the existing link. It retains
`io.github.chrislauinger77.code-convoy`; neither application startup nor its
notification implementation changes. No ApplicationID source is copied or
relicensed. The helper returns a checked status and releases its COM interfaces
and consumed NSIS stack nodes. It does not register accounts, pin shortcuts,
introduce notification features or change uninstall behavior.

The process and SemVer source files are byte-identical to Tauri
revision `c3a4447060a260c5e4e09d94284948c4f864da02`. Their actual MIT and Apache
notices are retained. A recorded `pluginapi-stack-safety.patch` addresses the
five PR CodeQL pointer findings with checked local pointer snapshots, checked
allocation, correctly sized UTF-16 buffers, bounded copies and node release on
invalid Unicode. Original/modified hashes and the exact patch are retained.
Native tests cover null/empty stacks, invalid capacities, truncation, Unicode,
invalid surrogates and ownership. The synchronous NSIS host still owns validity
of non-null pointers; the adapter is not a concurrent API. GitHub CodeQL
subsequently marked all five PR findings fixed (alerts 1–5); none was dismissed.
The new facade excludes the unused downloader and its
HTTP/TLS dependencies. Process name matching, host-PID exclusion, termination
behavior and success/error codes are unchanged. SemVer uses **1.0.18**, including
its ordering of invalid versions and build metadata. No application dependency,
agent behavior, scheduler or worktree implementation changes.

The complete helper lockfile includes three source dependencies:
`semver 1.0.18`, `windows-sys 0.48.0`, and `windows-targets 0.48.5`. The seven
architecture-specific Windows import-library packages remain identified in the
lockfile/inventory but are not linked into the shipped helpers. `windows_raw_dylib` generates imports
from the pinned bindings. One recorded source patch removes windows-sys's
obsolete `feature(raw_dylib)` attribute; this compiler feature has been stable
since Rust 1.71. Original archive, upstream revision and exact before/after
source hashes are recorded. No binding or API signature changes.

## Template and cache isolation

[The template](../packaging/nsis/installer.nsi) is copied from cargo-packager
0.11.8, with its upstream hash and notices. Changes add the independent plugin
directory, rename the five helper call sites and consume/check shortcut status.
Per-user scope, application identity, resources, registry values, upgrade and
downgrade selection, shortcuts and removal/data protection remain upstream.

cargo-packager still downloads its legacy helper cache and checksum-repairs
`nsis_tauri_utils.dll`. The replacements deliberately have different basenames
and live outside that cache. No cache DLL is replaced or patched. Final installer
extraction checks the full native payload set and exact DLL hashes; old or
unexpected plugins fail even when the installed application itself matches.
The portable ZIP still contains the same CodeConvoy.exe and existing notice
files, with no helper DLLs. Its source/notice bundle necessarily includes the new
source material shared across formats. Linux/macOS packaging code is unchanged.

## Source, build and license evidence

The distributed `codeconvoy-nsis-helpers-1.zip` contains original/reduced source,
Cargo.lock, dependency inventory, patch, notices, template and build scripts.
The three original registry source archives accompany it. The existing Rust
**1.99.0** standard-library notices and separate library lockfile apply to the
rebuilt helper too. [The kit README](../third-party/nsis/README.txt) gives the
build commands, API references and maintenance constraints.

The native builds select **MSVC 14.51.36231**, linker **14.51.36260.0**, and
Windows SDK **10.0.26100.0**, targeting **i686-pc-windows-msvc**. x86 plugins are
required by NSIS even for the x64 application. Evidence records compiler/linker
hashes, the exact Rust compiler identity, commands, flags, selected native
archives and members, response files, link maps, exports, imports and DLL hashes.

`CodeConvoyAppId.dll` uses `/NODEFAULTLIB /NOENTRY` and selects only SDK
kernel32/ole32 import libraries and UUID data; no CRT. The Rust helper retains
static linkage and selects x86 `libcmt`, `libvcruntime` and `libucrt`. The existing
Windows CRT inventory now binds each replacement's build evidence to the actual
installer payload and package hashes. CodeConvoy.exe's x64 CRT linkage is
unchanged. Microsoft's redistribution conditions remain a separate unresolved
publication obligation; an import list or successful link is not a license grant.

| Helper | SHA-256 |
| --- | --- |
| CodeConvoyAppId.dll | `3f4dc691bc1e9b4cff12ad5199b6330303249cddcd49d6ffddbd27e0efeb3c29` |
| codeconvoy_utils.dll | `218b7639ee9647cf34f26ad907c7a3f245be5a7360fa75e176fed231d62e22b2` |

[Native helper run 38899118720](https://github.com/ChrisLauinger77/code-convoy/actions/runs/38899118720)
built each helper twice into separate empty Cargo/native output trees. Both DLLs
were byte-identical between builds. The shortcut helper also matched the earlier
independent runner's output. `/Brepro` suppresses timestamp-based PE differences;
source paths are remapped. This demonstrates clean-build repeatability with
these selected inputs, not identical output across arbitrary SDK/compiler
updates. Raw evidence is retained in
[nsis-build-evidence.zip](../third-party/nsis-build-evidence.zip) and indexed by
[nsis-build-audit.json](../third-party/nsis-build-audit.json).

## Validation and remaining checks

Local Linux checks passed: application formatting, strict Clippy, all Rust tests,
release build, and helper workspace formatting. The application suite completed
with **621 passed, 10 ignored**; **101 packaging tests** passed. A read-only `/tmp/.git` marker
in the execution environment initially caused existing repository-safety tests
to reject temporary paths; rerunning with `TMPDIR=/var/tmp` passed. No runtime or
test logic was changed to accommodate that environment.

Packaging tests cover helper identity/exports, hashes, two-build comparison,
source revisions, dependency/source/notice delivery, linker provenance,
unexpected/stale plugins, source-kit changes, unchanged portable layout and
publication refusal. The Windows CRT publication blocker remains required.

The [first candidate run](https://github.com/ChrisLauinger77/code-convoy/actions/runs/38895333211)
built and extracted the installer, verifying exact notices, sources, application
bytes and every embedded plugin. A second extraction on Linux, using the exact
candidate source and hash-reconstructed Windows checkout line endings, passed
the same checks. The old DLLs were absent before their catalog entries were
removed. Default-path installation, shortcut identity, maintenance with the
real application running, uninstall and data preservation passed. Unicode-path
testing exposed a shortcut path comparison failure; the test now expands Windows
short paths and reads the Shell Unicode target property instead of the legacy
WScript reader, which returned `??` for the Chinese path. Native revalidation is
pending; this run is not reported as a successful overall CI run.

The native Windows tests exercise default
and Unicode/space-containing per-user paths, registration, shortcut targets and
persisted AppUserModelID, process detection/termination, install while the actual
CodeConvoy process is running, upgrade/downgrade registration scenarios,
uninstall and user-data preservation. Upgrade/downgrade cases use synthetic
prior `DisplayVersion` values with the same application bytes; they are not a
historical-installer compatibility claim. Separate NSIS ABI fixtures cover
multiple/missing processes, case-insensitive names, version edge cases and
stack balance. Tests refuse existing installations/processes/shortcuts and run
only in disposable Windows Actions accounts.

Interactive maintenance choices, taskbar pinning/grouping, toast activation and
manual desktop operation remain unperformed. Persisted shortcut identity is
automatically testable; those shell interactions must not be reported as tested
from that property check alone. Silent uninstall tests use NSIS `_?=` so the
caller can wait for completion; the uninstaller’s final self-deletion is not
exercised in this mode.

Any helper source, dependency, compiler, SDK or template update requires new
source/notice review, two clean builds, updated binary pins and final installer
verification. Never resolve `windows-crt` through this NSIS workflow, substitute
an old cached helper, silently accept changed hashes or publish from diagnostics.
