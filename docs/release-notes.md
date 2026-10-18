# CodeConvoy 1.2.0 — Antigravity CLI support

## Changes since 1.1.0

### Experimental Antigravity CLI support

- Add Google Antigravity CLI as a sixth backend, using the locally installed
  `agy` (1.2.16 or newer) and its existing sign-in. Add executable discovery,
  backend-specific model, effort, permissions, terminal sandbox and time-limit
  settings, plus UTF-8 text attachments.
- Run one headless turn over stdin with structured output. Show Activity
  summaries, bounded Raw output, denied-action counts and recovery advice for
  sign-in, model, quota and slash-command failures. History, reuse and Retry
  preserve the original backend settings; retries start a fresh convoy.
- Use the existing scheduler, repository leases, Direct and isolated worktree
  modes, process-tree cancellation and shutdown handling. Retain isolated
  results on success, failure and cancellation; uncertain process cleanup keeps
  the repository and execution slot reserved.
- Antigravity remains **experimental**. macOS `agy` 1.4.2 passed Check CLI,
  accepted CodeConvoy's argument forms and failed real Direct and isolated jobs
  at the sign-in gate as expected. Authenticated tasks, real permission and rule
  enforcement, and full protocol compatibility remain unverified. See the
  [Antigravity guide](antigravity.md) and [readiness record](antigravity-readiness.md).
- Antigravity keeps its own local conversation history; CodeConvoy cannot
  disable it or continue sessions. Its detached auto-updater can outlive a job;
  that behavior was observed on macOS, while Windows behavior remains unverified.

### Dependencies and release validation

- Refresh 27 locked crate versions and their license evidence without adding
  application dependencies or changing package formats.
- Update the release compiler to Rust 1.99.1 with matching standard-library
  license evidence. The source-build minimum remains Rust 1.95.
- Check current dependency license obligations across all targets in ordinary
  CI. Reserve exact release-inventory and dependency-fingerprint checks for
  release preparation and publication, while retaining notice/source integrity
  and native package verification.
- Reuse the existing Windows licensing-risk decision for unchanged components
  and distribution, with targeted reassessment on material input changes.
  Application version changes require no renewed acceptance; residual uncertainty,
  Microsoft terms and all objective checks remain.

**Windows Microsoft-component terms:** before using or redistributing Windows
downloads, review the [component terms](https://github.com/ChrisLauinger77/code-convoy/blob/v1.2.0/third-party/MICROSOFT-DISTRIBUTABLE-CODE.txt),
also included in `THIRD-PARTY-NOTICES.txt`. Permission for the Microsoft portions
is conditional on agreement to those terms; if you do not agree, do not use or
redistribute those portions. CodeConvoy's independent MIT license is unchanged.
Managed and downstream distributors must arrange recipient agreement; notice
delivery or silent installation alone does not establish it.

macOS remains ad-hoc signed without notarization, and Windows remains unsigned.
Native package and desktop verification remains a separate release step; see
the [platform matrix](platform-validation.md) and [release procedure](releasing.md).
