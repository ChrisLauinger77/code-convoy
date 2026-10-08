# CodeConvoy 0.3.0

CodeConvoy 0.3.0 adds isolated execution, convoy-wide review and explicit result
resolution, plus retries and follow-up convoys. Run one task across local Git
repositories using your installed Codex CLI, GitHub Copilot CLI, OpenCode or
Claude Code, then inspect each repository's result in the native desktop UI.

## Changes since 0.2.0

- Choose **Direct** (the default) or **Isolated worktree** for each convoy.
  Isolated jobs start from the exact committed HEAD captured at launch review,
  without importing uncommitted source edits. Each attempt has a fresh detached,
  owned worktree; preparation failure never falls back to Direct.
- Retain isolated results after success, failure, cancellation or interruption,
  including unchanged and partially prepared worktrees. Startup validates saved
  ownership and availability in the background without resuming agents or
  recreating missing results. Unresolved ownership protects history from removal.
- Review all jobs in a compact convoy grid with execution status, mode,
  availability, resolution and Git-derived file/line statistics. Isolated Diff
  uses the saved base commit; Direct statistics describe the current working tree.
  Agent success and actual repository changes remain separate observations.
- Explicitly **Apply** a reviewed isolated result to its still-registered source
  only when the destination is clean at the saved base and ownership and changes
  pass revalidation and binary Git patch preflight. Apply changes working files,
  preserves HEAD and both real indexes, and retains the result copy for inspection.
- Explicitly **Discard** an isolated result with safe-default confirmation.
  Verified cleanup removes only that owned worktree through Git and leaves the
  source files intact. Applied copies can also be explicitly cleaned up. No
  automatic worktree deletion or pruning is performed.
- **Retry** a failed, cancelled or interrupted job as a new one-repository convoy
  with the original task, settings and attachment references. Normal launch review
  checks current Git, registration, backend capabilities and context again; isolated retries use a
  fresh worktree from newly reviewed HEAD. The earlier attempt stays independent.
- Use **New convoy from selected** to populate the draft from selected Review
  rows, copying registered repositories, backend settings, mode and per-convoy
  concurrency. Task and attachments start empty, omissions are reported, and
  nothing launches automatically. Retry/follow-up provenance links available
  source history without creating scheduling dependencies.
- Persist System/Dark/Light appearance. Existing v0.1/v0.2 state remains
  compatible, including registrations, groups, templates, settings and history;
  older jobs default to Direct mode.

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
coverage on Debian Forky; those checks have not been repeated for 0.3.0 packages.
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

Deterministic fixtures exercise all four backends, real Git worktrees, scheduling,
cancellation, retained-result recovery, Apply/Discard, retries, follow-up drafts
and old-state compatibility. Native macOS fixture checks cover isolation,
cancellation, restart, a 24-row Review, successful and failed-result Apply,
destination blocking, keyboard Discard, Retry and selected-result follow-up.
These use disposable repositories and a local fixture agent rather than
authenticated model execution. Detailed evidence is recorded in:

- [Worktree execution validation](worktree-validation.md)
- [Recovery and cleanup validation](worktree-recovery-validation.md)
- [Review, Apply and Discard validation](review-apply-discard-validation.md)
- [Retry, follow-up and v0.3 readiness](retry-followup-validation.md)

Codex and Copilot have earlier user-verified authenticated E2E coverage on Linux
and macOS. Authenticated attachment/model acceptance remains unverified for all
four backends; OpenCode and Claude authenticated E2E remain unverified overall.
Native Linux/Windows desktop checks, Intel macOS runtime checks and fresh 0.3.0
package installation checks remain outstanding. Local source validation does not
replace the manual native package candidate workflow and exact-artifact checks
described in [the release procedure](releasing.md).
