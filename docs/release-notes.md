# CodeConvoy 0.4.0 — Attention & Completion

CodeConvoy 0.4.0 adds native desktop notifications and convoy completion awareness.
Run one task across local Git repositories using installed Codex CLI, GitHub Copilot
CLI, OpenCode or Claude Code, and receive one configurable alert when the whole
convoy finishes. This is unreleased source preparation; no tag or publication is
part of the implementation.

## Changes since 0.3.1

- Native XDG notifications on Linux, modern UserNotifications on macOS and WinRT
  toasts on Windows. Delivery and activation run outside the UI and scheduler.
- **Settings → Desktop notifications** provides a master switch and success,
  failure, cancellation and review-needed filters. All default to enabled except
  cancellation. Preferences persist with backward-compatible defaults.
- Foreground suppression defaults to enabled for success, cancellation and review.
  Failure alerts can still notify while focused; minimized windows count as
  unfocused. OS permissions and Focus/Do Not Disturb remain authoritative.
- One deterministic event wins: failure, cancellation, review needed, then success.
  Git changes and unavailable results can require review even after successful
  agent execution. No agent prose is treated as evidence of repository changes.
- Notification clicks use stable convoy IDs and the existing Review view. Removed
  history produces a notice; unavailable results keep their normal diagnostics.
  Restore is requested before focus, subject to desktop restrictions.
- History restoration, interruption recovery, refresh and Apply/Discard never
  replay completion alerts. Disabled, suppressed and failed alerts are not retried.
  Notification diagnostics stay separate from agent outcomes.

The native adapter uses notify-rust on Linux and its underlying modern macOS and
Windows transports directly for asynchronous/callback interaction. No web runtime,
tray, service, new agent backend or workflow feature is added. Existing dependency
versions remain unchanged; Cargo.lock adds only the new native notification trees.

Scheduler admission, concurrency limits, canonical/nested repository protection,
process-tree cancellation, immutable snapshots and result safety are unchanged.
See [notification preferences, dependencies and platform limitations](notifications.md).

## Result safety and limits

Apply conservatively refuses unsupported transfers, including submodules/nested
repositories, sparse or unmerged index state, divergent staged alternatives and
Git conversion settings that cannot be preserved. Changes must be reviewed again
if the retained result changes. Patches are capped at 32 MiB and rendered diffs
at 2 MiB; unavailable statistics are reported explicitly.

External tools can still change repositories outside CodeConvoy's leases. A
failure after an Apply write attempt is recorded as uncertain, retains evidence
across restart, and blocks retrying Apply, Discard and cleanup of that result.
There is no in-app uncertainty-resolution flow; manual inspection is required.
CodeConvoy never resets, cleans or stashes to recover a failed Apply.

Removing history only removes eligible CodeConvoy metadata. It never deletes
repository contents or grants permission to remove retained worktrees. Direct
execution can leave partial edits when stopped. Review changes before committing.

## Downloads and platform notes

Downloads include Linux x86-64 DEB/RPM/AppImage, Windows x86-64 per-user setup and
portable ZIP, and one macOS Universal DMG with Apple Silicon and Intel slices.
`SHA256SUMS` covers the six packages. Linux packages are built on Ubuntu 26.04.
Earlier DEB and AppImage packages have user-verified installation/startup
coverage on Debian Forky; those checks have not been repeated for 0.4.0 packages.
The minimum glibc version and compatibility with older distributions still need
verification after the runner update. A working desktop graphics environment is
required; the native picker uses your desktop portal.

On macOS, drag CodeConvoy to Applications. The app is **ad-hoc signed, not
Developer ID signed or notarized**. After verifying the source and attempting a
first launch, you may need the CodeConvoy-specific **Open Anyway** option in
System Settings → Privacy & Security. Follow
[Apple's guidance](https://support.apple.com/en-us/102445); do not disable
Gatekeeper globally. Windows packages are not Authenticode signed and may display
an unknown-publisher warning.

Install Git and your chosen coding-agent CLI separately. Desktop applications
may inherit a different PATH from your shell. Use an absolute agent executable
path where necessary; Git must also be available to the application.

## Validation and remaining limits

Linux x86-64 source validation on 2026-10-09 with Rust 1.95.0 passed:
`cargo fmt --check`, strict all-target/all-feature Clippy, all-feature Rust tests
(**297 passed, four optional CLI probes ignored**), `cargo build --release`, all
**16 packaging unit tests**, and the `v0.4.0` version/tag consistency check.
The latter validates consistency only; no tag was created.
Notification coverage includes mock transport errors/panics, event/focus policy,
exactly-once lifecycle tracking, restart silence, stable click routing and
headless egui restore/navigation tests. Real temporary Git repositories verify
Direct result classification while preserving HEAD and the real index.

Native notification smoke tests have **not** been performed on Linux, macOS or
Windows. macOS/Windows builds and current package installation also remain
unverified on this Linux host. The [native smoke checklist](notifications.md#validation-and-native-smoke-checklist)
is required before publishing:

- GNOME/Wayland and X11 delivery, focus suppression, multi-convoy clicks and missing
  notification service/action support. Wayland may require manual app activation.
- Packaged macOS `.app` authorization, Focus mode and body/button callbacks on both
  architectures. Bare source executables have no notification bundle identity.
- Installed and portable Windows 10/11 sender identity, permissions, banner and
  Notification Center callbacks, minimized restore and foreground restrictions.
- All platforms: removed history, unavailable retained results, shutdown while
  alerts exist, and no replay after restart. Cold-start click routing is unsupported.

A single submission is not a guarantee that the OS displayed a banner. Notification
failure does not affect execution, and a crash/exit can prevent pending delivery.
Windows callbacks after banner expiry require native verification. Apply uncertainty
resolution and older Linux/glibc compatibility retain their existing limitations.
Earlier authenticated agent and Git safety validation is documented separately;
this implementation does not claim new authenticated agent or native package tests.
Follow [the release procedure](releasing.md) before creating a release candidate.
