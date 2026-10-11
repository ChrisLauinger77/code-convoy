# Managed worktree inspection and cleanup

Open **Settings → Storage… → Manage Worktrees**. Opening or **Refresh** measures
known storage and inspects ownership and current Git state in the background.
Nothing is removed until you select eligible entries, choose **Clean Up Selected…**,
review the exact paths and explicitly confirm. Cancel is initially focused;
Escape cancels. There is no automatic cleanup, monitoring or Force Delete.

## Inventory and protection

A managed worktree has CodeConvoy's original ownership record outside its checkout.
Its attempt name, run/job identity, repository, fixed base and original absolute
path must agree with the saved lifecycle and sibling manifest. Custom locations
also need their exact original binding in the Store. Changing the current base
preference does not hide older bound attempts. Arbitrary directories and
user-created Git worktrees are never discovered as deletion candidates.

Each entry shows repository, exact path, convoy/job, logical size, lifecycle,
change/review status and eligibility. Expand **Inspect details** for identity,
base, existence, Git registration, detached HEAD and the protection reason.
Git status details use `??` for untracked, `!!` for ignored, index/worktree status
columns for tracked edits, and conflict codes such as `UU`.

| State | Meaning |
| --- | --- |
| Active | Running/queued execution, validation or conflicting repository use; protected. |
| Retained | Unresolved result kept for review, including clean, failed and cancelled attempts; protected. |
| Cleanup Candidate | Resolved Applied result, available this session, owned, registered, unchanged from its fixed base, with no local/ignored data or pending operation evidence. |
| Modified / Needs Review | Local, staged, untracked, ignored, conflicted or committed data could be lost; ordinary cleanup is blocked. |
| Unavailable | Missing worktree or unavailable registration/source; protected. |
| Unknown | Ownership, inspection, lifecycle, platform support or previous cleanup evidence cannot establish safety; protected. |

Modification is also shown independently: a retained worktree can contain changes.
A completed convoy and a clean working directory do **not** release retention.
Use the existing **Apply result** or **Discard result** workflow to resolve an
unresolved result deliberately. This feature adds no automatic review approval or
new release-retention action. In particular, Applied copies still containing the
applied edits are protected by ordinary cleanup; applying changes does not make
those live edits disappear. The existing explicitly confirmed result-disposal
workflow remains separate from this conservative storage action.

Known orphan attempts can be inspected, but missing/ambiguous historical lifecycle
records cannot authorize their deletion. Malformed inventory records appear in
inventory diagnostics. Missing directories keep their ownership records; Git
registration metadata and CodeConvoy inventory are distinct. No Forget Record,
ownership migration or Git pruning is provided.

## Final safety checks

Every selected identity is frozen for confirmation; newly discovered entries are
never added. Exact duplicate identities are processed once; conflicting records
are protected. Each target is checked independently against live application state,
then goes through the existing Store intent/outcome transaction and an exclusive
manager lifecycle lease plus the common-repository administration lock. Other
CodeConvoy execution, validation and result operations cannot use the affected
repository while removal is in progress. Busy or stale targets are reported;
other safe targets continue. Unrelated jobs in the same convoy do not block cleanup.
If a target becomes busy or its preview changes before any cleanup journal write
is attempted, the saved rejection leaves it retryable after a fresh result Review
and cleanup confirmation. Nothing is retried automatically.

The worker re-reads original bindings/manifests, canonical source/common Git
identity, base object, detached registration, CodeConvoy ownership lock and both
Git backlinks. It rejects symlinks/reparse points in the trusted path chain and
compares session filesystem identities with the selected target. Current status
includes ignored content; **all ignored content blocks ordinary cleanup**, even
build output. Staged edits, conflicts, untracked files, changed HEAD, pending Git
operations, sparse/assume-unchanged entries, submodules, unsupported conversion
attributes and inspection failures also protect the worktree. Detached HEAD is
expected for managed worktrees and is not itself a reason to remove one.

A fresh disposable index forces Git to compare tracked contents without trusting
the live index's timestamp cache. The real indexes, HEADs and source files are
preserved. Only temporary index data is written for this check; there is no
reset, clean, stash, commit or object-writing snapshot step.

After durable intent, the worker rechecks, unlocks **only** the verified CodeConvoy
registration, rechecks again and runs `git worktree remove -- <exact path>` with
no force flags. There is no recursive filesystem fallback. Failure attempts to
restore the ownership lock only when the same target is still verifiable. Failures
after a journal write attempt and unconfirmed process cleanup stay protected;
failure and crash journals remain for diagnosis, and ordinary cleanup cannot retry them
without a separate resolution flow. Success requires verified absence of both the
checkout and its Git registration, a completed journal and a saved Store outcome.
Small ownership/journal tombstones outside the checkout remain.

Closing during a batch skips remaining targets. Once unlocking begins, normal
shutdown waits for the bounded Git operations to finish removal or attempt safe
lock restoration. An interrupted or uncertain transition remains protected across
restart. Abrupt operating-system termination still cannot guarantee completion.

These locks coordinate CodeConvoy operations, not external editors, Git commands,
or hostile processes with the same filesystem permissions. Do not replace or edit
these paths externally during cleanup. Repeated path/content checks and non-force
Git removal narrow external races; they are not a filesystem security sandbox.

## History, size and platforms

Cleanup preserves convoy/job IDs, immutable task/settings, saved completion and
result observations, validation history and resolution. Available in-memory Review
statistics are preserved as observations. A cleaned entry says the live copy is
removed; validation never falls back to the original repository. Diffs are live
views in this version, not persisted patches: existing snapshots/observations are
not fabricated after removal, and a removed checkout cannot supply a new Diff.
History removal remains a separate metadata-only action.

Confirmation shows the **previously measured logical size**, a subtotal that can
include partial measurements. It is not a promise of reclaimed disk allocation.
Any cleanup completion invalidates old measurements; an open Storage Overview
refreshes after the batch, and manual **Refresh** remains available. Each target
has a Removed, Skipped or Failed outcome with diagnostics.

Ordinary cleanup is enabled on Unix platforms using device/inode identity checks.
It is disabled on Windows in this release because this implementation does not
provide equivalent stable filesystem identity checks there. Windows inspection
and measurement remain available, and reparse points are rejected. Linux automated
validation does not imply native macOS or Windows validation. See the
[validation record](worktree-cleanup-validation.md) for performed checks.
