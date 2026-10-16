# Claude Code validation

Recorded 2026-10-05 on macOS. Claude is CodeConvoy's fourth and final backend for the v0.1.0 cycle. At that time, authenticated Claude E2E was unverified. `claude` was not found on PATH; no CLI was installed, no login was attempted, and no credential/configuration file was inspected or edited.

## User confirmation — 2026-10-12

The maintainer confirmed Claude Code is working on macOS. The supplied screenshot
shows convoy #8 running the task `read readme.md and summarize it` across five
repositories in Current working tree mode, with both concurrency limits set to 5.
Model and effort use Claude defaults; permissions are Accept edits / file commands.
Claude is marked Available, and Activity shows initialization, a Bash command,
and a Read tool event for a repository README.

This records user-confirmed authenticated execution. The screenshot itself shows
0/5 finished and does not establish terminal success for every job. The installed
CLI version, file-editing behavior, attachments and real-CLI cancellation were not
provided as part of this confirmation. The original fixture and contract evidence
below remains a historical record of the 2026-10-05 checks.

## Auto permission mode — 2026-10-14

Auto is now an explicit third choice, generating `--permission-mode auto`.
`dontAsk` remains the default, and `acceptEdits` retains its existing behavior.
Claude owns the option definitions, execution explanation and output decoding.
The runner passes immutable job options through a default `output_with_options`
factory, so Claude can limit Auto diagnostics to jobs that selected Auto. Other
backends still use their existing `output()` factories; scheduling, supervision
and the Git/worktree lifecycle are unchanged. No dependency or persisted-state
schema changes are required.
Templates still contain only their name and task text.

The installed Claude Code **2.1.300** accepts the generated arguments. Its help
and the current official [CLI reference](https://code.claude.com/docs/en/cli-reference)
and [permission-mode documentation](https://code.claude.com/docs/en/permission-modes)
confirm Auto. Availability is determined by Claude's CLI, account, model and
organization settings. Check CLI only checks the core help contract and version;
it does not certify Auto support or entitlement, and no numeric minimum version
is asserted. Unsupported invocations remain failed, with stderr preserved and
an actionable completion diagnostic. No automatic permission retry is added.

Claude can accept Auto yet report Manual (`permissionMode: default` or `manual`).
The decoder shows reported modes in Activity and retains a Manual/Auto-not-active
notice in the terminal summary for Auto jobs, even when the task succeeds under
Manual permissions. Missing init mode information is explicitly unverified for
Auto jobs. Default, `dontAsk` and `acceptEdits` jobs receive neither Auto fallback
warnings nor Auto availability hints on failure. Regression tests reproduce the
original misleading warning and exercise simultaneous jobs with different saved
permission modes, without changing backend Raw output.
CodeConvoy does not alter Claude policy/settings
or add bypass flags. Reporting depends on the CLI's stream and diagnostics; it
is not independent enforcement or continuous inspection of organization policy.

Authenticated Linux checks used Debian forky/sid, the existing Claude login,
default model/effort and disposable Git repositories, with a four-turn limit:

- Auto reported `permissionMode: auto` and model `claude-opus-5-5`, read a fixture,
  wrote the exact `AUTO_MODE_OK` line, and returned a successful final result.
- A separate repository with `.claude/settings.json` containing
  `{"permissions":{"disableAutoMode":"disable"}}` reported `default` despite
  the Auto argument. A no-tools response succeeded with no stderr. This confirms
  why exit status alone cannot establish Auto availability. The setting is a
  local simulation of the documented restriction, not an organization-admin test.

No credential file was inspected or edited. These checks do not establish native
UI behavior, every account/provider/model combination, organization policy updates
during execution, authenticated cancellation, or macOS/Windows Auto execution.
Existing process and lifecycle fixture tests cover cancellation and supervision.
The optional authenticated regression in `tests/claude.rs` repeats both scenarios
through CodeConvoy's command builder, capture and output decoder; it stays ignored
in ordinary tests because it requires authentication and makes model calls.

The authenticated regression initially exposed an existing decoder incompatibility:
Claude 2.1.300 returned a successful final result, then this notification:

```json
{"type":"system","subtype":"ui_invalidate","event":"ui.render","prompts_screened":true}
```

Previously every record after the result failed completion. The decoder now
accepts only this observed UI-render shape after a result, with optional nonempty
string `session_id`/`uuid` fields. It preserves the record in output. Additional
fields, malformed variants, execution messages, errors and duplicate results
still fail; a notification alone cannot establish success. This narrow exception
uses installed-CLI evidence, not a claim that the private notification is a stable
documented SDK contract. Future shape changes fail conservatively and may require
a compatibility update. The real Auto and disabled-Auto tests both passed after
this correction. The later background-turn correction is recorded below.

Final local validation after the decoder and review fixes:

| Check | Result |
| --- | --- |
| `cargo fmt --check` | Passed |
| `cargo clippy --all-targets --all-features -- -D warnings` | Passed |
| `cargo test --all-features` | 602 passed, 0 failed, 10 opt-in tests ignored (excluding seven nested fixture-test summaries) |
| `cargo build --release` | Passed on Linux |
| `cargo test --all-features --test claude -- --include-ignored --nocapture --test-threads=1` | All 20 passed, including the installed help/version probe and authenticated Auto/disabled-Auto probe |

Deterministic Claude coverage includes exact arguments for all three modes,
unchanged defaults and approval explanations, rejection of unknown/bypass modes,
CLI rejection diagnostics, Manual fallback, missing mode reports, Auto warning
scope across concurrent jobs with different modes, persisted
draft/preferences/history/reuse and unchanged templates, plus the narrowly
accepted trailing notification and rejected malformed/error variants. The full
suite reran existing scheduler, worktree, cancellation and supervision regressions.
No Windows/macOS build or native Auto UI verification was performed in this task.

## Background-task follow-up results — 2026-10-15

The maintainer supplied a raw stream from CodeConvoy **1.0.0
(9f0801ee01af)**, Claude Code **2.1.300**, model **claude-opus-5-5**, Auto mode,
and reported process exit zero. Both result records have the existing success
shape. The incompatibility is the assumption that one invocation emits only one
result, not a changed success-field schema or Auto permission rejection:

1. A `task_started` event identifies a background Bash task and its `run_id`.
2. A successful result with `result_index: 0` ends the initial turn.
3. `task_updated`, `task_notification` and an empty `background_tasks_changed`
   snapshot report completion of that same task.
4. A second `system/init` with the same session precedes more tool activity.
5. A successful result with `result_index: 1` has an `origin` identifying
   `kind: task-notification`, `producer: session-task`, and that task's `runId`.

The first `task_updated` after the initial result sets the old decoder's sticky
protocol-error flag. The second result is parsed, but cannot clear that flag.
The existing UI-render exception does not cover this sequence. The repeated init,
recoverable tool error, failed MCP-server availability entry and rate-limit
metadata are not terminal failures in the supplied trace.

The current official [Python SDK ResultMessage documentation](https://code.claude.com/docs/en/agent-sdk/python#resultmessage)
also describes results for injected background-task turns, their `origin`, and
one result per notification when several notifications are answered together.
The supplied print-mode trace establishes that this occurs with CodeConvoy's
existing invocation too. This is observed with 2.1.300; no claim is made that this
version introduced the behavior. CLI arguments and permission selection do not
change.

`tests/fixtures/claude-auto-background.jsonl` is a reduced, sanitized replay of
that stream. It preserves the event order and correlation fields, both results,
the repeated init, Auto/model/version metadata and a recovered tool failure.
Repository paths, task/session/tool IDs, commands, outputs and prose are replaced;
opaque signatures, account/plugin details and unrelated payloads are omitted.
It contains no executable reproduction of the original task. The regression
failed with the old decoder and passes with the fix, including byte-wise UTF-8
input and an unterminated final JSON line.

Completion now tracks background tasks within the Claude decoder only. A matching
completed notification and same-session re-init authorize a follow-up; its result
must identify the notified run and advance the result index by one. Each result
independently passes the per-turn success checks. Pending tasks/answers and
undrained queued turns prevent final success. Earlier failures, bad correlations,
duplicates, explicit errors, invalid JSON and unconfirmed endings cannot be erased
by a later success. Coalesced notifications require all attributed results in
notification order; an empty
zero-turn placeholder cannot finish the batch. Snapshots cannot clear obligations.
The existing UI-render exception remains allowed.
Scheduler, cancellation, process supervision, Git/worktree handling, other
backends, settings and CLI arguments are unchanged.

Tracked background notifications must report `status: completed`. A `failed`,
`killed`, `stopped` or other non-completed status permanently fails completion
validation, even if Claude later handles it and emits a successful follow-up.
Untracked notifications remain ordinary activity before the first result and
during an authorized answer; while waiting after a result, they fail correlation.

Every result may omit `queued_turn_count` or report a nonnegative integer. A
positive count keeps completion pending until a later result explicitly reports
zero; omitting the field cannot clear an earlier positive count. Counts do not
authorize untracked turns or replace task/session/run/index correlation. An
ordinary single-turn result still needs an absent or zero count to complete;
malformed counts fail permanently, even if a later result reports zero. An
initial result may omit `result_index` for single-turn compatibility, but a
continuation then fails because consecutive indices cannot
be established. The completion diagnostic points to these correlation fields in
Raw output.

Deterministic regressions cover every truncated prefix of the fixture, missing
start/notification/init/result, altered session/task/run/origin/index fields,
failed/interrupted/malformed earlier and final results, nonzero exit, explicit
errors during either turn, unsolicited trailing output, sequential and coalesced
notifications, early notifications, and correlation-memory bounds. Existing
failure, permission, persistence and buffer-limit tests remain enabled.
Review follow-up tests also pin untracked-notification handling at each turn
boundary, sticky failed/killed/stopped notifications before and after the first
result, ordinary-result queued-turn validation, and the missing-index diagnostic.

Compatibility limits: this recognizes the observed `session-task` continuation
shape, not arbitrary injected turns, peer messages or future result origins.
Continuation support requires the observed IDs, explicit background marker,
repeated init and result indices; older/different continuation shapes fail
conservatively. Missing IDs are not guessed. Correlation is bounded to 128 pending
tasks and 256 bytes per identifier retained from `task_started`; later events
remain subject to the existing 1 MiB record limit. Notifications received before
the initial result also require an attributed follow-up; an initial result alone
cannot prove they were answered. These bounds do not change process ownership or
cancellation.
No new authenticated reproduction of the original long-running task or native
Windows/macOS Claude execution is claimed.

Final local verification on Linux after the implementation and error-guard tests:

| Check | Result |
| --- | --- |
| `cargo fmt --check` | Passed |
| `cargo clippy --all-targets --all-features -- -D warnings` | Passed |
| `cargo test --all-features` | 608 passed, 0 failed, 10 opt-in tests ignored (excluding seven nested fixture-test summaries) |
| `cargo build --release` | Passed on Linux |
| `cargo test --all-features --test claude -- --include-ignored --nocapture --test-threads=1` | All 26 passed, including installed Claude 2.1.300 help/version and authenticated Auto/disabled-Auto checks |

The authenticated checks used existing CLI authentication and disposable Git
repositories. Auto wrote the requested fixture file and reported `auto`; the
local disabled-Auto setting reported `default`, with the existing warning retained.
These are fresh ordinary single-turn checks, not an authenticated reproduction
of the background-task sequence. That sequence is verified by the supplied trace
and deterministic replay. No account authentication files were read or edited.

PR #70 review follow-up verification on macOS, 2026-10-16:

| Check | Result |
| --- | --- |
| `cargo fmt --check` | Passed |
| `cargo clippy --all-targets --all-features -- -D warnings` | Passed |
| `cargo test --all-features -- --test-threads=1` | 616 passed, 0 failed, 9 opt-in tests ignored (excluding eight nested fixture-test summaries) |
| `cargo build --release` | Passed |

The sandboxed suite could not create a temporary Unix socket in an existing
storage test; the final suite ran outside that sandbox. A parallel run then hit
the fixture startup timeout in
`cancellation_racing_leader_failure_stops_descendants_but_preserves_a_peer`;
that test passed alone and in the complete serial run. The review follow-up
changes diagnostics, documentation, comments and tests, not completion rules.
Installed/authenticated Claude probes were not rerun. The declared minimum Rust
version is 1.95, which supports the existing `Option::is_none_or` call.

### Queued follow-up correction — 2026-10-16

The next review identified that a successful intermediate result may report
`queued_turn_count > 0` when another notification arrives during an answer. The
[official TypeScript SDK 0.3.243 changelog](https://github.com/anthropics/claude-agent-sdk-typescript/blob/main/CHANGELOG.md#03243)
defines this optional field as the number of queued sends still pending when the
result was produced, indicating another turn and result will follow. The previous
decoder incorrectly made a positive count a permanent protocol failure. The
Claude-local continuation state now retains that pending obligation until an
explicit zero, while every result still passes the independent success and
correlation checks. Malformed fields and earlier failures remain sticky.

A fixture-derived regression adds a second tracked notification during the first
follow-up, then a result with count one and a correlated final result with count
zero. It failed before the fix and passes afterward. Three new tests cover every
truncated prefix, initial queued counts, sequential and coalesced answers,
malformed counts on every result, missing/positive terminal counts, omission
after an earlier positive count, missing task starts/notifications, invalid
session/run/index fields and earlier failed/interrupted results. The new sequence
is synthetic protocol coverage, not an authenticated reproduction. No CLI flags,
permissions, other backends, shared process handling or scheduling changed.

Verification on macOS after the queued-turn correction:

| Check | Result |
| --- | --- |
| `cargo fmt --check` | Passed |
| `cargo clippy --all-targets --all-features -- -D warnings` | Passed |
| `cargo test --all-features -- --test-threads=1` | 619 passed, 0 failed, 9 opt-in tests ignored (excluding eight nested fixture-test summaries) |
| `cargo build --release` | Passed |

The suite used the same serial execution and local Unix socket access described
above. Installed/authenticated Claude probes and native Windows/Linux execution
were not repeated.

## Contract evidence

The implementation was checked against current official sources, not remembered flags:

| Source | Evidence used |
| --- | --- |
| [CLI reference](https://code.claude.com/docs/en/cli-reference) | Print/input/output formats, verbosity, model/effort, permission mode, turn limit, session persistence, and the warning that help omits some supported flags |
| [Programmatic execution](https://code.claude.com/docs/en/headless) | Piped prompt, message streaming and final result; behavior without a permission host; print-mode trust/configuration behavior |
| [Permissions](https://code.claude.com/docs/en/permissions) | `dontAsk` and `acceptEdits`, existing allow rules, filesystem-command approvals and additional directories |
| [Python Agent SDK reference](https://code.claude.com/docs/en/agent-sdk/python) | `ResultMessage` schema, success/error subtypes, `is_error`, `terminal_reason`, deferred tools and assistant errors |
| [Stop reasons](https://platform.claude.com/docs/en/build-with-claude/handling-stop-reasons) | Explicit final token/context truncation and paused turns must not appear complete |
| [Official SDK message parser](https://github.com/anthropics/claude-agent-sdk-python/blob/main/src/claude_agent_sdk/_internal/message_parser.py) | Actual assistant-error and result-field parsing |
| [Official SDK CLI transport](https://github.com/anthropics/claude-agent-sdk-python/blob/main/src/claude_agent_sdk/_internal/transport/subprocess_cli.py) | `_DEFAULT_MAX_BUFFER_SIZE = 1024 * 1024`, used to justify the backend's record bound |
| [TypeScript SDK 0.3.243 changelog](https://github.com/anthropics/claude-agent-sdk-typescript/blob/main/CHANGELOG.md#03243) | A positive `queued_turn_count` means queued sends remain and another turn/result will follow, not that the completed turn failed |

These are rolling documentation/source references inspected on the recorded date, not a claim about an installed Claude release. `tests/fixtures/claude-help-contract.txt` is explicitly synthetic. The shared fixture's version string is also synthetic and only tests display; no Claude semantic-version parser or minimum-version claim is made. Check CLI cannot prove account/model entitlement or validate every hidden flag on an older binary. A rejected invocation fails visibly.

## Implementation choices

The [README contract](../README.md#claude-code-contract) gives the exact command, controls and permission limitations. Claude receives only the task through stdin; no shell interpolates it. Native repository paths are passed as the process working directory. Authentication/provider configuration stays with Claude, except that inherited repository/PWD overrides are removed.

Only model, effort, permission mode and maximum turns are exposed alongside executable selection. No maintained model catalog is needed. Custom tool-rule strings, additional directories and agent/subagent definitions add policy/configuration surface not needed for this release. Bypass remains absent; classifier-based Auto was added on 2026-10-14 as described above. `ultracode` is a composite mode, not exposed as another effort level. No session resume, remote attachment or worktree flags are used.

`--permission-prompts none` is documented only for newer releases. This implementation relies on the documented print-mode behavior without a permission host, plus the explicit permission mode, rather than requiring that newer flag. Existing permission hooks can still decide requests. `--bare` is not used because the backend is intended to inherit the user's established configuration. Print skips trust prompts: repository hooks/MCP servers can run. None of the offered permission modes is an OS sandbox or promises file access is confined to the selected repository.

Claude owns result interpretation. Required success fields and conservative failure rules are documented in [architecture](architecture.md#fourth-backend-review-claude-code). A success subtype alone is insufficient: the official schema permits `is_error=true` even on a success-shaped result. Recovered tool failures remain distinct from session errors. Output retains metadata for inspection, with the parsed final result held locally by the decoder. No normalized Activity model was added.

## Original validation evidence — 2026-10-05

| Area | Result |
| --- | --- |
| Command/options | Exact default and optional arguments, stdin Unicode/metacharacters, working directory, inherited auth/config environment, validation and help/version contracts tested |
| Output | Valid final result, usage/denials, split UTF-8, retries/tools/unknown messages, explicit errors, missing/malformed/interrupted results, duplicate/post-result activity, nonzero/signal exit and large/oversized records tested |
| Lifecycle | Four simultaneous backend convoys, independent Claude snapshots, global/per-convoy limits, canonical lock release after success/failure/cancel, build/spawn failure and zero-exit protocol failure tested |
| Cancellation | Fixture descendants terminated; peer job completes; queued cancellation and baseline-change prevention tested using the shared runner |
| State/UI logic | Old version-1 states, all backend preferences, restart recovery, accepted-launch clearing, immutable history and Claude reuse without launch tested; generic terminal-history cleanup regression retained |
| Native macOS UI | Actual egui App with isolated disposable state and synthetic history: selector, all four backend switches, missing CLI, Claude controls, Tab/Enter navigation, history/settings/reuse, 780×560 and expanded layouts, dark/light/system themes checked |
| Installed Claude / authentication | User-confirmed working on macOS, 2026-10-12; five-repository README summarization convoy (see above) |
| Other platforms | No native Linux/Windows Claude run performed at the original validation; see the later Linux Auto checks above |

The temporary UI launcher and app wrapper were removed after inspection. Its history was visibly labelled synthetic and never represented a Claude execution. Native UI checks used a separate temporary store, not the user's saved CodeConvoy state.

Local checks all passed: `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test --all-features` (**78 passed, 3 ignored installed-CLI probes**), and `cargo build --release`.

## Outstanding validation

Use the [safe two-repository E2E procedure](../README.md#safe-manual-claude-e2e-test) on a machine with Claude already configured. Verify real final-result shape, configured hooks/permissions, independent edits/diffs and cancellation with the installed release. Native Windows executable resolution and Job Object behavior, Linux desktop behavior, and the remaining macOS editing/cancellation behaviors still need real-CLI checks. On Windows use native `claude.exe`; explicit `.cmd`/`.bat` wrappers are rejected. The existing process-group limitation for deliberately detached descendants remains.

The initial backend addition needed no abstraction change. Auto diagnostics now use the small options-aware decoder factory described above. Codex, Copilot and OpenCode implementations remain unchanged; the backend abstraction is feature-frozen for v0.1.0 except for bug fixes.
