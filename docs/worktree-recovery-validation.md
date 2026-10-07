# Part 3.1B — worktree persistence, recovery and safe cleanup

Validated on macOS on 2026-10-07. Work remains on
`codex/v0.3-worktree-core`; no merge, release, PR or Part 3.2 action was performed.

## Existing implementation audited

Part 3.1A's `ExecutionMode`, immutable source/base snapshot, sibling ownership
manifest, detached locked checkout, shared common-directory administration mutex,
isolated/direct scheduler leases, managed process cancellation, Git-based result
observation, fixed-base diff, Activity events and version-1 state were retained.
Backend command construction, attachment transport, group/template behavior and
round-robin admission were not redesigned.

## Completion report

1. **Persisted metadata:** existing run/job, repository/common Git identity, native
   checkout path, base commit, owner marker and result observation remain. Added
   defaulted result availability; session trust resets on load. No file contents,
   credentials, backend output or raw diagnostics are newly persisted.
2. **Startup reconciliation:** background, sequential per-result checks deliver
   reports promptly and save received transitions deliberately. No agent resume,
   replay or silent worktree recreation occurs.
3. **Ownership verification:** trusted Store boundary, exact manifest, source/common
   identity, detached locked Git registration, private administration/backlink,
   usable base object and successful Git inspection. Traversal, symlinks/reparse
   points, substitutions and mismatches fail closed.
4. **Availability:** Available is freshly validated; Missing means absent storage
   or checkout; Stale means source/base/Git inspection is unavailable; Invalid means
   ownership mismatch. CleanupPending/Failed/Cleaned describe removal independently
   from agent outcome and last-observed changes.
5. **Internal cleanup:** `Store::cleanup_result` persists intent, obtains a lifecycle
   lease, verifies ownership, journals intent, rechecks, removes the exact worktree
   through Git, verifies absence and saves the outcome. No UI Discard is exposed.
6. **Locking:** cleanup rejects active preparation/execution and competing operations
   for the same repository. Existing direct/nested safety remains; unrelated
   repositories continue. Leases do not consume agent slots. Unconfirmed process
   cleanup quarantines access, and health is updated before releasing administration.
7. **Retention:** changed and unchanged results remain; nothing is purged because of
   age, startup, failure, history size or an unavailable source.
8. **Terminal/interrupted recovery:** failed and cancelled changes remain available.
   Saved unfinished execution becomes interrupted/cancelled and its result is
   independently inspected. Processes are never resumed.
9. **History cap:** unresolved ownership metadata pins its convoy outside the usual
   30 completed-run allowance. Even saved Cleaned state needs current validation
   before history protection is released.
10. **Remove/Clear:** unresolved isolated runs remain represented, with disabled
    removal and explanatory text. Clear removes eligible history only. Direct
    history behavior remains unchanged.
11. **Diff:** recovery and Refresh diff validate the resource and compare its live
    files/index against the original base. Later source commits cannot shift that
    baseline. External retained-file edits appear on refresh; missing results do
    not display a cached patch as live state.
12. **Orphans:** scan immediate storage entries, report probable unreferenced or
    malformed resources, never delete/import ambiguous state. Diagnostic output is
    bounded at 100 entries. Completed absent-checkout tombstones are excluded.
13. **Crash consistency:** creation before a state save becomes discoverable orphan
    metadata; partial preparation remains missing/stale. Cleanup intent is durable
    before removal. Matching intent plus verified absence recovers completed cleanup,
    finishes its journal, then permits history removal. Interrupted/partial cleanup
    remains failed and represented. Interrupted reconciliation simply runs next time.
14. **Platforms:** native paths and shell-free arguments; Git porcelain `-z`; Windows
    drive/extended-path normalization and reparse-point rejection. Tests include
    Unicode/spaces, Unix symlinks and real removal refusal (Unix directory permissions;
    Windows exclusive file handle). No packaging dependencies/artifact names changed.
15. **Compatibility:** populated v0.1 and v0.2-style state loads with Direct defaults;
    groups, templates, attachment metadata, preferences, concurrency and history are
    preserved. Manifest layout and application schema version remain unchanged.
16. **Checks:** all required local checks passed; results below.
17. **Native scenarios:** A–D passed with actual Git and the native egui application;
    evidence below. No authenticated agent CLI was used.
18. **Limitations:** no automatic orphan deletion or repair, no storage manager, no
    immutable archives, no user-facing cleanup, no stale-process resumption. Source
    availability and ownership must be restored before uncertain resources can be
    inspected/cleaned. Partially failed removal may require manual Git/filesystem
    inspection. Coordination cannot prevent hostile/external filesystem races or
    control processes using another data directory. Existing Unix escaped-process
    limitations remain. Linux/Windows native execution and remote CI were not run.
19. **Ready for Part 3.2:** validated live result metadata, fixed-base inspection,
    protected history, an exclusive repository lifecycle lease, durable cleanup
    intent/outcome handling and a verified exact-removal primitive. Explicit Apply,
    user-facing Discard and convoy-wide Review still need their own design and UI;
    none was implemented here.

## Automated validation

- `cargo fmt --check` — passed.
- `cargo clippy --all-targets --all-features -- -D warnings` — passed.
- `cargo test --all-features` — **230 passed**, 4 existing optional installed or
  authenticated CLI probes ignored. Subprocess self-tests also passed.
- `cargo build --release` — passed.
- `python3 -m unittest discover -s packaging -p 'test_*.py'` — **16 passed**.
- `git diff HEAD --check` — passed.

`tests/worktree_recovery.rs` adds **38** deterministic integration tests, using
real temporary Git repositories. They cover restart/change/status persistence,
fixed-base diff after source commits and external edits, inspectable diffs after failed cleanup, directory/storage removal,
external remove/prune, missing registration/source/base, manifest/path/job identity
mismatch, source/unrelated/registered checkout protection, successful and idempotent
cleanup, missing checkout cleanup, actual Git removal failure, failed intent save,
active peer protection and lock release, terminal/interrupted/unchanged retention,
history cap/Remove/Clear, fresh reuse, legacy defaults, orphan preservation, crash
windows and symlink/traversal attacks. Every fixture uses spaces and Unicode in its
source and storage ancestry. Permission-refusal testing is skipped if a Unix root
process can bypass the tested directory permissions; it ran successfully here.

Scheduler/UI tests additionally cover repository-local maintenance without agent
slot consumption, quarantine behavior, saved-result transitions, one Activity event
per transition, Raw output preservation, history protection notices and concise
recovery diagnostics. Existing direct-mode, all-backend, cancellation, attachment,
group/template, persistence, packaging and worktree-core suites remain green.

## Native validation

The feature-gated `v03_recovery_validation` example keeps its disposable fixture
workspace across app launches. `--prepare <new directory>` seeds a committed source,
an unrelated user worktree and fixture-only backend settings. Running without
arguments opens the real UI; `--cleanup` invokes only the internal cleanup path for
the newest historical result. The workspace pointer is stored beside the canonical
test executable, avoiding bundle/LaunchServices path aliases. This helper is not
shipped or used as a real backend.

Workspace used: `/private/tmp/codeconvoy-native-recovery-b-20261007`.

- **A — restart:** launched isolated fixture convoy #1 through native review. It
  changed `tracked.txt` from `committed native base` to `native`, succeeded and
  retained changes. Quit/reopened: Succeeded plus Isolated changes retained,
  `[CodeConvoy] Recovered isolated result`, and Diff showed the exact old/new lines
  against base `f91a22234ed0`. Task & settings retained mode and short base. Source
  and unrelated worktree still contained the original bytes.
- **D — history:** native Remove was disabled, and Clear removable history (0)
  preserved the convoy and files with a clear explanation. A summary-classification
  bug found during validation was fixed and covered by a regression test; the
  corrected notice was verified in the native UI after rebuilding.
- **C — internal cleanup:** after closing, invoked `--cleanup`. Only convoy #1's
  verified checkout and its exact Git registration disappeared; source and unrelated
  worktree remained byte-for-byte unchanged. Cleaned was persisted and displayed
  after restart alongside the original Succeeded execution status. Corrected the
  old generic inspection-error text so successful cleanup is not shown as an error.
- **B — external removal:** reused convoy #1 through native review, producing a
  different run #2 checkout. Quit and externally removed that exact disposable
  worktree through Git. Reopened: Succeeded remained, result showed Missing with
  restoration guidance, Activity recorded the missing result, and Refresh diff
  reported unavailability instead of showing an old patch. Nothing was recreated.

The native validation application was closed afterward. Fixture resources remain
in temporary storage for inspection. No ordinary application state or user
repositories were modified.
