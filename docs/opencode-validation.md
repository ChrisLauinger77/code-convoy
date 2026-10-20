# OpenCode backend validation

Recorded 2026-10-05. OpenCode is implemented as CodeConvoy's third independent backend. Claude was still planned at the time of this OpenCode validation; see the subsequent [Claude validation](claude-validation.md). This record does not claim authenticated OpenCode E2E success. The [OpenCode 2 compatibility](#opencode-2-compatibility) section was added on 2026-10-20.

## OpenCode 2 compatibility

Recorded 2026-10-20 for CodeConvoy 1.3.0. A compatibility watch reported that OpenCode 2 may no longer accept `--dir`. That is confirmed: OpenCode 2 (tags `v2.0.0`, 2026-09-11, to `v2.0.32`, 2026-10-16) has no `--dir` for `run`, and its parser rejects unknown flags. Every OpenCode 2 job failed in that parser, and **Check CLI** already rejected OpenCode 2 for the missing flag. OpenCode 1 (`opencode-ai` on npm, latest `1.19.3`) still supports `--dir`. OpenCode 2 is supported on Windows only for now; macOS and Linux refuse it explicitly (see the cancellation decision below). OpenCode was not installed on the development Mac, so no installed-CLI or authenticated check was possible.

Sources:

- [Migrate from V1](https://github.com/anomalyco/opencode/blob/v2.0.32/services/www/src/docs/content/migrate-v1.mdx): OpenCode 1 and 2 both use the `opencode` command and are no longer installed side by side by default. OpenCode 2 is distributed as `@opencode/cli` (bins `opencode` and `opencode2`), Homebrew `anomalyco/tap/opencode-v2` and the v2 installer; the [v2 docs](https://opencode.ai/v2/docs) list these.
- [v2.0.32 `run` command](https://github.com/anomalyco/opencode/blob/v2.0.32/packages/cli/src/commands/commands.ts): message, `--standalone`, `--server`, `--continue`, `--session`, `--fork`, `--model` ("provider/model#variant"), `--agent`, `--format default|json`, `--file`, `--title`, `--thinking` and `--auto`. There is no `--dir`, `--variant`, `--attach` or `--port`.
- [effect 4.0.1 CLI parser](https://github.com/Effect-TS/effect/blob/460272d30457f4697d8b8c52cad41caccbcace08/packages/effect/src/cli/internal/parser.ts), used by OpenCode 2: an unrecognized flag is an `UnrecognizedOption` error before the command runs; the [lexer](https://github.com/Effect-TS/effect/blob/460272d30457f4697d8b8c52cad41caccbcace08/packages/effect/src/cli/internal/lexer.ts) accepts `--name=value`.
- [v2.0.32 run implementation](https://github.com/anomalyco/opencode/blob/v2.0.32/packages/cli/src/run/run.ts): the directory is `PWD`, else the process working directory; piped stdin is merged into the message; setup errors are written as a JSON `error` event with exit 1.
- [v2.0.32 server connection](https://github.com/anomalyco/opencode/blob/v2.0.32/packages/cli/src/services/server-connection.ts): without `--standalone` or `--server`, `run` discovers or starts a shared background service for the user account. Only the service's own `service.json` can disable that, and CodeConvoy does not edit it. The [CLI command docs](https://github.com/anomalyco/opencode/blob/v2.0.32/services/www/src/docs/content/cli/commands.mdx) use `opencode run --standalone` for CI.
- [v2.0.32 standalone server](https://github.com/anomalyco/opencode/blob/v2.0.32/packages/cli/src/services/standalone.ts): `serve --stdio` is started as a child with the run's working directory and inherited environment, and [ends when its stdin closes](https://github.com/anomalyco/opencode/blob/v2.0.32/packages/cli/src/server-process.ts). The [spawner](https://github.com/anomalyco/opencode/blob/v2.0.32/packages/util/src/cross-spawn-spawner.ts) detaches it into its own process group on macOS/Linux.
- [v2.0.32 non-interactive events](https://github.com/anomalyco/opencode/blob/v2.0.32/packages/cli/src/run/noninteractive.ts): the same `step_start`, `text`, `tool_use`, `step_finish` and `error` records, with [finish reasons](https://github.com/anomalyco/opencode/blob/v2.0.32/packages/schema/src/llm.ts) `stop`, `length`, `tool-calls`, `content-filter`, `error` and `unknown`. Errors set exit 1; an interrupt exits 130. A permission request is rejected with model-visible feedback and the model continues.
- OpenCode 1 [v1.19.3 run](https://github.com/anomalyco/opencode/blob/v1.19.3/packages/opencode/src/cli/cmd/run.ts) matches v1.18.34 apart from discontinued sharing: without `--dir`, a local run uses the real path of `PWD`, else the working directory. Its [yargs setup](https://github.com/anomalyco/opencode/blob/v1.19.3/packages/opencode/src/index.ts) is strict, so OpenCode 1 rejects `--standalone`.

Decisions:

- One argument list cannot serve both versions. OpenCode 2 requires `--standalone` to keep the server out of the shared service, and OpenCode 1 rejects that flag. OpenCode 2 also rejects `--dir` and `--variant`.
- OpenCode 1's invocation is unchanged. Its `--dir` also makes an OpenCode 2 binary fail before reading the task; likewise `--standalone` makes OpenCode 1 fail. Neither runs in another directory, and CodeConvoy never retries with other arguments.
- A new OpenCode setting, **OpenCode version** (`version`: `1` default, `2`), selects the invocation. It is part of the immutable launch snapshot, so retries and reuse keep it, and existing saved state still means OpenCode 1. `AgentBackend::build` cannot observe detection results, and recording the detected version automatically would need new preflight plumbing into the snapshot. Instead, a default trait method `check_detection_for(options, help)` lets **Check CLI** and launch preflight verify the selected version against `run --help`. When the help matches the other version, the message names it and the setting. Other backends keep the default.
- The OpenCode 2 help contract is the effect usage line `run [flags]`, JSON format, and `--standalone`, `--format`, `--model`, `--agent` and `--auto` as flag tokens. Text in a description, such as `--print-logs`' mention of `--standalone`, does not count. The version string stays informational.
- OpenCode 2 runs `run --standalone --format json` with the process working directory set to the exact repository or isolated worktree. The backend requires an absolute path and removes `PWD` and `GIT_*` overrides as before. A Variant is sent as `--model=provider/model#variant`. A variant without a model, or a `#` in either value, is rejected because OpenCode 2 cannot express it.
- The JSON decoder and its fail-closed completion rules are unchanged, because the event shapes and finish reasons are the same.
- **Cancellation (review finding on #95):** OpenCode's spawner starts the standalone server detached into its own process group on macOS and Linux. `run` terminates and awaits that server only when it exits through its own lifecycle; CodeConvoy's cancellation, timeout and quit SIGKILL the `run` process group, which orphans the server until it notices its stdin lease closing. CodeConvoy could then release the repository while the server or its tools still write, which AGENTS.md forbids. OpenCode 2 is therefore refused on macOS and Linux in `detection` and in the help check, before any process starts, with an explicit message; with OpenCode 1 selected, an OpenCode 2 executable is named as such. Windows remains supported: CodeConvoy's Job Object (`KILL_ON_JOB_CLOSE`, no breakaway) contains the server and its tools, and cleanup waits until the job has no active process. Supporting macOS and Linux needs confirmed containment of those detached groups and is left to a follow-up.

Fixtures: `tests/fixtures/opencode-2-run-help.txt` and `tests/fixtures/opencode-2-events.jsonl` are **synthetic source-derived** fixtures, not captured CLI output. The test agent behaves as OpenCode 2 when it is hard-linked under the name `opencode2`, which is also OpenCode 2's alternative bin name. In that mode it prints the v2 help and `opencode v2.0.32` and rejects flags outside the v2 `run` set, such as `--dir`. As OpenCode 1 it rejects `--standalone`.

Checks on macOS (Darwin), Homebrew Rust/Cargo 1.99.1:

| Check | Result |
| --- | --- |
| `cargo fmt --check` | Passed |
| `cargo clippy --locked --all-targets --all-features -- -D warnings` | Passed (main including #96, which fixed an unrelated `egui::WidgetType` deprecation) |
| `cargo test --locked --all-features` | 789 passed, 0 failed, 12 ignored; OpenCode 2 invocation tests are Windows-only and additionally passed once on macOS with the platform gates forced on |
| `cargo build --locked --release --bin codeconvoy` | Passed |
| `python3 -m unittest discover -s packaging -p 'test_*.py'` | 110 tests passed, 1 skipped |
| Installed OpenCode 1 or 2 help/version probe | Not performed: OpenCode absent |
| Authenticated OpenCode 1 or 2 E2E | **Unverified** |
| Linux and Windows | Not run locally; left to the hosted CI matrix |

Other OpenCode 2 changes, reported but not in this fix's scope: a new plugin API, server API and client contracts; TUI configuration moved to `cli.json`; and `external_directory` allowed by default in OpenCode 2 (OpenCode 1 asks). In non-interactive runs, permission asks and questions are rejected or cancelled with feedback instead of interrupting the session. Session continuation (`--continue`/`--session`) is unused by CodeConvoy.

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
OpenCode 1 (default):
<executable> run --format json --dir <absolute-repository-path>
  [--model=<provider/model>] [--agent=<primary-agent>] [--variant=<variant>] [--auto]

OpenCode 2 (working directory = absolute repository or worktree path):
<executable> run --standalone --format json
  [--model=<provider/model>[#<variant>]] [--agent=<primary-agent>] [--auto]
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
7. OpenCode 2 support is Windows-only, and source- and fixture-verified only; no OpenCode 2 binary was run. Its invocation tests run in the Windows CI job; on macOS they were also run once with the platform gates forced on. Users must select **OpenCode version** themselves. Check CLI detects a mismatch but does not change the setting, and saved settings default to OpenCode 1. Retry reuses the original setting, so after a major upgrade start a new or reused convoy with the new version selected.
8. On macOS and Linux, OpenCode 1's shell-tool commands run in their own process groups, outside CodeConvoy's process-group cancellation; CodeConvoy cannot confirm that they ended. OpenCode 2 is refused there because its whole server would be detached the same way. Windows Job Objects contain both.
9. OpenCode 2 cannot express a variant without a model. CodeConvoy rejects that combination instead of choosing a model.

Use the [safe two-repository procedure in README](../README.md#safe-manual-opencode-e2e-test). On Linux, also check all three selections and `Check CLI`, resize to 780×560 and larger sizes, confirm scroll access to every field and fixed execution controls, and inspect light/dark/system themes. Record the real OpenCode version, selected primary agent/model and outcome without credentials before marking OpenCode E2E verified.
