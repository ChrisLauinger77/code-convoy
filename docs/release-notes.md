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
