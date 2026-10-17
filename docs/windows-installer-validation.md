# Windows installer without custom helpers

The installer keeps cargo-packager 0.11.8's supported custom-template mechanism.
`packaging/windows-installer.nsh` supplies the operations below using NSIS 3.09's
bundled headers and System plugin. There is no custom DLL, helper workspace,
dependency lockfile, patched dependency, source kit or helper build toolchain.

| Operation | Implementation and failure contract |
| --- | --- |
| Version comparison | Validate exactly three canonical decimal components, each 0–65535, then call `WordFunc.nsh` `${VersionCompare}`. Return equal/upgrade/downgrade or error. Validate installed registration in `.onInit`, including `/S`; unsupported versions fail with 1603 before changes. `windows.allowDowngrades=true` makes the previous effective policy explicit. |
| Process detection | `CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS)`, `Process32FirstW` and `Process32NextW`, including the first entry. Case-insensitive basename policy excludes `GetCurrentProcessId`. Only `ERROR_NO_MORE_FILES` ends enumeration successfully. Open matches with limited query/synchronize rights and verify their basename through `QueryFullProcessImageNameW` before acting. |
| Process termination | Open the verified process with termination rights; `TerminateProcess`, a bounded 10-second `WaitForSingleObject`, close handles, then re-enumerate. Denial, timeout, unexpected errors and surviving/new matches fail with 1603. No debug privilege or process-tree termination. GUI retains consent; `/S` and `/P` stop matching processes automatically. |
| Shortcut identity | `CreateShortcut`, `CoInitializeEx`, `IShellLinkW`/`IPersistFile`, `IPropertyStore::SetValue(PKEY_AppUserModel_ID)`, commit and save. `SHStrDupW` transfers an owned string to a PROPVARIANT; `PropVariantClear`, interface releases and balanced COM cleanup cover failure paths. Identity remains `io.github.chrislauinger77.code-convoy`; failure returns 1603. |
| Prerequisites | The application remains dynamically linked and requires user-installed x64 VC/UCRT. Check real file versions and PE architecture through Sysnative. No registry override, download or bundling. New uninstall needs no VC runtime. A released 1.0.0 uninstaller retains its own x86 prerequisite when invoked. |
| Removal | Check processes before deleting known application files. Keep per-user registry/shortcut identity, leave `appdataPaths` unset, and use nonrecursive directory removal. Failure to remove the executable stops cleanup. |

## Native acceptance gate

Before any helper infrastructure was removed, commit `e8b6938` passed the
[focused Windows run](https://github.com/ChrisLauinger77/code-convoy/actions/runs/39011781766).
The fixture covers numeric ordering including 1.9/1.10, malformed versions,
case-insensitive detection, multiple real x64 processes from the x86 NSIS host,
query and termination denial, an actual bounded wait timeout, confirmed
termination, unrelated-process preservation, self exclusion, Unicode shortcut
target/AppUserModelID readback, and silent failure propagation. The administrative
runner's inherited debug privilege is removed only in the test parent so denied
access exercises ordinary per-user permissions. Production never enables it.

The native fixture is a small NSIS executable compiled by the same NSIS compiler
as cargo-packager. The process fixture is the installed Windows `cmd.exe` held
open through its input pipe; no additional compiled binary is maintained.

## Integrated regression checks

`packaging/windows-smoke.ps1` covers fresh installation, `/D`, `/S`, `/P`, `/NS`,
`/R`, reinstall while running, malformed registration, shortcut identity/target,
uninstall while running, and preservation of user data and unrelated files.
`-UnicodePath` exercises non-ASCII paths.

`-ReleasedUpgrade` uses the actual published 1.0.0 installer, verified against
its published SHA-256. A test-only next-minor installer is compiled from the
current production template/configuration. Only that fixture's package version
changes; root Cargo files and distributables do not. The GUI path observes the
real legacy uninstaller child through Windows process events; the Unicode path
tests passive overwrite. Both then downgrade through the current installer.
Registry version edits are used only for the malformed-version rejection test,
not as a substitute for released-installer upgrade testing.

`packaging/windows_runtime.py test` exercises copied private runtime files for
missing, outdated, wrong-architecture and changed prerequisites, and removal
without runtime files. It never edits Windows system DLLs or runtime registration.
These fixtures verify checker behavior; they are not a clean-OS runtime rollout
test. The actual released uninstaller is run with the runner's installed x86
runtime; its binary and prerequisite checks are not patched or bypassed.

Final package verification retains application linker/import evidence, rejects
bundled Microsoft runtimes, checks notice/source bytes, and permits only the
reviewed standard `System.dll`, `StartMenu.dll` and `nsDialogs.dll` plugins.
Both custom and legacy AppID/Tauri DLLs are rejected. Required NSIS,
cargo-packager/template, Microsoft and application notices remain. The existing
version-bound licensing-risk decision is not expanded to a future release.

The integrated Windows checks passed at implementation commit
`d0c942155860cb37abc40e7e0dcba5b224fb0a93` in
[diagnostic run 39014148245](https://github.com/ChrisLauinger77/code-convoy/actions/runs/39014148245).
This includes fresh installation, the observed released 1.0.0 uninstaller,
Unicode passive overwrite, a real next-version install followed by downgrade,
running-process maintenance/removal, shortcut identity and targets, user-data
preservation, native failure cases, runtime fixtures and final payload checks.
The GUI fixture reads controls with bounded `WM_GETTEXT` and sends their
`BN_CLICKED` notifications; it does not depend on desktop activation.

The retained `windows-crt-audit` artifact binds these diagnostic packages to
the implementation commit (they are not a release candidate):

| Package | SHA-256 |
| --- | --- |
| Installer | `0e67a530c9572b853fb30747fcdf958d007c3a4ca3fcb9d92bec32a8adbe8992` |
| Portable ZIP | `f1f1b8ffd7620388aa59c47399af8a84a26da05d8bebd00a92a4bf47b640fd74` |

The extracted installer contains `CodeConvoy.exe`, `uninstall.exe`, CodeConvoy's
license, all four third-party material files, and only `System.dll`,
`StartMenu.dll` and `nsDialogs.dll` under `$PLUGINSDIR`. The portable ZIP contains
the application and the five license/material files. Its source ZIP's README
matches the corrected `third-party/SOURCE-README.txt`; the five retired helper
archives and their reconstruction instructions are absent. The NSIS source
archive remains. Final package checks also verify these same source bytes in
the installer and installed directory.

Local formatting, strict Clippy, all-feature Rust tests, release build and all
101 packaging tests passed. Cross-platform results for the same implementation
are recorded in [CI run 39014149051](https://github.com/ChrisLauinger77/code-convoy/actions/runs/39014149051),
covering Linux, macOS and Windows. The final documentation commit only records
these results and does not change the tested implementation.

## Maintenance

Maintain the custom template diff against `packaging/nsis/UPSTREAM.json`, the
small native operations include, and the focused behavior tests. Normal Rust
dependency updates no longer touch installer helpers, making the helper-only
updates in Renovate PR #75 obsolete. The application dependency graph is unchanged.
The two-stage release workflow, candidate provenance, checksums and publication
guards remain in place; only obsolete helper build/test/evidence steps change.
The published 1.0.0 release and its historical source/evidence remain unchanged.
In total, 32 tracked files are removed, including 20 helper-workspace files and
five source archives (about 3.9 MB across the removed files). cargo-packager
0.11.8 still populates its ordinary upstream plugin cache; unused legacy DLLs
in that cache are not embedded. No fork or cache-replacement mechanism is added.
