# Storage Overview validation — 2026-10-11

## Automated checks

Passed on Linux:

- `cargo fmt --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `TMPDIR=/var/tmp cargo test --all-features` — 495 top-level tests passed,
  4 existing optional installed/authenticated CLI probes ignored; 3 subprocess
  self-tests also passed.
- `cargo build --release`
- `git diff --check`

The sandbox's `/tmp` has a synthetic `.git` ancestor and prohibits socket fixtures.
The first targeted run therefore failed at fixture setup. Tests were rerun outside
those restrictions using ordinary temporary storage, without weakening ownership
checks or skipping the failing cases. The final full suite passed.

The 19 new deterministic tests in `src/storage/tests.rs` and
`src/ui/storage_tests.rs` cover:

- Empty/legacy history, multiple unrelated files and exports excluded, actual
  state-file length, missing/corrupt saves, permission errors when supported by
  the host, and a real history cleanup reflected only after manual refresh.
  Multiple internal history files are not an existing supported layout; no
  artificial layout or directory-wide JSON scan was introduced.
- Default and multiple older custom worktree bases, orphan manifests/bindings,
  duplicate references, unavailable checkouts, incomplete/unknown ownership,
  malformed records, conflicting attempt identities and bounded diagnostics.
- Completed absent cleanup tombstones excluded, reappeared directories included
  without modifying resolution, and unrelated Git directories excluded.
- Empty/nested directories, generated output under target/node_modules/dist,
  4,096 files, a 5 GiB sparse file, broken/external/cyclic symbolic links, Unix
  hard links and sockets, permission failures, disappearing roots/children and
  replacement of a child with an external directory link during traversal.
- Partial byte aggregation, unknown versus zero, complete/partial totals and IEC
  formatting. State, manifest and generated file contents remain unchanged.
- Initial/manual refresh, repeated requests coalesced into a single pending scan,
  cancellation, stale success/failure rejection, worker-failure feedback and
  retention of successful timestamps. Drop/quit signal cancellation without
  waiting for filesystem workers or treating a scan as active agent work.
- Real egui widgets at 780 × 560 under system/light/dark themes; keyboard Refresh,
  Escape/Close and navigation to existing History Retention settings.
- Background measurement leaves active run records, retention transactions,
  manager/validation state and repository refresh state unchanged. No measurement
  code path invokes Git, process spawning, retention, validation or writes.

## Native Linux checks

The existing `v09_history_validation` fixture was built by the full all-features
suite and launched with 1,002 disposable history entries plus a temporary retained
Git worktree. The 780 × 560 main window was inspected in an X11 launch in the
GNOME/Wayland session. A separate default native Wayland launch stayed running
for an eight-second bounded startup check without startup errors. Neither launch
used user repositories or installed coding agents.

This is **startup coverage**, not native verification of Storage interactions.
Desktop automation could not reliably activate the Settings menu. Destroying the
X11 fixture window through that automation caused an upstream winit `BadWindow`
shutdown panic; normal application closing was not validated by that operation.
The fixture's temporary directory was removed by unwinding. The bounded Wayland
fixture was terminated and its exact disposable directory was removed separately.
The storage dialog's interaction/layout coverage is from headless egui tests.

## Remaining checks and limits

No native macOS or Windows runs/cross-compiles were available. Before release,
exercise native Storage opening/refresh/close, long paths and diagnostics on each
platform, Windows junctions/reparse points, permission failures and normal quit.
Also manually check refresh alongside an executing convoy and a large real build
directory on GNOME/Wayland. Automated active-state preservation and cancellation
are covered, but these manual responsiveness checks are not claimed here.

See [Storage Overview](storage.md) for shared history/settings size semantics,
logical versus allocated size, Unix/Windows hard-link differences, snapshot races,
lost custom-location bindings and the full ownership/transaction checks that PR 4
must retain. No cleanup, new dependency, schema migration or release preparation
was performed.
