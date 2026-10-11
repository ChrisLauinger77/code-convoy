# 0.10.0 PR 2 — Accessibility and keyboard navigation

Validated on 2026-10-11, after PR 1 merged. Package version remains 0.9.0.
This phase covers existing controls and their input behavior only.

## Findings and changes

| Priority | Reproduced issue | Correction |
| --- | --- | --- |
| High | Closing a dialog lost keyboard focus; several dialogs opened without a focused control | Remember the opener and any hidden parent; choose a safe action or editor on opening and restore focus when the dialog disappears |
| High | A repeated Enter arriving with Tab could activate the button after a multiline editor | Filter repeated button activation during focus transitions while preserving ordinary editor repeat |
| High | Held Enter/Space and modified Space could activate focused confirmation buttons | Ignore repeats, repeated layout-pass activation and modified button activation outside editors; preserve the existing Command/Ctrl+Enter dispatch |
| Medium | History cleanup's widget identity changed when result resolution removed a nearby action | Give the menu launcher a stable UI identity; verify cancellation, resolution and reopening in one keyboard flow |
| Medium | Populated task/path fields and selectors lacked accessible names | Associate visible Task, Agent and Execution labels; name repository/worktree paths, history search/filter and per-run worktree checkboxes |
| Medium | Primary buttons override egui's focused fill, reducing the visual focus distinction | Paint a white inset focus outline against the existing primary fill in both themes |

All application modals use the small `ui/dialog` helper. egui retains responsibility
for Tab order, modal input blocking, popup priority and Escape/backdrop dismissal.
The helper tracks only transient widget IDs. It retains a hidden parent during
cleanup or quit confirmation, skips disabled/unavailable openers, and respects an
explicit move to another field. It neither keeps a popup open nor restores a
disappeared menu item. History's save-in-progress modal remains intentionally
non-dismissible until the pending save finishes.

Preflight opens on **Back to task**. About, shortcuts, CLI discovery, storage and
history settings open on Close/Cancel; library dialogs start in Name. Existing
validation fields and safe-default destructive confirmations retain their focus
behavior. No Run/Apply/Discard/Cleanup/Save operation was added or moved into the
focus helper. The existing shortcut reference now explains Tab, activation and
history navigation.

## Stack and accessibility limits

The lockfile resolves egui/eframe 0.36.2; eframe already enables `accesskit`.
No dependency, persisted schema, backend command or process/Git behavior changed.
The implementation uses the pinned source's supported `Response::labelled_by`,
`Response::widget_info`, `Memory::request_focus` and `plugin::Plugin` pass callbacks.
The existing [egui Modal](https://docs.rs/egui/0.36.2/egui/containers/modal/struct.Modal.html)
provides the input barrier; opener restoration is application bookkeeping.

AccessKit tests inspect control names, values, focus, enabled state and geometry.
Names remain available after placeholder text disappears. Native text inputs,
combo boxes and checkboxes keep their existing values/selection semantics.
Worktree checkboxes identify the repository, convoy and job instead of repeating
only the repository name. Visible text labels continue to explain status,
eligibility and disabled actions, with details/tooltips for longer diagnostics.

Existing small text remains 11.5 logical points, body/buttons 13 points and
monospace output 12 points. Theme tests retain at least 4.5:1 contrast for status
and muted text on the tested surfaces and for white on the primary fill. The
new focus ring uses that same contrasting white. Status words and the success
mark remain meaningful independently of color.

This does **not** certify Orca, VoiceOver or NVDA operation. Live announcements,
reading order, text selection, virtualized-history discovery and native file
picker behavior still need assistive-technology testing. Large history remains
keyboard navigable through its existing Up/Down/Home/End handling; only rendered
rows enter the accessibility tree. Desktop-reserved combinations may never reach
the application. No system-wide shortcuts are registered.

## Automated verification

The new regressions cover:

- Safe initial focus, normal close and Escape returning to the opener.
- Settings-menu launchers, hidden parent dialogs and explicit focus changes.
- Tab and Shift+Tab wrapping within the modal without trapping focus after close.
- Popup Escape priority inside validation configuration.
- Held/modified Enter and Space on launch/discard controls, and held Enter when
  Tab leaves a multiline editor; normal text-editor repeat remains intact.
- Populated draft/path/history control names and retained field values.
- Worktree keyboard selection, cancel-safe cleanup and focus restored to Storage.
- AccessKit logical bounds and root scale at 100%, 150% and 200%, in System,
  Dark and Light themes at a 780×560 logical viewport.

Existing suites also cover native modifier mapping (Command on macOS, Ctrl on
Linux/Windows), text copy/paste/select-all, repository groups/filtering, history
navigation with 5,000 entries, validation presets/assignment, export and cleanup
confirmations, and layouts at 780×560, 1180×820 and 1600×1000.

All required checks passed:

- `cargo fmt --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --all-features`: **561 passed, 0 failed, 6 ignored**, excluding
  filtered fixture subprocess re-executions. The ignored cases are four optional
  installed-provider probes and two manual performance measurements.
- `cargo build --release`
- `git diff --check`

The full filesystem suite used `TMPDIR=/var/tmp` outside the sandbox, following
the existing [history-validation environment workaround](history-reliability-validation.md).
Fourteen tests were added; the existing history-menu regression was extended to
assert restored focus and successful reopening after result resolution.

## Native Linux rendering

A disposable wrapper renders the real application under GNOME/XWayland with
software OpenGL. It supplies synthetic egui keyboard/focus input, tabs to About,
opens it with Enter, waits for the dialog to settle, then captures the focused
Close control and exits through the application close path. State and repository
fixtures are temporary; no installed agent is run. This verifies native rendering
with synthesized input, not physical keyboard delivery or screen-reader behavior.
The wrapper is removed from the repository after the check.

All three fixture processes exited successfully. Captures were inspected for
legible text, visible Close-button focus, dialog bounds and backdrop separation:

| Scale | Theme | Captured pixels | Result |
| --- | --- | --- | --- |
| 100% | Dark | 780×560 | Passed |
| 150% | Light | 1170×840 | Passed |
| 200% | Dark | 1560×1011 | Passed; the desktop limited window height |

The requested logical viewport was 780×560. The 200% capture therefore exercised
a shorter viewport after the window manager's clamp; the dialog remained fully
visible. These are focused smoke cases, not an exhaustive platform matrix.

## Phase boundary

Native macOS/Windows interaction and native Wayland rendering are not exercised
by these local checks. Prior platform checks are not counted as new PR 2 evidence.
Performance work, platform hardening and release preparation remain separate.
Stop after PR 2 and wait for explicit maintainer approval before PR 3.
