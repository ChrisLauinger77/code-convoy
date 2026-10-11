# Managed worktree cleanup validation — 2026-10-11

## Automated validation

Linux checks:

- `cargo fmt --check` — passed.
- `cargo clippy --all-targets --all-features -- -D warnings` — passed.
- `TMPDIR=/var/tmp cargo test --all-features` — 516 top-level tests passed,
  4 existing optional provider probes ignored; 3 subprocess self-tests also passed.
- `cargo build --release` — passed.
- `git diff HEAD --check` — passed.

The sandbox injects a `.git` marker into `/tmp`, which correctly prevents worktree
fixture creation under that ancestor. The initial targeted test failed at setup;
the tests were then run outside that restriction, in disposable temporary Git
repositories. No ownership checks were weakened. No user repository was used as
a cleanup target and no provider CLI was invoked.

The 21 new tests live in `tests/support/managed_cleanup.rs`,
`src/worktrees/cleanup_tests.rs` and `src/ui/storage_tests.rs`:

- Managed detached registration and ownership lock, old custom location bindings,
  duplicate inventory/selection identities, missing manifests, user-created trees,
  unregistered/missing trees, out-of-scope paths, source-root/child confusion,
  changed source identity, owner/backlink/lock mismatch and stale lifecycle identity.
- Completed clean results remain Retained. Queued/running/preparing jobs, validation,
  uncertain Apply and unknown/orphan lifecycle records stay protected. Live process
  and validation leases block deletion even when UI metadata is stale.
- Busy leases and changed previews rejected before any journal write can be reviewed
  and retried, including after restart. Failed outcome saves retain pending intent;
  unconfirmed process cleanup and failures after unlocking retain sticky protection.
  Unrelated queued/preparing/running jobs in the same convoy do not block cleanup;
  same-repository, nested-repository and shared-common-directory work still does.
- Modified tracked files, index changes, untracked files, ignored content, unmerged
  index entries, agent commits, sparse/assume-unchanged flags, nested repositories,
  in-progress Git evidence and same-size edits hidden by the live Git stat cache.
  Real indexes remain byte-for-byte unchanged. Unsupported clean filters are
  rejected before status can execute them.
- Checkout and parent symlink substitutions, same-path replacement of a directory,
  changes after preview and after unlock. External/source files remain intact.
- Single and bulk success, identical selections deduplicated, protected targets
  skipped, independent continuation after a late ignored-file failure, repeat
  requests blocked, and intent-save failure before any Git mutation.
- Actual Git non-force removal rejection when an untracked file appears immediately
  before the command. Files, registration, restored ownership lock and unfinished
  journal remain. There is no force/prune/recursive-fallback path in this action.
- Injected cancellation after unlock still completes the critical removal and
  durable journal. Queued bulk targets stop on quit. Existing scheduler tests cover
  maintenance exclusion, same/common/nested repository conflicts, validation locks,
  admission during maintenance and quarantine after uncertain process cleanup.
- Saved history, validation, completion/result observations, task and stable ownership
  identity survive removal; live validation is rejected afterward. Inventory omits
  verified removed checkouts and retains small ownership tombstones.
- Real headless egui confirmation widgets: empty selection, exact target display,
  keyboard confirmation, initial Cancel/Enter, Escape, duplicate-operation blocking,
  result reporting, shutdown and post-cleanup inventory/size refresh. Existing storage
  tests retain compact system/dark/light theme and keyboard navigation coverage.

Existing ownership/recovery, configurable-location, Apply/Discard, storage,
history, validation, process-tree cancellation and scheduler suites also pass.
Failures/crashes between unlink and save reuse the existing journal reconciliation
and history-pinning tests; ordinary cleanup introduces no automatic journal repair.

## Native/platform coverage

The existing `v09_history_validation` fixture was launched in the local Linux
GNOME/Wayland session using its X11 backend. It created only temporary repositories
and 1,002 fixture history entries. The process started and accepted a normal
window-close request; its exit assertions confirmed that the retained fixture
contents and Git backlink were preserved. The X11 window remained unmapped to the
capture tools, so native dialog layout and interaction were **not** verified.
Headless egui tests provide the interaction coverage described above.

No local native macOS or Windows run or cross-compilation was available. Ordinary cleanup
is intentionally disabled on Windows because the implementation lacks a stable
filesystem identity proof there. Unix device/inode proof is used on Linux/macOS;
macOS support is implemented but untested natively here. Source and worktree paths
with spaces/non-ASCII text are covered on Linux. Existing permission-failure tests
run where permissions can be enforced; root-capable hosts may bypass those checks.

The Windows fixtures assert the platform's explicit cleanup-disabled diagnostic
while still checking retention and active-work protection. Worktree paths passed
to Git use the existing native-path adapter, including canonical Windows paths.

Before release, manually verify native Storage selection, protected-state details,
confirmation, mixed bulk outcomes and normal quit during removal on GNOME/Wayland
and macOS. Check macOS filesystem errors, Windows inspection/reparse/long-path
behavior and the disabled destructive action. External tools must not concurrently
replace/edit cleanup paths; CodeConvoy leases do not lock external applications.
No automatic or forced storage cleanup, metadata reconciliation/forget action,
release tag or publication is part of this change.
