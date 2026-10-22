# CodeConvoy 1.3.1 — Bounded unattended agent runs

## Changes since 1.3.0

### GitHub Copilot

- Add a Copilot **Execution time limit** setting: 3600 seconds by default,
  1–86400 seconds, with blank restoring the default. Copilot execution
  previously had no deadline. The limit also applies to existing saved settings
  without this field. It starts after spawn and excludes queueing and worktree
  preparation. Expiry fails only the affected job after its process tree has
  been cleaned up, and retains its worktree and partial edits.
- CodeConvoy does not add Copilot's new `--trust-folder` flag, even in
  all-tools mode, because it also trusts repository hooks, workspace MCP
  servers and plugin settings. Failure messages and the safety summary now
  mention workspace trust. Review the job's exact working directory with
  Copilot outside CodeConvoy if it asks for a trust confirmation. See
  [workspace trust and unattended execution](../README.md#workspace-trust-and-unattended-execution).
- The help of the 1.0.99-1 prerelease is captured as a test fixture and passes
  the existing capability checks. No new CLI minimum is imposed.

### Cursor

- A blank or missing Cursor **Time limit** now uses 3600 seconds instead of
  running without a limit. This bounds unresolved approval and other
  unattended waits without granting any permission, including for older saved
  drafts. Explicit values from 1 to 86400 seconds are unchanged. See the
  [permission audit](cursor-permission-audit.md).

### Performance

- Registration, status, worktree, recovery and validation checks ask Git for
  repository identity with one `rev-parse` process instead of three. This
  matters most on Windows, where each Git start is expensive. Status still
  reads the common directory last, so a checkout repointed during inspection
  still fails the identity comparison.

### Dependencies and release validation

- Update tokio to 1.54.1, ureq to 3.5.0 and ksni to 0.3.7, plus lock-file
  maintenance. ksni now brings `arc-swap` and `crossbeam-queue` to Linux.
- Refresh the third-party notice inventory for 10 added and 8 removed crate
  versions. See the [1.3.1 license refresh](release-1.3.1-license-validation.md).
- The recorded Microsoft/NSIS inputs, terms, runtime policy and Windows
  packaging configuration are unchanged, so the existing Windows
  licensing-risk decision applies without renewed acceptance.

**Windows Microsoft-component terms:** before using or redistributing Windows
downloads, review the [component terms](https://github.com/ChrisLauinger77/code-convoy/blob/v1.3.1/third-party/MICROSOFT-DISTRIBUTABLE-CODE.txt),
also included in `THIRD-PARTY-NOTICES.txt`. Permission for the Microsoft portions
is conditional on agreement to those terms; if you do not agree, do not use or
redistribute those portions. CodeConvoy's independent MIT license is unchanged.
Managed and downstream distributors must arrange recipient agreement; notice
delivery or silent installation alone does not establish it.

macOS remains ad-hoc signed without notarization, and Windows remains unsigned.
Native package and desktop verification remains a separate release step; see
the [platform matrix](platform-validation.md) and [release procedure](releasing.md).
