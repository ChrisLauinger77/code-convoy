# History management

History is unlimited by default. Automatic retention is disabled, including when
loading existing 0.8.0 settings. The previous implicit 30-entry cap is removed;
entries already removed by older versions cannot be restored by this change.

Large histories use visible-row rendering in the convoy selector. Arrow keys and
Home/End reach off-screen convoys; search never changes the selected convoy merely
because it is filtered out. Ordering, completion observations and validation records
still come from retained history. See the [repeatable measurements](history-reliability-validation.md).

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

## Persistence and recovery

The existing version-1 shared state file remains the storage format. Writes use an
owner-only temporary file in the same directory, buffered JSON, explicit flush,
file sync and atomic replacement. Unix also syncs the containing directory.
Periodic and recovery saves perform disk work on a worker. Their snapshots omit
only session-only output/diagnostics, not persisted content. A single writer stays
reserved through acknowledgement, so older saves cannot overwrite a cleanup or
newer durable intent. New edits stay dirty until their own save. Closing waits for
an outstanding writer; failed writes remain visible and do not claim success.

Explicit launch, validation/configuration and result intent/outcome transactions
still wait for their buffered durable save before proceeding. They do not queue
behind a periodic writer on the UI thread; conflicting actions wait or report busy.
Those explicit saves and the final exit save can still pause on slow storage.

A missing state file starts an empty application. An empty file, invalid UTF-8,
truncated JSON, missing core `version`/`next_run`/`runs` fields, invalid records,
malformed retention settings, duplicate convoy IDs or an unsupported future version
is an error. Startup stops; the original file is left unchanged and that Store
refuses further writes, including retention. Back up the file and use a compatible version or restore a
known-good backup before restarting. Move the preserved file aside only if you
intend to start fresh. Exports are reports, not backups or import files.

Legacy optional fields still default normally. A lagging run counter advances past
the highest retained ID; a missing counter is not guessed. Store loading never writes
automatically, resumes an agent, invents completion times or deletes a worktree.
The UI subsequently saves explicit recovery observations through its usual worker.
No partial-entry salvage or automatic destructive migration is attempted.

Write/create/flush/sync/replacement errors preserve the prior destination before
publication. An error syncing the directory *after* replacement is explicitly an
uncertain durability result; the previous disk contents cannot be promised then.
Windows uses the existing tempfile platform replacement, not delete-then-rename;
sharing violations can fail safely. Native Windows/macOS and power-loss testing
remain release-candidate checks, not inferred from Linux tests.

## Data-format compatibility

The application version, internal state format and public export schema are
independent. Preparing application version 1.0.0 does not require changing either
the internal `state.json` `version: 1` or the public JSON export's
`schema_version: 1`.

Supported 0.x settings and retained history remain readable. New optional fields
use documented defaults: for example, saves predating retention keep unlimited
history with automatic retention disabled. Present but malformed retention settings
are errors, not instructions to reset preferences. Unfinished jobs and validation
recover as interrupted/cancelled; agents and validation commands never resume.
Saved worktree observations require fresh ownership/Git verification.

Internal JSON, sibling worktree ownership manifests, original-location bindings
and cleanup journals are application-managed recovery data, not public extension
interfaces. Their implementation may evolve with compatible loading or explicit
migrations; their individual serialization details are not frozen for all 1.x
versions. This readiness change needs no migration. Preserve the complete original
application-data directory and retained worktree locations when making a backup.
Copying `state.json` alone is insufficient to back up ownership evidence or results.

Unknown additive fields in ordinary state objects are accepted but are not retained
when the application saves again. Validation command objects reject unknown fields.
Unknown enum values and unsupported state version numbers fail loading and disable
saving. Downgrading after a newer application has saved data is not guaranteed safe;
keep the original backup and use a compatible application. Do not treat arbitrary
extra state fields as a supported way to extend CodeConvoy.

The [public JSON history export schema](history-export.md#json-schema-1) is a
separate, documented report contract. It is unchanged by this work. Use that schema
for report consumers; internal state is not an export format, and exports cannot
restore configuration, ownership or complete history. See the
[stable-readiness persistence audit](history-reliability-validation.md#stable-readiness-pr-1--compatibility-and-data-integrity)
for tested upgrade boundaries and limitations.
