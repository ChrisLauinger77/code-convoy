# v0.3 Part 3.1A-2 integration and validation

This is the historical Part 3.1A record. Part 3.1B is now covered by the
[recovery and cleanup validation record](worktree-recovery-validation.md).

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
