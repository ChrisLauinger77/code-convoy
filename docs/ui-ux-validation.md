# Second UI/UX pass — v0.1.0

Validated on macOS on 2026-10-05. This pass changes presentation and build metadata, not the backend contract or execution machinery.

This records the preceding UI pass. The subsequent
[CLI availability lifecycle fix](cli-availability-validation.md) replaces its
explicit-only CLI checking behavior; packaged macOS validation of that change
is recorded separately.

## Initial audit

Before editing product code, a disposable launcher opened the real native `ui::App` with 24 Git repositories, clean and dirty working trees, and 30 synthetic historical convoys. The audit switched all four backend forms, launched concurrent local fixture jobs, inspected output and historical settings, stopped a queued job and a convoy, reused a convoy, and removed history.

Concrete problems:

- A repository-lock waiter said it was waiting for an available job slot in the empty output area.
- CLI failures mixed long error chains into the form and shared a busy flag with repository work.
- History showed raw option keys and blank values, without friendly dates or a complete repository list.
- Repository selection required individual clicks. Keyboard traversal reached Remove before each checkbox.
- Execution controls were rendered before the editor, so their Tab order did not match their visual position.
- Success used an ambiguous plus sign; recovered interruption was only explained in details.
- Individual history removal sat beside ordinary reuse; there was no About/version surface.

The two-pane design, bounded run selector, themes and visible-line text renderer already worked well and were retained.

## Changes

- Backend forms share aligned labels and field widths while continuing to render each backend's own option specifications. CLI status is Unchecked, Checking, Available, Unavailable, or Invalid configuration; details remain expandable and copyable.
- CLI checks have independent, configuration-scoped asynchronous state. A stale result cannot describe a different draft or clear repository work. No automatic probing was added.
- Select all/none supports the existing registered repository list. Names align left, long names truncate with full-path tooltips, and branch/change information remains visible. Checkboxes precede Remove in keyboard order.
- Run choices retain Active/History groups, ID, backend, state and progress, and add elapsed duration and creation-time tooltips. Stop job, Stop Convoy and the separate All convoys menu make scope explicit. Individual and bulk history removal share the explanatory cleanup menu.
- Status text includes Interrupted. Success gets a small painted check mark because the bundled fonts lack the Unicode check glyphs. Color is supplementary.
- Empty output distinguishes scheduler waits, a running agent, cancellation before execution, completed jobs without text, and session-only historical logs. Failed jobs and repository errors show concise summaries with bounded, expandable raw diagnostics.
- Task & settings shows friendly backend labels and choice values, UTC creation/start/finish dates, repository paths and the original concurrency. Missing historical values say Not recorded; the live global limit is explicitly not a saved snapshot value. Only declared options are displayed.
- The fixed execution area is measured and rendered after the scrolling editor. Repository/job focus scrolls into view. Launch review and About are modal with Escape dismissal.
- About shows package version, optional build-time base Git revision, MIT license and project link. It performs no runtime Git or network work and explains that local changes may be present.
- Backend configuration, repositories, diagnostics, historical snapshots and About have small separate UI modules. No new UI framework or persistence schema was introduced.

## Native validation

All executions used the repository's local test agent and disposable repositories. No authenticated provider task or important working tree was used. Synthetic histories intentionally covered failures and recovered interruption.

| Scenario | Observed result |
| --- | --- |
| Four backend forms | Codex, Copilot, OpenCode and Claude controls retained distinct options; fixture CLI checks reported Available for all four |
| Missing / invalid CLI | A nonexistent absolute executable showed Unavailable with an installation/path hint; invalid Claude max turns showed Invalid configuration with diagnostics |
| Repository counts | 1, 5, 10 and 24 repositories inspected; bulk selection/deselection verified for 10 and 24; five-repository dirty review and successful fixture execution verified |
| No runs / one run / full history | Concise empty pane, successful one-repository convoy, full 30-history selector, reuse, individual removal and clearing history exercised |
| Concurrent work | Two active convoys on the same repository: second explicitly waited for the first convoy's repository access; stopping its queued job left the first running |
| Cancellation | Individual queued stop, convoy stop during the audit, and separate Stop All action exercised; stopped work retained repository changes |
| Statuses | Queued, running, succeeded, failed, cancelled and interrupted inspected; interruption includes a repository-inspection hint |
| Output | Large synthetic output (4,000 lines per populated job), real fixture output, Copy output, session-only history messages and scrolling inspected |
| Diff | Actual dirty tracked-file diff, headers, additions/removals, context, Copy diff, vertical scrolling and horizontal scrollbar movement inspected in dark/light themes |
| Snapshot | Backend-specific friendly values, original concurrency, all repository paths and UTC dates inspected; global-limit limitation visible |
| Launch lifecycle | Accepted launch cleared task and selection while retaining backend options and concurrency; focus returned to the task |
| Keyboard | Task entry, backend popup and fields, CLI check, repository checkbox, numeric global-limit editing, Run/Start review, dirty acknowledgement, run/job selection, result tabs, reuse, queued Stop job and individual history removal exercised with keys |
| Sizes | 780×560 and 1180×820 viewports; requested 1600×1000 was constrained by the display to approximately 1600×978; live zoom/restore also exercised |
| Themes / identity | System, Dark and Light selected; visible focus, disabled controls, selected jobs, status/diff contrast and About version/base revision/license inspected |

Keyboard menus use arrows after opening; numeric fields become editable on focus. Native automation needed window zoom/restore to reliably deliver some initial input events. The deliberate keyboard workflow was completed in the expanded window. Compact layout and scrolling were separately inspected; this is not a screen-reader certification.

## Automated verification

All required checks passed:

- `cargo fmt --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --all-features`
- `cargo build --release`

Result: **87 tests passed**, 3 optional installed-CLI probes ignored, 0 failures. The metadata helper was additionally exercised with a Git checkout, an unavailable Git executable, a source archive and an archive nested inside another checkout; only the actual checkout produced a revision.

The new tests cover CLI error classification and asynchronous scoping, bulk selection, status/empty-output labels, backend-owned historical values, Gregorian timestamp boundaries, About metadata fallback, editor bounds and narrow result-action layout. Existing contrast, visible-row rendering, lifecycle, all four backend, scheduler, locking, baseline, cancellation and persistence tests remain in place.

Source hashes taken before the pass confirmed no changes to `agents/*`, `runner*`, `process`, `domain` or `persistence`. Output/diff indexing and visible-row rendering are unchanged: there are no per-frame Git/CLI probes, new whole-log clones, or per-frame persistence writes. Extra session-run IDs are retained only while their corresponding runs remain in state.

## Limits and deferred work

This macOS fixture validation does not upgrade authenticated E2E status: Codex/Copilot retain their prior Linux verification; OpenCode/Claude authenticated E2E remains unverified. Native Linux/Windows checks were not performed in this session. Large-output rendering has both native fixture and automated coverage; the diff native check used a small diff with a long line, not a large-diff benchmark.

Activity normalization, further backends, repository groups, pipelines, worktrees, Git write controls, embedded terminals, cloud execution, packaging and release publishing remain outside this pass. Temporary native launchers and app wrappers are not shipped.
