# CodeConvoy 0.7.0 — Validation & Confidence

## Changes since 0.6.0

- Optional per-repository validation configuration: executable plus literal
  arguments, edit/remove, persisted in the existing state file. Disabled by default.
- Explicit **Run Validation** and **Cancel Validation** in repository result details,
  a separate Validation output tab, and compact comparison indicators.
- Correct original Direct/isolated directory targeting with identity and ownership
  checks; unavailable historical worktrees never fall back to the source checkout.
- Responsive background execution with streamed bounded stdout/stderr, elapsed
  time, exit status, timestamp, process-tree cancellation and coordinated Quit.
- Latest validation record and 64 KiB output saved with the exact convoy/job.
  Existing history remains compatible; interrupted validation never resumes.
- Agent outcomes, Git completion observations and result resolution stay independent.
  Saved passes describe past executions, and current Git views refresh after commands
  modify files. No automatic execution, staging, commits, push or approval.
- Existing repository leases coordinate validation, agents and result operations;
  unrelated convoys keep their normal concurrency and scheduling. No new scheduler,
  database, dependency, backend, pipeline or validation dashboard.

See [validation usage and configuration](validation.md) and
[architecture](architecture.md#v07-explicit-validation).

## Validation and remaining platform work

Linux x86-64 source validation on 2026-10-10 with Rust 1.95 passed:

- `cargo fmt --check`
- `cargo clippy --locked --all-targets --all-features -- -D warnings`
- `cargo test --locked --all-features`: 372 tests passed, four optional installed-CLI
  probes ignored (excluding duplicate subprocess-test reporting).
- `python3 -m unittest discover -s packaging -p 'test_*.py'`: 16 passed.
- `cargo build --locked --release`
- `git diff --check`

Headless egui coverage includes all three requested sizes and System/Dark/Light
modes, keyboard configuration/Run/Cancel, Escape dismissal and active-validation
quit confirmation. A native Linux X11 debug build started with disposable state
and repositories; Review and the Validation tab were visually inspected at
1180 × 820, including the displayed command and original target path. Native
Run/Cancel/history interaction acceptance was not completed; automated X11 input
was not reliable in this desktop session. Process execution/cancellation and
history behavior are covered by the deterministic tests, not claimed as native
interaction acceptance.

macOS/Windows builds and native runtime checks, full Linux/Wayland interaction
smoke, native theme/size/keyboard coverage, and exact release-package checks remain
required. No tag, push or package publication is part of this implementation.

Known limits: historical passes are timestamped observations, not continuous
certificates. Commands execute with local user permissions and may modify files;
side effects are retained on failure/cancellation. Intentionally detached Unix
children can escape process groups. Windows batch launchers require a directly
configured native interpreter such as node.exe; CodeConvoy never inserts a shell.
