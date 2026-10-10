# Validation Presets & Bulk Assignment — 0.8.0 PR 4

Configuration only: one executable and literal argument list per registered
canonical repository. No new execution path, Git operation, dependency, state
schema, preset inheritance or automatic validation. See [usage](validation.md).

## Implementation

- `src/validation/presets.rs`: nine static templates with tested stable IDs.
- `src/validation/configuration.rs`: independent canonical-path selection,
  overlapping-group deduplication, immutable review and preserve/overwrite policy.
- `src/persistence/validation.rs`: stage edits using the existing
  `AppState::save_validation`, save once through Store, publish the command map
  only after success. Missing or changed targets produce individual failures;
  persistence failure fails every proposed update without leaking it into live state.
- `src/ui/validation.rs`: draft-only Preset/Custom editing, literal command preview,
  Save/Remove and safe dismissal. Long commands scroll with actions still accessible.
- `src/ui/validation_assignment.rs`: independent target selection, review and
  per-repository outcomes. Default preservation; overwrite has a separate explicit
  confirmation with Cancel focused. Keyboard focus scrolls target rows into view.
- `src/ui/shortcuts.rs`: the new modal participates in the existing modal gate;
  shortcut bindings and dispatch semantics are unchanged. Copy Results retains
  its existing command label; configuration uses a separate readable preview.

Preset identity is never persisted. Existing custom commands and version-1 state
remain compatible. Registered unavailable folders may receive configuration without
inspection; unregistered references are failures. A command changed after overwrite
review must be reviewed again. Save failures also disclose that a post-replacement
directory-sync failure can leave disk durability unconfirmed; reload or retry after
fixing state-directory access. There is no per-target persistence loop.

## Automated coverage

Eight integration tests in `tests/validation_presets.rs` cover exact preset IDs,
executables and argument boundaries; readable complex arguments; single Save,
edit, Remove and reload; old settings; empty/single/multiple selections; one or
multiple overlapping groups; deduplication; preserved custom commands; explicit
overwrite; removed/unregistered targets; changed commands after review; accurate
per-target reports; and deterministic atomic-replacement failure. Missing filesystem
paths deliberately remain absent, with no worktrees or unrelated state changes.

Four headless widget tests in `src/ui/validation_presets_tests.rs` use real egui
keyboard events and controls. They cover preset selection without persistence,
manual edits becoming Custom, switching presets and Custom, Save/Remove, Cancel
and Escape, group overlap, preserving and overwriting commands, default Cancel
focus, disabled empty selection, modal shortcut blocking, and unchanged convoy
selection, history, group membership and execution services.

Both dialogs, review and outcome views fit 780 × 560, 1180 × 820 and 1600 × 1000
in System, Dark and Light themes. Tests include more than 100 long repository
names and a long custom argument array to exercise scrolling. Existing validation
execution/cancellation, Copy Results, keyboard, scheduler and worktree suites
remain part of the full check.

## Native macOS smoke — 2026-10-10

Built and launched `v08_validation_presets` in a temporary native app bundle.
The fixture owns disposable Git repositories/settings, starts with one custom
command, and configures missing agent executables. Ordinary application state and
provider installations are not used.

At 1180 × 820, verified through native controls and saved state:

- npm Lint assigned to GNOME Extensions: one updated, one custom
  configuration preserved; executable `npm`, arguments `["run", "lint"]`.
- Rust Tests assigned to Rust projects: one updated, the same custom configuration
  preserved; executable `cargo`, arguments `["test"]`.
- Selecting both overlapping groups showed three unique targets. Overwrite review
  showed three replacements. Escape preserved every saved command.
- Explicit Confirm overwrite updated exactly three targets once. Source files,
  Git status and groups stayed unchanged; no runs or worktrees were created.

At 780 × 560, verified the final compact bulk selector and single editor visually.
Selected npm Lint, edited a command to Custom via native keyboard input,
saved `["run", "validate", "literal spaces", ""]`, and checked exact saved argument
boundaries. Reopening showed Custom with the saved executable. Safe Escape and
application Quit worked. Native screenshots used the system's dark appearance.

The native smoke preceded the display-name change to npm Lint; the command
and stable preset identifier are unchanged.

Native Linux, Windows, light appearance and 1600 × 1000 were not exercised.
Their layout/theme behavior has headless coverage, which is not native platform
proof. No actual preset command or authenticated agent was executed during these
configuration smoke tests. Existing Windows `.cmd`/`.bat` restrictions still apply;
use the documented native executable/Custom configuration for npm where needed.

The reusable fixture is available with:

```sh
cargo run --all-features --example v08_validation_presets -- 780 560
```

## Required checks

On macOS, 2026-10-10:

- `cargo fmt --check`: passed.
- `cargo clippy --all-targets --all-features -- -D warnings`: passed.
- `cargo test --all-features`: passed; four existing optional installed/provider
  CLI probes remain ignored.
- `cargo build --release --locked`: passed. The final rebuild required network
  access to restore cached metadata for the existing locked dependency version;
  `Cargo.lock` and dependency versions are unchanged.
- `git diff --check`: passed.

Before release, repeat the native configuration/keyboard smoke on Linux and
Windows, including npm launcher adaptation, and perform the normal exact-package
checks in [releasing](releasing.md). This feature does not bump the package version,
create a commit/tag, push changes or publish packages.
