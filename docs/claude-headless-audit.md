# Claude Code headless permission audit

Checked 2026-10-26 on Linux. Official releases **2.1.304–2.1.310** were
reviewed; the official release feed had no later release at the time of this
audit. Real execution used a temporary, manifest-verified **2.1.310** native
binary. The installed **2.1.303** CLI was left in place. This is not an execution
matrix for every intervening release or platform.

## Verified upstream changes

| Release | Relevant official changes | CodeConvoy impact |
| --- | --- | --- |
| [2.1.304](https://github.com/anthropics/claude-code/releases/tag/v2.1.304), Oct 19 | Mod permission decisions can require a person and refuse when none is available. Fixes cover Linux sandbox deny-read globs, plugins extending headless execution past turn/input limits, blocked reads ignoring interruption, and hook deny reasons being obscured by callback timeouts. Claude's own worktree cleanup and large-session resume also received fixes. | Inherited mods/hooks can refuse operations without an interactive host. Preserve diagnostics and fail on reported permission denials. CodeConvoy does not use Claude's worktree or resume flags. |
| [2.1.305](https://github.com/anthropics/claude-code/releases/tag/v2.1.305), Oct 20 | Adds optional Linux nested user namespaces, explicitly weakening sandbox file protection. Changes managed MCP allowlist treatment of variable-based environments; updates MCP replacement and consent behavior. Fixes the 2.1.304 tool-search regression on some providers/gateways. | These are upstream configuration/enforcement changes. CodeConvoy neither enables nested namespaces nor rewrites MCP policy. Existing user/managed configuration still determines exposure. |
| [2.1.306](https://github.com/anthropics/claude-code/releases/tag/v2.1.306), Oct 21 | Fixes print-mode cloud-session authentication without session environment variables and misleading prompt-length errors in MCP-heavy sessions. | The cloud-specific authentication fix does not describe this local adapter. Retain actual CLI authentication diagnostics. |
| [2.1.307](https://github.com/anthropics/claude-code/releases/tag/v2.1.307), Oct 21 | Locked macOS login keychains now produce accurate MCP authentication diagnostics and reconnect after unlock without another sign-in. Sleep during MCP connection no longer produces persistent false timeouts. HTTP MCP retries are limited to read-only/idempotent tools. Adds safety telemetry and fixes a gateway first-byte timeout setting. | Do not replace these failures with login flows or automatic retries. macOS keychain and suspend/resume behavior require native validation. CodeConvoy consumes stdout/stderr, not OpenTelemetry. |
| [2.1.308](https://github.com/anthropics/claude-code/releases/tag/v2.1.308), Oct 22 | General bug fixes/reliability, without specific published details. | No additional behavior inferred. |
| [2.1.309](https://github.com/anthropics/claude-code/releases/tag/v2.1.309), Oct 22 | Fixes shell startup outside the sandbox, symlinked macOS write denies, MCP-config protection, disabled MCP reconnection in print/SDK sessions, compound shell permission checks, worktree guards, and Linux deny protection after background work outlives a session. Unavailable-sandbox fallbacks become more restrictive. | Protection belongs to the installed CLI. Existing CodeConvoy flags cannot repair older upstream enforcement. Claude-managed worktree fixes do not replace CodeConvoy's ownership/lifecycle safeguards. |
| [2.1.310](https://github.com/anthropics/claude-code/releases/tag/v2.1.310), Oct 23 | Changes unanswered noninteractive MCP elicitation timeout errors to tell the model that no answer arrived and to avoid repeating the call. Improves Linux sandbox startup diagnostics, Windows permission-hook enforcement, and noninteractive compaction memory use. | An MCP tool error can still precede a successful terminal conversation result. Preserve and classify explicit timeout/confirmation errors separately. |

## Invocation and policy

The production invocation remains:

```text
claude --print --input-format text --output-format stream-json --verbose \
  --no-session-persistence --permission-mode dontAsk
```

Text travels through stdin, which closes after the prompt. Images select the
existing stream-json input transport. Optional model, effort and maximum turns
remain backend-specific. The working directory is the assigned checkout; Git
environment overrides and `PWD` are removed. Authentication, settings, hooks,
plugins and MCP configuration otherwise remain Claude's responsibility.

`dontAsk` remains the default. `acceptEdits` and explicit `auto` remain the other
choices, with the existing Auto-to-Manual diagnostics. There is no permission
callback or interactive host, no bypass flag and no sandbox override. No new
application configuration, dependency or shared backend behavior was added.

These [permission modes](https://code.claude.com/docs/en/permissions#permission-modes)
are not OS confinement. A Git worktree also is not a sandbox. Claude's
[Bash sandbox](https://code.claude.com/docs/en/sandboxing) follows inherited
settings and does not wrap built-in file tools, hooks or MCP server processes.
[Print mode](https://code.claude.com/docs/en/headless) skips workspace/server
trust dialogs and loads project configuration. Only trusted registered
repositories should be run. No policy is relaxed to make a test or job pass.

## Confirmed defects and minimal fixes

Two real 2.1.310 streams demonstrated false success:

1. A denied Bash call and a human-only MCP call each exited zero and produced a
   `success` result with `is_error=false`. The old decoder only warned about
   `permission_denials`. Nonempty lists now prevent success; malformed lists
   fail validation. Explicit rejected tool permission metadata and the
   upstream `system/permission_denied` event also latch failure, so a later
   clean result cannot erase the denial.
2. A local MCP server requested a form. The headless client replied `cancel`,
   and the server deliberately kept the call pending until its one-second
   timeout. Claude returned a success result with an empty denial list. The
   decoder now latches the explicit MCP timeout diagnostic reported in both
   the errored tool-result block and its CLI `tool_use_result` field. The
   2.1.310 unanswered-question wording is recognized in the same error wrapper.

The timeout match uses backend error fields, not assistant prose. Ordinary
recoverable tool errors remain recoverable. A recognized timeout is conservative:
even a later successful retry does not erase it. Activity and terminal detail
explain the failure; Raw output remains unchanged. CodeConvoy makes no automatic
retry, policy change or inference that a timed-out remote operation had no effects.

Existing completion checks still require exit zero and valid successful terminal
results for every correlated turn. Explicit assistant/session errors, API errors,
interruption, turn limits, truncation, malformed streams, outstanding background
work and queued turns still prevent success. See the official
[result schema](https://code.claude.com/docs/en/agent-sdk/python#resultmessage).

## Tests and evidence

The opt-in audit uses the existing builder, decoder and process supervision in
detached worktrees of disposable repositories. Its MCP fixture is local and
has no external effects. A test-only exact-tool allow enables elicitation;
the human-only annotation still prevents the confirmation tool from executing.
The probes operated only on disposable fixture repositories and did not inspect
or edit authentication files.

| Case | Real 2.1.310 result after the fixes |
| --- | --- |
| Normal no-tools headless response | Exit 0; Succeeded |
| Denied Bash operation | Exit 0, one denial; Failed |
| MCP tool requiring a person | Exit 0, one denial; Failed; server tool never called |
| MCP elicitation followed by timeout | Exit 0, no denials; Failed |
| Missing authentication with empty temporary configuration | Exit 1, assistant `authentication_failed`, success subtype with `is_error=true`/`api_error`; Failed |
| Cancellation while MCP call awaits confirmation | Cancellation confirmed; no terminal result; local MCP server's OS lock released before supervision returned |
| Session continuation | Existing deterministic background-turn, reused-task/new-run, correlation and truncation regressions pass; no persisted-session resume feature is introduced |
| Terminal events and exits | Sanitized captures replay under byte-wise/chunked input; exit zero cannot override denial/timeout/error/incomplete output, and nonzero exits cannot succeed |

[Sanitized captures and reproduction instructions](../tests/fixtures/claude-headless-2.1.310/README.md)
are checked in. Decoder regressions reproduce the denial failure before the fix.
The manager regression also verifies that zero-exit denials release capacity and
the repository lease, allowing the next job to run. Existing platform process,
convoy and isolated-worktree tests retain cancellation and peer-job coverage.

| Required validation | Result |
| --- | --- |
| `cargo fmt --check` | Passed |
| `cargo clippy --all-targets --all-features -- -D warnings` | Passed |
| `cargo test --all-features -- --test-threads=1` | Passed: 817 tests, 15 optional tests ignored, excluding filtered subprocess reruns |
| `cargo build --release` | Passed on Linux |
| Real 2.1.310 headless audit (opt-in) | Passed all six cases |
| Installed 2.1.303 generated-argument help/version probe (opt-in) | Passed |

The complete suite used a dedicated `/var/tmp` temporary root. Initial runs using
`/tmp` failed existing worktree/storage safety tests because this session exposes
`/tmp/.git`, including during escalated execution. The application checks were
preserved. Final decoder/Activity checks were repeated after the small diagnostic
matcher refactor: 46 passed, three opt-in probes ignored.

## Remaining limits

- macOS locked-keychain behavior, native Windows enforcement, OS suspend during
  MCP startup and provider-specific gateway fixes were verified in official
  notes, not reproduced on this Linux host. An unavailable optional MCP server
  in initialization does not by itself fail an otherwise completed task;
  inspect its reported status and Raw diagnostics. Unlocking/authenticating
  remains outside CodeConvoy.
- The live form request received `cancel`, so the new unanswered-question error
  itself was not emitted in that probe. Its wording and the separate permission
  event schema were checked in the manifest-verified official binary, with
  synthetic regression coverage using the observed error transport shape.
- Optional/undocumented error metadata and English MCP diagnostic wording can
  change upstream. Unknown wording or omitted signals cannot establish a
  permission failure independently; terminal success still does not prove task
  fulfillment. The audit does not certify all plugins, hooks, policies or CLI
  versions.
- CodeConvoy neither continues saved sessions nor resumes agents after restart.
  Its continuation decoder validates background follow-ups within one supervised
  invocation. Cancellation uses the existing process group/Job Object and
  precedes result interpretation; existing detached-process platform limitations
  remain. Claude has no application wall-clock deadline here; maximum turns is
  not a timeout. No timeout configuration was added.
