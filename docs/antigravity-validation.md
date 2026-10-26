# Antigravity CLI feasibility and validation

Assessed on **2026-10-17** from Google's public documentation, the public
`google-antigravity/antigravity-cli` changelog and issue tracker. On
**2026-10-18** an isolated **agy 1.4.2** was installed and checked without sign-in
on macOS (see [Installed CLI check](#installed-cli-check-agy-142)). Nothing was
authenticated; everything else below is documentation- or fixture-based unless
stated.

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
| Platforms | macOS, Linux and Windows releases (changelog fixes for each); Auto mode is Linux/macOS only until 1.4.4 adds Windows 11 24H2 for personal accounts | Changelog |

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
rather than by a product name. The installed 1.4.2 confirmed this layout and
showed that `--help`, `-h` and `help` write the help text **only to stderr**;
`AgentBackend::help_stream()` lets this backend select stderr, while every other
backend keeps reading stdout. It then requires the flags the invocation uses:
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
Google's sandbox guide adds that from 1.4.4 `--sandbox` has no effect on Linux
hosts and containers that block unprivileged user namespaces. CodeConvoy does not
gate that case; the setting help and review summary state it. See the
[1.4.3–1.4.7 audit](antigravity-1.4-audit.md).

Not used: `-p`, `--print-timeout`, `--continue`, `--conversation`, `--agent`,
`--json-schema`, `--mode`, Antigravity's own worktrees, `/fork` and remote control. Tasks starting
with a CLI-handled slash command end with an `ERROR` result in this transport.

## Installed CLI check (agy 1.4.2)

Performed on **2026-10-18**, macOS on Apple Silicon. No sign-in, no model call,
nothing written outside `/tmp/agy-sandbox`.

**Installation.** Google's `https://antigravity.google/cli/install.sh` was
downloaded and read, not executed. It queries
`https://antigravity-cli-auto-updater-974169037036.us-central1.run.app/manifests/<platform>.json`,
downloads the listed archive, verifies its SHA-512, copies `antigravity` to
`~/.local/bin/agy` and runs `agy install` to edit the shell profile. Those steps
were repeated by hand: manifest version `1.4.2`, archive
`storage.googleapis.com/antigravity-public/antigravity-cli/1.4.2-4835981233356800/darwin-arm/cli_mac_arm64.tar.gz`
(56,104,322 bytes) with matching SHA-512, binary code-signed by Apple team
`EQHXZ8M8AV`. `agy install` was skipped. Every run used a temporary `HOME`/XDG
directory, an empty working directory, `PATH=/usr/bin:/bin` and a hard timeout.

| Check | Observation | CodeConvoy consequence |
| --- | --- | --- |
| `agy --version` | Exit 0, stdout `1.4.2` | Version gate parses it (test) |
| `agy --help`, `-h`, `help` | Exit 0, **stdout empty**, help on stderr; `Usage of agy:`; identity and required flags present | Fixed: stderr selected via `help_stream()`; contract fixture uses the 1.4.2 flag set |
| Side effects of help/version | No files written, no processes started | Check CLI is side-effect free |
| `--print-timeout` | Help: default `0s`, waits until the turn completes | Confirms passing no deadline |
| Extra 1.4.2 flags | `--effort` `xhigh`/`max`, `--mode`, `--disable-slash-commands`, `--remote-control`, `--json-schema`, `--agent` | Not used; no setting added |
| CodeConvoy invocation, not signed in | Exit 1 after about 1 s; stdout one `result` (`ERROR`, empty conversation ID, `authentication failed or timed out`) **without `init`**; stderr `authentication required` | Classified as authentication failure (captured fixtures) |
| `-p /usage` (no agent turn), not signed in | Prints an OAuth URL, waits up to 60 s for a code on stdin; with empty stdin ends with exit 1 | Not used by CodeConvoy; the stdin transport fails fast instead |
| Files written | Only below `~/.gemini/antigravity-cli` and `~/.gemini/config`; none in the working directory | No repository side effects observed |
| Auto-updater | First run (then at most every 15 minutes) starts `agy --bg-updater …` in its own session/process group, reparented to PID 1; exits on its own within seconds; the caller's pipeline finished after 0.5 s while it still ran | Outside CodeConvoy ownership; not stopped mid-update and does not delay completion on macOS |

The installed release passes the opt-in test
`installed_agy_passes_check_cli` (`CODECONVOY_AGY=/path/to/agy cargo test
--all-features --test antigravity installed_agy_passes_check_cli -- --ignored`).

## Argument syntax and end-to-end jobs

Checked on **2026-10-18** with the same isolated, never signed-in agy 1.4.2.

Every argument form CodeConvoy produces passed agy's flag parser and stopped at
the authentication gate (exit 1, the `ERROR` result above):

| Arguments after `--input-format stream-json --output-format stream-json` | Result |
| --- | --- |
| none | Authentication gate |
| `--model=gemini-3.8-flash-medium --effort=high` | Authentication gate |
| `--dangerously-skip-permissions --sandbox` | Authentication gate |
| all of the above | Authentication gate |
| `--model=--literal;$(no shell)` (value starting with `--`, shell syntax as data) | Authentication gate |
| `--effort=extreme` (invalid value) | Authentication gate; values are checked only after sign-in, CodeConvoy offers fixed choices |
| `--definitely-not-a-flag` | Exit 2, `flags provided but not defined`, usage on stderr |

The opt-in test `installed_agy_job_without_sign_in_fails_with_sign_in_guidance`
then ran **8 real jobs** through CodeConvoy's `runner::prepare` (real Check CLI)
and `RunManager`: Direct and Isolated mode, each with default settings, model and
effort, Approve all tool requests, and terminal sandbox. Every job finished
Failed with exit code 1 and the sign-in guidance, Activity showed the reported
`ERROR` result, and the committed checkout stayed clean. Command (run with an
isolated `HOME`, keeping the real Cargo home):

```sh
CARGO_HOME="$HOME/.cargo" HOME=/tmp/agy-sandbox/home5 CODECONVOY_AGY=/tmp/agy-sandbox/bin/agy CODECONVOY_AGY_SIGNED_OUT=1 cargo test --all-features --test antigravity installed_agy_job_without_sign_in -- --ignored
```

`authenticated_agy_completes_a_disposable_task` (gated by
`CODECONVOY_AGY_AUTHENTICATED=1`) creates one file through a real model call; it
was not run. Google's Windows installer (`install.ps1`, read only) places a
native `agy.exe` in `%LOCALAPPDATA%\agy\bin`, which discovery now searches for
this backend.

## Fixtures

`antigravity-1.4.2-authentication.stdout.jsonl` and
`antigravity-1.4.2-authentication.stderr.txt` are the unmodified stdout and stderr
of CodeConvoy's invocation of the installed 1.4.2 without sign-in.
`antigravity-help-contract.txt` keeps the 1.4.2 help layout and flag set with
placeholder descriptions.

`tests/fixtures/antigravity-success.jsonl` reproduces the guide's recorded
single-turn stream with sanitized IDs, text and `cwd`.
`antigravity-activity.jsonl`, `antigravity-error.jsonl` and
`antigravity-interrupted.jsonl` are synthetic variants of the documented event
shapes. The `denied_actions` array of strings is an assumed shape; only its
presence and length are interpreted. `tests/support/antigravity.rs` emits the
same shapes from the test executable for process-level scenarios: success,
authentication failure, `ERROR` with exit 3 and with exit 0, interruption,
malformed and incomplete output, refused actions, edits and a different `cwd`.

## Lifecycle and safety

Checked on **2026-10-18** with the test executable's `agy` mode through the real
`RunManager`, leases, worktrees and process supervision
(`tests/antigravity/lifecycle.rs`). No Antigravity-specific process handling was
added; the shared infrastructure covered every scenario.

| Scenario | Verified behavior |
| --- | --- |
| Success, `ERROR` (exit 0), authentication failure, malformed, incomplete, `INTERRUPTED`, unfinished step, `AGY_ERROR`; Direct and Isolated | One start per job, the reported diagnostic, isolated result retained at the fixed base with the source unchanged, default flags never widened, slot and lease released for the next job |
| Stop during a tool process | Tool descendant stopped before a nested repository is admitted; unrelated peer keeps running |
| Two isolated jobs on one repository | Separate checkouts reported as `cwd`, independent edits; Direct access waits for both |
| Quit | Both Antigravity process trees cancelled, isolated evidence retained |
| Time limit | Failed after confirmed cleanup of the tool descendant; repository released; edits retained |
| Background process left by a successful run | Stopped before the job finishes, in Direct and Isolated modes |
| Retry of a failed job | Original `configured` permissions and sandbox reused; no widening flags |

Changelog facts relevant to cleanup: 1.2.9 made headless runs terminate daemon
background processes when the run ends; 1.1.28 and 1.2.6 describe daemon
background tasks being left running in earlier releases. CodeConvoy stops any
such process that remains in the owned process group or Windows Job Object
after `agy` exits. A process that starts a new Unix session or group escapes
that cleanup (general limitation in [architecture](architecture.md)).

These guarantees hold once process cleanup is confirmed. If supervision or
termination fails, or a descendant may still be alive, the shared manager still
publishes the terminal event but keeps the repository lease and execution slot
reserved, pins the run's history and tells the user to inspect remaining
processes. Runner edge cases that do not depend on the backend (event
backpressure, delayed or refused cancellation, unconfirmed cleanup) remain
covered by the Cursor and core suites rather than duplicated here.

## Session continuation and UX

Decided on **2026-10-18**. `agy` documents `--continue` and `--conversation <id>`,
scoped to the process directory. CodeConvoy has no continuation architecture for
any backend: Retry and follow-ups create fresh convoys, Codex runs ephemeral
sessions and Claude Code disables session persistence. Integrating Antigravity
resumption would be a new cross-backend feature and would reuse Antigravity's
directory-scoped state in retained worktrees, so it is not implemented.

The installed 1.4.2 created `conversations/`, `conversation_summaries.db` and
`brain/` below `~/.gemini/antigravity-cli` on its first run, and its help lists no
flag that disables history. Every Antigravity job therefore leaves local
conversation history that CodeConvoy neither stores nor removes.

Failure details state the recovery action before any colon, because the UI
summary shows the text before the first colon: recognized advice precedes
Antigravity's error text, which may contain colons (the documented unknown-model
error does), and every failure path ends with the exit code, or on Unix the
terminating signal number and name, and the known conversation ID. Advice is added only for causes
named in Antigravity's own text: sign-in (`authentication required`/`failed`/
`expired`), model selection (`invalid model selection`, the documented error
for an unknown `--model`), quota or credits (`quota`, `RESOURCE_EXHAUSTED`,
`credits balance`) and CLI-answered slash commands (the documented stream-json
error). Tests pin each case, the absence of invented advice and the summary.

## Unverified

- Authenticated execution: the real successful `stream-json` stream, model
  access, `denied_actions` shape, soft-deny stderr wording and permission
  enforcement, on every platform.
- Whether `init.cwd` reports a linked worktree path or its main repository.
- Windows behavior of the installed CLI, including whether the Windows
  auto-updater leaves CodeConvoy's Job Object and whether stopping it mid-update
  is safe. On Linux, agy 1.4.7 repeated the signed-out checks above; see the
  [1.4.3–1.4.7 audit](antigravity-1.4-audit.md#real-cli-evidence-agy-147-linux).
- Whether Remote Control or the Antigravity Hub start inside a CodeConvoy job.
- Concurrent headless `agy` processes sharing one sign-in, keyring and
  conversation store.
- Antigravity subagent worktrees (`.system_generated/worktrees`) and their effect
  on a repository's Git worktree list.
- The Windows install location in practice; discovery follows `install.ps1`
  (`%LOCALAPPDATA%\agy\bin`) without a Windows run.
