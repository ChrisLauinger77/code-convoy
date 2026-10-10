# Keyboard Shortcuts validation — 0.8.0 PR 3

Scope: local keyboard navigation in the existing native egui application.
The [user reference](../README.md#first-run) and Settings → Keyboard Shortcuts…
describe the same seven bindings. No dependencies, persistent settings, backend
changes, scheduler changes, release tag or package publication.

## Implementation and conflicts

There were no application-level key bindings to replace. Existing Tab, text
editing, menu navigation and modal Escape behavior came from egui. Its buttons
also treat modified Enter as activation, which could otherwise activate Start
or a destructive confirmation. The dispatcher consumes modified Enter outside
text editors even when Run is blocked. Plain Enter/Space retain normal activation.

`src/ui/shortcuts.rs` contains one binding table, a context gate, one dispatcher
per frame, and the help dialog. egui's logical Command modifier maps to physical
Command on macOS and Ctrl on Linux/Windows. Extra Alt/Shift and macOS Ctrl+Command
are rejected. The help uses egui's modifier formatter with ⌘ on macOS; the bundled
font supports that symbol even though egui's generic formatter falls back to
text when other modifier symbols are missing.

The button in `src/ui/editor.rs` and keyboard Run both call `run_convoy`, which
checks `can_run_convoy` and enters the existing `preflight`. No shortcut calls
Start or the run manager directly. The normal CLI/configuration checks, Git
review, dirty acknowledgment, saved snapshot and admission checks remain intact.

Search focus and Unicode character selection are installed before widgets read
the frame's text events. Immediately typing after Ctrl/Cmd+F therefore updates
the existing filter, including when focus was in the task editor. The picker
scrolls it into view. Result navigation changes only the existing tab state.
Modal Escape uses `Modal::should_close`; its existing handlers map to safe
Cancel/Back/Close. Held Escape cannot cascade through underlying dialogs.

## Automated coverage

Six mapping/dispatcher tests in `src/ui/shortcut_mapping_tests.rs` cover:

- Platform modifier events and labels, all bindings, wrong/extra modifiers and
  unsupported combinations.
- Context availability, modal blocking, text-edit precedence and unhandled
  search/navigation when unavailable.
- One dispatch per frame, multiple events, repeated presses and repeated calls
  within a frame; suppression of held Escape.
- Unmodified preservation of copy/cut/paste/select-all/undo and macOS Control-F
  input delivered to text editors.

Twelve widget tests in `src/ui/shortcut_tests.rs` use the existing headless egui
harness and real application controls. They cover shared button/shortcut
preflight with a missing fixture CLI, readiness/probe gates, dirty review and
modified Enter on a focused Start button; every egui dialog and native picker
state; safe quit/discard/bulk cancellation; topmost-only and held Escape;
filter focus/Unicode selection/query/group preservation; same-frame typing;
view selection with unchanged run/job/cache/filter/focus state; real text
copy/paste/select-all; keyboard help access; and the full UI at 780 × 560 and
1180 × 820 in System, Dark and Light themes. No provider agents run in these tests.

## Native coverage — macOS, 2026-10-10

The existing `v05_visibility_validation` example was built with all features and
launched in a temporary app bundle at 1180 × 820. It uses disposable repositories,
history and settings, with missing CLI paths rather than installed agents.

Verified through native keyboard input and window screenshots:

- Command-F focuses and scrolls the existing repository filter into view.
- Typing immediately after Command-F filters repositories; repeating Command-F
  selects and replaces the previous query. Command-2 stays inactive while the
  filter has text focus. Escape retains the query and selections.
- Command-1/2/3/4 selects Activity/Diff/Raw Output/Task & Settings.
- Settings opens the compact shortcut help; Command-1 and Command-Enter leave
  the help and background view unchanged. Escape closes help safely.

Native Linux GNOME/Wayland, X11 and Windows were not available on this macOS
host. Their modifier mapping and help labels have deterministic coverage; this
does not establish native runtime behavior. Actual successful agent launch,
held-key repeat delivery and destructive confirmations were not exercised
through native automation; their routing and safety behavior have automated
coverage. Existing optional authenticated CLI probes remain ignored.

Manual follow-up on each platform: select repositories and enter a task, leave
text focus, use Run's shortcut and review dirty state. Hold the chord and confirm
only one review appears. Check native copy/paste/undo, search replacement, Escape,
all view bindings, nested menus and each confirmation's safe Cancel. Verify
light/system appearance and smaller windows. OS/window-manager-reserved key
combinations may never reach the application; no global hooks are installed.

## Required checks

On macOS, 2026-10-10:

- `cargo fmt --check`: passed.
- `cargo clippy --all-targets --all-features -- -D warnings`: passed.
- `cargo test --all-features`: passed, including all 18 shortcut tests. Existing
  optional provider probes remain ignored.
- `cargo build --release`: passed.
- `git diff --check`: passed.

The native fixture is a development smoke test, not a published package.
