# CodeConvoy 0.9.0 — History & Reliability (unreleased)

## PR 1: History Retention & Cleanup

- Optional count and age limits in Settings → History Retention. Unlimited history
  and disabled automatic retention are the defaults, including existing settings.
  Removes the previous implicit 30-entry cap without changing the state version.
- Manual Clean Up History preview uses stable IDs and rechecks protection/policy
  before confirmation. Changed previews require confirmation again.
- Opt-in automatic retention runs once after startup loading, with no polling,
  repeated cleanup attempts or writes when no entries qualify.
- Newest means actual completion time, then convoy ID; both constraints apply.
  Missing/inconsistent/future dates remain outside retention limits.
- Remove, Clear and retention share protected deletion and background atomic saves.
  Failed saves retain live history; successful saves safely update selection.
- Runtime worker/lifecycle ownership, active validation and retained worktree
  evidence remain protected even when visible agent status is terminal. History
  removal never cleans worktrees or changes repository/validation configuration.

See [history usage](history.md) and [validation evidence](history-retention-validation.md).
Package version remains 0.8.0 pending a separate release preparation request.

## PR 2: History Export

- Export one terminal convoy or selected/filtered/all retained history to one
  Markdown or JSON file. Existing single-convoy selection and history filters are
  reused; active convoys are excluded.
- Shared Copy Summary Markdown formatter and independent JSON schema version 1.
  Saved completion observations, unknown values, validation and review resolution
  remain distinct; current Git and live Review are never consulted.
- Metadata-only confirmation snapshots survive later history/filter changes and
  cleanup. No logs, diffs, attachments, environment or internal ownership metadata.
  Bounded task/name/command fields follow the existing Copy Results privacy policy.
- Native save dialog, explicit overwrite review, cancellable background formatting
  and temporary-file publication, with concise completion/error feedback.
- Deterministic model, file-failure, privacy and headless egui interaction tests.
  No dependencies, persistence schema, backend, scheduler or release-version changes.

See [export formats and privacy](history-export.md) and
[export validation](history-export-validation.md).

## PR 3: Storage Overview

- **Settings → Storage…** shows persisted historical convoy count, actual shared
  state-file bytes, managed worktree count/sizes, original locations and current
  base, unavailable entries, per-category successful times and partial totals.
- Read-only ownership inventory reuses manifests, saved/live references and exact
  custom-location bindings, including previous bases and known orphan attempts.
  Duplicate references are counted once; incomplete ownership stays explicit.
- Logical regular-file sizes include generated output. Links/reparse targets,
  external Git administration, exports and unrelated data are excluded. Unix hard
  links are deduplicated within each checkout; physical allocation is not claimed.
- Cancellable background scans, coalesced manual refresh, stale-result rejection,
  bounded diagnostics and a compact, scrollable keyboard-accessible view.
- No cleanup, retention trigger, Git execution, ownership migration, scheduler,
  backend, dependency, persistence schema or release-version changes.

See [measurement semantics and PR 4 prerequisites](storage.md) and
[storage validation](storage-validation.md).


## PR 4: Stale Worktree Detection & Safe Cleanup

- Storage Overview now inspects managed lifecycle, ownership, Git registration,
  current changes and cleanup eligibility, including recorded older locations.
- Explicit selection and exact-path confirmation; active, unresolved/retained,
  modified, ignored, missing, orphaned and unverified copies remain protected.
  Completed or clean alone never authorizes removal.
- Independent final checks per target under existing Store transactions and
  execution/validation leases. The new ordinary action unlocks only verified
  ownership and uses non-force Git removal, with no prune or recursive fallback.
- Bulk progress and Removed/Skipped/Failed outcomes, preserved history and
  validation evidence, invalidated size measurements and post-batch refresh.
- Journaled failures preserve evidence; normal quit finishes the in-flight
  critical operation and skips remaining targets. No automatic cleanup or Force
  Delete. Windows removal is disabled pending stable filesystem identity support.
- Disposable-Git safety tests, failure injection and keyboard confirmation tests.
  Existing result Discard semantics, dependencies and package version are unchanged.

See [worktree cleanup](worktree-cleanup.md) and [validation](worktree-cleanup-validation.md).

## PR 5: History Performance & Reliability

- Buffered atomic state writes remove the measured long-save bottleneck. Periodic
  and recovery saves run on a worker using the existing single-writer reservation;
  acknowledgement preserves edits and validation events received during saving.
  Normal quit waits for that writer before its final lifecycle save.
- Persistence snapshots omit session-only logs, diagnostics and Review caches,
  while retaining every serialized field and ownership protection. No stored
  history, validation metadata or task text is truncated.
- Virtualized history rows preserve order, stable selection, current summaries,
  keyboard focus and Arrow/Home/End navigation. Empty search avoids allocating
  lowercase copies of every task; there is no new presentation cache.
- Failed loads disable writes through that Store. Missing core envelope fields,
  malformed records, duplicate IDs and unsupported versions preserve the original
  file and report errors. Legacy optional fields retain their defaults; lagging
  counters advance past retained IDs and exhausted counters cannot launch.
- Export publication also syncs its directory on Unix and reports uncertainty
  if that sync fails. JSON schema, Markdown formatting, cancellation, retention,
  scan generations and conservative worktree cleanup semantics are unchanged.
- Deterministic 100/1,000/5,000-convoy fixtures, repeatable performance checks,
  persistence failure injection and save/selection/shutdown regressions.
  No new dependencies, state schema, backend, scheduler or package-version changes.

See [measurements, audit and release-candidate checks](history-reliability-validation.md).

---

# CodeConvoy 0.8.0 — Workflow & Productivity

## Changes since 0.7.0

This release adds reusable validation command presets, bulk validation setup,
keyboard shortcuts, Markdown result copying and quick repository filtering.

### Validation Presets & Bulk Assignment

- Nine built-in command templates: npm Lint, Rust Tests, Rust Check,
  Rust Clippy, npm Test, npm Build, Python pytest, CMake Build and Make Test.
- Single-repository Preset/Custom selection fills the existing executable and
  literal JSON argument fields. Commands remain editable, with a readable preview;
  only Save changes configuration. Reopening preserves existing custom commands.
- **Assign Validation Preset…** selects repositories or multiple overlapping groups
  independently of convoy selection. Canonical paths deduplicate targets; group
  membership stays unchanged. Review shows target and existing-command counts.
- Preserve existing commands by default. Explicit overwrite requires a separate
  confirmation with Cancel focused. Updated, preserved and failed targets are
  reported individually, including removed registrations and stale overwrite reviews.
- Stage eligible updates and persist them with one atomic state replacement.
  Failed saves keep live settings unchanged and report unconfirmed disk outcomes.
  Existing command serialization and state version stay unchanged; no migration,
  preset IDs in saved settings, dependencies or additional validation backends.
- Configuration never executes commands or agents, inspects Git, creates worktrees,
  changes scheduling or rewrites saved validation execution records. One command
  per repository remains the rule; multiple checks belong in project-owned scripts.

See [validation usage](validation.md) and
[preset validation evidence](validation-presets-validation.md).

### Keyboard Shortcuts

- Ctrl/Cmd+Enter uses the Run Convoy button's shared action and existing preflight
  review. Readiness checks, dirty-tree acknowledgment and explicit Start remain
  in place. Held keys and repeated layout passes do not repeat dispatch.
- Ctrl/Cmd+F focuses and selects the existing repository filter text when the
  filter is available. Escape leaves focus without changing query, selection or
  group expansion; history search is unaffected.
- Ctrl/Cmd+1–4 select Activity, Diff, Raw Output and Task & Settings for the
  selected convoy using the existing tab state and cached views.
- Text editors retain Run/view key events; dialogs and menus take precedence.
  Modified Enter cannot accidentally activate a focused confirmation button.
  Escape uses each modal's existing safe dismissal, only for the topmost dialog.
- Compact **Settings → Keyboard Shortcuts…** reference uses egui's platform
  labels. No hotkey dependency, global hooks, settings schema or execution and
  scheduling changes.

See [keyboard shortcut validation](keyboard-shortcuts-validation.md) for checks
and platform coverage.

### Copy Results

- **Copy Summary** for terminal/historical convoys and **Copy Repository Result**
  in the existing job details copy concise Markdown via egui's system clipboard.
  Both actions support keyboard navigation and brief inline copy feedback.
- Saved execution metadata, UTC timestamps, durations, completion observations,
  review status and latest validation records remain distinct. The convoy table
  keeps original repository order, independent of view filters.
- Missing historical changes stay Unknown; partial counts are labelled “known”.
  Missing validation says “Not run / not recorded”, without consulting today's
  mutable configuration. Copy never invokes Git or reads live review statistics.
- Bounded, escaped task/name/command metadata; no automatic logs, diffs,
  environments, attachments or local path metadata. Recognizable local paths in
  free text are omitted. Review copied text for sensitive content before sharing.
- Pure formatting tests and headless egui clipboard/keyboard coverage; no new
  dependency, persistence field, execution behavior or export screen.

See [copy-results validation](copy-results-validation.md).

### Quick Repository Filter

- Find registered repositories immediately by name or path with case-insensitive
  substring matching using full Unicode case folding (for example,
  `Straße`/`STRASSE`); surrounding query whitespace is ignored.
- Matching groups open temporarily, retain their order and shared selection,
  and show only matching repositories. Ungrouped stays first when it has matches.
- Clear restores manual expansion. Hidden selections remain selected; global
  and group selection controls retain their full-membership behavior.
- Compact empty state and keyboard-accessible filter/Clear controls, using native
  egui styling. The overall selected count remains visible.
- Session-only cached matching; no search dependency, filesystem/Git work, new
  persistent settings, backend changes or scheduling changes.
- The small `unicase` text utility supplies Unicode folding without transitive
  dependencies or application-maintained character tables.

See [filter validation](repository-filter-validation.md) for automated coverage
and native smoke results.

### Maintenance

- Update `serde_json` to 1.0.152.
- Update GitHub Pages actions and use Python 3.15 in CI and release packaging.
