# History retention validation — 0.9.0 PR 1

Validation host: macOS arm64, 2026-10-11. Package version remains 0.8.0;
release preparation, tagging and package publication are outside this PR.

## Automated checks

Required commands:

```sh
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo build --release
```

All four required commands passed on the validation host. The additional
manager-owned terminal/cancelling-convoy regression test passed separately after
the full suite, followed by another all-targets Clippy check. Four existing opt-in
installed-provider checks were ignored (Claude, Copilot, OpenCode help/flags and
the authenticated Codex E2E probe); they were not run or newly certified.

Coverage:

- `tests/history.rs`: fixed-clock unlimited/count/age/combined limits, exact
  boundaries, equal timestamps, presentation-order independence, incomplete and
  future dates, legacy interrupted finish times, duplicate IDs, empty history,
  positive custom limits, malformed policy fallback, old settings, loading without
  trimming, protected statuses/validation/ownership, atomic save failure, no-op
  writes, conflicting writes and merge preservation.
- `src/ui/history_tests.rs`: exact preview IDs/counts, Cancel focus/Escape,
  confirmation changes and newly completed convoys, selected entry retained/removed,
  comparison/search/copy continuity, failed save with no automatic retries,
  one-shot startup opt-in, settings persistence and validation, late read-only
  Review responses, empty/unlimited cleanup and no redundant acknowledgement save.
  Headless egui checks cover 780 × 560 and System/Dark/Light themes.
- `src/runner/manager.rs` history tests: lifecycle ownership remains protected
  independently of visible/manager active count, including poisoned cleanup after
  lease release. Existing convoy, validation, process-tree cancellation, worktree
  recovery, Apply/Discard, bulk discard and compatibility suites also run.
- Existing individual Remove/Clear tests await atomic-save acknowledgement and
  still verify repository files, Git metadata, selection and stable IDs. Existing
  history/search/filter, Copy Results and validation-history tests are preserved.

## Native macOS smoke tests performed

`examples/v09_history_validation.rs` uses only temporary fixtures and missing CLI
paths. It creates one real detached, owned Git worktree, writes a sentinel in it,
saves 1,002 convoys and reloads the saved state before opening the native UI.
No authenticated agent is launched. Run with:

```sh
cargo run --example v09_history_validation --features test-support
cargo run --example v09_history_validation --features test-support -- --automatic
```

For a disposable macOS app bundle, the second mode can also be selected with
`LSEnvironment` key `CODECONVOY_HISTORY_FIXTURE_AUTOMATIC`. This affects only the
fixture executable, never the application or real user settings.

Observed at 780 × 560:

- Automatic retention disabled: all 1,002 records remain after loading.
- Open cleanup: count and age policy, exact removal counts, IDs and the
  “Worktrees affected: None” statement are readable without resizing the window.
  The long ID list scrolls within the dialog.
- Cancel has initial focus; Escape closes the preview with all history intact.
- Manual keyboard confirmation removed exactly 822 entries and persisted 180.
  Policy was 250 entries / 180 days; one retained worktree and one interrupted
  record remained outside the limits. The age cutoff includes elapsed seconds
  since fixture creation; counts can differ by one at the exact boundary.
- Explicitly enabled automatic retention: startup reduced a fresh fixture to 180
  records and saved the result, without a manual confirmation dialog.
- After both deletion paths, filesystem checks confirmed the retained sentinel,
  Git backlink and matching sibling ownership manifest still existed. The original
  repository registration and next convoy ID (1003) were unchanged. The selected
  interrupted record remained selected. Fixture windows were closed afterwards.

A first smoke pass found unnecessary protection of read-only Direct Review
requests. This was removed: those requests own their copied metadata and late
responses can safely be ignored. A regression test covers this, while validation,
resource-owning lifecycle operations and uncertain cleanup remain protected.

## Limits and remaining release checks

- Native Windows and Linux UI checks were not performed. Their packaging and
  platform-specific release checks remain required for a release.
- Native smoke uses historical fixtures; active/cancelling worker and validation
  protection is exercised by automated lifecycle/manager tests rather than real
  provider jobs. No provider authentication or installed-agent compatibility was
  changed or newly certified.
- Worktree-backed history intentionally stays pinned until existing verified
  cleanup succeeds. Startup retention also preserves records awaiting recovery
  verification; use manual history cleanup after verification if needed. This PR does not split ownership records into a new store or
  implement stale-worktree cleanup.
- Automatic retention runs only at startup. In-session maintenance remains manual.
  Unreliable dates remain outside both limits; total history may exceed the count.
- No new backup/restore flow is provided. Save errors preserve live history;
  a directory-sync error after atomic replacement can leave disk outcome uncertain.
