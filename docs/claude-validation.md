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
Only Claude's option definitions, execution explanation and output decoding
change; the backend interface, runner, Git/worktree lifecycle and other backends
are unchanged. No dependency or persisted-state schema changes are required.
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
notice in the terminal summary, even when the task succeeds under Manual
permissions. Missing init mode information is explicitly unverified.
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
this correction.

Final local validation after the decoder correction:

| Check | Result |
| --- | --- |
| `cargo fmt --check` | Passed |
| `cargo clippy --all-targets --all-features -- -D warnings` | Passed |
| `cargo test --all-features` | 600 passed, 0 failed, 10 opt-in tests ignored (excluding seven nested fixture-test summaries) |
| `cargo build --release` | Passed on Linux |
| `cargo test --all-features --test claude -- --include-ignored --nocapture --test-threads=1` | All 19 passed, including the installed help/version probe and authenticated Auto/disabled-Auto probe |

Deterministic Claude coverage includes exact arguments for all three modes,
unchanged defaults and approval explanations, rejection of unknown/bypass modes,
CLI rejection diagnostics, Manual fallback, missing mode reports, persisted
draft/preferences/history/reuse and unchanged templates, plus the narrowly
accepted trailing notification and rejected malformed/error variants. The full
suite reran existing scheduler, worktree, cancellation and supervision regressions.
No Windows/macOS build or native Auto UI verification was performed in this task.

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

No backend abstraction change was necessary. Codex, Copilot and OpenCode implementations are unchanged; the backend abstraction is now feature-frozen for v0.1.0 except for bug fixes.
