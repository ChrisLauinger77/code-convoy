# Architecture

CodeConvoy uses one egui UI thread and a small Tokio runtime. The unit of work is a task configuration applied to a set of existing repository roots. A run is a saved snapshot of that configuration plus one job per repository.

Backend status: Codex and Copilot are implemented and user-verified end-to-end on Linux with two concurrent real repositories each. OpenCode is the third supported backend, with source/fixture checks and macOS UI validation; authenticated E2E is unverified. Claude Code is the fourth supported backend, validated against official documentation/SDK source, fixtures and the native macOS UI; authenticated E2E is unverified.

## Modules

| Module | Responsibility |
| --- | --- |
| `domain` | Serializable tasks, repositories, runs, job states, validation, bounded in-memory logs |
| `agents` | Backend registry, backend-owned option descriptions, `AgentBackend` / per-job `AgentOutput` contracts |
| `agents/codex` | Codex flags, capability checks, existing JSONL formatting/completion criteria |
| `agents/copilot` | Copilot flags, bundled-version checks, raw text streaming, exit-status interpretation |
| `agents/opencode` | OpenCode run/help/version contract, model/agent/variant/permission options, native JSON event decoding and conservative completion |
| `agents/claude` | Claude print/help/version contract, model/effort/turn/permission options, stream-json decoding and final-result validation |
| `process` | Shell-free process spawning, pipes, cancellation tokens, process-tree ownership, short-command timeouts |
| `git` | Root validation, working-tree inspection, staged/unstaged diff and untracked status |
| `runner` | Preflight, immutable snapshots, per-job Git recheck and execution events |
| `runner/manager` | Application-owned multi-convoy lifecycle, worker ownership, cancellation, shutdown |
| `runner/schedule` | Pure round-robin admission policy, both concurrency limits, canonical path leases |
| `persistence` | Platform data directory, single-instance lock, atomic JSON replacement and recovery |
| `ui` | Task/options/repository editor, preflight review, history/results/output/diff views |

## Presentation

`ui/editor` groups task, backend options, repositories, and execution controls. `ui/results` presents history, job selection, and result tabs. `ui/theme` centralizes both palettes, typography, spacing, focus/selection, and primary actions. `ui/format` formats duration and run summaries; overall status and progress are derived in the domain.

The compact run selector groups active and terminal runs in a height-bounded scrolling popup. Active means any queued/running job, including a partially failed convoy that still has work. History cleanup methods in `AppState` guard against removing active runs; the UI offers individual removal only for terminal runs and bulk removal through History cleanup. Removal immediately saves metadata and reconciles selection (prefer an active convoy, then the newest history, then empty), clearing stale job/diff/output views. It never calls Git, the scheduler, cancellation, or agent configuration APIs.

Reuse copies a run's task/options and selects its still-registered canonical repository paths. It reports skipped registrations, leaves the global limit alone, and requires the normal preflight for any subsequent launch. Reuse is unavailable during preflight/review/shutdown. No historical configuration is mutated or automatically executed.

`ui/text_view` caches a line index for the selected output/diff and uses egui's visible-row layout. Copy actions preserve the original retained text. Diff coloring is presentation-only: no agent protocol parsing, Git behavior, or stored output format is changed. Main panes and long details scroll independently; execution controls remain visible at the bottom of the editor. Appearance follows the system by default; overrides are session-local, without a state-schema change.

## Backend extension

`AgentBackend` owns executable/help/version detection, supported options, invocation construction, and the execution summary shown in the editor/preflight UI. Its `output()` factory creates an independent `AgentOutput` for each job; that object owns byte framing, stream handling, and exit interpretation. It exposes overridable spawn/cancel methods and delegates ordinary subprocess handling to `process`. Async pipe I/O and scheduling remain shared, rather than being copied into every backend.

`OptionSpec` provides rendering metadata (text fields or backend-specific choice values). Persisted option keys belong to the selected backend. There is no cross-agent reasoning, model, or permission translation. Backends validate keys and values before invocation. The generic renderer only renders that backend's descriptions. Adding Copilot exposed two concrete Codex assumptions: shared line-based output with `turn.completed` observation fields, and hardcoded Codex approval/sandbox UI text. Both now live in the Codex backend. The runner only sends bytes and an exit status to an opaque per-job output handler. This permits Copilot to stream partial text immediately and interpret its own exit status, without inventing a common event schema. Codex's formatter and success criteria are unchanged. No plugin system or hypothetical capability layer was added.

With four real backends implemented, `AgentBackend` / `AgentOutput` are feature-frozen for the v0.1.0 stabilization cycle unless a bug requires changes. No additional backend or speculative capability layer is in scope.

Codex uses stdin for the prompt to avoid shell injection and command-line length limits. It sets `--no-daemon`, explicit sandbox permissions, and `--ask-for-approval never`; no interactive approval UI exists. Authentication remains entirely with the CLI. Session resume is intentionally absent while ephemeral execution is enabled. A successful exit must also have a `turn.completed` event and no `turn.failed` event. Unknown JSONL events remain visible for diagnostics.

Copilot uses its documented stdin pipe mode for a noninteractive prompt and keeps stdout as text. It uses `--no-ask-user`, explicit streaming/no-color/plain-diff, and disables remote control/export. No Codex flags or result events are applied to it. Its success criterion is process exit zero; reviewing the agent response is still necessary when tools were denied or a task was not fulfilled.

Detection and execution both use `--no-auto-update`. On the development machine that selects bundled 1.0.65, whereas launching without that flag reports cached 1.0.91. Compatibility is checked against the flags required by the backend, with useful version reporting and missing-executable errors. No detection runs at app startup. `Check CLI` results are associated with the agent and option snapshot, so a late response from a previously selected agent is not shown as the current agent's availability.

Copilot's default grants `write` permission and denies `shell`; choices also support existing CLI approvals or explicit all-tool approval. Temp-directory verification can be restricted. These are Copilot permission rules, not an OS sandbox. Saved CLI permissions/hooks/MCP configuration still apply. Authentication and `COPILOT_HOME` are inherited, never read or edited by CodeConvoy. `COPILOT_ALLOW_ALL` is removed in the child so the UI's chosen approval mode is not silently broadened by that environment variable. As with Codex, inherited `GIT_*` repository overrides are removed; authentication variables such as `GITHUB_TOKEN` do not match that prefix.

### Third-backend review: OpenCode

OpenCode fits the existing `AgentBackend` / `AgentOutput` contracts. No orchestration refactor or new universal capability is needed: options, help/version checks, command construction and output interpretation already belong to the backend. Codex and Copilot implementation files, process machinery, preflight and scheduler are unchanged. Adding the enum/registry entry and display label makes existing draft, results, history and reuse flows available automatically. No Claude-specific abstractions were added.

`agents/opencode` uses a dedicated local `run --format json --dir <repository>` invocation with exact stdin bytes. `--model=provider/model`, `--agent=NAME`, `--variant=NAME` and `--auto` are optional. `--key=value` preserves a leading dash in a configured value as data. The process working directory and explicit native path agree; removing inherited `PWD` is necessary because the inspected OpenCode source also reads that variable. `GIT_*` overrides are removed as in the existing backends. Provider/auth/config variables are inherited and never persisted by the backend. Permission selection relies on the CLI's unattended rejection or explicit auto-approval behavior; it does not rewrite permission files or manufacture a sandbox.

The decoder assembles each stream independently, including UTF-8 split across reads. It displays text events as text and retains other JSON records, including full tool payloads and unknown events. Plain CLI diagnostics are permitted because OpenCode can mix approval/fallback messages with JSON. A malformed JSON-looking line or an oversized stdout record prevents confirmed success. Assembly is bounded to 256 KiB per stream; oversized records stream through without parsing their fragments as fresh events. A later valid record cannot clear that failure. Existing queue/log limits remain in force.

Completion requires process exit zero, no session `error` event, and the last step having `reason=stop`. A new `step_start` clears completion. `tool-calls`, token-limit or unknown finish reasons alone cannot succeed. Tool failures stay visible without preventing a model from recovering; a success status still requires reviewing response/diff. Cancellation bypasses completion interpretation and uses the existing group/Job Object termination and scheduler cleanup. No remote server is attached, so local task processes stay owned by the shared runner.

The persisted enum value is `opencode`; its backend-owned option map uses `executable`, `model`, `agent`, `variant`, and `permissions`. Defaults are resolved during preflight and frozen in the run snapshot. The additive enum variant requires no version-1 migration: older Codex/Copilot state and missing option defaults keep their prior behavior. Downgrading to an older binary after saving an OpenCode run is not supported by that older binary. CodeConvoy saves only user-entered non-secret settings and prompt/run metadata, not OpenCode credentials or emitted events.

See [OpenCode validation](opencode-validation.md) for authoritative sources, tests and unverified E2E/platform work. The UI uses the existing backend option renderer and scrolling panes; no Activity view or general redesign is included.

### Fourth-backend review: Claude Code

Claude fits `AgentBackend` / `AgentOutput` unchanged. Registration replaces the existing Claude placeholder; the enum's serialized `claude` value already existed. No shared trait, scheduler, process, persistence or Git implementation change is required. Codex, Copilot and OpenCode implementation files remain unchanged. The abstraction is feature-frozen for v0.1.0 except for genuine bugs.

`agents/claude` owns print-mode arguments, stdin prompt transport, working context, help/version checks and five option keys: `executable`, `model`, `effort`, `permission_mode`, `max_turns`. Preflight resolves defaults into the immutable snapshot. Old version-1 drafts and history continue loading; no credential fields are introduced. Per-agent preferences, accepted-launch clearing, restart interruption recovery, terminal-history cleanup and reuse follow the existing paths.

The decoder treats stdout as JSONL, independently frames stderr, and handles split UTF-8. Final response text is displayed separately from retained result metadata. Other messages, including tool payloads, usage, denials, retries and unknown event types, remain visible. The complete parsed final result lives in the per-job decoder until interpretation; it is not persisted or exposed through a speculative common activity model.

Success requires exit zero and exactly one last `result` record with success subtype, boolean `is_error=false`, string response, nonempty session ID and nonnegative integer duration/turn fields. If supplied, `terminal_reason` must be `completed`; deferred tools, API error status and nonempty result errors prevent success. A final `stop_reason` reporting token/context truncation or a paused turn also prevents success, including on older CLIs without `terminal_reason`. Explicit session/assistant errors, invalid JSON and oversized stdout records also prevent success. A recoverable tool error or API retry alone does not fail the session. Stderr diagnostics do not override a valid result. This confirms protocol completion, not task fulfillment.

The 1 MiB record bound follows the official Python Agent SDK transport's default rather than reusing a smaller backend's limit. Oversized records remain displayable but fail completion; fragments cannot become false result records. Large valid records below the bound are tested. Existing log retention can still truncate their display.

No Claude-specific process management is added. Each invocation belongs to the shared process group/Job Object. Cancellation takes precedence over decoding, preserves edits, releases confirmed-cleanup slots/leases, and leaves unrelated jobs alone. Existing quarantine behavior remains authoritative if process cleanup cannot be confirmed. Detached descendants retain the platform limitations described below.

Permissions and omitted capabilities are explained in [Claude validation](claude-validation.md). CodeConvoy supplies no additional directories, unrestricted bypass, detached/background session, resume, agent definition or custom tool policy. Existing Claude configuration/hooks still apply; tool permissions do not sandbox that configuration. Future Activity UI and backend #5 are outside this release cycle.

## Scheduling and shutdown

The editor draft is separate from execution. Preflight takes owned copies of task/options and selected canonical repositories; confirmation saves `PreparedRun::snapshot` before handing owned data to `RunManager`. Each convoy keeps its own task/backend and job cancellation tokens. Run IDs route every event and cancellation, so selecting or editing another convoy does not affect workers.

Only successful manager admission clears the draft prompt and repository selection. Backend/options, concurrency preferences, and registrations remain. Preflight, persistence, backend, or admission failure preserves the draft. The cleared prompt is saved through the existing draft persistence path; repository checkboxes remain session-local.

One Tokio manager owns all admission decisions and a `JoinSet` of executing workers. A small pure scheduler rotates convoys after each admission. It selects the first eligible repository in that convoy, skipping busy paths and saturated convoys. A continuously eligible convoy therefore gets a turn per round; existing jobs are not preempted. There are no priorities or dependencies. Command arrival, completion, or cancellation wakes scheduling; the UI never waits for a slot.

Admission reserves the global slot, per-convoy slot, and canonical-path lease together, including the final Git recheck. Waiting jobs hold none of these resources. The global preference defaults to 4 (range 1–16); per-convoy limits are immutable snapshots. Raising the global limit admits more work immediately; lowering it stops new admissions until usage falls below the limit. Queued reasons distinguish each limit, repository access, and the Git recheck.

Path leases exclude identical and nested canonical roots across convoys. Success, ordinary failure, or confirmed cancellation releases the lease. A worker panic/process error after spawn with unconfirmed cleanup conservatively reserves its repository and capacity for the rest of the session, with a diagnostic; users must inspect remaining processes before restarting. External applications are outside this lease system. After waiting, Git state must still match preflight: previous-convoy changes require fresh review, rather than silently accepting a stale baseline.

Stop Convoy signals only its cancellation tokens, including queued jobs. Individual Stop uses `(run ID, job index)`. Stop All and application close signal every convoy. Watch tokens prevent lost cancellation; queued cancellation never needs a scheduling slot. Completion events precede replacement starts, and a failed job cannot terminate unrelated workers. Shutdown drains lifecycle events and waits for process cleanup; interrupted metadata is recovered on restart, never resumed.

Overall status is Running while any job is running, otherwise Queued while work remains. Once terminal, any failure wins, then cancellation, then success only if every job succeeded. Progress counts terminal jobs, not just successes; job rows preserve the individual outcomes.

`process-wrap` is a deliberate dependency: it supplies Unix process groups and Windows Job Objects without application-owned unsafe platform code. Windows children start suspended, join their Job Object, and then resume. Cancellation forcefully stops the process group/job and waits for termination. A drop guard also initiates cleanup if a worker future is dropped. Normal application close cancels all jobs and keeps servicing events until workers finish; an exit fallback attempts bounded cleanup.

Unix process groups cannot contain a descendant that deliberately creates a new session/group. Abrupt OS termination or a crash cannot guarantee cleanup on Unix. No promise is made to recover or kill stale processes by persisted PID (PID reuse would make that unsafe). The application does not manage detached sessions. These constraints should be tested further before packaging a release.

Stdout and stderr are drained concurrently in fixed-size chunks. Codex and OpenCode line assembly is capped at 256 KiB. Claude permits 1 MiB per record, matching the official Python Agent SDK's default; oversized stdout records invalidate completion without reinterpreting fragments. Copilot forwards each chunk immediately, retaining at most three bytes per stream to complete a split UTF-8 character; invalid bytes are replaced for display. Stderr is labelled, and no Copilot event normalization is performed. The event queue is bounded; if the UI cannot keep up, log events can be omitted with a visible count. Backend output processing continues even if a display event is dropped. Logs retain their latest 512 KiB per job, with a 32 MiB session budget that evicts the oldest job logs first. Stream interleaving is arrival order, not a claim about exact cross-stream ordering. Short Git/detection commands have a 20-second timeout and output caps.

## Git and state

Git CLI is the source of truth. Repository roots are canonicalized; subdirectories, bare repositories, duplicates, and overlapping selections are rejected. Linked worktrees can be registered as existing roots; CodeConvoy does not create or manage them. Paths remain OS-native when passed to subprocesses. Porcelain `-z` parsing handles rename records and unusual filenames. Display names use lossy conversion when filenames are not Unicode.

The review step shows a fresh status for every selected repository and requires explicit acknowledgment of dirty trees. Jobs check status again after acquiring admission and a path lease. This detects branch/HEAD/status changes during queueing, not every content change in an already-modified file. It is not a filesystem snapshot or a lock against external tools. Separate staged and unstaged diff calls also support repositories without an initial commit; untracked file contents are not added to the index just to generate a diff.

Application state is versioned JSON written to an owner-only temporary file, synced, then atomically replaced. A data-directory lock prevents simultaneous writes and duplicate batch execution from two instances using that directory. Initial run metadata is saved before spawning agents. Updates and draft changes are saved periodically and at exit. Output and diagnostics use `serde(skip)` so CLI material cannot accidentally enter saved history. Prompt and option fields are user input and are intentionally persisted.

An additive `agent_options` map preserves preferences independently when switching backends. It defaults to empty when loading old version-1 state; the existing draft remains authoritative for the selected backend, and historical run schemas are unchanged. No migration or rewriting of CLI configuration is necessary.

All active convoys and the most recent 30 completed convoys are retained; active snapshots are never evicted by the history cap. The additive `global_concurrency` preference defaults to 4 for old state; invalid stored values are clamped to 1–16. Relaunch converts unfinished saved jobs to cancelled/interrupted. There is no automatic resume or replay. History records initial Git metadata, but diff inspection always queries the current repository.

## Dependencies and scope

- `eframe`/egui: native windowing and widgets. Version 0.36 is used with Rust 1.95 or newer; OpenGL is used to avoid a direct WGPU renderer dependency. Wayland, X11, fonts, and accessibility are enabled.
- Tokio: background process I/O, timers, task ownership, cancellation notifications.
- serde/serde_json: local state and backend-specific Codex/OpenCode/Claude JSON records.
- `directories`: conventional platform data paths.
- `tempfile`: atomic state replacement and isolated tests.
- `fs2`: cross-platform file locking.
- `process-wrap`: Unix/Windows process-tree lifecycle.
- `anyhow`: actionable contextual errors at I/O boundaries.

No provider SDK, secret store, shell interpreter, database, plugin loading, Git hosting integration, or remote execution is included. Test-only fixtures exercise real local process and Git behavior without calling a model provider.
