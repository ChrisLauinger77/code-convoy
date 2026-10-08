# Part 3.3 completion and v0.3 readiness

Implemented on `codex/retry-followup-convoys`, based on merged Parts 3.1/3.2
(`5a3ad70`). This prepares the final Isolation & Review feature slice; the application
version remains 0.2.0 pending a separate release request. No dependencies, packaging
formats, backend protocols, tags or releases change.

## Completion report

1. **Retry model:** a new one-repository convoy uses the ordinary `(run_id, 0)` job
   identity. Optional Retry provenance records source run/job and attempt ordinal.
   This fits immutable run snapshots without duplicating the source's other jobs or
   introducing a generalized attempt scheduler.
2. **History:** the original status, timestamps, task, session Activity/Raw and retained
   result remain unchanged. New jobs start queued with independent timestamps/output.
   Attempt labels and source navigation identify each run. Logs remain session-only,
   as in earlier versions. Retrying a retry increments its ordinal; separate retries
   of the original are siblings with distinct convoy IDs and the same ordinal 2.
3. **Direct:** Retry reviews the current registered working tree, requires the ordinary
   dirty acknowledgment, and rechecks branch/HEAD/status at scheduler admission.
   It never resets to the old baseline or discards existing edits.
4. **Isolated:** Retry captures current committed HEAD at review and reserves a fresh
   owned worktree. No previous files/resources/baseline are imported. Old changed,
   unchanged, failed and cancelled results remain independent until explicit resolution.
5. **Attachments:** original references and metadata are reused, then revalidated off
   the UI thread and again at execution. Missing, unreadable, changed or incompatible
   context blocks Retry with concise recovery guidance and expandable diagnostics.
   No contents are persisted or reconstructed.
6. **Scheduling:** retries use ordinary manager admission, fair round-robin scheduling,
   both concurrency limits, canonical/common-directory/nested-tree locks and lifecycle
   leases. There is no priority lane. Stop job/convoy/all and quit remain shared;
   retry cancellation never mutates the original attempt.
7. **Follow-up:** independent Review checkboxes select one or multiple results. New
   convoy from selected replaces NEW CONVOY only, focuses an empty task and starts
   nothing. It accepts any result status/resolution, even absent old worktrees.
   Missing/unregistered repositories are omitted visibly and never registered implicitly.
8. **Copied settings:** explicit registered source paths (deduplicated), selected backend,
   that backend's options, Direct/Isolated mode and per-convoy concurrency. Global
   concurrency, groups, templates and appearance remain user preferences. No outputs,
   diffs, old paths, results, sessions or Apply/Discard state enter the draft.
9. **Follow-up attachments:** always empty by default. Users add new context explicitly;
   Templates remain the existing text-only way to choose a follow-up task.
10. **Provenance:** optional tagged Retry/FollowUp metadata on runs and an optional
    follow-up draft origin. History/details, launch review and Task & settings show
    source text. Source navigation is offered only when history still contains it.
    No dependency, sequencing or agent continuation is implied.
11. **History cleanup:** existing ownership protections remain authoritative. Retrying
    or creating a draft does not resolve/purge old results. Removing eligible source
    history does not invalidate a later retry or draft; saved source IDs still display.
12. **Migration:** additive defaulted fields preserve v0.1/v0.2 and Part 3.1/3.2 state;
    old jobs remain Direct without worktrees or artificial attempt objects. A populated
    v0.2 upgrade test preserves registrations, groups, templates, options, limits,
    attachments and history without filesystem work. Appearance now persists as
    System/Dark/Light. Earlier releases stored overrides only in memory, so a missing
    choice defaults to System rather than claiming to recover an unsaved override.
13. **GNOME E2E:** disposable native macOS group SmartAutoMoveNG, HeadsetControl and
    messagingmenu used real Git and a local fixture agent with attachment transport.
    messagingmenu was deliberately cancelled with retained changes; UI Retry produced
    successful convoy #3/attempt 2. Original #1 remained cancelled with an independent
    retained result. SmartAutoMoveNG Apply changed only the destination working file;
    HEAD and index stayed intact. Follow-up selected only HeadsetControl/messagingmenu,
    kept Isolated/limit 4, cleared context, focused the editable task and started nothing.
14. **Package E2E:** disposable homebrew-cask and scoop-bucket results were selected into
    a separate follow-up draft with exactly those registered sources. No package-specific
    runtime logic, production package checkout or actual release update was involved.
15. **Cross-platform:** new runtime code uses native PathBuf identities, existing Git/CLI
    services, serde metadata and egui keyboard widgets. No path splitting, shell
    commands or platform service was added. The unchanged PR CI matrix runs formatting,
    clippy, tests, packaging checks and release compilation on Linux/macOS/Windows,
    including the Windows software renderer smoke test. Consult PR checks for CI results;
    interactive Linux/Windows sessions were not performed on this macOS host.
16. **Packaging:** 16 Python packaging/release tests pass. Public macOS universal DMG,
    Windows x86_64 ZIP and Linux artifact conventions and version authority are unchanged.
    This task neither built signed distributable packages nor published artifacts.
17. **Checks:** local `cargo fmt --check`, all-target/all-feature clippy with warnings
    denied, `cargo test --all-features`, release build and packaging tests pass. There
    are 269 top-level passing Rust tests (16 more than Part 3.2), four existing optional
    installed/authenticated probes ignored, and two nested subprocess self-tests pass.
18. **Limitations:** retries are separate one-job convoys; there is no bulk Retry or
    editable Retry configuration. Use Reuse convoy for deliberate context/settings edits.
    Follow-up replaces the current draft; repository and Review selections remain
    session-local. Application leases cannot prevent external Git/filesystem races.
    Apply retains the conservative exclusions and uncertain-outcome protections from
    Part 3.2; an uncertain Apply has no in-app acknowledgment/repair flow. Authenticated
    model behavior is not established by fixture E2E.
19. **Release blockers:** no additional functional blocker was found in the completed
    local readiness checks. Cross-platform PR checks must pass before merge/release.
    Existing conservative Apply exclusions and manual inspection after an uncertain
    write remain documented product boundaries. A release still requires the separate
    version/release-notes/packaging workflow; this PR is not a v0.3.0 release.
20. **Deferred:** pipelines/DAGs, automatic chaining, commit/stage/push/PR actions,
    merge/rebase/conflict resolution, backend #5, cloud/collaboration, background agent
    continuation, generalized sessions, terminal and storage dashboard remain excluded.

## Reproduction and evidence

Build the feature-gated, non-shipping native helper and fixture agent:

```sh
cargo build --all-features --example v03_continuation_validation --bin codeconvoy-test-agent
cargo run --all-features --example v03_continuation_validation -- --prepare /path/to/new/disposable-directory
cargo run --all-features --example v03_continuation_validation
```

The helper refuses an existing directory and seeds five new local repositories,
two user-managed groups, a text template and a Markdown attachment. A handshake
cancels one GNOME job; all other fixture jobs succeed with retained changes. It
uses a separate Store and never touches the normal CodeConvoy data directory.
Local native evidence used `/private/tmp/codeconvoy-part33-native-01` on macOS.

Native verification covered original/attempt selection, Retry review and launch,
Activity origin, Apply, independent Review multi-selection, both follow-up groups,
task focus/editing, no automatic execution, restart provenance/result recovery and
explicit accessibility names. System/Dark layout was inspected, then Light was
selected and persisted/reopened. Keyboard behavior is also driven deterministically
through actual egui Tab/Space/Enter events in `task_context_tests`: both row
checkboxes, New convoy from selected, selected job and Retry are reachable.

`tests/continuation.rs` covers eligibility, independent snapshots/output, prerequisites,
current Direct review, deduplication/omissions/all backend settings, both modes,
source deletion, restart and populated v0.2 upgrade. `tests/worktrees.rs` adds actual
fresh isolated retry/cancellation and Direct retry waiting for global capacity or
repository locks, then rejecting a changed baseline. `tests/worktree_recovery.rs`
runs a successful retry and proves the old failed result can still Apply or Discard
without changing the retry. UI tests cover draft preservation, no auto-launch,
registration/quit guards, accessibility, diagnostics and appearance persistence.
All existing recovery, Apply/Discard, groups/templates/attachments, backend,
scheduler, cancellation and quit regressions remain in the full suite.
