# Quick Repository Filter validation

Scope: 0.8.0 PR 1 only. The filter changes visibility in NEW CONVOY's repository
picker. Launch still snapshots registered repositories from the shared path set.
Global selection includes all registrations; each group action uses its full
membership and existing availability rules, including hidden members.

## Automated coverage

`src/ui/repository_filter_tests.rs` extends the existing in-process egui keyboard
and accessibility harness, with no added testing dependency. Coverage includes:

- Case-insensitive name/path substrings, full Unicode case folding, surrounding
  whitespace, empty/whitespace-only queries and no matches.
- Expanding folds (`Straße`/`STRASSE`, `ẞ`/`ss`, `ﬃ`/`ffi`) and Greek sigma
  variants, in names and paths, matching in both directions.
- Ungrouped classification from complete membership, overlap, hidden nonmatching
  groups, established ordering, and reclassification after registry/group edits.
- Immediate text input, keyboard selection and Clear, focus return, unchanged
  persisted application state, and restoration of manual expansion choices.
- Searching before the first normal render creates no expansion preferences.
- Shared selection across occurrences, hidden selections, full group/ungrouped
  actions, global Select all/none with zero matches, and unique launch snapshots.
- Headless picker layout at 780 × 560, 1180 × 820 and 1600 × 1000 in System, Dark
  and Light modes, including matching and empty states. Filter/Clear bounds and
  matching overlapping checkbox occurrences are checked.

Existing group tests also cover rename/reorder identity, group editing/deletion,
unavailable members, pointer input, keyboard navigation and narrow header layout.

## Native smoke checklist

Use disposable state and repositories with overlapping groups and an ungrouped
repository. No authenticated agent execution is needed for this UI change.

1. Select a repository, collapse its group, then type a different repository's
   name or path in mixed case with surrounding spaces.
2. Confirm only matching groups/rows appear, collapsed matching groups open, and
   the overall selection count includes hidden selections.
3. Toggle an overlapping repository and check that both occurrences agree.
4. Use Select all/none and group checkboxes while filtering. Confirm full-member
   semantics, including hidden selections and unavailable-member handling.
5. Clear using keyboard and pointer; confirm manual expansion returns and task
   text is unaffected. Check no-match text and all three target sizes/themes.

## Results — Linux x86-64, 2026-10-10

- `cargo fmt --check`: passed.
- `cargo clippy --locked --all-targets --all-features -- -D warnings`: passed.
- `cargo test --locked --all-features`: 388 tests passed, four optional
  installed-CLI probes ignored (excluding duplicate subprocess-test reporting).
  This includes nine new filter tests and all ten existing group tests.
- `cargo build --locked --release`: passed.
- `git diff --check`: passed.

The initial full suite found a narrow-editor filter-row overflow; after bounding
the text field, all existing compact editor checks and the new size/theme checks
pass. Worktree fixtures initially failed because this environment's `/tmp` has a
`.git` marker, which the existing storage safety guard correctly rejects. The
successful full run used a fresh `TMPDIR` under `/var/tmp`, outside the sandbox.
No worktree safety code was changed.

PR review exposed a `Straße`/`STRASSE` mismatch with lowercasing. The Unicode
regression test failed before the fix and passes with full case folding on both
sides. It also covers Greek sigma variants and three-character ligature folds,
using `unicase` 2.10 (the cached 2.9 version mishandled the latter). All required
checks were rerun after the fix. Existing dependency versions are unchanged.

A native X11 debug build started at 1180 × 820 with disposable application state,
three temporary repositories, overlapping groups and disabled/missing agent CLIs.
Startup was visually inspected. Synthetic pointer/keyboard input did not reach
the picker reliably in this desktop session, so native filter interaction and
native size/theme acceptance are **not claimed**. The filter interactions and
the nine target size/theme combinations above were verified with real egui widgets
in the headless harness. The disposable process was closed after inspection.

Remaining manual checks: the native checklist above on Linux/Wayland, macOS and
Windows. No release, tag, authenticated agent task or package publication was
performed.
