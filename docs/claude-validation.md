# Claude Code validation

Recorded 2026-10-05 on macOS. Claude is CodeConvoy's fourth and final backend for the v0.1.0 cycle. **Authenticated Claude E2E is unverified.** `claude` was not found on PATH; no CLI was installed, no login was attempted, and no credential/configuration file was inspected or edited.

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

Only model, effort, permission mode and maximum turns are exposed alongside executable selection. No maintained model catalog is needed. Custom tool-rule strings, additional directories and agent/subagent definitions add policy/configuration surface not needed for this release. Bypass and classifier-based auto modes are deliberately absent. `ultracode` is a composite mode, not exposed as another effort level. No session resume, remote attachment or worktree flags are used.

`--permission-prompts none` is documented only for newer releases. This implementation relies on the documented print-mode behavior without a permission host, plus the explicit permission mode, rather than requiring that newer flag. Existing permission hooks can still decide requests. `--bare` is not used because the backend is intended to inherit the user's established configuration. Print skips trust prompts: repository hooks/MCP servers can run. Neither offered permission mode is an OS sandbox, and neither promises file access is confined to the selected repository.

Claude owns result interpretation. Required success fields and conservative failure rules are documented in [architecture](architecture.md#fourth-backend-review-claude-code). A success subtype alone is insufficient: the official schema permits `is_error=true` even on a success-shaped result. Recovered tool failures remain distinct from session errors. Output retains metadata for inspection, with the parsed final result held locally by the decoder. No normalized Activity model was added.

## Validation evidence

| Area | Result |
| --- | --- |
| Command/options | Exact default and optional arguments, stdin Unicode/metacharacters, working directory, inherited auth/config environment, validation and help/version contracts tested |
| Output | Valid final result, usage/denials, split UTF-8, retries/tools/unknown messages, explicit errors, missing/malformed/interrupted results, duplicate/post-result activity, nonzero/signal exit and large/oversized records tested |
| Lifecycle | Four simultaneous backend convoys, independent Claude snapshots, global/per-convoy limits, canonical lock release after success/failure/cancel, build/spawn failure and zero-exit protocol failure tested |
| Cancellation | Fixture descendants terminated; peer job completes; queued cancellation and baseline-change prevention tested using the shared runner |
| State/UI logic | Old version-1 states, all backend preferences, restart recovery, accepted-launch clearing, immutable history and Claude reuse without launch tested; generic terminal-history cleanup regression retained |
| Native macOS UI | Actual egui App with isolated disposable state and synthetic history: selector, all four backend switches, missing CLI, Claude controls, Tab/Enter navigation, history/settings/reuse, 780×560 and expanded layouts, dark/light/system themes checked |
| Installed Claude / authentication | Unavailable / unverified; help-only test intentionally ignored |
| Other platforms | No native Linux/Windows Claude run performed; CI must validate platform builds/tests |

The temporary UI launcher and app wrapper were removed after inspection. Its history was visibly labelled synthetic and never represented a Claude execution. Native UI checks used a separate temporary store, not the user's saved CodeConvoy state.

Local checks all passed: `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test --all-features` (**78 passed, 3 ignored installed-CLI probes**), and `cargo build --release`.

## Outstanding validation

Use the [safe two-repository E2E procedure](../README.md#safe-manual-claude-e2e-test) on a machine with Claude already configured. Verify real final-result shape, configured hooks/permissions, independent edits/diffs and cancellation with the installed release. Native Windows executable resolution and Job Object behavior, Linux desktop behavior, and authenticated macOS behavior still need real-CLI checks. On Windows use native `claude.exe`; explicit `.cmd`/`.bat` wrappers are rejected. The existing process-group limitation for deliberately detached descendants remains.

No backend abstraction change was necessary. Codex, Copilot and OpenCode implementations are unchanged; the backend abstraction is now feature-frozen for v0.1.0 except for bug fixes.
