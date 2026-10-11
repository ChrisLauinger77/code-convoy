# History management

History is unlimited by default. Automatic retention is disabled, including when
loading existing 0.8.0 settings. The previous implicit 30-entry cap is removed;
entries already removed by older versions cannot be restored by this change.

## Settings

Open **Settings → History Retention…**:

- **Maximum entries:** Unlimited, 100, 250, 500, 1,000, or a custom positive whole
  number. Zero, negative, fractional and overflowing values are rejected.
- **Maximum age:** Unlimited, 30, 90, 180 or 365 days.
- **Automatically apply retention policy:** disabled by default. When explicitly
  enabled and saved, the policy runs once on the next application startup after
  state loading and recovery have finished. It does not run while editing settings,
  on each completed job, on a timer or continuously during rendering.

Both limits apply independently. The count limit keeps the newest eligible
convoys by actual completion time (the latest job finish), then by higher convoy
ID for equal times. Display order, search and filters do not influence this order.
The age limit removes entries strictly older than the specified number of 24-hour
days. Entries exactly at the age limit remain unless the count limit removes them.

A reliable completion requires a finish time for every job, at or after creation
and any recorded start, and no future timestamps. Missing, inconsistent or future
dates remain outside **both** limits. Interrupted jobs recovered on restart have
no invented completion date. Legacy records remain viewable and individually
removable when otherwise safe. Protected entries also sit outside the limits, so
the total retained history can exceed the configured count.

## Manual cleanup

Open **History cleanup → Clean Up History…** in RUNS / RESULTS. Empty history also
has a **Clean Up History…** action. The dialog shows the saved policy, stored,
removal and retained counts, protected/undated counts, and stable convoy IDs.
It evaluates all history, including entries hidden by search or filters.

Opening the dialog changes nothing. Cancel (initial keyboard focus) or Escape
closes it without deletion. **Clean Up History** confirms the displayed removal
set. Immediately before saving, the app evaluates current history again. A changed
policy, count or removal set refreshes the preview and requires confirmation again;
a newly completed convoy cannot silently enter the confirmed set.

Cleanup uses the existing atomic state-file replacement on a blocking worker.
The UI continues receiving execution/validation events. Other state saves wait
until this transaction is acknowledged, preventing a stale snapshot from restoring
removed history. The live history and selection update only after a successful
save, and events received during the save are preserved. Errors retain in-memory
history and show a diagnostic; automatic cleanup does not retry in that session.
An error after replacement but during directory sync can leave the disk outcome
uncertain; the message says so. An unchanged policy result causes no write.

The current selection remains if retained. If removed, the app selects an active
convoy, otherwise the first remaining convoy, otherwise empty, and clears stale
comparison selections, job selection and Diff/output caches. Search, filtering,
validation history, result copying and reuse continue to use remaining records.

## Protection and worktrees

Running, preparing, queued and cancelling work is protected, including terminal
UI records whose worker or lifecycle lease still owns resources. Active validation
workers, result operations and unconfirmed process cleanup also protect history.
History cleanup never terminates processes or runs Git commands.

Retained isolated results, including Applied copies, unavailable paths, partial
attempts and uncertain operations, remain protected until physical cleanup has
been verified in this session. This intentionally retains their history records:
these contain ownership and transaction evidence needed for safe future management.
Sibling ownership manifests and original-location bindings are also left intact.
No separate ownership migration is introduced in this change. Startup retention
conservatively preserves worktree-backed records awaiting recovery verification;
use manual cleanup after verification for already-cleaned results.

**History removal is not worktree cleanup.** It never removes worktree directories,
Git metadata, validation artifacts, registrations, groups or validation commands.
Existing **Discard** and **Clean up retained copy** actions remain separate,
explicit worktree operations with their existing safety checks.

**Remove convoy #… from history** and **Clear removable history** retain their
existing meaning: remove eligible terminal history regardless of retention policy.
They share the same protected deletion and atomic save path. Selection alone does
not protect an otherwise removable convoy.

Deleted history may not be recoverable. There is no backup, restore, storage
overview or automatic worktree cleanup feature.

## Export before cleanup

**Export…** beside Copy Summary exports the selected terminal convoy; **Export
History…** exports the selected convoy, currently filtered history or all retained
terminal history as one Markdown or JSON file. A missing selected entry is an error.
The report snapshots current recorded metadata at confirmation and never inspects
Git. Export and file writing do not modify history or trigger retention.

Exported files remain independent after Remove, Clear or retention cleanup. Export
is manual; cleanup never exports automatically. Markdown reuses Copy Summary, and
JSON uses explicit schema version 1. These are concise reports, not restorable
backups. Review bounded task/name/validation metadata before sharing. See
[formats, scopes, privacy and file handling](history-export.md).
