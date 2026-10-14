# Cross-platform release verification

## 1.0 readiness PR 3 — 2026-10-14

This is verification of the merged PR 2 implementation (`77965f3`), plus one
focused long-path regression. Application version remains **0.10.0**. No product
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
  packaging unit tests and the release build passed.
- **Native Linux smoke:** both `desktop_performance 0 2` and
  `v09_history_validation --smoke` passed on Wayland with `DISPLAY` removed, and
  on X11 through XWayland with `WAYLAND_DISPLAY`/`WAYLAND_SOCKET` removed. Each
  exited through its ordinary close path. The history fixture verified that the
  retained worktree contents and Git backlink survived. No software-rendering
  override was set; this does not establish which physical GPU was selected.
- **Build/test matrix:** existing [CI run 38572066655](https://github.com/ChrisLauinger77/code-convoy/actions/runs/38572066655)
  checks the merged source on Linux, macOS and Windows. Results are pending;
  the new long-path test also requires its own native CI result.
- **Package matrix:** existing [Release run 38572061998](https://github.com/ChrisLauinger77/code-convoy/actions/runs/38572061998)
  checks source `77965f3c4fb63e372c8fd3d1921331bab29960dc` using the existing pinned
  runners/toolchain. Package results are pending. Its manual trigger skips both
  publication and downstream package-repository updates; no tag is created.

A compiled binary, an automated test, a live desktop smoke and a checked package
are separate evidence. Historical macOS interactive results below and in the
linked feature records are useful prior evidence, not a new native 1.0 check.

### Current verification matrix

Procedures for the remaining interactive checks are in the
[native smoke checklist](#native-smoke-checklist). **Not tested** is not a pass.

| Area / procedure | Linux | macOS | Windows |
| --- | --- | --- | --- |
| Native startup, render, normal close | **Passed:** both fixtures on GNOME Wayland and XWayland | **Not tested interactively:** candidate startup pending | **Not tested interactively:** WARP initialization is an automated CI check |
| Decorations | **Passed, protocol evidence:** resolved Adwaita feature plus five frame buffers attached/committed on native Wayland; decorated XWayland window captured | Existing AppKit/decorated viewport configuration audited; **not retested visually** | Existing decorated viewport configuration audited; **not retested visually** |
| Minimize/restore, Dock/taskbar, tray menus | Shared state/callback tests; **native interaction not verified** | Shared tests; prior Dock/quit evidence is historical | Shared tests; Explorer/notification-area interaction **not tested** |
| Notifications and click activation | Private-bus unavailable-service behavior **passed**; visible delivery/click **not tested** | Policy/callback tests; bundle permission/Focus/click **not tested** | Policy/callback tests; installed/portable delivery/click **not tested** |
| File dialogs | Parented portal configuration and selection/cancel tests; native chooser **not tested** | Main-thread parented sheets audited; native chooser **not retested** | COM dialog adapter audited; native chooser **not tested** |
| Clipboard and shortcuts | Mapping, modal gating and output tests; cross-application paste/native chords **not verified** | Command mapping tests; native copy/paste **not retested** | Control mapping tests; native copy/paste **not tested** |
| Process cancellation and shutdown | Real fixture processes, descendants, repeated Stop, future drop, quarantine and peer isolation **passed locally** | Same native process tests in CI, **pending** | Job Object/process-tree tests in CI, **pending** |
| Worktree creation/recovery/cleanup | Real Git regression suite **passed locally**; native history fixture retains checkout | Native Git tests in CI, **pending** | Native Git tests in CI, **pending**; ordinary Storage cleanup remains deliberately unavailable |
| Unicode, spaces, long paths | Existing literal invocation tests and new >300 UTF-16-unit custom worktree path **passed** | Same tests in CI, **pending** | Same tests plus drive/UNC spelling, junction and sharing tests in CI, **pending**; live UNC server not tested |
| Linux packages and environment | DEB/RPM/AppImage, host-Git/CLI-name probe and Xvfb package startup **pending** | — | — |
| macOS Universal bundle/distribution | — | arm64 + x86_64, lipo, bundle metadata/license, ad-hoc signature and mounted DMG **pending**; Intel native UI not tested | — |
| Windows installer/portable/removal | — | — | Metadata, per-user install, Start Menu, license, portable identity, uninstall/state preservation **pending**; interactive installed startup not tested |

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

`long_custom_location_preserves_identity_through_recovery_and_explicit_cleanup`
creates a custom base longer than 300 UTF-16 code units, including spaces and
Japanese text. It checks real checkout creation, canonical registration, saved
ownership reload, lifecycle inspection, explicit retained-result cleanup and
preservation of the source index/content and ownership tombstone. Only its
fixture repository enables Git's `core.longpaths`; application code does not
change global Git or Windows settings. This does not promise arbitrary-length
paths or compatibility with every external agent. Ordinary Windows Storage
cleanup remains subject to its existing stable-filesystem-identity restriction.

No confirmed platform product defect has been found in these completed checks.
Pending build/package results and the unperformed native desktop interactions
above remain release-verification gaps, not green gates. Before final 1.0
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
