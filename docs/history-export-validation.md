# History export validation — 0.9.0 PR 2

Validation host: Linux x86_64 / Wayland, 2026-10-11. Package version remains 0.8.0;
this feature does not prepare, publish or tag a release.

## Automated checks

Passed:

```sh
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
TMPDIR=/var/tmp cargo test --all-features
cargo build --release
```

The first full test invocation used the host's default `/tmp`, which contains an
existing `.git`. Three existing worktree/cleanup tests correctly refused that
location. The complete suite passed using `/var/tmp` for disposable fixtures; no
worktree safety rules or existing tests were weakened. Four existing opt-in
provider/CLI tests remained ignored. No authenticated provider checks were run.
After the final cancellation guard, privacy fixtures and wrapping adjustment,
all 34 focused export/shared-Markdown/UI tests passed again, followed by a successful
release rebuild. All-target/all-feature Clippy and formatting also passed.

Coverage:

- Explicit JSON envelope/schema/keys/statuses, Unicode, RFC 3339 dates, epoch/leap
  day/year bounds, missing values, recorded zero values and repository association.
- Successful, failed, cancelled, interrupted, mixed and empty/legacy records;
  known/unknown changes, review resolution and independent latest validation.
- Selected IDs, filtered/all terminal history, source ordering, deduplication,
  missing IDs, empty selection/history, active exclusion, and frozen snapshots
  after filter/history changes. A 2,500-convoy report exercises both formats.
- Byte-for-byte reuse of Copy Summary for single Markdown, nested multi-convoy
  sections, separation, escaping and the entire existing Copy Results privacy suite.
- No raw/Activity/validation logs, diffs, backend environment options, attachment
  references, source/worktree paths, ownership or Git identity in either format.
  Live Review and mutable worktree observations cannot fill historical unknowns.
- Atomic create/explicit overwrite, stale destinations, invalid paths, directories,
  links, permission failure where the host permits testing it, simulated disk-full
  failure, cancellation before/during writing, intact previous targets and temporary
  file cleanup. Tests use disposable directories and no interactive picker.
- UI cancellation/error callbacks, duplicate prevention, pinned single selection,
  confirmation-time filtering, cancellation/quit, keyboard replacement/Cancel focus,
  Escape and compact theme layouts. The dialog also fits a 390 × 560 pane, within
  the application's 780 × 560 minimum window.
- Background JSON/Markdown export alongside real metadata cleanup; removed source
  records do not invalidate captured reports, later cleanup leaves output files
  intact, and active history remains unchanged. Export itself creates no history
  transaction, agent/validation work or repository inspection.

## Native check performed

Built the existing `v09_history_validation` example with `test-support` and launched
it on Linux/Wayland with `TMPDIR=/var/tmp`. The native 780 × 560 fixture, containing
1,002 saved history entries and a retained temporary worktree, started without
reported startup errors. It ran for ten seconds, then was terminated by the bounded
smoke command. Its exact disposable directory was verified and removed afterwards.
No user repositories or installed agents were used.

This was a **native startup check only**, not visual verification or a native
save-dialog interaction test. Selection, overwrite, cancellation, large exports
and cleanup independence were exercised through deterministic tests instead.

## Remaining platform/release checks

- Manually exercise native save selection, successful single Markdown/JSON export,
  filtered history, replacement and cancellation on Linux GNOME/Wayland, macOS,
  and Windows 10/11. No native macOS/Windows checks were performed here.
- Verify platform permission prompts, portal availability, native extension handling
  and keyboard/screen-reader behavior against packaged builds.
- Existing packaging, signing and release checks remain required for a release.
  No new dependencies, backend CLI flags, scheduling rules, persistence fields or
  provider authentication behavior were introduced.

Known limits are documented in [history export](history-export.md): one selected
convoy in the UI, existing Copy Results metadata privacy limits, manual dismissal
of an open OS picker during cancellation, and possible orphan temporary files after
unexpected process termination. Reports are not restorable backups.
