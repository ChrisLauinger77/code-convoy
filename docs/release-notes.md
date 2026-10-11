# CodeConvoy 0.9.0 — History & Reliability

## Changes since 0.8.0

This release adds configurable history retention, Markdown and JSON exports,
storage measurement and explicit managed-worktree cleanup. Large histories save
and navigate faster, with stronger recovery checks and clear feedback when saving
temporarily blocks an action. Existing settings and history remain compatible;
automatic retention is disabled by default.

### History Retention & Cleanup

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

### History Export

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
  No new dependencies, persistence schema, backend or scheduler changes.

See [export formats and privacy](history-export.md) and
[export validation](history-export-validation.md).

### Storage Overview

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
- Storage measurement remains read-only and does not trigger cleanup or retention.
  Ownership and Git checks for explicit cleanup remain a separate operation.

See [measurement semantics](storage.md) and
[storage validation](storage-validation.md).


### Stale Worktree Detection & Safe Cleanup

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
  Existing result Discard semantics and dependencies are unchanged.

See [worktree cleanup](worktree-cleanup.md) and [validation](worktree-cleanup-validation.md).

### History Performance & Reliability

- Buffered atomic state writes remove the measured long-save bottleneck. Periodic
  and recovery saves run on a worker using the existing single-writer reservation;
  acknowledgement preserves edits and validation events received during saving.
  Normal quit and fallback native exit wait for that writer before the final save
  of interrupted jobs and validations.
- Start, result actions, validation and cleanup controls show when saving blocks
  them. Existing review acknowledgements and removal confirmations are preserved;
  completing a save never launches or confirms an action automatically.
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
  No new dependencies, state schema, backend or scheduler changes.

On the recorded Linux benchmark with 5,000 convoys, median atomic save time fell
from 768.2 to 26.6 ms and open-history selector layout from 13.2 to 0.31 ms.
Stricter load validation increased load time from 25.9 to 35.6 ms. These are local
measurements, not guarantees for other machines or filesystems; explicit durable
intent and final saves can still pause on slow storage.

See [measurements, audit and release-candidate checks](history-reliability-validation.md).
