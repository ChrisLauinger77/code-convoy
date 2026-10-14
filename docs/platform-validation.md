# Cross-platform release verification

## Linux glibc 2.39 baseline

This focused infrastructure change follows readiness PR 3 and does not start
readiness PR 4. Application version remains 0.10.0. It changes only Linux build
and package verification infrastructure, its disposable probe and documentation;
the application and Windows/macOS build configuration are unchanged.

The intended minimum supported binary ABI is **glibc 2.39**, targeting Ubuntu
24.04 LTS, Debian 13 Stable and newer compatible glibc-based x86_64 distributions.
Every final DEB/RPM/AppImage ELF, including the outer AppImage runtime, must pass
the 2.39 ceiling. This is not a complete distribution compatibility claim.

### Candidate and automated evidence

The non-publishing candidate run, final artifact measurements and validation
results will be recorded here after this change's candidate completes. No tag,
release publication or downstream package update is authorized by these checks.

| Check | Ubuntu 24.04 LTS | Debian 13 Stable | Fedora 44 (RPM) |
| --- | --- | --- | --- |
| Normal dependency-resolving install | Candidate APT/container check pending | Candidate APT/container check pending | Candidate DNF/container check pending |
| Installed application, Xvfb window and normal close | Pending | Pending | Pending |
| Exact AppImage extraction fallback | Pending | Pending | Pending |
| Mounted AppImage/FUSE | Candidate runner check pending when device available | Not tested | Not tested |
| Disposable Git worktree create/remove and process cancellation | Candidate probe pending | Candidate probe pending | Candidate probe pending |
| Native Wayland and XWayland startup | Not tested on this target | Not tested on this target | Not tested on this target |
| Hardware rendering, decorations, clipboard and file dialogs | Not tested | Not tested | Not tested |
| Native desktop normal Quit and task cancellation | Not tested | Not tested | Not tested |

Containers use the host kernel and a private virtual X server, not the target's
complete desktop. The probe exercises real CodeConvoy Git/process code with a
redirected descendant and disposable repositories; it is not the final GUI's
interactive agent workflow. Native target desktop environments are not currently
provisioned for this task. Unperformed desktop checks remain release-verification
gaps, even if all candidate ABI and container gates pass. Earlier Forky/GNOME
results below do not substitute for Ubuntu 24.04 or Debian 13 evidence.

For native acceptance, install the exact candidate DEB through APT on clean
Ubuntu 24.04 and Debian 13 desktops, launch from the desktop entry and terminal,
exercise native Wayland with `DISPLAY` unset and XWayland with Wayland variables
unset, and test both FUSE and extraction launches of the same AppImage. Check
rendering/decorations, cross-application clipboard, portal selection/cancel,
normal Quit, disposable worktree preparation/cleanup, and fixture-agent Stop and
shutdown. RPM requires the corresponding native Fedora check. Record the image,
kernel, desktop, Git versions, artifact hashes and outcomes. Do not force missing
dependencies, use provider credentials or operate on user repositories.

---

## 1.0 readiness PR 3 — 2026-10-14

This is verification of the merged PR 2 implementation (`77965f3`), plus focused
long-path regressions. Application version remains **0.10.0**. No product
feature, desktop framework, dependency, scheduler, ownership rule or release
infrastructure changes. The package run below is a non-publishing verification
candidate, not the final 1.0 candidate or authorization to tag it.

### Evidence and environment

Local host: Debian forky/sid, GNOME Shell 50.5 in a Wayland session, Linux
7.2.9 x86-64, Rust 1.96.1, Git 2.53.0 and Python 3.14.8. All repositories and
application stores used by the probes are disposable; no provider task or user
repository was run. `TMPDIR=/var/tmp` avoids the desktop sandbox's synthetic
`/tmp/.git`; subprocess/socket tests run outside that sandbox.

- **Automated tests:** formatting, strict all-target/all-feature Clippy and the
  full Rust suite passed locally: 592 tests, zero failures, nine existing optional
  tests ignored. The private D-Bus test was additionally run and passed; all 23
  packaging unit tests and the release build passed. Counts exclude subprocess
  repetitions; all required checks were repeated after the regression expansion.
- **Native Linux smoke:** both `desktop_performance 0 2` and
  `v09_history_validation --smoke` passed on Wayland with `DISPLAY` removed, and
  on X11 through XWayland with `WAYLAND_DISPLAY`/`WAYLAND_SOCKET` removed. Each
  exited through its ordinary close path. The history fixture verified that the
  retained worktree contents and Git backlink survived. No software-rendering
  override was set; this does not establish which physical GPU was selected.
- **Exact Linux packages:** downloaded `packages-linux` from the candidate run
  below. Its AppImage started with isolated empty state on both native Wayland
  and XWayland. Its DEB passed a real `dpkg` install/remove transaction in a
  disposable unprivileged root, installed-binary Wayland startup, license checks
  and unrelated-file preservation. Dependency resolution was bypassed in that
  empty root; this is not an apt compatibility test. The package startup script
  checks liveness then terminates its fixture, not interactive/normal Quit.
- **Build/test matrix:** [merged-source CI](https://github.com/ChrisLauinger77/code-convoy/actions/runs/38572066655)
  passed on all three platforms. The expanded regression commit `2204e06` passed
  the full native test suite on Linux, macOS and Windows in
  [CI run 38576285524](https://github.com/ChrisLauinger77/code-convoy/actions/runs/38576285524);
  Windows also passed WARP initialization. The
  [initial PR run](https://github.com/ChrisLauinger77/code-convoy/actions/runs/38573674622)
  exposed Git's overlong worktree-root limit below. Subsequent documentation-head
  results are available in the [PR checks](https://github.com/ChrisLauinger77/code-convoy/pull/59/checks).
  These are native CI tests, not interactive desktop certification.
- **Package matrix:** existing [Release run 38572061998](https://github.com/ChrisLauinger77/code-convoy/actions/runs/38572061998)
  **passed** on source `77965f3c4fb63e372c8fd3d1921331bab29960dc`: Ubuntu 26.04,
  Windows 2025 and macOS 26 runners, Rust 1.99.0 and Python 3.15. It built all six
  packages, checked the Universal macOS bundle and Windows installer/uninstaller,
  and generated the aggregate checksum manifest. All six downloaded checksums
  passed locally; the tested Linux files match the final aggregate. Publication
  and downstream package-repository updates were skipped; no tag was created.
  These binaries embed the baseline revision, not the PR head. The runtime source
  and packaging configuration are unchanged in PR 3; a final 1.0 candidate must
  still be built and tested from its own exact commit.

A compiled binary, an automated test, a live desktop smoke and a checked package
are separate evidence. Historical macOS interactive results below and in the
linked feature records are useful prior evidence, not a new native 1.0 check.

### Current verification matrix

Procedures for the remaining interactive checks are in the
[native smoke checklist](#native-smoke-checklist). **Not tested** is not a pass.

| Area / procedure | Linux | macOS | Windows |
| --- | --- | --- | --- |
| Native startup, render, normal close | **Passed:** both fixtures on GNOME Wayland and XWayland | Packaged startup/liveness **passed** on macOS CI; interactive/normal Quit **not retested** | **Not tested interactively:** WARP initialization is an automated CI check |
| Decorations | **Passed, protocol evidence:** resolved Adwaita feature plus five frame buffers attached/committed on native Wayland; decorated XWayland window captured | Existing AppKit/decorated viewport configuration audited; **not retested visually** | Existing decorated viewport configuration audited; **not retested visually** |
| Minimize/restore, Dock/taskbar, tray menus | Shared state/callback tests; **native interaction not verified** | Shared tests; prior Dock/quit evidence is historical | Shared tests; Explorer/notification-area interaction **not tested** |
| Notifications and click activation | Private-bus unavailable-service behavior **passed**; visible delivery/click **not tested** | Policy/callback tests; bundle permission/Focus/click **not tested** | Policy/callback tests; installed/portable delivery/click **not tested** |
| File dialogs | Parented portal configuration and selection/cancel tests; native chooser **not tested** | Main-thread parented sheets audited; native chooser **not retested** | COM dialog adapter audited; native chooser **not tested** |
| Clipboard and shortcuts | Mapping, modal gating and output tests; cross-application paste/native chords **not verified** | Command mapping tests; native copy/paste **not retested** | Control mapping tests; native copy/paste **not tested** |
| Process cancellation and shutdown | Real fixture processes, descendants, repeated Stop, future drop, quarantine and peer isolation **passed locally** | Native process tests **passed** in package CI | Native Job Object/process-tree tests **passed** in package CI |
| Worktree creation/recovery/cleanup | Real Git regression suite **passed locally**; native history fixture retains checkout | Native Git suite **passed** in package CI | Native Git suite **passed** in package CI; ordinary Storage cleanup remains deliberately unavailable |
| Unicode, spaces, long paths | Literal invocation tests and >300 UTF-16-unit tracked paths/custom base **passed** | Long tracked paths and deep custom base **passed** | Long tracked paths and overlong-root refusal/recovery regressions **passed**. **Known limitation:** Git rejects overlong worktree roots. Drive/UNC spelling, junction and sharing tests passed; live UNC server not tested |
| Linux packages and environment | **Passed:** DEB/RPM/AppImage assembly, host-Git/CLI-name probe, Xvfb startup; downloaded AppImage on Wayland/XWayland; disposable-root DEB install/start/remove. Native RPM install/dependency resolution **not tested** | — | — |
| macOS Universal bundle/distribution | — | **Passed:** arm64 + x86_64, lipo, bundle metadata/license, ad-hoc signature and mounted DMG; Intel native UI not tested | — |
| Windows installer/portable/removal | — | — | **Passed:** metadata, per-user install, Start Menu, license, portable identity, uninstall/state preservation; interactive installed startup not tested |

The additional XWayland interactive fixture rendered, but GNOME declined the
attempted automated activation/input (the compositor's active window did not
become the fixture). No keyboard, picker, clipboard or restoration result is
claimed from those attempts. The idle fixture was explicitly terminated without
starting any job; that termination is not normal-quit evidence.

### Wayland decoration verification

The resolved graph is `winit 0.30.13` with `wayland-csd-adwaita`, selecting
`sctk-adwaita 0.10.1` and its `ab_glyph` renderer. Both Wayland runs had no `DISPLAY`,
so they could not fall back to XWayland. Their `WAYLAND_DEBUG=1` traces showed an
`xdg_toplevel` and all five Adwaita decoration subsurfaces, with attached buffers,
input regions and commits. The empty fixture produced a 35-pixel header buffer
and the four surrounding frame parts, then rendered the application. This is
protocol/render evidence; decoration-button clicking and resize hit targets
still need interactive verification.

Reproduce using the existing fixtures and the build command below:

```sh
cargo tree --locked -e features -i sctk-adwaita
env -u DISPLAY WAYLAND_DEBUG=1 target/release/examples/desktop_performance 0 2
env -u DISPLAY WAYLAND_DEBUG=1 target/release/examples/v09_history_validation --smoke
env -u WAYLAND_DISPLAY -u WAYLAND_SOCKET target/release/examples/desktop_performance 0 2
env -u WAYLAND_DISPLAY -u WAYLAND_SOCKET target/release/examples/v09_history_validation --smoke
```

Use a real Wayland session for the first two launches. The first-render samples
were approximately 55 ms on Wayland and 51 ms on XWayland; these single samples
are smoke evidence, not a performance guarantee. Neither Xvfb nor an XWayland
launch proves native Wayland behavior.

### Focused regression and remaining release gates

`long_paths_preserve_identity_through_recovery_and_explicit_cleanup` creates
tracked paths longer than 300 UTF-16 code units, including spaces and Japanese
text. It checks real checkout creation, canonical registration, saved ownership
reload, lifecycle inspection, explicit retained-result cleanup and preservation
of the source index/content and ownership tombstone. Unix also exercises a custom
base longer than 300 UTF-16 code units. Only its disposable repository enables
Git's `core.longpaths`, as described in the
[Git for Windows FAQ](https://gitforwindows.org/faq); application code does not
change global Git or Windows settings.

**Known Windows limitation:** Git 2.55.0.windows.5 rejected that deep custom base
with `fatal: '$GIT_DIR' too big`, despite `core.longpaths=true`. Its
[`setup_explicit_git_dir` check](https://github.com/git-for-windows/git/blob/v2.55.0.windows.5/setup.c#L1156)
limits the UTF-8 byte length of `GIT_DIR` to `PATH_MAX - 40` before resolving the
worktree's `.git` file. Long tracked-file support does not remove this root-path
limit. This is an upstream boundary, not a failing safety check to bypass.
Choose a shorter worktree base for a **fresh** convoy; leave existing failed
attempts and their ownership records intact. No path aliases, automatic
relocation, Direct fallback or broad cleanup workaround was introduced.

`overlong_windows_worktree_root_fails_without_launch_and_retains_ownership`
requires that exact Git diagnostic and verifies no agent starts, failed status
and ownership survive save/reload, recovery does not claim Available/Cleaned,
source index/content stay intact, and a fresh request at a shorter base can run
after Git exits. The first CI failure is retained above as evidence; it is not
reclassified as successful long-root support. Ordinary Windows Storage cleanup
remains subject to its existing stable-filesystem-identity restriction.

No new runtime fix is claimed. Arbitrary-length roots and every external agent's
path behavior are not supported by this verification.
The candidate DEB and deployed AppImage require **glibc 2.43**, measured by the
existing packaging checks. Older glibc desktops cannot run these artifacts; the
intended oldest Linux distribution remains an explicit release compatibility
gate. The local Debian host meets this requirement. The tested AppImage SHA-256
is `bc1eb8844e3e6ff335fa08dce090b557d9a38f39c9d77ee43aab3c07c09488f4`.

One Windows attempt of `38576285524` timed out in the existing
`claude_build_spawn_and_zero_exit_protocol_failures_release_slots_and_repository_leases`
test's 10-second lifecycle wait, before reaching the new regressions. The unchanged
Windows rerun passed the full suite, including both new cases. No timeout,
assertion or test coverage was relaxed. The intermittent timeout's cause is not
established and remains a CI reliability observation.

The unperformed native desktop interactions above remain release-verification
gaps, not green gates. Before final 1.0
publication, test the exact candidate on the intended Linux distributions,
macOS Intel/Apple Silicon and Windows desktop versions using the checklist.
Ad-hoc macOS signing is not notarization; Windows Authenticode signing is absent.
Existing Unix process-group escape, external reaper and session-scoped quarantine
limits remain documented in [the PR 2 audit](worktree-validation.md#unix-process-exit-blocker-and-follow-up-fix).
PR 4 has not started. No release version, tag or publication is part of PR 3.

---

# Earlier v0.10 PR 4: cross-platform hardening

This is a platform audit and regression-coverage change. It adds no backend,
scheduling, persistence-format or packaging behavior. The Wayland Adwaita frame
fix (`ddc8205`) was already on main before this PR.

## Findings and changes

| Finding | Change or retained behavior |
| --- | --- |
| `desktop_performance` omitted macOS application initialization | Install the same quit-intercepting NSApplication subclass used by the application and other native fixtures, before constructing AppKit objects. |
| Process tests did not combine a Unicode/space-containing executable path, working directory and literal argument boundaries | A real executable-copy fixture checks empty arguments, quotes, trailing backslashes, shell metacharacters, Unicode and stdin without a shell. |
| Cancellation coverage concentrated on explicit Stop | A child-held OS lock verifies cleanup for both cancellation and dropping the executing future. Unix process groups and Windows Job Objects remain unchanged. |
| Windows file-sharing failures lacked direct regression coverage | Native Windows tests hold handles that deny deletion; state/export replacement must fail without losing the previous file or leaving a temporary file, then succeed after the handle is closed. |
| Windows junction and path-prefix handling needed explicit coverage | Test drive/UNC argument spelling and reject both a junction base and substitution of an existing owned base. Recovery/cleanup must preserve the checkout, source and ownership record. |
| Linux unavailable-service handling was only mock-tested | An opt-in test uses the real tray/notification transports on a private D-Bus session. The existing Linux CI job runs it explicitly. No user notification service or GNOME extension is needed. |

## Audit coverage

| Area | Implementation and existing coverage |
| --- | --- |
| Window startup and decorations | `src/main.rs`, `src/graphics.rs` and all native examples use decorated viewports. Linux supports both Wayland and X11; the chosen backend depends on the launch environment. Windows DX12/WARP selection does not change decorations. |
| Restore, tray and shutdown | `src/desktop`, `src/ui/desktop.rs`, `src/ui/quit.rs` and their tests cover absent/lost hosts, generation-scoped callbacks, restore before focus, repeated Quit, safe-default confirmation and active/queued work. |
| macOS Dock/application menu | `src/ui/quit_macos.rs` intercepts native terminate without replacing winit's delegate. `src/ui/about_macos.rs` preserves the native About panel and Cargo-derived metadata. Native interaction must be checked in a signed application bundle. |
| Notifications | `src/notifications` and `src/ui/notifications.rs` cover delivery policy, stable run IDs, foreground suppression, shutdown and restart silence. macOS requires bundle identity/permission; Windows registers a per-user AppUserModelID. |
| File dialogs | Repository, attachment, worktree-location and export dialogs reuse the parented `rfd::AsyncFileDialog`. Construction stays on the UI thread for macOS sheets; completion is asynchronous. Selection/cancellation/stale-result tests use no native dialogs. |
| Clipboard | Copy uses egui's native clipboard output. Existing Copy Results tests verify the requested text, not successful OS clipboard delivery. Native copy/paste remains a separate check. |
| Git/worktree safety | `core`, `convoys`, `worktrees`, `worktree_recovery`, `worktree_locations`, `validation` and `validation_presets` test canonical identity, nested conflicts, dirty/staged alternatives, lease release, retained ownership, permission failures and isolated validation targeting. |
| Universal builds and installers | The existing Release workflow builds both macOS architectures and checks the merged bundle with lipo/codesign. Windows smoke checks install, metadata and uninstall. These release workflows were inspected, not dispatched by this PR. Package validation belongs to PR 5. |

## Validation record

2026-10-12: Debian forky/sid, GNOME Shell 50.5 / Wayland, x86-64 Linux,
Rust 1.96.1, Git 2.53.0.
Only disposable fixture repositories/state and the local test executable are used.
No installed coding agent, account, credential or user repository is needed.

| Check | Result |
| --- | --- |
| Formatting, strict all-target/all-feature Clippy, full Rust suite, release build | Passed; 568 Rust tests passed, 9 optional tests ignored (excluding two subprocess repetitions). |
| Linux private D-Bus transport test | Explicitly run and passed in addition to the ordinary suite. |
| Existing packaging unit tests | 16 passed; no package was created or installed. |
| GNOME native Wayland | Empty performance fixture and 1,002-entry history fixture rendered and exited normally; retained worktree contents and Git backlink survived. |
| X11 through XWayland | The same two native fixtures rendered and exited normally; retained worktree contents and Git backlink survived. |
| macOS/Windows | New regression cases run in the existing PR CI matrix; this Linux host cannot verify their native UI. |

All four Linux window probes used `LIBGL_ALWAYS_SOFTWARE=1`; hardware rendering
was not tested. Native dialogs, clipboard transfer, live notifications, tray menu
interaction, compositor activation and interactive quit confirmation were not
exercised in these automatic runs. The history fixture checks normal shutdown
and preserved retained data, not every interactive checklist item.

The first sandboxed full-suite attempt was unable to create Unix sockets and
rejected temporary ownership paths. The suite is also run outside that sandbox;
these access failures must not be worked around by weakening production checks.

## Reproduce automated checks

```sh
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo build --release
CODECONVOY_PRIVATE_BUS_TEST=1 dbus-run-session -- \
  cargo test --locked --all-features --test linux_desktop -- --ignored --nocapture
```

The D-Bus command is Linux-only. Always use `dbus-run-session`: the opt-in marker
does not create an isolated bus itself. The test rejects a bus with a notification
service or tray watcher already running. It verifies error delivery and safe
close-to-tray fallback, not visible notifications, tray menus or activation.

The existing CI matrix executes Rust tests on Linux, macOS and Windows, including
the Windows-only file-sharing, junction and drive/UNC cases. WARP initialization
is explicitly tested on Windows. A successful build is not native UI verification.

## Native smoke checklist

Build the existing disposable fixtures first:

```sh
cargo build --release --features test-support --bin codeconvoy-test-agent \
  --example desktop_performance --example v06_desktop_validation \
  --example v09_history_validation
```

On Linux, exercise each display backend deliberately:

```sh
# In a GNOME Wayland session: retain WAYLAND_DISPLAY; do not force X11.
target/release/examples/desktop_performance 0 2
target/release/examples/v09_history_validation --smoke
# X11 compatibility, including XWayland on a Wayland desktop:
env -u WAYLAND_DISPLAY -u WAYLAND_SOCKET target/release/examples/desktop_performance 0 2
env -u WAYLAND_DISPLAY -u WAYLAND_SOCKET target/release/examples/v09_history_validation --smoke
```

`LIBGL_ALWAYS_SOFTWARE=1` can isolate driver problems; record it when used. Do not
describe an XWayland run as native Wayland. These probes close themselves through
the real App lifecycle; startup/render/exit success does not verify user interaction.

| Check | Linux GNOME/Wayland and X11 | macOS Intel/Apple Silicon | Windows 10/11 |
| --- | --- | --- | --- |
| Window | Resize, minimize, restore and close; compare Wayland/X11 frames | Dock restore, red close button, Cmd+Q, app-menu/Dock Quit | Taskbar restore, close, Alt+F4; GPU and WARP |
| Safe quit | Run two fixture jobs at concurrency 1; Cancel/Escape must keep work alive; confirm must stop the tree before exit | Repeat through Cmd+Q and Dock/menu Quit, including minimized state | Repeat through window close and tray Quit |
| Tray | Absent host must leave normal Quit; with a host check menus, toggle and host loss | Status-item menu, enable/disable and Show | Notification-area overflow, Explorer restart and Show |
| Notifications | Unfocused completion, focused failure, click/select correct convoy, missing service | Signed bundle, allow/deny, Focus, body/button click and Clear All | Installed/portable sender identity, Do Not Disturb, banner/Notification Center click |
| File dialogs | Portal FileChooser, cancel, Unicode directory and absent portal/manual entry | Parented sheets, cancel and Unicode paths | Spaces/Unicode, cancel, inaccessible directory and locked output |
| Clipboard | Copy/paste into a separate native editor on both backends | Copy/paste with Command shortcuts | Copy/paste with Control shortcuts |
| Recovery/safety | Restart with retained worktrees; no resume/notification replay; permission failures preserve data | Include `/var` aliases and separate volume paths | Include drive/UNC paths, junction rejection and file-sharing failures |
| Distribution | Installed DEB/AppImage desktop launch | Signed Universal bundle on both CPU architectures | Installed and portable launch/uninstall |

Use `v06_desktop_validation` for interactive desktop/quit checks. It disables
notifications and only runs the local fixture agent. Notification presentation
checks need a separate disposable configuration with notifications enabled.

## Platform limits retained

- Stock GNOME need not provide a StatusNotifier host. Core use and safe Quit work
  without an extension. Tray availability does not prove icon visibility.
- Wayland can refuse restoration/foreground focus; desktop activation remains the
  recovery route. No X11 workaround is injected into the Wayland path.
- Ordinary Storage cleanup remains disabled on Windows because its existing proof
  lacks stable filesystem identities. Do not relax that guard or substitute a
  recursive deletion to make tests pass. Explicit result actions retain their own
  separate ownership/transaction checks.
- Deliberately detached Unix descendants, force termination and OS crashes retain
  the documented process-group/shutdown limitations.
- Notification delivery, native file-picker interaction, clipboard delivery and
  foreground activation require native desktop checks; headless tests cannot
  certify them. No release, tag or package is produced by this PR.
