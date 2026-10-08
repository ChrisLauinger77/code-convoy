# Post-v0.3 bulk discard validation

Date: 2026-10-08. Focused UX/safety follow-up; package version stays 0.3.0.

## Behavior

- **Discard convoy…** beside Reuse selects unresolved retained isolated results
  from the selected terminal convoy. **History cleanup → Discard all unresolved
  results…** selects those results across terminal history.
- Whole active convoys are excluded, including terminal jobs with queued/running
  peers. No cancellation is performed. Existing lifecycle leases reject a target
  whose repository has conflicting active work; unrelated targets can continue.
- Confirmation shows the result count and convoy scope, explains effects and
  verification, and focuses Cancel. Enter on that initial focus and Escape cancel.
  The exact confirmed identities are frozen; later completed results are not added.
- Every target uses the existing Store intent/outcome transaction, manager lease,
  ownership verifier and exact Git removal/journal. Direct files, registered trees,
  unrelated worktrees, and already Applied/Discarded results are untouched.
- Successful targets become Discarded independently. Failures remain unresolved
  or DiscardPending/CleanupFailed under existing semantics. Counts and expandable
  per-result diagnostics remain in the results pane until dismissed or replaced by
  another batch; concise lifecycle Activity is prefixed `[CodeConvoy]`. Raw is unchanged.
- History removal stays separate and becomes available after verified cleanup of
  every protected copy. Failed results keep their convoy pinned. Applied retained
  copies still need **Clean up retained copy**; uncertain Apply remains blocked.
- Confirmed quit leaves remaining targets unattempted and waits for any current
  result operation to finish/cancel safely and merge. Restart reconciles durable
  per-result intent; it does not restart a batch or an agent.

## Deterministic coverage

`tests/support/bulk_discard.rs`, included by the existing real-Git recovery suite,
tests terminal convoy success and history unblocking; unchanged Direct jobs and
dirty registered-source bytes, HEAD and real index; unchanged Raw and agent status;
Applied/Discarded and active-convoy exclusion; multiple-convoy global cleanup;
unrelated user worktrees; ownership mismatch with independent peer success; durable
CleanupFailed and retry after restart; confirmation identity/registration/active
state revalidation; uncertain Apply refusal; real active fixture-process exclusion
without cancellation; and outcome-save failure after Git removal with protected
intent and journal-based restart reconciliation.

`src/ui/bulk_discard_tests.rs` tests confirmation counts, safe default Enter/Escape,
explicit keyboard confirmation, frozen selection, inspection coordination, all
active job statuses, concurrent-operation gating and quit handling. Its end-to-end
UI pump test uses real Git worktrees and the real manager, confirms globally through
egui keyboard input, merges background cleanup, verifies durable Discarded state,
and removes eligible history without reusing run IDs. It requires no desktop control.

## Checks

- `cargo fmt --check`: passed.
- `cargo clippy --all-targets --all-features -- -D warnings`: passed.
- `cargo test --all-features`: 281 top-level tests passed; four existing optional
  installed/authenticated CLI probes ignored. Subprocess self-tests also passed.
- `cargo build --release`: passed.
- `python3 -m unittest discover -s packaging -p 'test_*.py'`: 16 tests passed.

## Native Linux inspection and limitations

Built the existing `v03_review_validation` helper and fixture agent, then seeded
`/tmp/codeconvoy-bulk-discard-native-20261008`: 24 successful retained results in
convoy 1 and one failed-agent result in convoy 2. The native window rendered the
new convoy action, the history-protection explanation, and the 24-result confirmation.
Cancel's visible focus and the existing compact two-pane layout were inspected.
One fixture ownership lock was deliberately mismatched for a planned partial-failure
scenario; the UI correctly showed ownership mismatch and protected history.

Desktop input triggered a remote-control permission popup. Native interaction
testing was stopped and the newly launched fixture process was closed without
further desktop control. Native destructive confirmation, partial completion,
global completion and history removal are **not claimed as validated**. These
paths are covered by the deterministic real-Git and egui tests above. All 25 fixture
worktrees remained; source file contents, HEADs and real index hashes were checked
unchanged. The disposable fixture directory remains for optional manual validation.

Native macOS/Windows and authenticated agent runs were not performed. Existing
limitations remain: external Git/filesystem actors are outside manager leases;
Applied copies need individual cleanup, uncertain Apply needs separate resolution,
and aggregate diagnostics are session-only while per-result state survives restart.
