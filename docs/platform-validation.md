# v0.10 PR 4: cross-platform hardening

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
