# Desktop workflow and settings

CodeConvoy integrates with the desktop while preserving job ownership and safe
shutdown. These controls were introduced in 0.6.0; dated validation below records
the original checks. Use the [platform matrix](platform-validation.md) for later
evidence and remaining desktop limits.

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

## Worktree location

Open **Settings → Worktree location**. Enter an absolute base directory, or use
**Browse…** to open the native directory picker, then **Apply location**. The
current saved location stays visible; cancelling the picker or failing validation
does not replace it. **Use default** clears the override and restores
`<application data directory>/worktrees`. This optional `worktree_base` preference
uses the existing local version-1 settings file; older settings retain the default.

Each newly launched convoy captures the current preference. Already launched
convoys, including their queued jobs, keep their captured location. Retrying or
reusing a convoy creates new worktrees using the current preference. Existing
worktrees are never moved, renamed or deleted by a setting change.

The configured directory is a base, not a checkout: each job atomically reserves
`run-<id>-job-<index>-<random>/tree` below it. Missing directories are created during
preparation, after validation. Spaces and Unicode are supported. Use an absolute
native path (including drive/UNC paths on Windows), without `.` or `..`; `~` and
environment variables are not expanded. Directory aliases are resolved to their
canonical location when saved. A symlink/junction selected as the base is rejected.
Bases inside Git working trees, managed attempts (including partial attempts), or
CodeConvoy's internal ownership-record directory are refused.

Preparation revalidates the location and checks access by actually reserving the
unique attempt and writing its ownership records before invoking Git. Invalid,
missing-parent, permission and disk-space failures are explicit job errors; there
is no fallback to the default directory or Direct execution. Partially reserved
metadata is retained with its location in diagnostics. The Store-side location
record is written before initializing the sibling ownership file and hooks directory.
If any of these operations or their syncs fail, the job still records the reserved
attempt before failing, so its original location survives restart; Git is not started.
Successfully recorded locations also remain discoverable without a saved run, even
when the sibling ownership file could not be initialized.

Custom attempts have both the existing sibling `owner.json` and an exact original
location record under `<application data directory>/worktrees/.locations`. Retain
this application data alongside custom worktrees. Recovery, Apply and cleanup use
that record, never the current preference, and still verify the original Git
identity, detached ownership lock, backlink and exact checkout. Missing, altered
or redirected ownership records block cleanup. Cleanup removes only the recorded
checkout through Git and retains the existing metadata tombstones; it never removes
the base directory. Startup orphan diagnostics also cover recorded old locations.

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

## Update check

The footer **About** dialog has **Check for updates** next to the version. There
is no automatic, startup, background or scheduled check: CodeConvoy makes no
network request until you click the button. Each click sends one HTTPS `GET` to
`https://api.github.com/repos/ChrisLauinger77/code-convoy/releases/latest` with
a `CodeConvoy/<version>` User-Agent and a 10-second timeout. Like any web request,
it reveals your IP address to GitHub. No token, task, repository data or setting
is sent, and the result is not saved.

The button is disabled while **Checking…** and can be used again afterwards.

| Result | Meaning |
| --- | --- |
| Up to date | The latest release is not newer than this build, including a local build ahead of it. The latest release version is shown. |
| Version X.Y.Z available | **Open release page** opens the release on GitHub in your browser. |
| Update check failed | Expand **Diagnostics** to read or copy the HTTP status, rate-limit headers, network error or invalid response. |

One leading `v` is removed from the release tag, then versions are compared by
semantic-version precedence: build metadata is ignored and a pre-release is
older than its final release. A tag that is not a semantic version fails the check
rather than being guessed. GitHub limits unauthenticated API requests per IP
address; a rate-limit failure reports the limit headers but omits GitHub's message,
which names your IP address.

The check only informs. CodeConvoy never downloads, installs or replaces itself;
update with the package or installer you used originally.

Requests use rustls with the operating system's trusted root certificates. HTTP(S)
proxies from the standard proxy environment variables are honoured, including
`NO_PROXY`. A SOCKS proxy configured that way fails the check instead of being
bypassed by a direct connection. On macOS, the application menu's
**About CodeConvoy** is AppKit's static panel; use the footer **About** to check.

### Homebrew and Scoop

When a newer release is available and the running executable is inside this app's
Homebrew Caskroom (`…/Caskroom/code-convoy/…`) or, on Windows, its Scoop app
directory (`…\scoop\apps\code-convoy\…`), About also shows the matching command
with **Copy update command**:

```sh
brew update && brew upgrade --cask ChrisLauinger77/cask/code-convoy
```

```powershell
scoop update
scoop update code-convoy
```

Scoop commands are shown one per line because Windows PowerShell 5.1, which Scoop
supports, cannot parse `&&`. An executable under Scoop's default global root,
`%ProgramData%\scoop`, is a global installation; Scoop updates it only with
`--global` from a terminal running as administrator:

```powershell
scoop update
scoop update code-convoy --global
```

Other `scoop` roots are treated as per-user. If a custom global root is also
named `scoop`, Scoop itself reports that the app is installed globally and asks
for `--global`.

Package definitions can lag a GitHub release until the tap or bucket updater has
run, so the package manager may not offer the new version immediately. Detection
only inspects the executable path and runs no package-manager command. Any other
location shows only the release link. That includes the current cask, which moves
`CodeConvoy.app` into `/Applications`, where it cannot be distinguished from a
manually copied app, and Scoop installations under a root not named `scoop`.

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

**Windows:** tray-icon uses the notification area and existing UI message
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
application runtime without another UI loop or application thread. The lockfile
retains zbus 5.19 for tray, notification and accessibility dependencies: its
dual-backend runtime selection preserves async-io desktop consumers while
tray/notification tasks run on Tokio. Direct portal appearance calls use zbus 6
with its Tokio and proxy features; no fallback runtime is needed for those calls.
AppIndicator was evaluated but rejected because it would introduce GTK system
dependencies and additional event-loop integration. The [StatusNotifierWatcher specification](https://specifications.freedesktop.org/status-notifier-item/latest/status-notifier-watcher.html)
describes why a watcher alone is insufficient without a registered host.

## Validation

See the [v0.10 platform audit and smoke checklist](platform-validation.md) for
current cross-platform regression coverage and the native checks still required.

2026-10-10, macOS Apple Silicon, Homebrew Rust/Cargo 1.99:

- `cargo fmt --check`, strict Clippy, `cargo test --all-features` and
  `cargo build --release`: passed. 346 Rust tests passed, with four optional CLI
  probes ignored. Automated tests use no live desktop session.
- Thirteen additional worktree-location tests cover legacy/custom persistence,
  default and missing-directory resolution, concurrent unique reservations,
  invalid/traversing/nested paths, permission/link failures, queued-convoy setting
  snapshots, no fallback, settings picker cancellation, original-location cleanup
  after restart, missing/tampered ownership records, and persisted partial attempts
  after Store binding failure, plus orphan discovery and returned metadata after
  ownership-file or hooks-directory initialization failures. Existing recovery tests pass.
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
settings round-trip, native worktree folder selection on each platform, and no
replay/resume after restart. No tags or packages have
been published by this implementation.

### Storage measurements

**Settings → Storage…** shows the current worktree base and known attempts in
original locations, including older custom bases recorded under `.locations`.
It never moves, validates through Git, cleans or migrates worktrees. Refresh after
jobs or explicit history cleanup to update the read-only snapshot. See
[Storage Overview](storage.md) for logical-size semantics, exclusions and partial
inventory reporting.
