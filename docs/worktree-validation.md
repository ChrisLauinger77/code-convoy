# v0.3 Part 3.1A-2 integration and validation

This is the historical Part 3.1A record. Part 3.1B is now covered by the
[recovery and cleanup validation record](worktree-recovery-validation.md).
The [1.0 readiness PR 2 audit](#10-readiness-pr-2-audit-2026-10-13) below records
the current execution audit, Unix supervision fix and verification boundaries.

Validated on 2026-10-07 on macOS, on `codex/v0.3-worktree-core`.
Part 3.1A-1 was audited and extended in place. No merge, PR, release, version bump,
or Part 3.1B/3.2 implementation is included.

## Core audit

The existing execution-mode snapshots, exact reviewed committed-HEAD baseline,
random persistent attempt directories, sibling ownership manifests, detached and
locked Git worktree creation, hook suppression, common-directory administration
mutex, mode-aware scheduler leases and backend checkout substitution were retained.
All four backends keep their own command construction, transport and completion
rules. Direct mode retains dirty-tree review, baseline revalidation, nested-root
protection, exclusive repository access and live staged/unstaged diff.

The audit found three lifecycle gaps: uncancellable Git inspection steps around
creation, no application lifecycle Activity entries, and saved result observations
presented without an explicit restart-validation boundary. These are addressed.
An additional diff edge case is covered: staged content can differ from base even
when the working file has been restored to base.

## Lifecycle and result contract

- Individual Stop, Stop Convoy, Stop All and confirmed quit signal preparation and
  agent cancellation. Git/process-tree cleanup is awaited before releasing the
  lease and capacity. A job cancelled during preparation never reports Running
  or starts an agent. Admission closes synchronously on confirmed quit.
- Cancel in the quit dialog has no job side effects. The dialog counts Preparing
  separately from Running. Completed retained results do not prevent exit.
- Preparation's administrative mutex is released on success, failure and confirmed
  cancellation. Jobs waiting for it can be cancelled without disturbing its owner.
  Unconfirmed process cleanup retains the existing quarantine policy; a later
  inspection cannot overwrite that unsafe flag. Partial Git state is retained,
  never deleted to make a subsequent job work. Independent new attempts continue.
- Agent execution status remains Succeeded/Failed/Cancelled. `WorktreeResult` is a
  separate existence/change observation, with unknown represented explicitly.
  All six terminal-status and changed/unchanged combinations are tested.
- Changed means Git finds a difference between the fixed base and either working
  files or the index, or reports a nonignored untracked entry. Binary changes,
  deletions, renames, agent commits and Git-reported submodule differences count.
  Ignored files alone do not count. Exit status, output and timestamps are not
  evidence of changes. Comparison errors remain unknown.
- Both tracked comparisons use checked Git exit codes with external diff and
  text conversion disabled. Diff renders both comparisons and untracked names;
  untracked file contents are not converted into patches or staged.
- Retention is conservative: changed, unchanged, failed, cancelled and partially
  created worktrees remain. This also preserves useful ignored files. Final result
  inspection has a five-second deadline, then cancels and awaits Git cleanup;
  an unavailable observation does not discard data or invent “no changes.”
- Activity prefixes application lifecycle entries with `[CodeConvoy]`. Backend
  messages remain distinguishable; backend Raw output is unchanged. Errors give
  short actions with expandable diagnostics. Task & settings shows immutable
  mode, short base commit, repository/settings, concurrency, attachments and
  result observation; storage paths are collapsed.
- Saved observations reset their session-only trust flag on load. History says
  “not checked after restart.” Loading does not inspect, reconcile or claim that
  real retained checkouts have recovered. Missing fields in v0.2 state default to
  Direct and absent worktree metadata. State schema version remains 1.
- Reuse copies draft configuration and context under existing validation rules.
  It carries no old checkout path, base, process or result state into new jobs.
  Groups remain explicit deduplicated selections; templates remain task-only.
  Editing/deleting either library after launch cannot mutate job snapshots.

## Automated coverage

`tests/worktrees.rs` contains 22 real-Git fixture tests. No authenticated CLI is
required. They cover:

| Area | Evidence |
| --- | --- |
| Preparation cancellation | Gated Git checkout with a child process; individual/convoy/all/shutdown stops, no agent start, retained owner manifest, released process lock and subsequent admission |
| Convoy isolation | Stop Convoy cancels its preparing and queued jobs while a separate convoy continues |
| Preparation collisions | Two isolated jobs share source identity; cancelling the mutex waiter leaves the owner running and capacity available |
| Result inspection | Bounded Git filter cancellation leaves an unknown observation, retains edits and confirms process cleanup |
| Agent outcomes | All six terminal/change combinations, failed/cancelled retained files, peer continues after cancellation |
| Git semantics | Dirty-source exclusion, fixed base despite later source commits, working/index differences, ignored and untracked files, binary data and agent commits |
| Scheduling/locking | Global and per-convoy limits include preparation, failed preparation/spawn and cancellation release capacity, Direct waits for shared Git identity, nested protection and round-robin unit regressions |
| Attachments | All four backends receive external text/images using native transport, spaces/Unicode, missing or modified queued context rejected, capability mismatch rejected before creation, no copying into checkout |
| Persistence/reuse | v0.2 defaults, metadata round trip, interrupted preparation, saved observation remains unvalidated even if files moved, new path/base on reuse, metadata-only history cleanup |
| Ownership/paths | Destination collisions, manifest mismatch, missing repository, suppressed checkout hooks, separate Git administration with a trailing carriage return; Linux also exercises non-UTF-8 path bytes |

UI/domain tests additionally cover Cancel quit, confirmed quit event draining and
persistence, retained-result exit, current versus saved result labels, isolated
reuse, group overlap and library mutation, template mode preservation, keyboard
navigation and compact layout. Existing direct-mode, backend, scheduler, attachment,
CLI discovery/availability and library suites run with the same required checks.

## Native macOS scenarios

Used the `test-support`-gated `v03_worktree_validation` app and disposable repositories.
The helper's own temporary fixture workspace is removed when the helper exits;
ordinary CodeConvoy result retention is separately verified by persistence tests.

| Scenario | Observed result |
| --- | --- |
| A — cancellation | Preparing displayed; Cmd+Q showed Preparing 1 / Running 0; Cancel left preparation active. Stop job produced Cancelled, no agent launch, retained partial state and clean registered source. |
| B — failed result | Fixture changed `tracked.txt` then exited 7. UI showed Failed plus Isolated changes retained. File still contained `failed`; source retained committed content. |
| C — same repository | Two convoys showed Running concurrently in distinct paths. Their files contained independent `one`/`two` edits. Stopping the second left the first active; the first then succeeded. Source was unchanged. |
| D — Direct | Switched to Current working tree; fixture succeeded and wrote its input in the registered checkout. Saved job had Direct mode and no worktree/result metadata. Diff retained staged/unstaged sections and listed the untracked input file. |
| E — attachments/reuse | Reused an isolated convoy and added `outside context 日本語.md` via the native picker. Review showed the attachment, mode and base. Fresh worktree succeeded; captured stdin contained the exact sentinel and filename; the attachment was not copied. |
| Confirmed quit | Started another gated preparation, requested Cmd+Q and chose Stop jobs and quit. The app exited after cancellation. Automated tests verify persisted terminal state and unchanged retained history. |

Task & settings displayed a 12-character base, retained-result state, original
settings and a collapsed Worktree location. Activity showed application lifecycle
messages separately from real fixture events. Six saved native runs demonstrated
Direct success and isolated success/failure/cancellation with retained changes.

## Checks and platform boundary

- `cargo fmt --check` — passed.
- `cargo clippy --all-targets --all-features -- -D warnings` — passed.
- `cargo test --all-features` — passed; 188 top-level tests, 4 existing optional
  installed/authenticated-provider probes ignored. Subprocess self-tests also pass.
- `cargo build --release` — passed.
- `python3 -m unittest discover -s packaging -p 'test_*.py'` — 16 passed.
- `git diff --check` — passed.

No dependencies, release artifact names, packaging scripts or CI matrix changed.
DEB, RPM, AppImage, Windows setup/portable ZIP and Universal macOS DMG continue using
the existing Cargo binary and platform data-directory lookup. Paths are passed as
native arguments, relative/absolute Git common-directory paths are canonicalized,
and Windows drive/separator handling remains delegated to native path APIs. Unix
path bytes are preserved; CRLF trimming applies only on Windows. Manifest/state
serialization still requires representable paths and fails rather than silently
substituting a lossy path. Core behavior uses no Unix shell utilities.

Linux/Windows native execution, Linux non-UTF-8-path execution, exact packaged
artifacts and authenticated provider runs were not performed here. The existing
three-platform CI remains configured, but remote CI was not dispatched or claimed
as passing. Worktrees share Git administration; external Git tools and deliberately
escaping agent subprocesses remain outside the existing coordination boundary.

## Exact remaining Part 3.1B work

1. Discover persisted job records and orphan/partial attempt manifests on restart;
   reconcile them with actual Git worktree registrations and filesystem state.
2. Validate ownership, canonical paths, repository identity, base availability and
   worktree existence before promoting saved observations to recovered results.
   Missing, moved, damaged or ambiguous resources need truthful explicit states.
3. Handle interrupted preparation, stale administrative state and orphaned results
   conservatively, including proving that no active process/job owns a resource.
4. Implement explicit user-facing safe cleanup of proven owned resources, using
   Git-aware handling of retained locks/registrations and preserving uncertain paths.
   Keep history removal independent from filesystem cleanup and keep run IDs stable.
5. Add restart/crash/stale-state/cleanup tests and native platform validation for
   those recovery operations. Do not claim running agents resume after app exit.

Apply/Discard, convoy-wide Review, Retry and the other later-part features remain
outside this work; no such actions were added.

## 1.0 readiness PR 2 audit (2026-10-13)

Audited from merged PR 1 (`67ca0e2`) on Linux using disposable repositories and
fixture CLIs. This is execution/worktree stabilization only. The package remains
0.10.0; no release was prepared or published. The initial handoff identified a Unix
descendant-exit blocker. The maintainer-authorized follow-up implements the scoped
fix described below. This audit does not declare 1.0 ready.

### Execution paths and existing regression coverage

| Path | Checks and evidence |
| --- | --- |
| Convoy snapshots and selection (`domain`, `library`, `runner`) | `tests/core.rs`, `tests/library.rs`, `tests/workflows.rs`, `tests/convoys.rs`: overlapping selections, nested registration rejection, immutable task/settings/context, multiple convoys, deterministic admission, global/per-convoy limits, blocked jobs using no slot |
| Agent execution (`agents`, `process`, `runner`) | `tests/execution.rs`, `tests/convoys.rs`, `tests/platform_process.rs`: all four backend invocations, literal arguments/stdin/cwd, missing executables and startup/build/protocol failure, bounded output, cancellation scopes, repeated shutdown, descendant termination and peer isolation |
| Creation and location (`worktrees`, `worktrees/location`, manager leases) | `tests/worktrees.rs`, `tests/worktree_locations.rs`: reviewed committed base, dirty-source exclusion, concurrent unique attempts, missing repositories, invalid/unwritable storage, symlinks, manifest/binding write failure, retained partial state, custom and previous base directories, no Direct fallback |
| Git observations (`git`, `worktrees`, `review`) | `tests/core.rs`, `tests/repository_health.rs`, `tests/worktrees.rs`, `tests/worktree_recovery.rs`: dirty/index/untracked/conflicted changes, detached/unborn HEAD, missing Git state, index alternatives, fixed-base diffs, agent outcome independent of modifications, immutable real indexes |
| Validation (`validation`, lifecycle permits) | `tests/validation.rs`, `tests/validation_presets.rs`: original retained directory with no source fallback, explicit manual start, historical identity mismatch, unavailable executable/directory, process cancellation and shutdown, independent agent/validation outcomes, exclusion against cleanup and active work |
| Ordinary manual cleanup (`storage/management`, Store transaction, `worktrees/cleanup`) | `tests/support/managed_cleanup.rs`, `worktrees/cleanup_tests.rs`: exact ownership/backlinks, changed preview, late edits, dirty/staged/untracked/ignored files, unknown/user worktrees, active jobs/validation, exclusive lease, journal/save failures, bulk continuation and deduplication, no force removal or recursive fallback |
| Recovery and explicit result resolution (`persistence/results`, `worktrees/recovery`, `worktrees/apply`) | `tests/worktree_recovery.rs`: restart identity reconciliation, metadata pinned through history removal, missing/replaced directories, intent/outcome persistence, uncertain writes preserved, Apply/Discard ownership and concurrency guards |

Ordinary cleanup remains separate from the existing explicitly confirmed Discard
operation. Discard intentionally disposes of a reviewed owned result and uses its
existing guarded removal primitive; ordinary cleanup cannot route around its
cleanliness checks through that operation. No cleanup policy was relaxed.

### Safety fixes

1. **Protect the actual running checkout.** Previously an isolated job's scheduler
   lease tracked its source/common Git directory but omitted the created checkout.
   Registering a new real nested repository there allowed a Direct job to run
   concurrently. A real-Git regression failed before the fix. The worker now binds
   the reserved path through the manager before Git creation, rechecks conflicts,
   and retains that path through completion or unsafe-cleanup quarantine. Separate
   isolated attempts of the same source still run concurrently.
2. **Cancellation through a full Activity queue.** An agent could already be alive
   when the runner blocked on its isolated-startup Activity message, before entering
   the process executor. The regression failed with the parent and descendant
   retaining their OS locks until the UI queue drained. That send now observes
   cancellation, allowing the executor to terminate the tree first. Repeated Stop
   and repeated shutdown both pass with the UI queue deliberately left full.
3. **Respect Windows environment-name matching.** Git and all four agent launchers
   previously stripped only uppercase `GIT_*` names, unlike validation's existing
   case-aware rule. They now share the matching rule; inherited `Git_Dir` or
   `gIt_WoRk_TrEe` cannot bypass stripping on Windows. Unix matching remains case
   sensitive, original names are removed, and unrelated environment is preserved.
   `validation::tests::repository_overrides_follow_platform_environment_case_rules`
   exercises both `windows = false` and `windows = true` on every platform,
   including mixed-case names, unrelated names and non-ASCII boundary input.
   Native Windows CI additionally verifies actual command environment removal.
4. **Cancel a pending worktree binding without waiting for Activity.** The manager
   can be blocked sending a queue notification while a worker waits for its
   `BindWorktree` reply under the administration guard. That reply wait now selects
   cancellation, with cancellation/shutdown checks before enqueueing and after
   acknowledgement. The manager rejects abandoned, cancelled and stale requests.
   A late acknowledgement cannot authorize Git creation; cancellation drops the
   administration guard before Activity drains. Existing lifecycle-event delivery
   and scheduler completion ordering remain unchanged. Three deterministic tests
   cover backpressure, Stop/shutdown races and rejected manager requests; the
   backpressure regression failed before the fix.

New tests also cover late lease conflicts, quarantine protecting the actual
checkout, continued admission of an unrelated job, one completion after repeated
cancellation, and cancellation after a failed leader leaves a descendant holding
its inherited output pipes. That last test verifies the descendant's OS lock is
released while an independent process tree stays alive and finishes normally.

### Unix process-exit blocker and follow-up fix

A disposable diagnostic reproduced a separate existing gap in `process::execute`:
a launcher spawned the fixture's `validation-child` with stdin, stdout and stderr
redirected to the null device, waited for `child.ready`, then exited with code 7.
The descendant stayed in the same process group. `process::capture` returned code
7 while the descendant still held `child.lock`. The diagnostic explicitly wrote
the fixture's release marker and confirmed the lock was released afterward.

The original `process-wrap` 10.0.1 Unix group wait reaped the direct child and used
`waitpid` for children that the application can reap. A reparented grandchild is
not necessarily one of them. Once output pipes closed, CodeConvoy could treat
execution as finished and release its repository lease despite a surviving
writer. A later Stop no longer had an active job to cancel. This was more than the
known limitation of processes deliberately escaping their process group.

The fix retains the existing process groups and manager leases. A small Unix
guard observes leader exit with `waitid(WNOWAIT)` while keeping the child unreaped,
then terminates remaining group members before allowing the reaping wait. Every
destructive signal, including Drop and repeated cancellation, requires the owned
unreaped identity. Before reaping starts, signalling is permanently disarmed;
only the owned direct child is reaped, bypassing the library's group-wide wait
which could select another child after numeric-ID reuse. Only signal-zero group
probes remain. Completion requires group absence (`ESRCH`), never
just the leader's exit, pipe EOF or successful delivery of a kill signal. A reused
group ID can cause conservative refusal/timeout, but is never sent a late kill.
The CLI's original exit code survives this cleanup, including success or failure.
Leader-exit polling backs off from 5 ms to 80 ms to reduce idle wakeups during
long-running jobs. Cancellation interrupts that sleep; the short, five-second
post-reap cleanup still probes every 5 ms. A failed termination leaves the guard
anchored for a safe Drop retry while the leader remains unreaped.

Missing or invalid spawn IDs return an identity-construction error instead of
panicking. That error stays with the already-launched managed child and propagates
through wait/cancel, preserving the caller's unsafe-until-supervised protection.
Returning a spawn error after launch would incorrectly bypass that protection.
Drop cannot signal an unconfirmed identity.

The guard rejects `SIGCHLD=SIG_IGN` and `SA_NOCLDWAIT` before spawning. CodeConvoy
must remain the exclusive waiter for its children; an unexpected lost-child error
disables all further signalling. A five-second cleanup deadline bounds waiting
for init/launchd to reap orphan zombies. Errors and timeouts propagate even with
open pipes, leaving the existing repository/capacity quarantine and history pins
intact. A later inspection or best-effort Drop kill cannot mark that failure safe.
No scheduler redesign, supervisor daemon, global subreaper or dependency-version
change was needed; Unix now directly declares the already-resolved `libc` package.

Linux's [waitid documentation](https://man7.org/linux/man-pages/man2/waitid.2.html)
describes non-reaping observation. Apple's
[waitid implementation](https://github.com/apple-oss-distributions/xnu/blob/main/bsd/kern/kern_exit.c)
also supports `WNOWAIT`, but can leave the information buffer unchanged for
`WNOHANG`, so the buffer is zero-initialized. Apple's
[group signalling implementation](https://github.com/apple-oss-distributions/xnu/blob/main/bsd/kern/kern_sig.c)
can report `EPERM` for a zombie-only group. On macOS, that response after observed
leader exit allows reaping, but still requires a subsequent absence proof. It is
never treated as successful cleanup by itself. The
[signal-zero contract](https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man2/kill.2.html)
provides the non-signalling probe. Platform CI must verify these semantics natively.

Additional deterministic coverage:

- Closed/redirected stdin, stdout and stderr with a live descendant holding an OS
  lock; both parent exit 0 and exit 7. The test reproduced the original failure
  before the fix and now requires lock release before `execute` returns.
- Direct and isolated manager jobs with a same-repository waiter; completion and
  admission occur only after descendant cleanup. Injected termination failure
  instead preserves quarantine, history protection and retained worktrees, while
  an unrelated job still runs.
- Actual unexpected reaping and modelled numeric-ID reuse using a separate live
  fixture group; identity loss and a post-reap timeout forbid subsequent kills.
- Missing, zero, init and out-of-range spawn identities return errors without
  panicking. An injected construction failure with a real live child makes wait
  and cancellation fail promptly without signalling it; fixture cleanup uses the
  separately retained valid identity.
- A supervision failure while a descendant holds output open returns an error
  without waiting for EOF. Dedicated subprocess tests reject automatic reaping
  settings without launching the requested command.
- Existing future-drop, repeated cancellation, shutdown, backpressure, preparation,
  validation and peer-isolation tests continue through the same executor.

### Validation and remaining boundaries

On Linux 7.2.9 with Rust 1.96.1:

- `cargo fmt --check` — passed.
- `cargo clippy --all-targets --all-features -- -D warnings` — passed.
- `cargo test --all-features` — 591 top-level tests passed; 9 existing optional
  tests ignored. Subprocess environment probes also passed.
- `cargo build --release` — passed.
- `python3 -m unittest discover -s packaging -p 'test_*.py'` — 23 passed.
- `git diff --check` — passed.

The concurrency, backpressure and closed-output descendant regressions were first
reproduced failing, then passed after fixes.
All destructive fixture operations target disposable directories. Tests use
`TMPDIR=/var/tmp` outside the desktop sandbox because its synthetic `/tmp/.git`
marker causes the deliberate nested-storage rejection checks to reject fixtures.

No authenticated provider task, native GUI session, or packaged application
verification was performed locally. Native macOS/Windows follow-up CI results are
reported in the PR handoff; Linux test counts above are not cross-platform claims.
Existing optional tests remain ignored. Ordinary cleanup is deliberately unavailable on Windows until stable
filesystem-identity proof exists. External Git programs remain outside manager
coordination; worktrees are isolation, not a security sandbox. Unconfirmed cleanup
continues to quarantine ownership/capacity rather than admitting conflicting work.
The process-group boundary still cannot contain commands that deliberately create
a new session/group, and abrupt termination of CodeConvoy itself cannot guarantee
Unix cleanup. Stale PIDs are never signalled on restart. Unreaped orphan zombies
or rapid ID reuse may conservatively quarantine a repository even after all its
writers have stopped. Users must inspect processes before restarting after an
unconfirmed cleanup; the existing quarantine lasts for the current session.
PR 3 has not begun.
