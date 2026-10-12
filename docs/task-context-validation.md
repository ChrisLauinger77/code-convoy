# Task context validation

Historical validation for repository groups, task templates, attachments and
Activity output developed for v0.2. The three stages below were recorded on
2026-10-06 while the Cargo/package version was still 0.1.0; they are development
evidence, not release records or a statement of the current application version.

- [Initial implementation](#initial-implementation): PR #13 and initial native UI checks.
- [UX and regression follow-up](#ux-and-regression-follow-up): specification sections 8–21.
- [Combined workflows and completion](#combined-workflows-and-completion): Part 2.3 and remaining limits.

Test counts and platform/provider limitations belong to their recorded stage.
For current behavior, see [task context](task-context.md). Later user confirmation
of Claude execution is recorded in [Claude validation](claude-validation.md);
authenticated attachment execution remains separate evidence.

## Initial implementation

Date: **2026-10-06**. Platform: **macOS, Apple Silicon**. These changes develop
the requested v0.2 features; Cargo/package version remains 0.1.0 until release
preparation. Existing release and authenticated v0.1 evidence is preserved.

This record covers the initial implementation in PR #13. See the
[follow-up requirements and validation](#ux-and-regression-follow-up) for the
separate changes addressing specification sections 8–21.

### Automated checks

All required checks passed on the final Rust sources:

```sh
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo build --release
python3 -m unittest discover -s packaging -p 'test_*.py'
```

Rust: **145 tests passed**, with **3 optional installed-CLI tests ignored**
(Copilot, OpenCode and Claude). Python packaging: **9 tests passed**.
`git diff --check` passed. No model/provider requests were made by these tests.

New coverage includes:

- Version-1 state defaults, groups/templates round trips, metadata-only
  attachment persistence, missing membership retention and immutable history.
- Group overlap/deduplication and individual selection edits; task-only template
  loading without jobs, repository/backend/attachment changes or history mutation.
- Text type/UTF-8/NUL checks, image signatures, regular/missing/oversized files,
  canonical aliases, total limits, changed digests and visible reuse omissions.
- All four text transports and two-image native/structured argument construction,
  Unicode/space/metacharacter paths, unchanged permission settings, Claude base64
  input and Codex comma-path rejection. Native flag probes reject an incompatible
  image interface before execution and share the existing probe lock.
- A queued file disappearing or changing at the same size fails before process
  spawn, while unrelated work succeeds and the repository/capacity can be reused.
- Four concurrent fixture CLIs receive attached text and retain separate actual
  raw records. Activity follows real tool events, unknown details stay available,
  and independent Codex decoders do not mix thread identifiers.
- Raw UTF-8 chunk handling, combined log memory bounds, compact editor layout
  in both themes, reused-attachment pending/stale guards and post-launch clearing.
- Existing round-robin, global/per-convoy limits, canonical/nested leases, Git
  rechecks, failure isolation, cancellation/descendants, quit and history tests.

### Native macOS UI check

A temporary verification build used its own disposable Git repositories,
state directory and deliberately missing CLI paths. It never launched an agent,
loaded provider credentials, or touched the ordinary CodeConvoy state directory.
The temporary harness and fixture app/data were removed afterward.

Verified through the native UI:

- Templates menu loads task text while keeping attachment/backend settings;
  historical task and run state stay unchanged.
- Add files opens the native macOS sheet, supports simultaneous Markdown/PNG
  selection, adds the PNG and deduplicates the existing Unicode/space text path.
- Selecting a group selects its two registered members. Creating/saving another
  group retains the existing explicit selection and run.
- Missing group membership is visibly indicated; the group editor opens with
  name and membership controls, including actionable name validation.
- Activity and Raw output show separate fixture text/JSON. Task & settings
  retains historical attachment metadata even when the draft has more files.
- The scrolling editor keeps execution controls visible and the dark desktop
  layout remains compact. Automated layout checks additionally cover light
  theme and the minimum 310-point editor width.

### Remaining platform/provider evidence

Authenticated attachment execution has **not** been verified for any backend in
this change. Previous Codex/Copilot E2E checks remain valid for their original
invocations; OpenCode/Claude authenticated E2E remains unverified. Model-specific
vision/context limits, external image permission behavior and Claude partner
limits must be checked against a real configured CLI/provider before claiming
that evidence. Current source/contract checks are detailed in
[task context](task-context.md).

Linux/Windows native multi-file picker/drop behavior and Windows native path
runtime checks were not performed here. Native drag-and-drop is implemented
with egui's existing dropped-file API and no new dependency, but was not
interactively verified. Other-platform CI/runtime results are not claimed.

[Codex session investigation](codex-sessions.md) documents the installed CLI's
public help and ephemeral persistence constraint. No completed/cancelled-session
resume or desktop-visibility provider experiment was performed; no unsupported
continuation action is exposed.

## UX and regression follow-up

Date: **2026-10-06**, **macOS / Apple Silicon**. This follows the initial
implementation in PR #13 and checks sections 8–21 of the v0.2 specification.
The Cargo version remains 0.1.0; this is development work, not a release.

### Changes

- Template Load/Edit actions and attachment Remove actions expose the target's
  name to assistive technology. Group selection and library text fields have
  associated labels. Existing compact visible button labels are preserved.
- Attachment picker completion, including cancellation, restores focus to
  Add files. Removing a draft attachment also returns focus there; neither
  action changes the original file.
- Task & settings shows convoy and selected-job durations alongside the
  existing timestamps, immutable task/options/repositories and attachment
  metadata. It does not read historical attachment contents or display hashes
  and arbitrary saved option keys.

### Requirements checked

| Specification | Evidence and behavior |
| --- | --- |
| 8: New Convoy | Task/Templates → Attachments → Agent → Groups/Repositories, with Execution/Run fixed below. Existing compact light/dark layout tests pass. Accepted-launch tests now also verify that groups, templates and original attachment files survive clearing the draft. |
| 9: History | A rendered snapshot test removes the original attachment, then checks filename/type/size, reference guidance, repository list and both durations. Existing snapshot/reuse tests retain independent task/options/repositories. |
| 10–12: Safety, scheduler, backends | No execution, Git, scheduler, process or backend command code changed. Existing dirty-state/baseline, locking/nested-path, admission/fairness, cancellation, quit and restart tests pass. An additional queued-unreadable-file test verifies failure before spawn and subsequent reuse of the same repository/capacity. Exactly four backends remain. |
| 13: Persistence | A populated v0.1 JSON fixture preserves registrations, completed history, IDs, all four backend preferences and both concurrency preferences while defaulting new fields. Save/reload is checked too. Appearance remains the existing session-local system/dark/light choice, with no new persisted appearance format. |
| 14: Packaging | Release workflows, package scripts, artifact names, Homebrew and Scoop definitions are unchanged. All nine packaging tests pass. |
| 15: Cross-platform | Platform-neutral tests exercise native paths, Unicode/spaces, external files and backend arguments. egui drop events use a path-only fixture whose content-reading method must never run on the UI thread. Native platform evidence limitations are listed below. |
| 16: Keyboard/accessibility | Headless egui tests use Tab/Enter and text input to create, select, edit, rename and delete templates/groups; remove attachments; select repositories; request Run Convoy review; switch Activity/Raw; select history; and reuse a convoy. Add files is reached by Tab, with native cancellation tested at the completion-message boundary. Accessible names/label associations and focus restoration are asserted. Existing focus styling and both-theme contrast checks remain in use. |

### Deterministic test coverage

The numbered cases below refer to the lists in specification sections 17–20.
These tests use disposable files/repositories and fixture CLIs; none requires
provider credentials.

| Area | Cases and test locations |
| --- | --- |
| Groups 1–5 | Keyboard create/rename/edit/delete in [`task_context_tests.rs`](../src/ui/task_context_tests.rs); registration/selection preservation, shared members and validation in [`ui/tests.rs`](../src/ui/tests.rs) and [`library.rs`](../tests/library.rs). |
| Groups 6–9 | Explicit group selection, overlap deduplication, manual selection/deselection and unavailable/unregistered members in `group_selection_is_explicit_deduplicated_and_missing_members_remain_repairable`. |
| Groups 10–14 | Launch snapshots explicit jobs; library edit/deletion cannot change history. Library round-trip and populated/minimal v0.1 defaults are covered in `ui/tests.rs` and `library.rs`. |
| Templates 1–7 | Keyboard create/rename/edit/delete/load plus task-only loading and subsequent draft edits in `task_context_tests.rs` and `ui/tests.rs`; loading never admits jobs. |
| Templates 8–10 | Snapshot independence after template edits/deletion, persistence round-trip and v0.1 defaults in `library.rs`. |
| Attachments 1–6, 9–11 | Single/multiple drops, deduplication, Remove, cancellation, Unicode/spaces, unsupported PDF, supported text/image and validation in [`attachments.rs`](../tests/attachments.rs) and `task_context_tests.rs`. |
| Attachments 7–8, 12–17 | Missing/unreadable files, backend-specific capability limits, incompatible native image flags, launch rejection and queued deletion/change/unreadability in `attachments.rs` and [`convoys.rs`](../tests/convoys.rs). Unix permission tests restore permissions and explicitly skip their assertions when the account can bypass mode bits; they do not claim Windows ACL coverage. |
| Attachments 18–24 | Rendered historical metadata after deletion, async reuse of valid/missing/changed files, stale-result rejection, task-only templates, metadata-only persistence and shell-free native/structured transports in `task_context_tests.rs`, `ui/tests.rs`, `attachments.rs` and `library.rs`. |
| Activity | Representative contract fixtures for all four backends in [`fixtures`](../tests/fixtures) cover progress, available tool/file/command events, final text and warnings/errors. [`activity.rs`](../tests/activity.rs) checks split UTF-8, malformed records, plain Copilot output, decoder limits and retained final text. Existing `convoys.rs` integration checks separate Raw output for every backend; `library.rs` checks the combined session memory budget. Fixtures are synthetic protocol examples, not authenticated captures. |
| Codex sessions (21) | Remains investigation-only. The [public-interface evidence](codex-sessions.md) and ephemeral-session limitation still apply. No resume/open action or private desktop integration was added. |

### Validation results and limits

All required checks passed:

```sh
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo build --release
python3 -m unittest discover -s packaging -p 'test_*.py'
git diff --check
```

**157 Rust tests passed; 3 optional installed-CLI tests ignored.** The two
Unix unreadable-file cases executed on this macOS account. **9 packaging
tests passed.** Cargo/package versions and dependencies are unchanged.

This follow-up validates keyboard actions and dropped-file delivery at the
egui/application boundary. It does not claim a native OS drag/drop, screen
reader, Linux/Windows file-picker or Windows ACL test. The earlier native
macOS multi-file picker evidence remains recorded in the
[initial validation](#initial-implementation). Authenticated attachment execution
for all four backends remains unverified. Cross-platform CI results should be
read separately from these local results; no release artifact was produced.

## Combined workflows and completion

Date: 2026-10-06. Base: merged Part 2.1 and Part 2.2 on `main`, commit
`c291824`. This is a separate development PR; Cargo remains at **0.1.0**.
No release, tag or packaged distribution was published.

### Completion report

1. **Repository group model — implemented and tested.** Named groups contain
   canonical paths to existing registrations. They neither copy repositories nor
   introduce execution units. CRUD and overlapping membership are covered by
   domain/UI tests and the native macOS session.
2. **Selection semantics — implemented and tested.** Selecting adds available
   members to one explicit path set; duplicates collapse. Deselecting removes all
   current members, including individual/overlapping selections. Partial groups
   reflect the set. Native checks confirmed 1, 5, 10 and 24 selections and the
   overlap sequence 3 → 5 → 2.
3. **Unavailable group members — implemented and tested.** Missing/unregistered
   paths remain repairable; selection skips known unavailable members with a
   recovery message. Normal Git preflight still checks every selected repository.
   Editing/deleting a group changes neither registration nor active snapshots.
4. **Group migration — implemented and tested.** Existing version-1 JSON loads
   with empty groups. Existing registration paths, preferences and history remain
   intact. No schema-version bump or destructive rewrite is needed.
5. **Task templates — implemented and tested.** Templates hold name/task text.
   Native create, edit, load and delete checks passed. Loading changes only draft
   text, preserves settings/selection/context, focuses editing and starts no job.
6. **Attachment model — implemented and tested.** References hold canonical path,
   display filename, type, size and SHA-256. Files are inspected off-thread and
   checked again at preflight and admission. Snapshots stay independent of edits.
7. **Native picker — implemented and tested on macOS.** Add files opens a native
   multi-file sheet. Markdown plus PNG were selected together; cancellation left
   the draft intact and returned focus. An unsupported PDF could not be selected.
8. **Drag-and-drop — implemented, deterministic UI tests passed.** Drops within
   the attachment area use background inspection; outside drops are ignored.
   Mixed supported/unsupported files, duplicates, Unicode/spaces and focus
   preservation are covered. OS drag delivery was not manually verified in this
   pass; native Linux/Windows desktop checks remain outstanding.
9. **Backend capability handling — implemented and fixture-tested.** Backends own
   metadata validation and conditional native-interface probes. Unsupported input
   blocks launch with recovery guidance; no generic flag or permission mapping
   was introduced.
10. **Codex transport — implemented and fixture-tested.** Text is escaped context
    in stdin; images use native `--image` arguments. The installed CLI is 0.160.1.
    Authenticated attachment E2E was not run: the execution environment required
    additional approval for the provider transfer. A deliberate ignored probe is
    available below. Existing text-only user E2E evidence is separate.
11. **Copilot transport — implemented and fixture-tested.** Text goes through
    stdin, images through native `--attachment=PATH`. No installed Copilot CLI
    was available for a new authenticated attachment probe.
12. **OpenCode transport — implemented and fixture-tested.** Text goes through
    stdin, images through native `--file=PATH`. Current interface evidence is
    linked in [task context](task-context.md). Installed/authenticated E2E remains
    unverified; no CLI was installed for this work.
13. **Claude transport — implemented and fixture-tested.** Text-only stdin stays
    simple; image tasks use text/base64 image blocks in stream-json stdin. No
    installed/authenticated Claude execution was available. Existing documented
    provider/image limits still apply.
14. **Unsupported/missing/changed attachments — implemented and tested.** Type,
    signature, size, read failures and digest mismatches remain explicit. This PR
    adds concise recovery summaries instead of leading with raw OS error chains;
    full details stay in Diagnostics. Native missing-file reuse was exercised.
15. **Privacy and external paths — implemented and tested.** Attachment contents
    are transient input, never saved to application history or copied into user
    repositories. External paths need no implicit directory grant. Metadata is
    persisted; agent output may quote context. Each CLI/provider's own data and
    logging policies apply.
16. **History reuse — corrected and tested in this PR.** All original references
    remain in the draft. Missing/changed files are marked and block both Run and
    preflight until explicitly removed or removed and re-added. Adding another
    file cannot bypass this. Saving retains the missing reference; history is
    unchanged. Resolution also clears obsolete attention messages.
17. **Activity presentation — implemented and tested.** Codex/OpenCode/Claude
    summarize actual backend events; Copilot presents actual plain output. Native
    Activity displayed independent per-repository progress/results. No simulated
    provider progress or universal event semantics were added.
18. **Raw output — retained and tested.** Original CLI records remain separately
    inspectable; stderr is labeled. Native switching between Activity and Raw
    output passed. Both use bounded session logs and visible-line rendering.
19. **Codex sessions — investigated and deferred.** Existing `--ephemeral` jobs
    provide no trustworthy resumable session contract. No Resume/Open action,
    session manager or session metadata persistence was added. See
    [the investigation](codex-sessions.md).
20. **Persistence and backward compatibility — implemented and tested.** Additive
    defaults cover groups/templates/attachments. Existing preferences/history
    survive; unfinished jobs recover truthfully as interrupted. Stable run IDs,
    history-only cleanup and session-only appearance overrides remain unchanged.
21. **Native UI and keyboard — tested on macOS.** Two panes, independent scrolling,
    visible execution controls, focus outlines and textual status labels remain.
    Native Tab/Enter removed an unavailable reference and returned focus to Add
    files. All four backend settings remained distinct. System/dark/light and
    compact/default/enlarged windows were exercised.
22. **Architecture/performance — reviewed and improved.** Group rendering now
    builds one borrowed availability index instead of repeated registration scans
    and path clones. Missing paths are formatted on hover. No per-frame file
    hashing, Git work, CLI probing or unconditional saving was added. Existing
    text caches update on log changes. Backend/process/Git/scheduler/persistence
    boundaries remain intact; AGENTS now records task-context invariants.
23. **Dependencies and scope — preserved.** Part 2.3 adds no dependency and no
    backend. Test helpers reuse existing crates. No workflow engine, automatic
    Git actions, terminal, worktree manager, provider login or web frontend was
    introduced.
24. **Automated coverage — extended.** The new workflow test sends real text/image
    payloads through the runner, scheduler and all four fixture processes. Each
    of two repository jobs must receive the specification token and exact image
    digest, emit its own result, and leave Git clean. Reuse/save/unrelated-add/
    explicit-removal regression tests and recovery-message tests were added.
    Layout tests now include 1/5/10/24 repositories with groups/templates, both
    palettes, four backend views and three pane sizes.
25. **Realistic workflows — exercised with disposable repositories.** A GNOME
    Extensions group launched three independent jobs using its review template.
    A Package Repositories group launched homebrew-cask and scoop-bucket with its
    release template plus requirements.md and screenshot.png, concurrency 2.
    Receipt checks verified text/image delivery to both. The two convoys overlapped
    at four running jobs and one queued, and all five jobs succeeded independently.
    This validates orchestration, not the quality of model-generated maintenance.
26. **Local and CI validation — see the recorded results below.** Required Rust
    checks and packaging tests are run before completion. Cross-platform CI is
    checked on the separate PR; it does not substitute for native provider or
    file-dialog testing on those platforms.
27. **Packaging — unchanged and compatible.** Cargo remains the single version
    authority. Homebrew's `CodeConvoy-<version>-macos-universal.dmg` and Scoop's
    Windows ZIP naming/layout remain unchanged. No packaging script or release
    metadata changed; Python packaging tests cover the contracts.
28. **Known limits/deferred work — explicit.** Authenticated attachment/model
    acceptance remains unverified for all four backends. Native Windows/Linux
    picker and OS drop checks remain outstanding. Native image paths can still
    race with an external edit after final validation; documents are not locked.
    Model/context/request limits may be lower than application guards. Existing
    Codex continuation remains deferred. No optional feature is claimed complete
    merely because its fixture passes.
29. **Recommended next real workflow.** With an already authenticated chosen CLI,
    use two trusted disposable GNOME-extension clones, load a read-only
    compatibility-review template, attach a short specification and screenshot,
    select concurrency 2, review Git state and start. Check each response against
    the supplied facts, inspect Raw output and verify an unchanged diff. Repeat
    with the intended image-capable model for each backend before claiming its
    authenticated attachment E2E. Real package maintenance can then use the same
    ordinary group/template flow with explicit repository review.

### Native validation method

The `test-support` example creates its own temporary state and 24 disposable Git
repositories, with all four executable settings pointing at the test fixture.
It never invokes a real coding agent. The fixture writes receipts only inside
these repositories' `.git` directories. Nothing is added to normal app state.

```sh
cargo build --features test-support --bin codeconvoy-test-agent --example v02_validation
cargo run --features test-support --example v02_validation
# Minimum supported 780 × 560 content size:
cargo run --features test-support --example v02_validation -- --compact
```

On macOS the built example and fixture were launched in a temporary app bundle
for native UI automation. Default content size was 1180 × 820; the enlarged
window measured approximately 1710 × 979. The compact window retained visible
execution controls while 24 selected repositories scrolled. Group editing also
fit the compact window. Cmd+Q while four jobs ran and one waited showed accurate
counts; keyboard cancellation kept work running. Idle quit completed and removed
the temporary workspaces.

### Validation results

macOS Apple Silicon, Rust 1.99.0:

- `cargo fmt --check`: passed.
- `cargo clippy --all-targets --all-features -- -D warnings`: passed.
- `cargo test --all-features`: 160 passed, 0 failed; 4 deliberate installed-CLI
  probes ignored.
- `cargo build --release`: passed.
- `python3 -m unittest discover -s packaging -p 'test_*.py'`: 9 passed.
- Native fixture workflows and UI checks described above: passed, with the
  explicitly listed OS/provider limitations.

Cross-platform CI status is recorded on the separate Part 2.3 PR.

Authenticated provider tests are deliberately excluded from the default suite.
To run the optional Codex check after explicitly approving the synthetic
specification, existing public screenshot and empty repository context being
sent to the configured provider:

```sh
CODECONVOY_E2E_CODEX=/absolute/path/to/codex cargo test --all-features --test workflows \
  installed_codex_receives_text_and_image_in_two_disposable_repositories -- --ignored --nocapture
```

It requests read-only work, asserts facts obtainable from both attachments and
separate repository names, and checks Git remains clean. It uses the CLI's
existing authentication; no keys, authentication files or provider configuration
are read or managed by CodeConvoy.
