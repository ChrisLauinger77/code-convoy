# NSIS helper replacement — CodeConvoy 1.0.0

Status: replacement source/notices and the exact candidate payload have been
verified; `nsis-plugins` is removed from the inventory. `windows-crt` remains a
publication blocker. Native installer and helper regression checks, including
Unicode paths, passed. The changes are prepared for maintainer review; this
record authorizes no merge, tag or release. AppImage/LGPL and dispatch resolutions
are unchanged.

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
Cargo.lock, dependency inventory, patches, notices, template and build scripts.
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
publication refusal. The Windows CRT publication blocker remains required;
`python3 packaging/licenses.py release-ready` fails specifically on `windows-crt`.

[CI with the full Windows diagnostic](https://github.com/ChrisLauinger77/code-convoy/actions/runs/38899421790)
passed on Linux, macOS and Windows, including formatting, strict Clippy,
application tests and release builds. Its separately enabled Linux private-D-Bus
and Windows software-graphics startup checks also passed. The follow-up
[CodeQL run](https://github.com/ChrisLauinger77/code-convoy/actions/runs/38900088008)
passed all three analyses (Rust, Python and Actions). The regular
[PR CI matrix](https://github.com/ChrisLauinger77/code-convoy/actions/runs/38900092120)
also passed all three platforms on `5168223c5500b59c851a5c5efd0da8720ecfe748`.
The final follow-up commit contains only this validation document.

The [first candidate run](https://github.com/ChrisLauinger77/code-convoy/actions/runs/38895333211)
and independent Linux extraction proved the old DLLs absent before their live
catalog entries were removed. Its default-path native tests passed, but the
Unicode target comparison failed. A follow-up showed the legacy WScript reader
returning `??` for the Chinese path. The test now reads the Shell Unicode target
property and expands short paths. Neither failed run is claimed as overall CI
success.

The [final Windows candidate and native tests](https://github.com/ChrisLauinger77/code-convoy/actions/runs/38899421790)
passed on source **`3f72875547aacdb4cbb971f750a09dcb5fe214c9`**. Both helpers
were built twice again and matched the retained independent helper run's hashes.
The diagnostic artifact is `windows-crt-audit` (ID **12045189343**).
The exact downloaded installer and ZIP also passed independent Linux extraction
and byte verification against that source revision. Only checkout line endings
whose hashes matched the native capture were reconstructed. The complete
source/notice bundle, eight recorded PE payloads (including the installer),
helper/application linker evidence and final package hashes matched. The old
helper DLLs were absent; the portable ZIP contains no helper DLLs.

| Final candidate item | SHA-256 |
| --- | --- |
| `CodeConvoy-1.0.0-windows-x86_64-setup.exe` | `f2c0e6531efdf0616ab56c1808051c3052ac7f27ba515817de42911cd08e197f` |
| `CodeConvoy-1.0.0-windows-x86_64.zip` | `8815d5b3053f83dbc7cc149bef02f870ab8bfac0814bf871d041eb8d1a47e0a3` |
| `CodeConvoy.exe` in both packages | `c3c026ef1904f72b10cb13aa18017fb4639e63204672795d1502b23a42e5e522` |
| `THIRD-PARTY-SOURCES.zip` in both packages | `b253fcf0f891ff1bb20b1240b72e80c8a09a3ec760bc1d11c9ea9d4f9c603f31` |

The final hashes live in this external validation record to avoid a circular
reference inside the bundled inventory. Its `retired_helpers_absence_verification`
entry deliberately retains the historical first-candidate proof. Subsequent
commits only change documentation and Git whitespace checking; application
code, helper/template inputs and bundled source/notices are unchanged. A new
application build embeds its own Git revision and needs fresh artifact hashes.

The native Windows tests passed for default
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

## Review map

| Area | Changed files |
| --- | --- |
| Helper source and licenses | `third-party/nsis/`: original shortcut helper, reduced Rust workspace, lockfile, two recorded patches, input/dependency manifests and README/licenses |
| Build and payload verification | `packaging/nsis_helpers.py`, `packaging/windows_crt.py`, NSIS-only integration in `packaging/release.py`, `packaging/licenses.py` |
| Installer and native tests | `packaging/nsis/`: upstream-derived template, attribution and native fixtures; `packaging/windows-smoke.ps1` |
| Regression coverage | `packaging/test_nsis_helpers.py`, `test_licenses.py`, `test_release.py`; existing AppImage publication test now expects the remaining CRT blocker |
| Inventory and evidence | `third-party/catalog.json`, `texts.json`, notice/source introductions, helper audit/evidence, source kit and three dependency archives |
| Build automation | Windows helper/diagnostic steps in CI and Windows release job; byte-preservation attributes in `.gitattributes` |
| Maintainer documentation | This record, `docs/third-party-licenses.md`, `docs/license-blocker-evidence.md`, `docs/releasing.md` |

No application Rust code or application dependency versions changed. Linux/macOS
packaging implementation, AppImage/LGPL evidence, dispatch resolution and the
existing Windows CRT obligation are unchanged. No merge, tag or release was made.

## Dynamic CRT migration and prerequisite validation

The preceding sections record PR 73's static-CRT baseline. The subsequent 1.0.0
change selects `-C target-feature=-crt-static` independently for the application
and utility helper. The original AppId implementation, exports and binary remain
unchanged. The utility retains all three exports, existing source dependencies,
process/SemVer behavior and the Windows GlobalAlloc/GlobalFree stack interface.

`packaging/windows-runtime.json` records the observed CRT import lists and
prerequisite versions. The application imports VCRUNTIME140.dll plus UCRT heap,
locale, math, runtime, stdio and string l1-1-0 interfaces. The x86 utility imports
VCRUNTIME140.dll plus UCRT runtime and string l1-1-0 interfaces. Neither imports
VCRUNTIME140_1.dll or MSVCP140.dll. All other imports remain ordinary Windows API
dependencies. AppId imports only KERNEL32.dll and ole32.dll.

Both Rust outputs still embed startup/support members from msvcrt.lib and use
vcruntime.lib/ucrt.lib import libraries. Builds reject libcmt.lib,
libvcruntime.lib, libucrt.lib and libcpmt.lib; changing linkage does not eliminate
Microsoft code or the publication blocker. See the
[licensing distinction](license-blocker-evidence.md#7-dynamic-crt-and-user-supplied-prerequisites).

The NSIS gate uses actual DLL versions, PE architecture and x86 loadability;
registry claims cannot establish availability. System32 redirection is handled
through explicit SysWOW64/Sysnative paths. Checks run before the utility helper
and repeat before version/process operations. Missing prerequisites produce
Microsoft's official download-page guidance and error 1603. Silent/passive
operation has no blocking dialog. Uninstall requires x86 only; it must still
perform process checks. Invalid or missing plugin results fail safely rather than being
interpreted as no running process. The final prerequisite/process check runs
before the installation directory is created.

The native fixture compiles the same prerequisite include with private test
paths. That substitution is compile-time only and is not exposed in the shipped
installer. It copies genuine system runtime files into a disposable directory,
withholds private copies for missing/stale cases, and changes test version
thresholds for older/newer cases. It never deletes system DLLs, uninstalls a real
runtime or changes real runtime registration. The fixture DLL copies are removed
and never uploaded or packaged. Registry cases use isolated test keys in both
views; the production checker deliberately ignores registry claims altogether.

The simple portable ZIP gains no launcher or runtime files. It requires the x64
runtime before startup. Packaging rejects separate VC/UCRT DLLs and VC redist
installers; the existing closed native payload allowlist also rejects unexpected
or renamed binaries. The windows-crt publication guard and two-stage release
architecture remain unchanged.

### Exact import inventory for the dynamic binaries

The import names below preserve the spelling of the native PE inspection;
Windows resolves DLL names case-insensitively. The UCRT API-set contracts resolve
to the operating system's UCRT implementation (`ucrtbase.dll`); they are not
extra DLLs to copy into the package.

| Binary | Direct DLL imports |
| --- | --- |
| `CodeConvoy.exe` (x64) | `KERNEL32.dll`, `USER32.dll`, `VCRUNTIME140.dll`, `advapi32.dll`, `api-ms-win-core-synch-l1-2-0.dll`, `api-ms-win-core-winrt-l1-1-0.dll`, `api-ms-win-crt-heap-l1-1-0.dll`, `api-ms-win-crt-locale-l1-1-0.dll`, `api-ms-win-crt-math-l1-1-0.dll`, `api-ms-win-crt-runtime-l1-1-0.dll`, `api-ms-win-crt-stdio-l1-1-0.dll`, `api-ms-win-crt-string-l1-1-0.dll`, `bcryptprimitives.dll`, `combase.dll`, `comctl32.dll`, `dwmapi.dll`, `dxgi.dll`, `gdi32.dll`, `imm32.dll`, `kernel32.dll`, `ntdll.dll`, `ole32.dll`, `oleaut32.dll`, `opengl32.dll`, `setupapi.dll`, `shell32.dll`, `shlwapi.dll`, `uiautomationcore.dll`, `user32.dll`, `uxtheme.dll` |
| `codeconvoy_utils.dll` (x86) | `KERNEL32.dll`, `VCRUNTIME140.dll`, `api-ms-win-core-synch-l1-2-0.dll`, `api-ms-win-crt-runtime-l1-1-0.dll`, `api-ms-win-crt-string-l1-1-0.dll`, `kernel32.dll`, `ntdll.dll` |
| `CodeConvoyAppId.dll` (x86, unchanged) | `KERNEL32.dll`, `ole32.dll` |

The configured minimum is VC v14 **14.51.36247.0**, independently for x64 and
x86, plus the Windows UCRT **10.0.10240.0** or newer. This conservative deployment
floor uses the native runtime DLL version verified with the recorded build
toolset; it is not a measured oldest-export compatibility promise. The
14.51.36260.0 linker component version is not used as a runtime DLL threshold.
The installer needs both architectures, removal needs x86, and the portable application needs only x64. `VCRUNTIME140_1.dll` and
`MSVCP140.dll` are not direct dependencies of either Rust output and are not
added to the prerequisite policy.

### Files changed for the dynamic CRT work

| Area | Files |
| --- | --- |
| Release linkage and native evidence | `packaging/build.py`, `packaging/windows_crt.py`, `packaging/nsis_helpers.py`, `third-party/nsis/inputs.json` |
| Prerequisite policy and existing installer | `packaging/windows-runtime.json`, `packaging/windows_runtime.py`, `packaging/nsis/prerequisites.nsh`, `packaging/nsis/installer.nsi`, `packaging/nsis/UPSTREAM.json` |
| Packaging and runtime regression checks | `packaging/licenses.py`, `packaging/release.py`, `packaging/windows-smoke.ps1`, `packaging/nsis/prerequisite-tests.nsi`, `packaging/nsis/prerequisite-tests.ps1`, `packaging/test_windows_runtime.py`, `packaging/test_windows_crt.py`, `packaging/test_nsis_helpers.py`, `packaging/test_licenses.py` |
| Inventory and reproducible source kit | `third-party/catalog.json`, `third-party/nsis/README.txt`, `third-party/nsis-build-audit.json`, `third-party/nsis-build-evidence.zip`, `third-party/sources/codeconvoy-nsis-helpers-1.zip`, `third-party/evidence/windows-dynamic-crt-38919332474.zip` |
| CI and stable input bytes | `.github/workflows/ci.yml`, `.gitattributes` |
| User guidance and assessment | `README.md`, `docs/releasing.md`, `docs/third-party-licenses.md`, `docs/license-blocker-evidence.md`, this document |

Application Rust code, backends, dependencies, scheduler, worktrees, Linux/macOS
packaging and the two-stage release workflow are unchanged. Validation commits
are confined to the authorized `codex/windows-dynamic-crt-validation` branch;
there is no release tag, PR, merge or publication from this work.

### Validation limits

The hosted Windows job is automated native execution on Windows Server 2025;
it does not certify interactive operation on Windows 10/11. Missing-runtime
fixtures exercise the shipped checker with private file paths while the host's
real runtimes remain installed. Older/newer cases vary the required version,
not Microsoft's installed binaries. The x86 host checks x64 file version and PE
architecture but cannot load x64 DLLs; the actual x64 application startup test
provides the positive loading check. The gate is not a complete Windows image
integrity validator.

Interactive prerequisite-message presentation, wizard choices, taskbar behavior,
and clean consumer Windows desktops remain untested. Maintenance tests simulate
prior version registration using the same executable bytes, as in the existing
smoke suite. Silent uninstall uses `_?=` so its process can be awaited, leaving
uninstaller self-deletion outside this check. No genuine system DLL, runtime
installation or real runtime registry key is removed for testing.

### Retained dynamic linker reference

[CI diagnostic 38919332474](https://github.com/ChrisLauinger77/code-convoy/actions/runs/38919332474)
built source `ebf180f6ba28b0ca61ac72fb286456dcbf8496dd` with Rust 1.99.0,
VS Enterprise 18.10.12224.181, MSVC 14.51.36231, LINK 14.51.36260.0 and SDK
10.0.26100.0 on image `win25-vs2026/20261007.264.1`. Its application link,
portable startup/normal close, two matching helper builds and helper Rust tests
passed. Independent extraction verified both package contents, all eight native
PE hashes, sources/notices and linker provenance against that exact revision.

That diagnostic's installer rejected the original DLL minimum of 14.51.36260.0:
the actual x86 and x64 VCRUNTIME140.dll files are 14.51.36247.0. The corrected
policy uses the verified runtime-file floor; linker, DLL and Redistributable
package version numbers must not be conflated. Microsoft recommends the latest
[compatible v14 Redistributable](https://learn.microsoft.com/en-us/cpp/windows/latest-supported-vc-redist)
for the selected toolset. The observed UCRT files are 10.0.26100.33438.
This reference proves the link and package identities, not a passing installer
run; the final corrected run is recorded separately below.

The [retained archive](../third-party/evidence/windows-dynamic-crt-38919332474.zip)
contains 20 original diagnostic files: provenance, toolchain, linker trace/map,
response file, selected libraries, system runtime hashes/versions and final PE
inspections/package manifest. It contains no executable or Microsoft DLL,
library or object file. SHA-256:
`e246ba88023ed4cd631ec94311f4c781424a310c7ec17196a30875e8ad6dfdf4`.
The catalog retains this reference separately from the earlier static audit.

The application selects 34 distinct msvcrt.lib member names (including startup,
TLS, exception/security support and allocation support); the utility selects
24 startup/support member names, retained in its two-build evidence. These counts describe linker
selection, not a claim that all loaded objects survive optimization. Both use
vcruntime.lib and ucrt.lib import libraries; neither selects libcmt.lib,
libvcruntime.lib, libucrt.lib or libcpmt.lib. The application also uses SDK
kernel32/ntdll/user32 import archives and the existing windows.0.52.0 import
archive. AppId still uses SDK kernel32/ole32 imports and UUID data.

The remaining prebuilt NSIS payloads have these direct imports. Their pinned
plugin bytes are unchanged; these inspections establish imports, not complete
static-library provenance.

| Native component | Direct DLL imports |
| --- | --- |
| Setup stub and `uninstall.exe` | `ADVAPI32.dll`, `COMCTL32.dll`, `GDI32.dll`, `KERNEL32.dll`, `SHELL32.dll`, `USER32.dll`, `ole32.dll` |
| `nsDialogs.dll` | `GDI32.dll`, `KERNEL32.dll`, `SHELL32.dll`, `USER32.dll`, `comdlg32.dll`, `ole32.dll` |
| `StartMenu.dll` | `GDI32.dll`, `KERNEL32.dll`, `SHELL32.dll`, `USER32.dll`, `ole32.dll` |
| `System.dll` | `KERNEL32.dll`, `USER32.dll`, `ole32.dll` |

| Selected startup archive | Distinct loaded member names | SHA-256 |
| --- | ---: | --- |
| MSVC x64 `msvcrt.lib` | 34 | `d3f725923c6dd639bb5e5c52f3122c6fe084471f7ab0ef5f27d054f56b4d8a7e` |
| MSVC x86 `msvcrt.lib` | 24 | `5b0626d52414ac346eda0c19d6c4417411a232505748b56abf866df9af2e38d9` |
