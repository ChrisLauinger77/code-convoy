# v0.9 PR 5: history performance and reliability

## Baseline, recorded before implementation changes

2026-10-11, Linux x86-64, optimized Rust build, local temporary storage.
`cargo run --release --features test-support --example history_performance`
uses deterministic existing AppState/Run/Job/validation models: four repositories
per convoy, mixed outcomes, Unicode, unknown completion observations, optional
validation, and every tenth prompt roughly 15 KiB. No agents or user repositories
are accessed. Output buffers are session-only, so are excluded from disk timings.
Medians: three atomic saves, five load/cold-search/retention/export/scan samples,
25 warm searches. These are warm filesystem-cache timings, not cold boot times.
Export generation uses an I/O sink, excluding dialog, disk sync and publication.
Storage includes parsing history and empty ownership inventory, not Git eligibility.

| Convoys | JSON bytes | Save ms | Load ms | Cold search ms | Warm search ms | Retention ms | Export capture ms | JSON ms | Markdown ms | Storage ms |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 100 | 525057 | 15.659 | 0.678 | 0.268 | <0.001 | 0.002 | 0.377 | 0.032 | 0.128 | 0.780 |
| 1000 | 5248485 | 152.936 | 4.517 | 2.667 | 0.004 | 0.036 | 3.804 | 0.336 | 1.324 | 7.708 |
| 5000 | 26248946 | 768.228 | 25.923 | 14.805 | 0.029 | 0.198 | 19.320 | 1.706 | 6.464 | 42.910 |

Headless egui open-history selector, ten warmed frames (two warmups), four jobs
and long multiline tasks: 0.207 / 1.985 / 13.172 ms at 100 / 1000 / 5000 convoys.
Command: `cargo test --release --all-features --lib history_navigation_performance -- --ignored --nocapture`.
This measures layout, not native input or GPU rendering.

The baseline all-features suite passed outside the Codex filesystem sandbox.
Inside it, a protected `/tmp/.git` placeholder rejects temporary worktree bases
and Unix socket creation is denied; those are environment failures, not a reason
to weaken production safety checks.

## Prioritized findings

- State JSON writes serialize directly into an unbuffered file. The 5,000-convoy
  save blocks for roughly 0.77 seconds, including UI-initiated saves.
- The open selector lays out every history entry and uses linear membership
  checks inside its loop. Virtualization is justified by measured frame cost.
- Store rejects malformed JSON at startup, but its broad defaulted state parser
  accepts missing core history fields, and Store itself does not prevent saves
  after a failed load. Strengthen this boundary without changing version 1.
- Existing retention protections, export snapshots and cancellation, scan
  generations, cleanup intent/journals and manager leases should be preserved.

Final measurements, regression evidence and platform limitations are recorded
below after validation; no timing thresholds are imposed on normal tests.

## Final implementation measurements

Same fixture, machine and release-profile commands; Linux 7.2.9, AMD Ryzen 7
9800X3D, rustc 1.96.1. Files were local and warm in the filesystem cache. These
samples are observations, not CI thresholds or guarantees on other machines.

| Convoys | Save ms | Load ms | Cold search ms | Warm search ms | Retention ms | Export capture ms | JSON ms | Markdown ms | Storage ms |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 100 | 0.493 | 0.914 | 0.408 | <0.001 | 0.002 | 0.399 | 0.033 | 0.127 | 0.791 |
| 1000 | 5.187 | 7.535 | 2.863 | 0.003 | 0.039 | 3.991 | 0.341 | 1.256 | 8.825 |
| 5000 | 26.552 | 35.596 | 15.847 | 0.030 | 0.213 | 22.677 | 1.926 | 6.459 | 45.074 |

At 5,000 convoys the atomic save improved about 29x (768.228 to 26.552 ms).
Periodic disk writes additionally moved off the UI thread; the table measures the
writer itself, not snapshot capture/dispatch. Open-selector headless layout fell
from 0.207/1.985/13.172 ms to 0.101/0.150/0.314 ms: about 42x at 5,000.
The widget-count regression requires bounded visible widgets, not a timing limit.

Loading is deliberately more conservative: the extra envelope pass and ID checks
increase 5,000-entry load time from 25.923 to 35.596 ms in these samples. No speed
claim is made for filtering, retention, export or storage parsing. An experimental
lighter storage entry parser measured 29.379 ms, but was removed because it skipped
malformed domain-field validation. The final scanner keeps the existing Run parser.
No indexing database, summary cache, scheduler change or log migration was justified.

Inventory-only check (unchanged production inventory):

```sh
cargo test --release --all-features --lib inventory_scale_performance -- --ignored --nocapture
```

| Owned attempts | Inventory median ms | Full storage scan ms |
| ---: | ---: | ---: |
| 100 | 5.298 | 6.960 |
| 1000 | 54.669 | 74.040 |
| 5000 | 272.209 | 371.305 |

Five inventory samples include cloning minimal references, duplicate live/saved
references and matching manifests; one subsequent full scan includes a nested
128-byte generated file in every attempt. Setup is excluded. These are disposable
ownership fixtures, not 5,000 Git worktree creations or cleanup-eligibility proofs.
Git eligibility remains independently asynchronous and safety-first. No inventory
speed improvement is claimed. Existing large-directory/symlink tests also run.

## Reliability and lifecycle audit

| Area | Result and regression evidence |
| --- | --- |
| Startup/recovery | Required envelope, malformed/truncated/empty/UTF-8/future-version failures, duplicate/exhausted IDs and failed-load write guard tested. Real v0.1 fixture loads unchanged; existing backend compatibility tests now expect repaired counters. No salvage or automatic rewrite. |
| Atomic state/configuration | Buffered serialization explicitly flushes before sync and replacement. Mid-write StorageFull/PermissionDenied injection, actual Unix directory permissions and failed replacement preserve prior data and remove ordinary failed temporaries. Metadata snapshot serialized equality covers all saved fields. Existing transactional validation/configuration tests pass. |
| Retention | Existing count/age/combined policies, unreliable dates, active workers/validation, duplicate IDs, preview changes, save failure and ownership pinning tests pass. Staging now avoids copying logs; removal/publish semantics are unchanged. No automatic retries of retention. |
| UI/search/comparison | Scale tests cover all filters and case-insensitive metadata search against an independent reference, stable ordering, mutation/removal and selection. Off-screen keyboard navigation and AccessKit row bounds are tested. Summaries/validation remain live reads, not cached copies. Existing selected-result/comparison tests pass. |
| Export | 5,000-entry JSON and Markdown snapshot consistency, format schema, Unicode and privacy exclusions tested. Existing mid-write errors, overwrite revalidation, cancellation, UI quit and clipboard tests pass. Unix publication sync failure now describes uncertainty. Export has no Store or retention access. |
| Storage | Required envelope and ambiguous-ID errors; existing stale generations/coalescing, cancellation/drop, missing/permission/partial results, large trees, symlink loops/substitution and deduplication tests pass. Worker scans remain read-only. Measurement is not cleanup authority. |
| Cleanup | Existing disposable-Git tests cover changed preview, ignored data appearing after unlock, path substitution, missing/invalid ownership, active job/validation conflicts, concurrent leases, non-force Git failure and lock restoration, batch partial failure, intent/outcome-save failures and shutdown in the critical section. Production cleanup/proof code is unchanged. |
| Concurrency/quit | One reserved writer through acknowledgement; new edits and validation survive it. Failure restores dirty state; launch/cleanup cannot overtake a writer. Pending quit suppresses another periodic save. Existing export cancellation, history save, storage close, validation/process-tree shutdown and cleanup-journal tests pass. No manager lock is acquired by the state writer. |
| Output/notifications | Existing bounded log rendering, process cancellation and notification tests pass. Metadata snapshots do not clone raw/activity/validation output, diagnostics or Git review caches. Persisted data is neither truncated nor reformatted into a new schema. |

New regression sources: `tests/history_scale.rs`, `tests/support/history_scale.rs`,
`src/persistence/tests.rs`, `src/ui/history_tests.rs`, `src/storage/tests.rs`.
The two ignored performance checks are reproducible manual checks, not flaky
wall-clock assertions in ordinary CI.

PR review added fallback-exit coverage: native teardown joins an outstanding
periodic writer before the final save, including when its UI acknowledgement was
never polled. Queued, completed and failed writes are exercised; direct file reads
verify that newer edits and interrupted job/validation states were saved without
relying on startup recovery to repair a stale snapshot.
The preflight dialog also disables Start with a visible saving explanation while
a state writer is pending. Keyboard coverage verifies that review acknowledgement
survives the save and launch still requires explicit activation after it finishes.
Result actions, validation launch, history cleanup and managed-worktree cleanup
also disable conflicting controls with visible saving status. Keyboard regressions
cover both Review and Validation views, retained-copy removal, and confirmations
opened before a save begins. Confirmation state survives completion, Cancel stays
available, and no result operation or validation starts automatically. The existing
disposable-worktree cleanup test also exercises these controls around a real save.

Key implementation files: `src/persistence.rs`, `src/persistence/history.rs`,
`src/domain.rs`, `src/ui/persistence.rs`, `src/ui/results.rs`, `src/ui/quit.rs`,
`src/storage/history.rs`, `src/export/file.rs`, `src/visibility.rs`.

## Native checks and remaining release-candidate work

- Linux desktop: existing 1,002-convoy disposable fixture rendered at 780x560 on
  GNOME/Wayland through XWayland. Software OpenGL completed startup and the ordinary
  close path, then verified retained worktree contents and Git backlink. No
  installed backend was invoked. Reproduce with
  `LIBGL_ALWAYS_SOFTWARE=1 env -u WAYLAND_DISPLAY cargo run --release --features test-support --example v09_history_validation -- --smoke`.
- Hardware OpenGL/Wayland smoke attempts stalled in the system EGL/Mesa display
  queue (main-thread debugger stack), before desktop interactions could be verified.
  Those fixture processes were stopped. This is a remaining native GPU/compositor
  check; the smoke does not certify hardware-rendered responsiveness. Graphics code
  was not changed. Headless egui tests verify keyboard/filter/retention interactions.
- macOS and Windows native UI, filesystem-sharing/rename behavior, long Windows
  paths and Windows reparse-point behavior were not tested here. Existing Windows
  managed cleanup remains disabled pending stable filesystem identity support.
- POSIX file and directory sync are requested; no real disk-full, power-cut or
  crash-durability guarantee was experimentally established. Disk-full tests inject
  the actual I/O error class. Windows file sync/replacement does not add a portable
  directory-sync guarantee. Filesystems and hardware may weaken crash durability.
- Startup parsing and explicit durable intent/outcome/configuration/final saves
  remain synchronous. Buffering reduces their measured cost, but unusually slow
  disks/network filesystems can still delay those explicit boundaries. Periodic
  saving, exports, storage measurement, Git inspection and cleanup run on workers.
- A blocked OS filesystem call cannot be forcibly cancelled. Scans do not hold up
  shutdown; destructive cleanup retains the established lease/journal protections.
  Filesystem measurements remain approximate and never follow known symlink targets;
  this is not a sandbox against a hostile same-permission process racing path checks.

No release tag, package publication, push or release-version bump is part of PR 5.

## Memory and final automated checks

A fresh 5,000-convoy benchmark process peaked at 83,852 KiB RSS before changes
and 83,804 KiB after changes (Linux `resource.getrusage(RUSAGE_CHILDREN).ru_maxrss`).
These effectively unchanged process-wide peaks include fixture construction,
load, search, report capture and scanning; they do not measure native GPU memory
or prove reduced total application RSS. Snapshot tests separately establish that
large session logs and validation output are not copied into persistence work.
To repeat after building the example:

```sh
python3 - <<'PY'
import resource, subprocess
subprocess.run(['target/release/examples/history_performance', '5000'], check=True)
print('peak KiB:', resource.getrusage(resource.RUSAGE_CHILDREN).ru_maxrss)
PY
```

Final validation on Linux:

- `cargo fmt --check`: passed.
- `cargo clippy --all-targets --all-features -- -D warnings`: passed.
- `cargo test --all-features`: 536 passed, zero failed, six intentionally ignored.
  This includes two manual performance checks and four installed-backend checks.
  Both performance checks were run separately and passed. Counts exclude filtered
  child-process re-executions used by environment-isolation tests.
- `cargo build --release`: passed.
- `python3 -m unittest discover -s packaging -p 'test_*.py'`: 16 passed.
- `git diff --check`: passed.

In this Codex environment the complete native filesystem suite used
`TMPDIR=/var/tmp cargo test --all-features` outside the sandbox, to avoid the
sandbox's transient `/tmp/.git` protection marker. Production ancestor checks were
not weakened. This environment override is unnecessary on ordinary developer/CI
hosts without that marker. No authenticated backend runs or native macOS/Windows
checks are implied by these test results.
