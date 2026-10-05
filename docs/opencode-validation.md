# OpenCode backend validation

Recorded 2026-10-05. OpenCode is implemented as CodeConvoy's third independent backend. Claude Code remains planned. This record does not claim authenticated OpenCode E2E success.

## Evidence and invocation

Authoritative references inspected before implementation:

- [CLI documentation](https://opencode.ai/docs/cli/#run): `run`, JSON format, model/agent/variant options, `--auto`, help/version and discovery commands. Model discovery is available through `opencode models`; CodeConvoy uses a default or free-form qualified model override instead of maintaining a catalog.
- [Permission documentation](https://opencode.ai/docs/permissions/): allow/ask/deny rules, explicit-deny preservation in auto mode, permissive defaults and agent-specific overrides. These are tool permissions, not an operating-system sandbox.
- [Agent documentation](https://opencode.ai/docs/agents/): primary agents and subagents. CodeConvoy leaves agent definitions in OpenCode.
- [Released v1.18.34 run implementation](https://github.com/anomalyco/opencode/blob/v1.18.34/packages/opencode/src/cli/cmd/run.ts): piped stdin via `Bun.stdin.text()` and `resolveRunInput`, local in-process server when not attaching, directory handling using `PWD`, automatic rejection of approval requests without auto mode, JSON event framing, error exit paths and agent fallback warnings.
- [Released v1.18.34 processor](https://github.com/anomalyco/opencode/blob/v1.18.34/packages/opencode/src/session/processor.ts): finish reason carried in step-finish parts.
- The development source at [`a2aea963d1fb340396e66626c9e9d730f5bfee81`](https://github.com/anomalyco/opencode/blob/a2aea963d1fb340396e66626c9e9d730f5bfee81/packages/opencode/src/cli/cmd/run.ts) was also compared. The features used here agree with the released source and documentation. Local installed-version differences cannot be assessed because OpenCode is absent.

Exact non-secret argument shape (brackets indicate optional arguments, not literal arguments):

```text
<executable> run --format json --dir <absolute-repository-path>
  [--model=<provider/model>] [--agent=<primary-agent>] [--variant=<variant>] [--auto]
```

The prompt is sent as exact stdin bytes followed by EOF, never interpolated into a shell or placed on the command line. Cwd is the selected repository. Native path arguments are retained; inherited `PWD` and `GIT_*` overrides are removed. Detection uses `<executable> run --help` and `<executable> --version`, without a task. Version text is displayed, not converted into a guessed semantic-version compatibility rule.

CodeConvoy never requests, stores or edits provider credentials. It inherits OpenCode's provider, authentication and configuration environment. No provider manager, keys UI, custom model API, Claude backend, Activity normalization, scheduler feature or release automation was added. OpenCode's own plugins, hooks, MCP processes, session persistence and configured automatic sharing remain its responsibility.

## Implementation decisions

- Keep `AgentBackend` and `AgentOutput` unchanged. The existing option metadata, command specification and per-job decoder already allow a third backend with independent semantics. Codex and Copilot implementations are unchanged.
- Expose executable, optional model, primary agent, provider/model variant and permission mode. Blank model/agent/variant means OpenCode default. No fixed models or universal reasoning controls. `--thinking` controls display rather than reasoning effort and is not exposed.
- Default permission mode adds no override. OpenCode's existing allowed operations can execute; requests requiring approval are rejected. Explicit auto mode adds `--auto`, retaining configured denies. Neither mode provides filesystem/process isolation.
- Use JSON for a real completion check, not merely display. Exit zero plus a final `step_finish` with reason `stop`, no session error and no malformed/oversized JSON record is required. Starting a later step clears the previous stop. Unknown events and complete tool records remain visible; assistant text is rendered directly. Stderr and plain diagnostics remain visible. Tool failures alone can be recoverable and do not prove task failure or success.
- Reuse shared process-group/Windows Job Object cancellation. No attach/server reuse or daemon is launched by CodeConvoy. Confirmed termination releases the repository and both capacity slots. Unconfirmed cleanup uses the existing conservative quarantine. A Unix descendant that deliberately detaches cannot be guaranteed contained.
- Persist the additive `opencode` agent ID and its non-secret options in the existing version-1 schema. Defaults are materialized by preflight; snapshots are immutable. Accepted launch clears only prompt/selection. Reuse restores task/settings/registered selection without starting jobs. Existing Codex/Copilot state loads unchanged; older binaries cannot read a newly saved OpenCode variant.

## Checks performed

Environment: macOS (Darwin), Rust/Cargo 1.99.0, project minimum Rust 1.95. OpenCode was not on PATH. It was not installed. No provider credentials were requested and no authenticated agent task was run.

| Check | Result |
| --- | --- |
| `cargo fmt --check` | Passed |
| `cargo clippy --all-targets --all-features -- -D warnings` | Passed |
| `cargo test --all-features` | 62 passed, 0 failed; 2 installed-CLI probes intentionally ignored |
| `cargo build --locked --release` | Passed (optimized macOS binary) |
| Native macOS UI | Launched the actual `ui::App` using temporary state; OpenCode controls, missing-CLI error, Codex/Copilot switching, minimum 780×560 viewport and larger resized window inspected; dark and light rendering checked |
| Headless egui layout | All three implemented backends fit 310/360/520-point editor widths in both themes; detection remains tied to selected agent/settings |
| Linux launch / Linux desktop UI | Not performed: this session runs on macOS with no configured Linux execution environment |
| Real OpenCode help/version/generated-argument probe | Not performed: CLI absent; optional help-only test is provided |
| Authenticated OpenCode E2E | **Unverified** |
| Hosted CI matrix | Not run in this session; workflow unchanged, local equivalent checks passed |

The temporary UI launcher and app wrapper are not shipped. No production app state was used for that check.

Coverage includes missing and incompatible executables, help capabilities and displayed version, default/optional arguments, literal stdin and actual repository cwd, permission mapping, invalid settings, split UTF-8, tool/error/unknown events, final-stop requirements, malformed/oversized records, nonzero and signal exits, descendant cancellation and unaffected peers, stale Git baselines, version-1 round trips, backend preferences, snapshotting, draft clearing, reuse, mixed-backend concurrency, multiple OpenCode convoys and canonical repository-lock release after success/failure/cancellation. Existing Codex/Copilot command regression assertions remain in place.

`tests/fixtures/opencode-run-help.txt` is explicitly a **synthetic source-derived compatibility fixture**, not real captured help. The executable in `tests/support/agent.rs` is a deterministic test process using disposable repositories and handshake gates. Its success and error events match the documented source protocol but are not evidence of provider execution.

## Limitations and follow-up verification

1. Authentication, provider/model availability, platform-specific OpenCode behavior and actual CLI flag acceptance need installed-CLI and authenticated tests. Help checks verify exposed capabilities, not every runtime semantic. Older CLIs lacking any required option are rejected even when that particular optional setting is blank.
2. OpenCode can fall back to its default agent on an unknown or subagent selection. Its warning stays visible. No provider entitlement or full agent-registry validation is attempted by CodeConvoy.
3. Existing permissions can allow shell and files; auto mode can broaden asked permissions. Neither is an OS sandbox. There are no synthetic “read-only” or “file edits only” guarantees.
4. A 256 KiB stdout record exceeds the decoder's bounded protocol verification and fails conservatively, even if the process later exits zero. CLI completion, including a valid stop, is not proof of task fulfillment. Always inspect response and diff.
5. OpenCode can retain its own sessions or auto-share according to its existing configuration. CodeConvoy does not rewrite that configuration or duplicate its management UI.
6. Linux desktop and real provider E2E remain outstanding. Codex/Copilot's earlier user-verified Linux E2E status is preserved, not re-claimed as a new run here.

Use the [safe two-repository procedure in README](../README.md#safe-manual-opencode-e2e-test). On Linux, also check all three selections and `Check CLI`, resize to 780×560 and larger sizes, confirm scroll access to every field and fixed execution controls, and inspect light/dark/system themes. Record the real OpenCode version, selected primary agent/model and outcome without credentials before marking OpenCode E2E verified.
