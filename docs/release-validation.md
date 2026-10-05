# v0.1.0 release audit and validation

## Audit before implementation

Reviewed Cargo metadata/lockfile, build script, all source areas, tests, CI,
Renovate preset, README/AGENTS, architecture, backend/UI validation records,
persistence, native picker, Git safety, process management, scheduler, platform
code and About. There were no release assets or release-related runtime TODOs.

Concrete gaps were: no native package configuration, release automation, desktop
entry or icon; Windows used the console subsystem; hidden-console startup needed
visible error reporting; revision tracking could watch unrelated `.git` changes.
The existing `0.1.0` version, offline About, state recovery and repository guards
were already suitable. No scheduler, persistence format, Git validation, backend
protocol, permissions or repository-safety behavior was changed.

The Windows audit confirmed use of process-wrap 10 Job Objects: the process is
started suspended, assigned before resume, and descendants are terminated through
the job. Individual Stop, Stop Convoy, Stop All and shutdown all reach the same
managed process cancellation. The new no-console flag uses its CreationFlags
wrapper, so it is not overwritten by JobObject's suspended creation flag. Native
Windows execution tests remain required; source inspection is not runtime proof.

## Local results, 2026-10-05

Host: Apple Silicon macOS. Application checks used the installed Rust 1.99.0;
both Universal slices were also built with isolated Rust 1.95.0, matching the
release workflow. No installed agent was invoked for an authenticated task.

| Check | Result |
| --- | --- |
| `cargo fmt --check` | Passed |
| `cargo clippy --locked --all-targets --all-features -- -D warnings` | Passed |
| `cargo test --locked --all-features` | 91 passed; 3 optional installed-CLI checks ignored |
| `cargo build --locked --release --bin codeconvoy` | Passed |
| Release-gate Python tests | Passed: exact tag, complete assets/checksums, empty/missing/extra assets, both architectures |
| Python syntax and actionlint 1.7.12 | Passed |
| Separate arm64 and x86_64 release builds | Passed with Rust 1.95.0 and locked dependencies |
| Universal app and bundle versions | Both slices; both version strings `0.1.0` |
| Final app ad-hoc signing | Native codesign verification passed; `Signature=adhoc`, no team identity |
| DMG creation, checksum, mounted contents | Passed; verified signed app and Applications link |
| Native library linkage | Both slices link only Apple system libraries/frameworks |
| Windows source check | All targets/features and release-only startup code type-check for x86_64-pc-windows-msvc from macOS; no native execution or Windows resource-link claim |
| Source outside Git | Check/build-script revision fallback validated separately |
| Git revision tracking | Loose and packed refs tested; nested source archive omits revision; no broad `.git` directory watch |
| Release path privacy | Final Universal executable contains neither local home nor checkout prefix after source-path remapping |
| Automated fresh-start probe | Passed outside the filesystem sandbox; sandboxed launch lacked native window-server access |

The packaged executable was opened through a temporary launch wrapper with a
fresh HOME and a system-only PATH. Its app bundle/executable were unchanged.
It created the absent data directory, showed zero repositories/history and
started with no optional coding-agent installation. Check CLI reported an
actionable missing-Codex message without preventing repository entry. Existing
user state was hashed before/after and remained unchanged.

Native UI checks on that packaged build:

- About showed CodeConvoy, Version 0.1.0, the optional 12-character base revision,
  MIT and project link, with no network request or exposed build paths.
- The native NSOpenPanel opened as a sheet; Tab/Enter activated Browse and Escape
  cancelled it, preserving the existing field and restoring path focus.
- Choosing a valid repository containing spaces and Unicode filled the field
  without registration. Explicit Add repository succeeded and showed clean Git
  state through the existing validation path.
- A nonexistent manually entered path failed without registration.
- Re-entering the same repository via `/tmp` after selection through canonical
  `/private/tmp` was rejected as already registered.
- Selecting an ordinary non-Git folder filled the field only; Add repository
  rejected it and retained the single valid registered repository.
- Closing the test window saved only the isolated test state.

These checks supplement the existing [folder-picker](folder-picker-validation.md),
[UI](ui-ux-validation.md), [OpenCode](opencode-validation.md) and
[Claude](claude-validation.md) records. The repository's safety/execution tests
continue to cover dirty state, revalidation, nested/canonical locks, admission,
bounded output, descendant cancellation and state recovery.

## Required hosted and desktop follow-through

The new release workflow has been statically validated; it has **not yet run on
GitHub Actions** in this work. Linux/Windows packages were not built or executed
on this macOS host. Run its non-publishing manual mode before tagging and inspect
its native package/installer checks. The AppImage host-process test is committed
and is a release-job gate; its Linux runtime result is still pending.

| Platform | Remaining validation |
| --- | --- |
| Linux X11 and Wayland | Install DEB/RPM on compatible distributions; launch AppImage; test portal selection/cancellation, clipboard/URLs, host Git and installed agent lookup, concurrent cancellation and shutdown |
| Windows x86-64 | Verify CI installer/uninstaller and Job Object fixture results; on a real desktop check no console flashes, picker, keyboard navigation, installed/portable startup and descendant cancellation on Stop/Stop Convoy/Stop All/close |
| Intel macOS | Intel slice is built/inspected but was not run on Intel hardware |
| Downloaded macOS app | Local ad-hoc integrity was verified; actual browser quarantine/Gatekeeper first-launch flow and oldest supported macOS version remain unverified |
| Agent authentication | No new authenticated E2E claims; OpenCode and Claude remain unverified |

No Git tag or public release was created. The signing pipeline needs no Apple
Developer account secrets; ad-hoc verification does not imply Apple trust.
