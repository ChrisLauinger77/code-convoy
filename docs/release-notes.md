# CodeConvoy 1.3.2 — Current CLI compatibility and stricter completion

## Changes since 1.3.1

### GitHub Copilot

- Support Copilot CLI 1.0.103, which retired `--no-remote` and renamed
  `--no-remote-export` to `--sync-session`. **Check CLI** previously rejected
  1.0.103 with `Copilot CLI is missing --no-remote` before any job started.
  Launch preflight now reads the selected executable's help and passes
  `--sync-session=off` when it offers that flag instead of both legacy flags;
  1.0.102 and earlier keep the legacy flags. Each generation rejects the other's
  flags, so a CLI replaced after the check fails at startup instead of syncing
  the session. See the [Copilot contract](../README.md#copilot-contract).
- Audit headless workspace trust against the shipped 1.0.103 code. Prompt mode
  loads repository hooks and workspace MCP servers only with saved folder trust,
  `--trust-folder`, `COPILOT_ALLOW_ALL=true` or explicit prompt-mode opt-ins.
  CodeConvoy still never passes `--trust-folder` and removes an inherited
  `COPILOT_ALLOW_ALL`, which therefore cannot act as trust either.

### Claude Code

- A run now fails when Claude reports a permission denial, even if Claude
  reports successful conversation completion. A denial stays sticky across later
  turns, and a malformed denial list fails validation.
- An explicit MCP tool timeout or unanswered MCP confirmation also fails the run.
  CodeConvoy does not retry the call or broaden permissions. Activity reports
  both cases with a `[CodeConvoy]` prefix and points to Raw output. See the
  [2.1.310 headless audit](claude-headless-audit.md).

### Process supervision

- On macOS, a stopped agent CLI, for example one stopped by a terminal read
  while asking for confirmation, is no longer mistaken for an exited process.
  Darwin reports such stops through the non-reaping exit check, which previously
  killed the live CLI and reported `SIGKILL`. A stopped CLI now stays supervised
  until **Stop** or its time limit, as on Linux.

### Compatibility audits

No production change was required for these backends; each audit adds
regression tests from real or documented CLI behavior:

- [OpenCode 2.1.0–2.1.4](opencode-2.1-audit.md): no incompatibility with the
  CLI event fields CodeConvoy consumes.
- [Codex 0.164.0](codex-0.164-audit.md): invocation, detection and result
  classification are unchanged.
- [Antigravity 1.4.3–1.4.7](antigravity-1.4-audit.md): no protocol change. The
  Terminal sandbox help now notes that agy 1.4.4 and later ignores `--sandbox`
  on Linux hosts that block unprivileged user namespaces, and that 1.4.6 and
  later sandboxed commands cannot use `~/.docker`, `~/.kube` or `~/_netrc`
  without a grant. CodeConvoy adds no grants.
- [Cursor v2026.10.22 subagent completion](cursor-subagent-completion-audit.md):
  no runtime incompatibility confirmed.

### Release process

- The Release workflow no longer publishes on tag push. It builds and verifies
  a candidate, waits for environment approval, validates candidate checksums and
  release readiness, then tags the validated commit and publishes that run's
  artifacts without rebuilding. Retries reuse an annotated tag only for the same
  validated commit and candidate run, and never move or overwrite tags.

### Dependencies and licensing

- No dependency changes. The recorded Microsoft/NSIS inputs, terms, runtime
  policy and Windows packaging configuration are unchanged, so the existing
  Windows licensing-risk decision applies without renewed acceptance.

**Windows Microsoft-component terms:** before using or redistributing Windows
downloads, review the [component terms](https://github.com/ChrisLauinger77/code-convoy/blob/v1.3.2/third-party/MICROSOFT-DISTRIBUTABLE-CODE.txt),
also included in `THIRD-PARTY-NOTICES.txt`. Permission for the Microsoft portions
is conditional on agreement to those terms; if you do not agree, do not use or
redistribute those portions. CodeConvoy's independent MIT license is unchanged.
Managed and downstream distributors must arrange recipient agreement; notice
delivery or silent installation alone does not establish it.

macOS remains ad-hoc signed without notarization, and Windows remains unsigned.
Native package and desktop verification remains a separate release step; see
the [platform matrix](platform-validation.md) and [release procedure](releasing.md).
