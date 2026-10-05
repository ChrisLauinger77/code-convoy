# Architecture

CodeConvoy uses one egui UI thread and a small Tokio runtime. The unit of work is a task configuration applied to a set of existing repository roots. A run is a saved snapshot of that configuration plus one job per repository.

Backend status: Codex and Copilot are implemented and user-verified end-to-end on Linux with two concurrent real repositories each. Claude Code and OpenCode are planned, in that order.

## Modules

| Module | Responsibility |
| --- | --- |
| `domain` | Serializable tasks, repositories, runs, job states, validation, bounded in-memory logs |
| `agents` | Backend registry, backend-owned option descriptions, `AgentBackend` / per-job `AgentOutput` contracts |
| `agents/codex` | Codex flags, capability checks, existing JSONL formatting/completion criteria |
| `agents/copilot` | Copilot flags, bundled-version checks, raw text streaming, exit-status interpretation |
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

New backend support should add its module, enum/registry entry, real CLI contract, option schema, and tests; do not return simulated success for an unimplemented agent.

Codex uses stdin for the prompt to avoid shell injection and command-line length limits. It sets `--no-daemon`, explicit sandbox permissions, and `--ask-for-approval never`; no interactive approval UI exists. Authentication remains entirely with the CLI. Session resume is intentionally absent while ephemeral execution is enabled. A successful exit must also have a `turn.completed` event and no `turn.failed` event. Unknown JSONL events remain visible for diagnostics.

Copilot uses its documented stdin pipe mode for a noninteractive prompt and keeps stdout as text. It uses `--no-ask-user`, explicit streaming/no-color/plain-diff, and disables remote control/export. No Codex flags or result events are applied to it. Its success criterion is process exit zero; reviewing the agent response is still necessary when tools were denied or a task was not fulfilled.

Detection and execution both use `--no-auto-update`. On the development machine that selects bundled 1.0.65, whereas launching without that flag reports cached 1.0.91. Compatibility is checked against the flags required by the backend, with useful version reporting and missing-executable errors. No detection runs at app startup. `Check CLI` results are associated with the agent and option snapshot, so a late response from a previously selected agent is not shown as the current agent's availability.

Copilot's default grants `write` permission and denies `shell`; choices also support existing CLI approvals or explicit all-tool approval. Temp-directory verification can be restricted. These are Copilot permission rules, not an OS sandbox. Saved CLI permissions/hooks/MCP configuration still apply. Authentication and `COPILOT_HOME` are inherited, never read or edited by CodeConvoy. `COPILOT_ALLOW_ALL` is removed in the child so the UI's chosen approval mode is not silently broadened by that environment variable. As with Codex, inherited `GIT_*` repository overrides are removed; authentication variables such as `GITHUB_TOKEN` do not match that prefix.

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

Stdout and stderr are drained concurrently in fixed-size chunks. Codex line assembly is capped at 256 KiB. Copilot forwards each chunk immediately, retaining at most three bytes per stream to complete a split UTF-8 character; invalid bytes are replaced for display. Stderr is labelled, and no Copilot event normalization is performed. The event queue is bounded; if the UI cannot keep up, log events can be omitted with a visible count. Backend output processing continues even if a display event is dropped. Logs retain their latest 512 KiB per job, with a 32 MiB session budget that evicts the oldest job logs first. Stream interleaving is arrival order, not a claim about exact cross-stream ordering. Short Git/detection commands have a 20-second timeout and output caps.

## Git and state

Git CLI is the source of truth. Repository roots are canonicalized; subdirectories, bare repositories, duplicates, and overlapping selections are rejected. Linked worktrees can be registered as existing roots; CodeConvoy does not create or manage them. Paths remain OS-native when passed to subprocesses. Porcelain `-z` parsing handles rename records and unusual filenames. Display names use lossy conversion when filenames are not Unicode.

The review step shows a fresh status for every selected repository and requires explicit acknowledgment of dirty trees. Jobs check status again after acquiring admission and a path lease. This detects branch/HEAD/status changes during queueing, not every content change in an already-modified file. It is not a filesystem snapshot or a lock against external tools. Separate staged and unstaged diff calls also support repositories without an initial commit; untracked file contents are not added to the index just to generate a diff.

Application state is versioned JSON written to an owner-only temporary file, synced, then atomically replaced. A data-directory lock prevents simultaneous writes and duplicate batch execution from two instances using that directory. Initial run metadata is saved before spawning agents. Updates and draft changes are saved periodically and at exit. Output and diagnostics use `serde(skip)` so CLI material cannot accidentally enter saved history. Prompt and option fields are user input and are intentionally persisted.

An additive `agent_options` map preserves preferences independently when switching backends. It defaults to empty when loading old version-1 state; the existing draft remains authoritative for the selected backend, and historical run schemas are unchanged. No migration or rewriting of CLI configuration is necessary.

All active convoys and the most recent 30 completed convoys are retained; active snapshots are never evicted by the history cap. The additive `global_concurrency` preference defaults to 4 for old state; invalid stored values are clamped to 1–16. Relaunch converts unfinished saved jobs to cancelled/interrupted. There is no automatic resume or replay. History records initial Git metadata, but diff inspection always queries the current repository.

## Dependencies and scope

- `eframe`/egui: native windowing and widgets. Version 0.33.3 targets Rust 1.88; OpenGL is used to avoid a direct WGPU renderer dependency. Wayland, X11, fonts, and accessibility are enabled.
- Tokio: background process I/O, timers, task ownership, cancellation notifications.
- serde/serde_json: local state and Codex JSONL.
- `directories`: conventional platform data paths.
- `tempfile`: atomic state replacement and isolated tests.
- `fs2`: cross-platform file locking.
- `process-wrap`: Unix/Windows process-tree lifecycle.
- `anyhow`: actionable contextual errors at I/O boundaries.

No provider SDK, secret store, shell interpreter, database, plugin loading, Git hosting integration, or remote execution is included. Test-only fixtures exercise real local process and Git behavior without calling a model provider.
