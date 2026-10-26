# OpenCode 2.1 event compatibility audit

Recorded 2026-10-26 against CodeConvoy **1.3.1**, commit
`5b61e9cdb7acf83425d828a8f6b67788b980e7b7`, on Linux x86_64. The existing
OpenCode 1 and 2 implementation was audited without changing production code,
dependencies, persisted state, or the package version.

**No incompatibility was established between OpenCode 2.1.0–2.1.4 and the CLI
fields CodeConvoy consumes.** The reported SDK, HTTP and plugin breaking changes
are real. OpenCode's CLI translates those interfaces into its existing JSON
output format. Cancellation has separate existing limitations, described below;
this is not a claim that every process or provider scenario passes.

## Confirmed upstream changes and CodeConvoy exposure

The official [2.1.4 changelog](https://github.com/anomalyco/opencode/blob/v2.1.4/CHANGELOG.md)
records these releases. V2 is published as `@opencode/cli`; the GitHub Releases
collection also contains the separate OpenCode 1 release line. A missing V2
GitHub Release page does not mean that its npm release or Git tag is absent.

| Version and interface | Confirmed change | CodeConvoy exposure |
| --- | --- | --- |
| 2.1.0 SDK, HTTP messages/events, plugin contracts | Replay fields `state`, `providerState`, and `resultState` become `native`; tool call/result replay blobs merge into `tool.native`; exported schema and plugin compaction results change accordingly. | No SDK dependency, HTTP subscription, plugin implementation, or replay-data lookup. CodeConvoy reads the CLI's `part.state.status`, `input`, `tool`, text and finish reason. Tool execution `state` was explicitly **not** renamed. |
| 2.1.0 compaction contracts | Remove `session.compacted`, `recent`, and `providerContext`; use `session.compaction.started`/`ended`, `metadata.keepID`, and `native.messages`. | CodeConvoy does not consume these events or reconstruct session history. OpenCode owns compaction and continuation. CLI compaction failures remain `error` records. |
| 2.1.0 credential writes | `Credential.Key.configuration` moves to `metadata`; storage/import compatibility remains. | CodeConvoy performs no credential writes or imports. |
| 2.1.2 structured errors | Add/preserve `retryAfterMs` for provider errors. | Additive field. The generic JSON decoder retains the entire error in Raw and Activity, marks the session failed, and does not schedule automatic retries. Captured quota errors preserve `retryAfterMs: 2000`. |
| 2.1.4 MCP elicitation and UI | Restore located input-form events and desktop/web forms; fix cancellation of request ID zero. | CodeConvoy supplies plain stdin, without implementing MCP prompts, forms, SDK callbacks, or a user-answer channel. Tool outcomes and CLI exit status still determine the result. Global MCP input can wait for its server timeout. |

The replay and compaction changes are documented in
[upstream #57260](https://github.com/anomalyco/opencode/pull/57260).
The MCP behavior is documented in
[#58184](https://github.com/anomalyco/opencode/pull/58184) and
[#58305](https://github.com/anomalyco/opencode/pull/58305).

## Actual invocation and completion

The existing [adapter](../src/agents/opencode.rs) runs OpenCode 2 with
`run --standalone --format json`, the job's absolute working directory and the
prompt on stdin. It removes inherited `PWD` and Git overrides. OpenCode 1 still
uses `run --format json --dir <absolute-directory>`. Help detection checks the
selected interface; model, agent, variant and permission arguments are unchanged.

The [V2 CLI formatter](https://github.com/anomalyco/opencode/blob/v2.1.4/packages/cli/src/run/noninteractive.ts)
constructs `step_start`, `text`, `tool_use`, `step_finish`, and `error` records from
internal session events. Comparing 2.0.32 with 2.1.0 shows the formatter adopting
`native` internally while preserving the fields CodeConvoy reads. The formatter
is byte-identical at tags **2.1.0, 2.1.1, 2.1.2, 2.1.3 and 2.1.4**
(SHA-256 `2e0f5cecf05794eacc363a59484d2b9c1c358ac730aa6d6c4c4b43328b93a923`).
CLI tool metadata may carry provider replay information, but CodeConvoy does not
interpret it.

OpenCode 2 may close its event subscription after waiting for execution and
reconciling messages, before printing the final `step_finish`. Some new captures
include that final record and others omit it. The existing rule covers both:
exit zero, the last started step has its own nonempty text or named tool response,
no reported session error, no malformed/oversized JSON, and no reported final
finish reason other than `stop`. A tool error alone can be recovered by the model.
An absent final finish reason still prevents CodeConvoy from independently
distinguishing a token-limit truncation; that limitation predates 2.1.

OpenCode 1 still requires its final `step_finish` with reason `stop`. A malformed
JSON record or a session error overrides a later apparent success. Unknown
records remain visible and cannot establish completion by themselves.

Session continuation flags remain unused. The upstream `--session` probe reused
the captured session ID successfully, but CodeConvoy retries/follow-ups continue
to create ordinary new jobs; no session-resume feature was added.

## Real CLI evidence

Official npm platform binaries **2.1.0, 2.1.2, 2.1.4 and 1.19.4** were downloaded
and verified against their npm SHA-512 integrity. Each version used isolated
XDG data/config/cache/state directories and a disposable repository with spaces
and Unicode in its path. No account login, provider credential, or authentication
file was requested or changed.

The binaries ran against a deterministic local HTTP/SSE provider. This exercises
the actual released CLI, event projection, real file tools, session database,
automatic summary compaction and operating-system processes. The provider's
answers and reported token usage are synthetic; these checks do **not** establish
hosted-provider or authenticated inference compatibility. Earlier real free-model
captures for 1.19.3 and 2.0.32 remain in the existing regression suite.

The [capture manifest](../tests/fixtures/opencode-2.1-audit/captures.json) records
binary provenance, scenario, exit code and expected decoder outcome for **22
unmodified stdout streams**. Independent session exports establish that automatic
summary compaction completed; the CLI streams contain no compaction lifecycle
records. Large reported token usage triggers compaction without a long or costly
conversation. Native provider compaction and provider-owned replay blobs were not
exercised.

| Check | Observed result |
| --- | --- |
| Headless success | 2.1.0, 2.1.2, 2.1.4 and 1.19.4 exit zero and satisfy their existing completion rule. |
| Failed tool execution | A real missing-file read produces `tool_use` with `part.state.status=error`; a subsequent response succeeds. Activity retains the tool failure. |
| Session failure | A local HTTP 401 becomes a structured CLI `error`, exit 1; the adapter fails the job. |
| Retry timing | Local HTTP 429 with insufficient quota and `Retry-After: 2` produces `provider.quota`; 2.1.2 and 2.1.4 retain `retryAfterMs: 2000`, exit 1. |
| Completion and summary compaction | All three tested V2 releases compact and continue successfully. A deliberately invalid summary yields `compaction.failed`, exit 1. V1 success/tool/error/completion regression captures also pass. |
| Session continuation | Direct CLI `--session` runs reuse the prior session ID and return `AUDIT_CONTINUED`. This is an upstream probe, not a new CodeConvoy capability. |
| MCP input | Real local stdio MCP tools request input in 2.1.0 and 2.1.4. Both wait for the fixture's two-second timeout, emit a tool error, then recover with a response and exit zero. |
| Cancellation | Through the real adapter and process executor, 2.1.0 and 2.1.4 return Cancelled/exit 130; no observed live server or shell descendants remain at return. |
| Unexpected/malformed records | Captured success streams combined with synthetic unknown, malformed and truncated records preserve the existing conservative rules. Existing split UTF-8 and 256 KiB limit tests remain. |
| Real help/generated arguments | The installed-CLI ignored probe passes separately with official 2.1.0, 2.1.4 and 1.19.4 binaries. |

The MCP timeout is consistent with
[the run implementation](https://github.com/anomalyco/opencode/blob/v2.1.4/packages/cli/src/run/run.ts):
its `attached` option defaults to true, while global form cancellation in the
formatter is reserved for exclusive non-attached clients. `--standalone` does not
provide an interactive input channel. The 2.1.4 UI fix therefore does not guarantee
that a headless MCP input request will complete promptly. No MCP prompt contract
is consumed by CodeConvoy, and no new parser branch is justified.

## Validation and remaining risks

Four focused tests were added to [tests/opencode.rs](../tests/opencode.rs), using
the captures, help and compaction exports. No adapter rewrite, compatibility
fallback, dependency update, release or version bump was made. The historical
[OpenCode validation record](opencode-validation.md) is preserved.

Formatting, Clippy with all targets/features and warnings denied, the release
build, **120 packaging tests**, and the separate Linux private-D-Bus test pass.
The OpenCode integration target has **22 passed, 1 intentionally ignored**; that
ignored installed-CLI probe also passed separately for the three binaries above.
The final full suite has **810 passed, 1 failed, 14 ignored**. Its sole failure
matches the unmodified baseline failure below. The logs of both runs are not part
of this repository.

Two cleanup findings prevent an unconditional all-green compatibility claim:

1. The unmodified baseline full suite has **806 passed, 1 failed, 14 ignored**.
   The failure is
   `platform_process::detached_groups::a_detached_server_that_outlives_cancellation_keeps_cleanup_unconfirmed`:
   execution returned a successful cancellation/cleanup result where the test
   expected unconfirmed cleanup. It matches the timing-sensitive failure already
   documented by environment setup. It is independent of the event-field changes
   and was left unchanged.
2. A real **OpenCode 1.19.4** cancellation probe twice observed a detached `sleep`
   process after CodeConvoy's group-based cancellation returned. Its process group
   was separate from the CLI's. The disposable sleep later terminated. This
   confirms the existing V1 detached-descendant limitation described by the older
   validation record; the passing fixture cancellation tests do not cover it.
   OpenCode 1 invocation and completion behavior are unchanged, but complete V1
   shell cleanup is not established. A separate process-containment fix needs its
   own scope and validation.

Runtime checks here cover Linux only. macOS, Windows, native provider compaction,
large opaque replay payloads, external plugins, other MCP servers and hosted
providers remain unverified. Source comparison covers 2.1.1/2.1.3; their binaries
were not run. A syntactically valid oversized CLI record still fails at the
existing 256 KiB bound. No speculative parsing of SDK events or `native` data was
added to address these risks.
