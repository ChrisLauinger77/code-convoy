# Codex CLI 0.164 compatibility audit

Recorded 2026-10-26 against CodeConvoy **1.3.1**, commit
`a83386de697018685dafc8ba0f3a9863d8451e53`, on Linux x86_64. The existing Codex
adapter was audited against Codex CLI **0.164.0** (released 2026-10-23) and the
previous **0.163.0**. No production code, dependency, persisted state or package
version was changed.

**No incompatibility was found.** The 0.164.0 fixes to session resume, `-c` /
`--config` handling and pending permission selections are real upstream bug
fixes. None changes the command, JSONL events, exit codes or completion rule that
CodeConvoy uses, so no adapter fix or version check is needed.

## Upstream fixes and CodeConvoy exposure

Source: the official [0.164.0 release notes](https://github.com/openai/codex/releases/tag/rust-v0.164.0)
and the `rust-v0.163.0..rust-v0.164.0` source comparison.

| Fix | Affected versions | CodeConvoy exposure |
| --- | --- | --- |
| Config overrides kept across command levels ([#56022](https://github.com/openai/codex/pull/56022)) | 0.163.0 and earlier keep only the deepest scope when both `codex` and its subcommand get `-c` | None. CodeConvoy passes one `--config model_reasoning_effort="…"` after `exec` and none before it. Both versions honored it. A root/exec split reproduced the drop on 0.163.0; 0.164.0 applied both. |
| Resume restores saved thread settings and finds compressed rollouts ([#55923](https://github.com/openai/codex/pull/55923), [#54008](https://github.com/openai/codex/pull/54008)) | 0.163.0 and earlier replace a resumed thread's saved reasoning summary, service tier and personality with global defaults | None. Jobs are `--ephemeral` and never resumed; retry and follow-up start new jobs. Resuming a CodeConvoy thread ID fails with `no rollout found`, exit 1, on both versions. |
| Queued prompts wait for pending permission selections ([#55682](https://github.com/openai/codex/pull/55682)) | Interactive TUI only; every changed file is under `codex-rs/tui` | None. `exec` submits one turn from stdin and has no prompt queue. It rejects every approval request at once (unchanged source), and approval policy `never` rejects escalation before any request. |
| Auto-cancelled MCP tool approvals marked non-interactive ([#54679](https://github.com/openai/codex/pull/54679)) | `exec` in 0.164.0 | No parser exposure. The approval is still cancelled, the tool fails and the turn continues. Not exercised with a real MCP server. |

## Completion and events

`exec_events.rs`, the JSONL event processor, `exec` CLI flags and its thread/turn
notification filter are unchanged between the tags. `exec` reports only its own
turn: `turn.completed` for a completed turn, `turn.failed` for a failed one and
nothing for an interrupted one. It exits 1 after a failed or interrupted turn or
a non-retried error. CodeConvoy still requires exit zero, `turn.completed` and no
`turn.failed`. Retried `error` records, such as `Reconnecting… n/5`, remain
Activity only. `exec --help` and `exec resume --help` are byte-identical; top-level
help adds only `migrate-nfs-db`. **Check CLI** accepts both versions.

## Real CLI evidence

Both official binaries ran with a fresh `CODEX_HOME`, a dummy key and
`openai_base_url` pointing at a local deterministic Responses API SSE fixture.
Repositories had spaces and Unicode in their paths. The default model in that
configuration uses code mode, so shell commands ran through nested `exec_command`
calls. CLI checks used CodeConvoy's exact argument vector. Runner checks drove
the unchanged adapter, RunManager and process executor with concurrent convoys.

| Check | 0.163.0 | 0.164.0 |
| --- | --- | --- |
| Successful execution, two repositories concurrently | Exit 0, `turn.completed`, Succeeded | Same |
| Configuration override (`exec --config model_reasoning_effort="high"`) | Request used effort `high` | Same |
| Overrides split across `codex -c` and `exec -c` (not used by CodeConvoy) | Root value dropped | Both applied |
| Persisted session continuation (outside CodeConvoy) | History resumed; saved reasoning summary reset | History resumed; summary restored |
| Resume of an ephemeral CodeConvoy thread ID | `no rollout found`, exit 1 | Same |
| Escalation request under approval `never`, workspace-write | Rejected immediately; turn completed, Succeeded, no file written | Same |
| Write in the read-only sandbox | `Read-only file system`; turn completed, Succeeded, no file written | Same |
| Failed execution (HTTP 400) | `error`, `turn.failed`, exit 1, Failed | Same |
| Cancellation during a running shell command | Cancelled; no process tree survivor | Same |
| Cancellation during an open model stream | Cancelled | Same |
| Unrelated concurrent convoy | Succeeded | Same |

With thread IDs removed and the fixture's WebSocket fallback records dropped,
stdout was identical across the versions in all eleven CLI scenarios.
Neither version was ever left waiting on a permission decision. Codex counted
a turn whose action was refused as complete, so the agent's answer and the Git
diff remain the result to review.

The cancelled job's tree spanned four process groups. Codex led CodeConvoy's
group. `codex-code-mode-host` had its own group. The command ran in separate
sessions under `bwrap --new-session --die-with-parent --unshare-pid --as-pid-1`.
CodeConvoy's group kill reaches only the first group. bwrap's parent-death
signal and PID namespace ended the sandboxed shell, and the host exited with
Codex. In two runner runs on 0.163.0 and three on 0.164.0, none of the job's
recorded processes was alive when CodeConvoy reported Cancelled or five seconds
later. The command's marker file was never written.

## Regression coverage and validation

[tests/codex.rs](../tests/codex.rs) replays 12 unmodified captures and both help
outputs from [the audit fixtures](../tests/fixtures/codex-0.164-audit/README.md).
It checks success, failure, rejected escalation, sandbox denial and both
cancellation streams. It also checks that a reported `turn.completed` with a
failed, interrupted or stopped exit is never success. Existing Codex argument,
decoder, Activity, concurrency and cancellation tests are unchanged.

`cargo fmt --check`, Clippy with all targets/features and warnings denied, the
release build, and the full suite (**829 passed, 0 failed, 15 ignored**) pass.
The opt-in authenticated Codex workflow probe was not run.

## Remaining risks

- Linux only. macOS Seatbelt and Windows were not run. Without bwrap's PID
  namespace, sandboxed commands outside CodeConvoy's process group rely on
  Codex's own cleanup; that was not observed here.
- The provider was local and synthetic. Hosted inference, authentication,
  WebSocket transport, MCP servers and real token usage were not exercised.
- In code mode neither version emits `command_execution` items for nested shell
  commands. Activity then shows the agent's messages and labelled stderr (such as
  the approval rejection), not each command. This predates 0.164.0.
