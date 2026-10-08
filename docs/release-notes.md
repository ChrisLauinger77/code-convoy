# CodeConvoy 0.3.1

CodeConvoy 0.3.1 adds convoy-wide and global discard of unresolved isolated
results, and improves macOS branding and About links. Run one task across local
Git repositories using your installed Codex CLI, GitHub Copilot CLI, OpenCode or
Claude Code, then inspect each repository's result in the native desktop UI.

## Changes since 0.3.0

- Use **Discard convoy…** to discard unresolved retained isolated results from
  one terminal convoy, or **History cleanup → Discard all unresolved results…**
  to select them across terminal history. Confirmation shows the scope and count,
  defaults to Cancel, and freezes the selection so later results are not added.
- Each confirmed result uses the existing ownership checks, exclusive repository
  lifecycle lease and durable cleanup transaction. Active convoys and already
  Applied/Discarded results are excluded. Source files and unrelated worktrees
  remain untouched; unverifiable ownership and uncertain Apply remain blocked.
- Bulk discard reports successful and failed results independently, with
  expandable diagnostics. A failed target does not stop unrelated targets, and
  protected results keep their history pinned. Quit waits for the current cleanup;
  restart reconciles per-result state without replaying the batch.
- macOS application menus consistently use **CodeConvoy** for the application,
  About, Hide and Quit labels. The native About panel includes a clickable GitHub
  repository link; the in-app About link uses the same Cargo repository metadata.
- Refresh the README screenshot.

Both concurrency limits, fair admission, canonical/nested repository protection,
process-tree cancellation and immutable launched snapshots remain in force.
Isolated preparation consumes scheduler capacity; result actions take exclusive
repository lifecycle leases while unrelated repositories can continue.
No automatic commits, staging, pushes, pull requests or agent continuation are
introduced. Agent installation and authentication remain under your control.

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
coverage on Debian Forky; those checks have not been repeated for 0.3.1 packages.
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

Release preparation on Linux x86-64 with Rust 1.95.0 and Python 3.14.8 passed
formatting, strict Clippy, all-feature Rust tests (281 passed, four optional CLI
probes ignored), the release build, all 16 packaging unit tests and the
`v0.3.1` tag/version check. No dependency versions changed.

Deterministic fixtures exercise all four backends, real Git worktrees, scheduling,
cancellation, retained-result recovery, Apply/Discard, retries, follow-up drafts
and old-state compatibility. Bulk discard fixtures additionally cover frozen
confirmation, keyboard cancellation/confirmation, mixed outcomes, active-convoy
exclusion, ownership refusal, restart reconciliation and history protection.
Earlier native macOS fixture checks cover isolation, cancellation, restart,
a 24-row Review, successful and failed-result Apply,
destination blocking, keyboard Discard, Retry and selected-result follow-up.
These use disposable repositories and a local fixture agent rather than
authenticated model execution. Detailed evidence is recorded in:

- [Worktree execution validation](worktree-validation.md)
- [Recovery and cleanup validation](worktree-recovery-validation.md)
- [Review, Apply and Discard validation](review-apply-discard-validation.md)
- [Retry, follow-up and v0.3 readiness](retry-followup-validation.md)
- [Bulk discard validation](bulk-discard-validation.md)

The bulk discard Linux window and Cancel focus were inspected natively. Native
confirmation, partial/global completion and history removal were not validated
through desktop input; those paths have deterministic real-Git and egui coverage.
The macOS branding changes still require native macOS verification.

Codex and Copilot have earlier user-verified authenticated E2E coverage on Linux
and macOS. Authenticated attachment/model acceptance remains unverified for all
four backends; OpenCode and Claude authenticated E2E remain unverified overall.
Broader native Linux/Windows desktop checks, Intel macOS runtime checks and fresh
0.3.1 package installation checks remain outstanding. Local source validation
does not replace the manual native package candidate workflow and exact-artifact checks
described in [the release procedure](releasing.md).
