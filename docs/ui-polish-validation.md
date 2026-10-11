# 0.10.0 PR 1 — UI/UX consistency and polish

Validated on 2026-10-11. This is the first quality-and-polish PR only; the
accessibility, performance, platform-hardening and release phases remain separate.
The package version stays at 0.9.0 until release preparation.

## Audit and changes

The audit covered the task/attachment/backend editor, repository picker and
groups, Activity, Diff, Raw output, Task & settings, history filters, result
comparison, validation, Storage Overview, managed-worktree controls, Settings and
confirmation dialogs. Existing theme, format and diagnostic helpers remain the
presentation boundary. No dependencies, persisted fields, agent commands, Git
operations or scheduling rules changed.

| Priority | Reproduced issue | Change |
| --- | --- | --- |
| High | Settings with notification/tray diagnostics measured 954 pixels tall in a 780×560 window; lower actions were outside the window | Bound menu width and height; scroll its existing contents vertically |
| High | A long selected group name expanded the group dialog beyond 3,000 pixels wide at 780×560 | Truncate group choices and member rows; retain complete names/paths in tooltips |
| Medium | Unchanged diffs showed only boilerplate, while isolated output could point to an unavailable retained result | Detect empty diffs from Git observations and distinguish Direct observations from available comparisons with the saved isolated base |
| Medium | Running validation with no output displayed a restart-oriented message | Show that validation is running and awaiting output; failed/unavailable records direct users to diagnostics |
| Medium | Review statistics errors were visible only after opening Diagnostics | Show an explicit unavailable/retry explanation above the original diagnostic details |
| Low | Review indicators used “Review needed” while filters used “Needs Review” | Use “Needs Review” in result indicators and notification settings; execution outcome stays independent of Git changes |
| Low | Technical error chains were presented directly in Settings, Storage and Validation | Reuse concise explanations and expandable, copyable Diagnostics; preserve original details and Raw output |
| Low | Save confirmations and other informational notices were colored as errors | Render the generic notice channel neutrally; dedicated failure/status surfaces retain their labels and colors |
| Low | Destructive confirmation buttons and library-dialog footers differed | Reuse the theme palette for explicit destructive labels, keep safe Cancel focus, bound discard dialogs and wrap action rows; put Cancel alongside library actions |

Result job controls already fitted the tested pane sizes, so their compact row
layout was retained. Activity and Diff toolbars now permit wrapping. Existing
vertical scrolling, backend-specific forms, repository selection semantics,
history filtering, validation configuration and cleanup eligibility are retained.
Horizontal scrolling inside verbatim output/diff viewers remains intentional.

### Review corrections

The diff cache carries the change observation produced by the same background
inspection as its text. Direct inspection includes staged, unstaged and untracked
entries, including unborn repositories. Isolated inspection uses the existing
fixed-base snapshot and patches, including staged alternatives and empty new
files. An unsupported snapshot without a detected patch remains unknown and
keeps its diagnostic text; it cannot be presented as unchanged. Copy preserves
the complete original text.
No additional Git subprocesses or persisted fields are introduced.

Empty output recommends the retained diff only when result availability has been
verified in this session. Discarded and cleaned results report that Diff is no
longer available; missing, unchecked, pending or otherwise unavailable results
direct users to Review. Agent status remains unchanged.

Five additional regressions exercise actual Direct and isolated Git inspections,
index preservation, unsupported snapshots, rendered empty-versus-unknown diff
states, and output hints across availability/resolution states. Existing cache,
navigation, cleanup and applied-result tests use the structured observation.

## Automated verification

All required checks passed:

- `cargo fmt --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --all-features`: **545 passed, 0 failed, 6 ignored**. Ignored tests
  are four installed-provider probes and two manual performance measurements.
  Totals exclude filtered fixture subprocess re-executions.
- `cargo build --release`
- `git diff --check`

The complete filesystem suite used `TMPDIR=/var/tmp cargo test --all-features`
outside the sandbox, following the existing
[history-validation environment workaround](history-reliability-validation.md).
The initial sandbox UI run hit three existing cleanup-fixture failures because
the sandbox's `/tmp/.git` marker correctly triggers the ancestor safety check.
No production check was weakened. An interrupted full-suite attempt was rerun
to completion.

Four added presentation tests cover:

- Settings popup bounds, including long notification and tray diagnostics.
- Long Unicode group/member names with 24 registrations and bounded dialog
  actions. Both Settings and group tests reproduced their failures before fixes.
- Button bounds in all six result tabs at the narrowest supported results-pane
  widths, with running, queued and successful jobs and long repository names.
- Empty validation/output/diff wording, including retained isolated results.

Geometry tests run at 780×560, 1180×820 and 1600×1000 in System, Dark and Light.
Existing passing tests also cover all backend editor forms, group/filter
selection, validation presets and assignment, storage/cleanup dialogs, history,
safe-default confirmations, bounded output rendering and theme contrast. These
are egui layout/state tests, not native screen-reader certification.

## Native Linux smoke

Environment: Debian forky/sid, GNOME Wayland session, XWayland with software
OpenGL (`LIBGL_ALWAYS_SOFTWARE=1`, `WAYLAND_DISPLAY` unset). All repositories,
history and state belonged to disposable fixtures. No installed agent was run.

A temporary wrapper around `examples/v05_visibility_validation.rs` initialized
the saved appearance, rendered the real `ui::App`, captured an egui viewport
screenshot after startup, and requested `ViewportCommand::Close`. The wrapper
delegated `logic`, `ui` and `on_exit` to the application. It was removed from the
repository after validation. Captured pixel dimensions matched requested sizes;
all nine processes exited successfully through the application close path.

| Native viewport | System (dark on this desktop) | Dark | Light |
| --- | --- | --- | --- |
| 780×560 | Passed | Passed | Passed |
| 1180×820 | Passed | Passed | Passed |
| 1600×1000 | Passed | Passed | Passed |

Screenshot inspection covered the two panes, fixed execution controls, wrapping
tabs/actions, scroll boundaries, review outcomes, unknown observations and
unavailable-review diagnostics. The compact Settings popup and its vertical
scrolling were additionally inspected in a native window. Detailed interaction
coverage for the other views comes from the automated tests above.

External X11 automation was unreliable for pointer/keyboard targeting. Its
forced-window-destruction command produced an upstream winit `BadWindow` panic;
synthetic Alt+F4 did not reliably reach the window manager. Those attempts are
not counted as successful theme-selection or close tests. The self-capturing
fixture avoided those automation mechanisms and verified application-driven
close successfully.

## Limits and handoff

Native macOS/Windows, hardware-rendered Wayland, high-DPI interaction,
screen-reader behavior, notification delivery and tray-host integration were not
verified by this PR. Performance, packaging and release work were not performed.
No release tags or packages were published. Branch creation, commit, push and PR
creation were subsequently authorized by the maintainer. Stop after PR 1; begin
PR 2 only with explicit maintainer approval.
