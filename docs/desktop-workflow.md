# Desktop workflow and settings

CodeConvoy 0.6.0 integrates with the desktop while keeping the application's
existing job ownership and safe shutdown behavior.

## Settings

Open **Settings → Desktop workflow**:

| Setting | Default | Behavior |
| --- | --- | --- |
| Enable system tray | Off | Create one tray icon; enable/disable applies immediately. |
| Close window behavior | Quit application | Minimize to tray can be selected only with a working tray. |
| Show running convoy count in tray | On | Display the count row and tooltip when the tray is active. |

Settings use the existing version-1 JSON Store under an additive `desktop` field.
Missing fields in 0.5.0/older state receive defaults; run IDs, jobs and all existing
preferences remain unchanged. Tray availability, errors, menu IDs and hidden/window
state are not persisted. A saved minimize preference is retained if support is
unavailable, but the current close behavior safely falls back to Quit. No restart
is needed to change close behavior. After fixing tray-host support, toggle tray
off/on to explicitly retry; initialization never retries continuously.

## Close, minimize, restore and Quit

- **Close + Quit application:** follow the existing active-work confirmation.
- **Close + Minimize to tray:** cancel the close event and minimize the same window
  only if the tray is operational. Running/queued convoys continue and results,
  notifications and persistence continue updating without UI rendering.
- **Show CodeConvoy:** restore the existing window and retain its selected view.
- **Show Active Convoys:** select an active convoy (retain an already-active
  selection), switch to Review and open the run selector's existing ACTIVE group.
  A zero count still opens that group, or reports no active convoys with no history.
- **Quit CodeConvoy:** always use the safe shutdown workflow. Settings also has
  this command, and native macOS Quit retains its usual meaning.

Minimize-to-tray deliberately retains Dock/taskbar access rather than removing the
main window from all desktop navigation. This recovery route remains available if
the icon disappears, the shell restarts, or tray visibility is restricted. Linux
watcher loss additionally requests window restoration automatically. Supported
restore APIs clear minimization before requesting focus on a later loop pass;
focus remains subject to the OS. No second main window is created.

Active-work confirmation includes preparing, running and queued jobs and retained
result operations. Cancel/Escape dismisses confirmation without cancellation.
Confirm closes admission, signals existing Stop All cancellation tokens, waits for
process-tree/result cleanup and persists terminal state before exiting. Repeated
close or Quit requests cannot duplicate cleanup or hide a pending confirmation.
Stop Convoy/Stop All retain their existing behavior. Hiding/minimizing a window is
not process termination; CodeConvoy does not execute jobs after application exit.

## Platforms

**Linux (Wayland and X11):** ksni speaks StatusNotifierItem/D-BusMenu to the session
bus. A compatible StatusNotifierWatcher and host must be present. Stock GNOME
Shell commonly has no host; CodeConvoy works normally without one, reports tray
unavailable once and retains the normal Quit behavior. No GNOME extension or
GTK/libappindicator package is required. Traditional XEmbed-only trays are not
supported. Host registration is checked during initialization; watcher loss ends
that service with a single diagnostic. A host advertising support cannot prove
that its icon is actually visible, so taskbar/desktop recovery remains available.
No X11-only activation or Wayland window-manager hacks are used. Wayland may ignore
focus/unminimize requests; select CodeConvoy from the desktop if needed.

**macOS:** tray-icon supplies NSStatusItem/menu objects on eframe's existing main
thread. The normal application activation policy, Dock icon and app menu stay
intact. Cmd+Q, app-menu Quit and Dock Quit send explicit Quit actions; window-close
preferences cannot turn them into minimization. Native notification delivery still
requires a suitable app bundle and OS permission.

**Windows 10/11:** tray-icon uses the notification area and existing UI message
loop. Creation verifies native icon registration before enabling minimize-to-tray;
close-to-tray rechecks registration. Explorer's TaskbarCreated handling belongs to
the maintained library. Taskbar access remains available during shell/tray loss.
Windows may restrict foreground activation. Normal shutdown uses the existing
confirmation and process-tree cleanup path.

Notifications retain the existing delivery backends, filters and foreground
suppression. Minimized windows are not foreground. Notification actions select the
corresponding convoy when supported; duplicate callbacks are ignored. Clicks do
not launch a new application or resume old jobs after exit. See [notifications](notifications.md).

## Library choice

[tray-icon 0.26.1](https://docs.rs/tray-icon/0.26.1/tray_icon/) is maintained by the
Tauri project and offers standalone native menus on macOS/Windows, compatible with
the existing eframe loop. Its default Linux backend requires GTK/AppIndicator;
its optional KSNI backend owns a worker thread and abstracts away watcher-loss
handling. Neither is needed by this application.

[ksni 0.3.6](https://docs.rs/ksni/0.3.6/ksni/) directly provides asynchronous SNI,
host detection and watcher-offline callbacks. Using its Tokio mode reuses the
application runtime without another UI loop or application thread. zbus >=5.19
is constrained explicitly because its dual-backend runtime selection preserves
async-io desktop consumers while tray/notification tasks run on Tokio. AppIndicator
was evaluated but rejected because it would introduce GTK system dependencies and
additional event-loop integration. The [StatusNotifierWatcher specification](https://specifications.freedesktop.org/status-notifier-item/latest/status-notifier-watcher.html)
describes why a watcher alone is insufficient without a registered host.

## Validation

2026-10-10, macOS Apple Silicon, Homebrew Rust/Cargo 1.99:

- `cargo fmt --check`, strict Clippy, `cargo test --all-features` and
  `cargo build --release`: passed. 331 Rust tests passed, with four optional CLI
  probes ignored. Automated tests use no live desktop session.
- 16 packaging tests and `packaging/release.py check-tag v0.6.0`: passed using
  Python 3.14. The system Python 3.9 lacks the required `tomllib` module.
- Linux and Windows target checks were attempted; both stopped because their Rust
  standard-library targets are not installed. No Linux/Windows build success is claimed.
- The actual Linux adapter source compiled in a temporary Unix harness with ksni,
  Tokio and eframe. This verifies Rust/API compatibility, not Linux runtime behavior.
- A separate disposable macOS `.app` verified native settings availability,
  disable/re-enable without restart and close-to-tray control gating. A fixture
  convoy had one running and one queued job; Cmd+Q opened confirmation, Cancel
  left both active, and explicit Quit after Close again presented confirmation.
  Stop jobs and quit terminated the fixture app. Only the local test-agent ran.
- macOS status-bar automation timed out. Tray-menu clicking, icon visibility,
  restoration through Dock/menu, native notification delivery/activation and
  Explorer/Linux host-loss behavior are not claimed as native-tested.

Reproduce the disposable native fixture with:

```sh
cargo build --features test-support --bin codeconvoy-test-agent
cargo run --features test-support --example v06_desktop_validation
```

It creates temporary repositories and state, configures only the local fixture
agent, disables native notifications and offers a 30-second job per repository.
Select two available repositories for running/queued confirmation checks. Existing
seeded history is synthetic. No installed coding agent or user repository runs.

Before release, smoke-test Linux GNOME/Wayland with and without a host, an X11 SNI
host, macOS Intel/Apple Silicon and Windows 10/11. Verify icon/menu and count updates,
disable/re-enable, Show preserving view, Show Active navigation, close with active
and queued work, Cancel, repeated Quit, full process-tree cleanup, notification
clicks while minimized, and recovery when the tray host/shell disappears. Confirm
settings round-trip and no replay/resume after restart. No tags or packages have
been published by this implementation.
