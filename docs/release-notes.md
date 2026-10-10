# CodeConvoy 0.6.0 — Desktop Workflow

CodeConvoy 0.6.0 brings three workflow improvements: an optional system tray
for running convoys while minimized, configurable storage for isolated worktrees,
and collapsible repository groups with shared selection and clear counts.

## Changes since 0.5.0

- Optional native system tray with Show, running-convoy count, Show Active Convoys
  and safe Quit. Off by default; counts include queued and preparing convoys.
- Immediate, persisted close-window preference: Quit by default, or minimize with
  an operational tray. Jobs continue while minimized; Dock/taskbar access remains
  available for recovery. Missing/failed trays use the normal Quit workflow.
- Shared restore-before-focus handling for tray, notification and quit actions.
  Show retains the current view; Show Active opens the existing ACTIVE group.
- Native macOS Cmd+Q, application-menu and Dock Quit remain explicit Quit actions
  regardless of the close preference. Repeated requests share one confirmation
  and cancellation path. Cancel leaves jobs and notification listeners unaffected.
- Linux StatusNotifierItem support without GTK/AppIndicator system libraries or a
  required GNOME extension. Missing hosts fail once; watcher loss restores the
  window and disables tray minimization until explicitly re-enabled.
- Settings and run changes persist while the window is minimized. Existing v1
  settings (including 0.5.0) default safely to tray disabled and Quit behavior.
- Configurable **Settings → Worktree location** with a native directory picker,
  absolute custom base or the existing default. New convoys capture the setting;
  queued and existing worktrees retain their original location. Unique attempts,
  pre-creation validation and Store-owned original-location records preserve
  recovery/Apply/cleanup safety without fallback, relocation or automatic deletion.
  Partial attempts retain their location after ownership or hooks initialization
  errors, with Git creation blocked until all preparation succeeds.
- Collapsible repository sections with **Ungrouped** first, followed by saved
  groups. Headers show repository and selected counts, with group selection
  available while collapsed and an overall unique selection count. Overlapping
  groups share selection state and still produce only one job per repository.
  Expansion survives navigation, renames and membership edits during the session;
  keyboard navigation and **Select all / Select none** remain available. Cached
  membership mappings and compact headers support narrow windows in every theme.
- Tests for settings compatibility, tray failure/routing/count changes, stale
  callbacks, close/quit decisions and notification restoration; custom worktree
  locations, recovery and cleanup; repository classification, shared selection,
  deduplication, counts and expansion through group edits/deletion. Includes
  headless keyboard/layout checks and a disposable native desktop validation
  example.

Selected dependencies are tray-icon 0.26.1 for macOS/Windows and ksni 0.3.6 for
Linux. Linux requires zbus >=5.19 for safe runtime selection when Tokio and
async-io are both enabled. See [rationale and platform behavior](desktop-workflow.md).
Scheduling, process-tree cancellation, repository safety, history, notification
filters and execution backends retain their existing behavior.

## Validation and remaining platform work

See the dated [macOS validation record](desktop-workflow.md#validation) for build,
test and native-smoke evidence. Linux x86-64 source validation on 2026-10-10 with
Rust 1.95 passed formatting, strict Clippy, all-feature tests (355 passed, four
optional CLI probes ignored) and the release build. Repository-group interaction
and narrow-layout checks ran headlessly across dark/light/system themes.

Windows native builds, Linux/Windows desktop smoke checks, macOS Intel execution,
status-menu interaction and native notification activation remain platform
checks. These source checks did not exercise release packages or authenticated
backend tasks.

Known limits: tray presence cannot guarantee an icon is visible through every OS
or third-party menu-bar policy. Dock/taskbar access is retained deliberately.
Wayland and Windows can restrict focus; native notifications require existing OS
permissions and activation support. Jobs run only while the application process
exists; no daemon, cold-start notification routing or automatic restart is added.
