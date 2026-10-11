# Storage Overview

Open **Settings → Storage…** for a read-only snapshot of persisted history and
known CodeConvoy worktree directories. Opening starts one background measurement;
**Refresh** measures again. Nothing watches or periodically scans the filesystem.
**Manage History…** opens the existing retention settings. It does not start cleanup.
**Manage Worktrees** adds current lifecycle/ownership/Git inspection and explicitly
confirmed cleanup. Opening and refreshing never remove anything. See
[managed worktree cleanup](worktree-cleanup.md) for eligibility and platform limits.

The view reports historical convoy count, measured bytes, storage locations,
category-specific last successful measurement times and expandable diagnostics.
The total combines only known measured bytes. **Partial measured total** means
some storage could not be measured; **Unavailable** is never rendered as zero.
An empty, successfully scanned directory really is zero bytes. **Complete** means
the known inventory was measured, not that every byte related to CodeConvoy or its
agents has been found. Unknown ownership is reported separately from known
worktrees with missing or inaccessible contents. Partially measured worktrees
have a separate count.

## History

CodeConvoy currently has one supported persisted history file: `state.json` in the
Store's platform application-data directory, shown in the view. Legacy version-1
saves use this same file. It also contains settings, registrations, the draft and
active run metadata. The overview labels this as **shared state file** storage;
it does not pretend to isolate the byte cost of just the historical convoys.

The size is the actual regular-file length, not a serialization estimate. The
historical count uses the persisted Run model and excludes runs with nonterminal
jobs. Both are read from the saved file on the worker, without recovery, rewriting
or triggering retention. Unsaved job changes appear after normal persistence and
the next refresh. After explicit history cleanup finishes, refresh to see its saved
count and size. Missing, unreadable, invalid or unsupported saves produce a
clear diagnostic and an unknown count; measurable bytes can still be shown.

Exports, temporary save files, application locks, logs owned by agent CLIs,
attachments, caches and unrelated files in the data directory are excluded.
The scanner does not search for other JSON or Markdown files.

## Managed worktrees and older locations

Discovery uses the existing immutable WorktreeMetadata references in saved and
current runs, sibling `owner.json` manifests under the Store's default worktree
root, and exact custom-location bindings in that root's `.locations` directory.
Bindings were introduced with configurable locations in v0.6. They preserve
original paths even after the configured base changes or history is removed.
Only the immediate default storage entries and individual recorded attempts are
inspected. The current custom base and source repositories are never searched.

Existing ownership/layout, bounded-record, canonical-location and manifest checks
are reused. The random reservation identity deduplicates multiple references;
run/job alone is not an attempt identity. Conflicting records are excluded with a
diagnostic. Known references with unavailable manifests remain visible but their
contents are not traversed. A completed, matching cleanup tombstone with an absent
checkout is excluded. A reappeared checkout is counted again if measurable; this
never changes its lifecycle state or authorizes any action.

The view shows the current configured base separately from the number and paths
of original bases used by known retained attempts, including unavailable attempts.
A location does not move when the preference changes. Canonical paths from existing
ownership metadata and platform-native Path handling are retained; directory names
alone never establish ownership.

An unreadable root or malformed record makes the inventory incomplete, without
hiding valid entries elsewhere. Diagnostics are bounded to 100 per measurement;
additional error counts remain visible. Arbitrary Git worktrees are not included.
If all references and bindings to an old custom attempt are lost, there is no safe
way to rediscover it without searching unrelated directories. This overview reports
the known subset, does not migrate metadata and cannot detect every lost record.

## Meaning of sizes and refresh

Worktree size is the sum of regular-file lengths **inside the checkout directory**.
Tracked, untracked, ignored, generated and hidden files all count, including
`node_modules`, `target`, `build` and `dist`. File contents are not read to size
them. Sparse files count their logical length, not their allocated blocks. Sizes
use B, KiB, MiB and GiB (and larger IEC units as needed).

Symbolic links, broken links, Windows reparse points/junctions and their targets
are skipped. A linked root or ancestor is rejected. Sockets, FIFOs and devices are
excluded. The ordinary `.git` pointer file counts by its length; it is never
followed into shared Git administration. Sibling manifests, location bindings,
hooks directories, cleanup journals and other metadata outside the checkout are
excluded from worktree bytes, so the total is not a full application-data total.

On Unix, hard links with the same device/inode within one checkout count once.
Hard links between different worktrees, and hard links on Windows, can count once
per path. Mounted directories that are ordinary directories are included. No
claim is made about unique physical disk allocation, compression or deduplication.

A single blocking worker uses the existing Tokio runtime and application message
channel. Traversal is streaming and cancellation is checked between entries and
history read buffers. Refresh cancels the current request and coalesces pending
requests; the next worker starts only after the old worker returns. Generation
checks reject stale completions and failures. Closing the view or quitting signals cancellation. Git inspection awaits process
cleanup before releasing its lease. A blocked operating-system filesystem
measurement call cannot be forcibly interrupted; it does not block egui or hold
application shutdown open.

The filesystem measurement phase acquires no repository leases, executes no Git or
agent commands, starts no validation and modifies no application, repository or
ownership data. The following management inspection uses cancellable read-only Git
checks under existing manager lifecycle leases, including a disposable index for
fresh tracked-content checks. It never starts an agent or validation, repairs
ownership metadata or triggers deletion. Existing jobs,
history cleanup and exports continue independently. Files can change or disappear
while scanning; errors preserve a partial measured subtotal. These are approximate
point-in-time observations, not a filesystem transaction or an ownership security
boundary against hostile concurrent path replacement. Refresh after work settles
for a more stable observation. Previous successful times remain visible if a later
refresh fails; the previous snapshot is explicitly labelled while a scan runs or
a worker fails.

## Cleanup authority

Storage inventory and logical size are **not cleanup authority**. Management
separately verifies trusted storage, original bindings, manifests, canonical Git
identity, ownership locks, backlinks, lifecycle conflicts and crash journals.
Each selected target must pass the Store transaction and manager lease again at
execution. Unavailable sources, unknown ownership, missing directories and zero
sizes never authorize deletion. Missing custom bindings and orphan lifecycle
records remain inspection-only; no broad search, prune or ownership migration is
introduced. See [cleanup rules](worktree-cleanup.md) and
[cleanup validation](worktree-cleanup-validation.md).

The original measurement coverage remains documented in
[storage validation](storage-validation.md).

PR 5 also requires the core history envelope and rejects ambiguous convoy IDs
during measurement. It continues parsing historical entries with the domain Run
model, so malformed entries do not become a complete empty-history measurement.
The scanner does not run Store recovery or validate a result for deletion. An
experimental lighter entry parser was dropped to preserve this error behavior.
See [scale measurements and regression evidence](history-reliability-validation.md).
