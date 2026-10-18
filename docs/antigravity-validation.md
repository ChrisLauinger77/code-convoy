# Antigravity CLI feasibility and validation

Assessed on **2026-10-17** from Google's public documentation, the public
`google-antigravity/antigravity-cli` changelog and issue tracker. No Antigravity
CLI was installed on the assessment machine; nothing below was executed,
authenticated or observed locally unless stated.

## Interface decision

Antigravity has three surfaces: the desktop app, the Python SDK and the
terminal CLI `agy`. Only `agy`'s documented headless (print) mode fits
CodeConvoy's execution model: one local child process per repository, shell-free
arguments, stdin/stdout pipes and process-tree cancellation. The desktop app is
not automated. The SDK would be a custom Google agent implementation. Gemini CLI
is not a substitute.

| Requirement | Documented capability | Source |
| --- | --- | --- |
| CLI / automation | `agy -p` / `--print` runs one prompt and exits | [Headless mode](https://antigravity.google/docs/cli/headless/) |
| Prompt transport | Flag argument, or `--input-format stream-json` over stdin (requires `--output-format stream-json`) | Headless mode |
| Structured output | `json` envelope or `stream-json` NDJSON: `init`, `step_update`, one `result` | Headless mode |
| Completion | `result.status` is `SUCCESS`, `ERROR`, `CANCELED`, `INTERRUPTED`, `INVALID`, `WAITING` or `RUNNING`; exit codes 0, 1, 2 and 3 (`AGY_ERROR`, since 1.2.6/1.2.10) | Headless mode, changelog |
| Working directory | Process cwd; `init.cwd` reports the effective workspace; no headless workspace flag documented | Headless mode, [conversations](https://antigravity.google/docs/cli/conversations/) |
| Permissions | Settings rules (`permissions.allow/deny/ask`), soft-deny in headless, `--dangerously-skip-permissions`, `--sandbox`, `--mode` | Headless mode, [permissions](https://antigravity.google/docs/permissions?tab=cli), [modes](https://antigravity.google/docs/cli/modes/) |
| Session continuation | `--continue`, `--conversation <id>`; conversations scoped to cwd | Headless mode, conversations |
| Authentication | Google sign-in (OS keyring), Gemini Enterprise, ADC (`AGY_ADC_AUTH=true`), Gemini API key with `modelProvider=gemini`; headless reuses cached credentials and fails with `authentication required` without a terminal | [Install and auth](https://antigravity.google/docs/install?tab=cli), headless mode |
| Platforms | macOS, Linux and Windows releases (changelog fixes for each); Auto mode is Linux/macOS only | Changelog |

Third-party orchestration of `agy` with account sign-in is described as
supported only by an unofficial forum answer; no official terms statement was
found. Invoking the binary through pipes is the boundary; extracting tokens or
calling backend endpoints is out of scope and never done.

## Minimum version

CodeConvoy requires **agy 1.2.16 or newer**. Check CLI fails closed when
`agy --version` fails or has no `MAJOR.MINOR.PATCH`. The floor is the newest
changelog fix the planned protocol depends on:

| Version | Dependency |
| --- | --- |
| 1.1.1 | Server errors in print mode go to stderr with non-zero exit (issue #76: silent exit 0 with empty stdout) |
| 1.1.3 | Tools needing approval are soft-denied instead of hanging or silently approved |
| 1.1.8 / 1.1.15 | `--output-format stream-json`; `--input-format stream-json` |
| 1.1.27 | Refused actions reported as `denied_actions` |
| 1.2.10 | Streamed runs ending in error exit 3 with the partial response |
| 1.2.16 | Headless runs no longer exit before answering a finished background command |

The version gate is a backend hook (`AgentBackend::check_version`); other
backends keep version output informational.

## Help contract

Google's [spec-driven development codelab](https://codelabs.developers.google.com/sdd-agy-cli)
shows real `agy --help` output from a release older than 1.1.8. It starts with
`Usage of agy:`, lists double-dash flags, mentions no product name and does not
yet show `--output-format`, `--input-format` or `--effort`. Its
`--print-timeout` default of `5m0s` also places it before 1.2.6.

Check CLI therefore identifies `agy` by two flags from that capture that no
other supported CLI advertises, `--print-timeout` and `--prompt-interactive`,
rather than by a product name. It then requires the flags the invocation uses:
`--input-format`, `--output-format`, `--effort`, `--model`,
`--dangerously-skip-permissions` and `--sandbox`. A release like the captured one
is recognized as agy but rejected with an update message. Claude Code and Cursor
help are rejected as not agy.

## Execution contract

Checked on **2026-10-18** against the
[headless guide](https://antigravity.google/docs/cli/headless/), including its
example invocations and recorded `stream-json` output, and the changelog through
**1.4.2**. No `agy` was installed; nothing was executed.

**Transport.** The guide's driver example runs
`agy --input-format stream-json --output-format stream-json` without `-p` and
writes `{"event":"user","message":{"content":…}}` lines to stdin. It states that
closing stdin ends the session after the current turn, with its final `result`
still delivered, and that prompts passed with `-p` are dropped in this mode.
CodeConvoy writes exactly one such line and closes stdin. This keeps the task out
of process arguments and avoids command-line length limits. `--model=`,
`--effort=`, `--dangerously-skip-permissions` and `--sandbox` are added only
when selected.

**Deadline.** CodeConvoy passes no `--print-timeout`. The 1.2.6 changelog
changed the headless default from five minutes to unlimited, and the minimum
supported release is 1.2.16. Since 1.1.28 an explicit expiry returns partial
output and exits successfully, so passing any deadline would cap runs whose
CodeConvoy time limit is blank and could end them with partial output. The
guide's flag table still lists a five-minute default; the more specific
changelog entry is followed. A configured CodeConvoy time limit stops the
process tree itself and fails the job. If an Antigravity deadline did end a
turn early, a step left `ACTIVE` without `DONE` would still fail the job.

**Events.** Records are NDJSON with an `event` name and a payload under the same
key. `init` carries the conversation ID at top level and `cwd`,
`permission_mode` and optional `model` in its payload. `step_update` and
`result` carry the conversation ID in their payload. Steps have `step_index`,
`state` (`ACTIVE`/`DONE`) and `step_type` (`user_input`, `agent_response`,
`tool`, `checkpoint` observed). Results have `status`, `response`, `num_turns`,
optional `error` and `usage`.

**Classification** (`agents/antigravity/output.rs`):

| Evidence | Outcome |
| --- | --- |
| Authentication text on stderr or in a result error | Failed, sign-in guidance |
| Any result whose status is not `SUCCESS` | Failed with the reported status and error |
| `AGY_ERROR` line on stderr | Failed |
| Non-zero exit | Failed, even after a `SUCCESS` result |
| Invalid/unknown/oversized record, missing payload, changed conversation ID, duplicate init, `cwd` not the assigned checkout | Failed |
| Result `error` that is present but not a string, or a non-empty error on a `SUCCESS` result | Failed |
| `SUCCESS` result while any step (tool, streamed response or other type) is still `ACTIVE` without its `DONE`, or more than 1,024 unfinished steps | Failed |
| No init, no result, or step activity after the last result | Failed (incomplete) |
| `denied_actions` present | Succeeded; the detail and Activity name the refused count |
| `tool_info.error` | Shown in Activity; not fatal (Antigravity treats benign tool errors as non-fatal since 1.1.20) |

Streaming sessions can report a turn `ERROR` and still exit 0 (1.2.10 note), so
the result status, not the exit code, decides failure. One stdin turn should
produce one result. Additional results are accepted only if each one is
`SUCCESS` for the same conversation and the stream ends on a result. This avoids
the 1.0 Claude Code misclassification of background follow-up results while an
earlier failure can never be erased by a later success.

**Windows sandbox.** The 1.4.1 changelog says Antigravity turns the terminal
sandbox off and asks before commands on Windows 10 and Windows 11 before 24H2;
earlier releases fail to start sandboxed commands there. An explicit **Enable for
this job** is therefore rejected at review unless the registry's
`CurrentBuildNumber` is 26100 or newer, or when it cannot be read.

Not used: `-p`, `--print-timeout`, `--continue`, `--conversation`, `--agent`,
`--json-schema`, `--mode`, Antigravity's own worktrees, `/fork` and remote control. Tasks starting
with a CLI-handled slash command end with an `ERROR` result in this transport.

## Fixtures

`tests/fixtures/antigravity-success.jsonl` reproduces the guide's recorded
single-turn stream with sanitized IDs, text and `cwd`.
`antigravity-activity.jsonl`, `antigravity-error.jsonl` and
`antigravity-interrupted.jsonl` are synthetic variants of the documented event
shapes. The `denied_actions` array of strings is an assumed shape; only its
presence and length are interpreted. `tests/support/antigravity.rs` emits the
same shapes from the test executable for process-level scenarios: success,
authentication failure, `ERROR` with exit 3 and with exit 0, interruption,
malformed and incomplete output, refused actions, edits and a different `cwd`.

## Unverified

- `agy --help` text from an installed 1.2.16+ release.
  `tests/fixtures/antigravity-help-contract.txt` follows the layout of Google's
  published capture with the flags documented since 1.1.8; see
  [Help contract](#help-contract).
- `agy --version` output format.
- Installation locations; discovery uses the shared generic directories only.
- The real `stream-json` stream from an installed release, `denied_actions`
  shape, stderr wording of soft-deny and authentication notices, and whether
  the guide's five-minute or the changelog's unlimited default applies to
  stdin stream-json sessions.
- Whether `init.cwd` reports a linked worktree path or its main repository.
- Concurrent headless instances sharing one sign-in and conversation store.
- Process-tree cancellation leaving no daemon (fixed in 1.2.9 according to the
  changelog).
- Authenticated execution, model access and permission enforcement on every
  platform.
