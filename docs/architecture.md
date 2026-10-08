# Architecture

CodeConvoy uses one egui UI thread and a small Tokio runtime. The unit of work is a task configuration applied to a set of existing repository roots. A run is a saved snapshot of that configuration plus one job per repository.

Backend status: Codex and Copilot are implemented and user-verified end-to-end on Linux and macOS. Linux testing used two concurrent real repositories each; successful macOS testing was also confirmed by the user. OpenCode is the third supported backend, with source/fixture checks and macOS UI validation; authenticated E2E is unverified. Claude Code is the fourth supported backend, validated against official documentation/SDK source, fixtures and the native macOS UI; authenticated E2E is unverified.

## Modules

| Module | Responsibility |
| --- | --- |
| `domain` | Serializable tasks, repositories, groups, task-only templates, runs, job states, validation and bounded logs |
| `attachments` | Canonical attachment metadata, bounded reads, digest validation and text-context escaping; no saved contents |
| `agents` | Backend registry, backend-owned option descriptions, `AgentBackend` / per-job `AgentOutput` contracts |
| `agents/discovery` | Filesystem-only executable discovery; candidates still require the selected backend's compatibility check |
| `agents/availability` | Session-local per-backend asynchronous checks, executable resolution, cancellation and stale-result rejection |
| `agents/codex` | Codex flags, capability checks, existing JSONL formatting/completion criteria |
| `agents/copilot` | Copilot flags, bundled-version checks, raw text streaming, exit-status interpretation |
| `agents/opencode` | OpenCode run/help/version contract, model/agent/variant/permission options, native JSON event decoding and conservative completion |
| `agents/claude` | Claude print/help/version contract, model/effort/turn/permission options, stream-json decoding and final-result validation |
| `process` | Shell-free process spawning, pipes, cancellation tokens, process-tree ownership, short-command timeouts |
| `git` | Root/common-directory identity, working-tree inspection, direct diff and isolated diff dispatch |
| `worktrees` | Owned attempt reservation, detached Git creation, ownership verification and fixed-base result inspection |
| `runner` | Preflight, immutable snapshots, per-job Git recheck and execution events |
| `runner/raw_output` | Bounded UTF-8 stream decoding for original CLI text, separate from Activity |
| `runner/manager` | Application-owned multi-convoy lifecycle, worker ownership, cancellation, shutdown |
| `runner/schedule` | Pure round-robin admission policy, both concurrency limits, canonical path leases |
| `persistence` | Platform data directory, single-instance lock, atomic JSON replacement and recovery |
| `ui` | Task/options/repository editor, preflight review, history/results/output/diff views |

## Presentation

`ui/library` owns group/template editing and explicit selection actions. `ui/attachments_ui` owns native file selection, optional drops, background inspection/reuse and stale-result rejection. `ui/editor` coordinates the task, scrolling editor, fixed execution controls, and modal preflight review. `ui/agent_config` renders only the selected backend's option specifications; `ui/repositories` handles registration, Git-state presentation and bulk selection. `ui/results` presents run navigation, cleanup, jobs and result tabs; `ui/snapshot` displays declared option labels, repository paths and UTC timestamps. `ui/diagnostics` keeps concise messages separate from expandable, copyable raw details. `ui/about` shows offline application metadata. `ui/theme` centralizes palettes, typography, spacing, focus/selection, primary actions and a painted success mark (the bundled fonts lack check glyphs). `ui/format` formats duration, dates, options and status text; overall status and progress remain domain-derived.

The compact run selector groups active and terminal runs in a height-bounded scrolling popup. Active means any queued/preparing/running job, including a partially failed convoy that still has work. History cleanup methods in `AppState` guard against removing active runs; the UI offers individual removal only for terminal runs and bulk removal through History cleanup. Removal immediately saves metadata and reconciles selection (prefer an active convoy, then the newest history, then empty), clearing stale job/diff/output views. It never calls Git, the scheduler, cancellation, or agent configuration APIs.

Reuse copies a run's task/options and selects its still-registered canonical repository paths. It reports skipped registrations, leaves the global limit alone, and requires the normal preflight for any subsequent launch. Reuse is unavailable during preflight/review/shutdown. No historical configuration is mutated or automatically executed.

`ui/text_view` caches a line index for the selected output/diff and uses egui's visible-row layout. Copy actions preserve the original retained text. Diff coloring is presentation-only: no agent protocol parsing, Git behavior, or stored output format is changed. Main panes and long details scroll independently; execution controls remain visible at the bottom of the editor. Appearance follows the system by default; overrides are saved as an optional appearance preference (Part 3.3); older state defaults to System. The editor is rendered before execution controls to match visual and Tab order. CLI probes have independent per-backend state, so their completion cannot clear repository/preflight work. An in-memory set of retained runs launched this session distinguishes empty live output from unrestored history logs.

App construction schedules checks for all four configured/default executables on Tokio without awaiting them. `agents/availability` resolves exact launcher names with the existing filesystem-only discovery locations on blocking workers, then runs each backend's normal help/version check. Explicit absolute paths never fall back to another installation. Matching results adopt resolved paths into the existing per-agent settings so GUI launches and subsequent jobs use the same executable. The UI reports Checking… while a probe or its latest replacement is pending; success/error details are backend-local. Availability and probe state are never serialized.

Run Convoy is disabled while the selected backend's check is pending. The preflight entry point also reconciles executable edits and refuses to snapshot a pending selection. This ensures a discovered absolute path is applied before preflight captures immutable options, including when the default launcher is absent from the GUI PATH. Checks for other backends do not delay review; task editing, repository loading and existing convoys stay independent.

The Agent selector restores that backend's preferences and explicitly requests validation for its executable. The existing session result or in-flight check is reused when the executable matches; an unchecked backend or a changed executable starts its asynchronous check. Switching away does not cancel another backend's work or discard its result. `Check CLI` bypasses the session result and requests fresh validation, while still suppressing duplicate in-flight probes.

Changing an executable immediately invalidates the result, cancels its old probe and queues only the latest selection. A 300 ms delay coalesces typing. Generation IDs reject late results, including A → B → A edits and discovery results arriving after manual selection. Process cancellation is awaited before a replacement starts. `Check CLI` explicitly refreshes an idle backend; duplicate requests during a check are ignored. Unchanged settings only compare in-memory values, with no continuous rediscovery. Availability checks use executable options only; model/permission edits do not spawn probes and full job settings still receive normal preflight validation. A per-backend async mutex also serializes these probes with preflight help/version and conditional attachment-interface checks. For tasks without attachments, agent execution commands and authentication remain unchanged. See [CLI lifecycle validation](cli-availability-validation.md).

`build.rs` reads an optional short Git revision at compile time, tracks Git reference changes, and tolerates missing Git/source metadata. Rendering About metadata performs no process or network work at runtime. Its project hyperlink uses eframe's explicitly enabled `links` feature (and its `webbrowser` dependency) to open the host's browser when activated. A source archive nested in another checkout does not inherit that checkout's revision.

`Find CLI` runs filesystem discovery on a blocking worker via Tokio, never on the egui thread. It includes a valid configured absolute path, then searches absolute `PATH` entries and common user/system installation directories. It checks regular executable files, ignores empty/relative search directories, and deduplicates canonical targets while retaining stable launcher/symlink paths. Windows discovery only offers native `.exe` files. It does not execute candidates, load shell profiles, change process search paths, install CLIs, or read credentials. Common locations cover [Homebrew Codex](https://formulae.brew.sh/cask/codex), [Copilot installation methods](https://docs.github.com/en/copilot/get-started/cli-quickstart), [OpenCode's installer](https://opencode.ai/install), and [Claude's native launcher](https://code.claude.com/docs/en/setup).

`ui/cli_discovery` presents a cancellable, keyboard-accessible candidate chooser. A request ID and agent/options snapshot reject cancelled or stale results. Chooser results never replace manual settings by themselves. `Use and check` writes the chosen path into the existing per-agent draft options and requests asynchronous validation, including when an older executable is still being checked; the normal store saves it without a schema change. Other backend settings, history, repository work, and job execution are untouched. Missing results retain manual path entry and installation guidance.

About displays the Cargo package version, followed by the Git revision in parentheses only when available. On macOS, `ui/about_macos` supplies these values to AppKit's standard About panel, explicitly passing an empty build value when Git metadata is absent. This avoids AppKit's fallback to the numeric `CFBundleVersion` while preserving the native panel and icon. The macOS-only direct `objc2`/AppKit/Foundation dependencies reuse versions already present through the UI dependencies; they provide the typed native menu action and panel options. Bundle version fields remain numeric and Cargo-derived. Tagged release checkouts still obtain the actual short `HEAD` commit through `build.rs`, independently of those bundle fields. The native application menu explicitly labels About, Hide and Quit with `CodeConvoy`, keeping their existing selectors and shortcuts. The About panel also sets that display name, suppresses the bundle copyright line and supplies a centered, clickable GitHub repository link through AppKit's attributed Credits field. The repository URL comes from Cargo metadata and is shared with the egui About dialog; native text styling uses additional features of the existing AppKit/Foundation dependencies.

Repository browsing uses `rfd::AsyncFileDialog`, parented to eframe's root window. The UI thread constructs the folder-picker future (required for native macOS sheet setup); the existing Tokio runtime awaits it and sends an optional path through the UI message channel, requesting repaint. A separate in-flight flag prevents duplicate pickers and disables Add while a selection is pending, without changing repository/runner busy state. Completion returns keyboard focus to the editable path. Cancellation leaves its contents untouched. A picked path retains its exact spelling until edited, including trailing whitespace; unrepresentable Unicode paths are rejected instead of being converted lossily. Only the explicit Add action invokes the existing `git::register` / `git::status` and canonical duplicate checks. See [implementation rationale and validation](folder-picker-validation.md).

## Backend extension

`AgentBackend` owns executable/help/version detection, supported options, invocation construction, and the execution summary shown in the editor/preflight UI. Its `output()` factory creates an independent `AgentOutput` for each job; that object owns byte framing, stream handling, and exit interpretation. It exposes overridable spawn/cancel methods and delegates ordinary subprocess handling to `process`. Async pipe I/O and scheduling remain shared, rather than being copied into every backend.

`OptionSpec` provides rendering metadata (text fields or backend-specific choice values). Persisted option keys belong to the selected backend. There is no cross-agent reasoning, model, or permission translation. Backends validate keys and values before invocation. The generic renderer only renders that backend's descriptions. Adding Copilot exposed two concrete Codex assumptions: shared line-based output with `turn.completed` observation fields, and hardcoded Codex approval/sandbox UI text. Both now live in the Codex backend. The runner only sends bytes and an exit status to an opaque per-job output handler. This permits Copilot to stream partial text immediately and interpret its own exit status, without inventing a common event schema. Codex's formatter and success criteria are unchanged. No plugin system or hypothetical capability layer was added.

The v0.1.0 abstraction was frozen during stabilization. The requested v0.2
work adds backend-owned attachment validation, conditional native-flag probes,
transport descriptions, and optional `take_activity()` summaries of actual
backend events. Raw bytes are separately decoded by the runner. No universal
provider/model/permission semantics or additional backend is introduced.

Codex uses stdin for the prompt to avoid shell injection and command-line length limits. It sets `--no-daemon`, explicit sandbox permissions, and `--ask-for-approval never`; no interactive approval UI exists. Authentication remains entirely with the CLI. Session resume is intentionally absent while ephemeral execution is enabled. A successful exit must also have a `turn.completed` event and no `turn.failed` event. Unknown JSONL events remain visible for diagnostics.

Copilot uses its documented stdin pipe mode for a noninteractive prompt and keeps stdout as text. It uses `--no-ask-user`, explicit streaming/no-color/plain-diff, and disables remote control/export. No Codex flags or result events are applied to it. Its success criterion is process exit zero; reviewing the agent response is still necessary when tools were denied or a task was not fulfilled.

Detection and execution both use `--no-auto-update`. On the development machine that selects bundled 1.0.65, whereas launching without that flag reports cached 1.0.91. Compatibility is checked against the flags required by the backend, with useful version reporting and missing-executable errors. Automatic startup checks and explicit refreshes use the same backend contract; executable generations prevent late responses from replacing a newer selection's availability.

Copilot's default grants `write` permission and denies `shell`; choices also support existing CLI approvals or explicit all-tool approval. Temp-directory verification can be restricted. These are Copilot permission rules, not an OS sandbox. Saved CLI permissions/hooks/MCP configuration still apply. Authentication and `COPILOT_HOME` are inherited, never read or edited by CodeConvoy. `COPILOT_ALLOW_ALL` is removed in the child so the UI's chosen approval mode is not silently broadened by that environment variable. As with Codex, inherited `GIT_*` repository overrides are removed; authentication variables such as `GITHUB_TOKEN` do not match that prefix.

### Third-backend review: OpenCode

The initial OpenCode addition fitted the existing `AgentBackend` / `AgentOutput` contracts. No orchestration refactor or new universal capability is needed: options, help/version checks, command construction and output interpretation already belong to the backend. That initial addition left Codex, Copilot, process machinery, preflight and scheduler unchanged. Adding the enum/registry entry and display label makes existing draft, results, history and reuse flows available automatically. No Claude-specific abstractions were added.

`agents/opencode` uses a dedicated local `run --format json --dir <repository>` invocation with exact stdin bytes. `--model=provider/model`, `--agent=NAME`, `--variant=NAME` and `--auto` are optional. `--key=value` preserves a leading dash in a configured value as data. The process working directory and explicit native path agree; removing inherited `PWD` is necessary because the inspected OpenCode source also reads that variable. `GIT_*` overrides are removed as in the existing backends. Provider/auth/config variables are inherited and never persisted by the backend. Permission selection relies on the CLI's unattended rejection or explicit auto-approval behavior; it does not rewrite permission files or manufacture a sandbox.

The decoder assembles each stream independently, including UTF-8 split across reads. It displays text events as text and retains other JSON records, including full tool payloads and unknown events. Plain CLI diagnostics are permitted because OpenCode can mix approval/fallback messages with JSON. A malformed JSON-looking line or an oversized stdout record prevents confirmed success. Assembly is bounded to 256 KiB per stream; oversized records stream through without parsing their fragments as fresh events. A later valid record cannot clear that failure. Existing queue/log limits remain in force.

Completion requires process exit zero, no session `error` event, and the last step having `reason=stop`. A new `step_start` clears completion. `tool-calls`, token-limit or unknown finish reasons alone cannot succeed. Tool failures stay visible without preventing a model from recovering; a success status still requires reviewing response/diff. Cancellation bypasses completion interpretation and uses the existing group/Job Object termination and scheduler cleanup. No remote server is attached, so local task processes stay owned by the shared runner.

The persisted enum value is `opencode`; its backend-owned option map uses `executable`, `model`, `agent`, `variant`, and `permissions`. Defaults are resolved during preflight and frozen in the run snapshot. The additive enum variant requires no version-1 migration: older Codex/Copilot state and missing option defaults keep their prior behavior. Downgrading to an older binary after saving an OpenCode run is not supported by that older binary. CodeConvoy saves only user-entered non-secret settings and prompt/run metadata, not OpenCode credentials or emitted events.

See [OpenCode validation](opencode-validation.md) for authoritative sources, tests and unverified E2E/platform work. The UI uses the existing backend option renderer and scrolling panes; the initial backend addition included no Activity view or general redesign. The requested v0.2 presentation adds Activity alongside retained Raw output.

### Fourth-backend review: Claude Code

The initial Claude addition fitted `AgentBackend` / `AgentOutput` unchanged. Registration replaces the existing Claude placeholder; the enum's serialized `claude` value already existed. No shared trait, scheduler, process, persistence or Git implementation change is required. That addition left the other backend implementations unchanged. The requested v0.2 attachment and Activity extensions are described above.

`agents/claude` owns print-mode arguments, stdin prompt transport, working context, help/version checks and five option keys: `executable`, `model`, `effort`, `permission_mode`, `max_turns`. Preflight resolves defaults into the immutable snapshot. Old version-1 drafts and history continue loading; no credential fields are introduced. Per-agent preferences, accepted-launch clearing, restart interruption recovery, terminal-history cleanup and reuse follow the existing paths.

The decoder treats stdout as JSONL, independently frames stderr, and handles split UTF-8. Final response text is displayed separately from retained result metadata. Other messages, including tool payloads, usage, denials, retries and unknown event types, remain visible. The complete parsed final result lives in the per-job decoder until interpretation; it is not persisted or exposed through a speculative common activity model.

Success requires exit zero and exactly one last `result` record with success subtype, boolean `is_error=false`, string response, nonempty session ID and nonnegative integer duration/turn fields. If supplied, `terminal_reason` must be `completed`; deferred tools, API error status and nonempty result errors prevent success. A final `stop_reason` reporting token/context truncation or a paused turn also prevents success, including on older CLIs without `terminal_reason`. Explicit session/assistant errors, invalid JSON and oversized stdout records also prevent success. A recoverable tool error or API retry alone does not fail the session. Stderr diagnostics do not override a valid result. This confirms protocol completion, not task fulfillment.

The 1 MiB record bound follows the official Python Agent SDK transport's default rather than reusing a smaller backend's limit. Oversized records remain displayable but fail completion; fragments cannot become false result records. Large valid records below the bound are tested. Existing log retention can still truncate their display.

No Claude-specific process management is added. Each invocation belongs to the shared process group/Job Object. Cancellation takes precedence over decoding, preserves edits, releases confirmed-cleanup slots/leases, and leaves unrelated jobs alone. Existing quarantine behavior remains authoritative if process cleanup cannot be confirmed. Detached descendants retain the platform limitations described below.

Permissions and omitted capabilities are explained in [Claude validation](claude-validation.md). CodeConvoy supplies no additional directories, unrestricted bypass, detached/background session, resume, agent definition or custom tool policy. Existing Claude configuration/hooks still apply; tool permissions do not sandbox that configuration. Future Activity UI and backend #5 are outside this release cycle.

## Scheduling and shutdown

The editor draft is separate from execution. Preflight takes owned copies of task/options and selected canonical repositories; confirmation saves `PreparedRun::snapshot` before handing owned data to `RunManager`. Each convoy keeps its own task/backend and job cancellation tokens. Run IDs route every event and cancellation, so selecting or editing another convoy does not affect workers.

Only successful manager admission clears the draft prompt, attachment references and repository selection. Backend/options, concurrency preferences, and registrations remain. Preflight, persistence, backend, or admission failure preserves the draft. The cleared prompt is saved through the existing draft persistence path; repository checkboxes remain session-local.

One Tokio manager owns all admission decisions and a `JoinSet` of executing workers. A small pure scheduler rotates convoys after each admission. It selects the first eligible repository in that convoy, skipping busy paths and saturated convoys. A continuously eligible convoy therefore gets a turn per round; existing jobs are not preempted. There are no priorities or dependencies. Command arrival, completion, or cancellation wakes scheduling; the UI never waits for a slot.

Admission reserves the global slot, per-convoy slot, and canonical-path lease together, including the final Git recheck. Waiting jobs hold none of these resources. The global preference defaults to 4 (range 1–16); per-convoy limits are immutable snapshots. Raising the global limit admits more work immediately; lowering it stops new admissions until usage falls below the limit. Queued reasons distinguish each limit, repository access, and the Git recheck.

Direct-mode leases exclude identical/nested canonical roots and shared common Git directories across convoys. Isolated admission is described below. Success, ordinary failure, or confirmed cancellation releases the lease. A worker panic/process error after spawn with unconfirmed cleanup conservatively reserves its repository and capacity for the rest of the session, with a diagnostic; users must inspect remaining processes before restarting. External applications are outside this lease system. For direct mode, after waiting Git state must still match preflight: previous-convoy changes require fresh review, rather than silently accepting a stale baseline.

Stop Convoy signals only its cancellation tokens, including queued jobs. Individual Stop uses `(run ID, job index)`. Stop All and **confirmed** application close signal every convoy. Watch tokens prevent lost cancellation; queued cancellation never needs a scheduling slot. Completion events precede replacement starts, and a failed job cannot terminate unrelated workers. Shutdown drains lifecycle events and waits for process cleanup; interrupted metadata is recovered on restart, never resumed.

Window close asks for confirmation when either the manager owns unfinished work or saved job metadata remains nonterminal. `ui/quit` owns a single modal decision with live running/queued counts. Cancel (also Escape and the initially focused button) only dismisses that decision. Confirm closes UI and manager admission, sets a shared shutdown flag before signalling the existing cancellation tokens, and waits for the manager to finish while polling events. Admission checks the flag both before scheduling and after backpressured event delivery; workers check it again immediately before agent spawn. Final lifecycle events are drained and state is saved before eframe receives permission to exit. The handler runs in `App::logic`, including hidden/minimized frames. Repeated requests cannot bypass confirmation or repeat shutdown. CLI availability checks are cancelled separately and never count as active convoys or delay the quit decision. App drop shuts down Tokio in the background so a stuck discovery filesystem worker cannot hold exit open; dropped async probes retain the existing process-tree kill guard.

On macOS, `ui/quit_macos` creates an `NSApplication` subclass before eframe initializes AppKit. Its `terminate:` action routes native Quit (including Cmd+Q) into the same root viewport close request; winit retains its delegate and event loop. This is necessary because native termination otherwise reaches winit's exit notification after cancellation can no longer be vetoed. The standard menus, About panel and icon remain intact. Linux and Windows use the shared cancellable viewport close path. No new dependency is needed. See [quit confirmation validation](quit-confirmation-validation.md).

Overall status is Running while any job is running, otherwise Preparing while any isolated job is preparing, otherwise Queued while work remains. Once terminal, any failure wins, then cancellation, then success only if every job succeeded. Progress counts terminal jobs, not just successes; job rows preserve the individual outcomes.

`process-wrap` is a deliberate dependency: it supplies Unix process groups and Windows Job Objects without application-owned unsafe platform code. Windows children start suspended, join their Job Object, and then resume. Cancellation forcefully stops the process group/job and waits for termination. A drop guard also initiates cleanup if a worker future is dropped. Normal application close first confirms active work, then cancels all jobs and keeps servicing events until workers finish; an exit fallback attempts bounded cleanup.

Unix process groups cannot contain a descendant that deliberately creates a new session/group. Abrupt OS termination or a crash cannot guarantee cleanup on Unix. No promise is made to recover or kill stale processes by persisted PID (PID reuse would make that unsafe). The application does not manage detached sessions. These constraints should be tested further before packaging a release.

Stdout and stderr are drained concurrently in fixed-size chunks. Codex and OpenCode line assembly is capped at 256 KiB. Claude permits 1 MiB per record, matching the official Python Agent SDK's default; oversized stdout records invalidate completion without reinterpreting fragments. Copilot forwards each chunk immediately, retaining at most three bytes per stream to complete a split UTF-8 character; invalid bytes are replaced for display. Stderr is labelled, and no Copilot event normalization is performed. The event queue is bounded; if the UI cannot keep up, log events can be omitted with a visible count. Backend output processing continues even if a display event is dropped. Activity and raw logs each retain their latest 512 KiB per job/view, with a shared 32 MiB session budget that evicts the oldest job logs first. Stream interleaving is arrival order, not a claim about exact cross-stream ordering. Short Git/detection commands have a 20-second timeout and output caps.

## Git and state

Git CLI is the source of truth. Repository roots are canonicalized; subdirectories, bare repositories, duplicates, and overlapping selections are rejected. Linked worktrees can be registered as existing roots; optional isolated execution creates owned detached worktrees as described below. Paths remain OS-native when passed to subprocesses. Porcelain `-z` parsing handles rename records and unusual filenames. Display names use lossy conversion when filenames are not Unicode.

The review step shows a fresh status for every selected repository and requires explicit acknowledgment of dirty trees. Direct jobs check status again after acquiring admission and a repository lease. This detects branch/HEAD/status changes during queueing, not every content change in an already-modified file. It is not a filesystem snapshot or a lock against external tools. Separate staged and unstaged diff calls also support repositories without an initial commit; untracked file contents are not added to the index just to generate a diff.

Application state is versioned JSON written to an owner-only temporary file, synced, then atomically replaced. A data-directory lock prevents simultaneous writes and duplicate batch execution from two instances using that directory. Initial run metadata is saved before spawning agents. Updates and draft changes are saved periodically and at exit. Output and diagnostics use `serde(skip)` so CLI material cannot accidentally enter saved history. Prompt and option fields are user input and are intentionally persisted.

An additive `agent_options` map preserves preferences independently when switching backends. It defaults to empty when loading old version-1 state; the existing draft remains authoritative for the selected backend, and historical run schemas are unchanged. No migration or rewriting of CLI configuration is necessary.

All active convoys, unresolved isolated results, and the most recent 30 other completed convoys are retained; protected snapshots are never evicted by the history cap. The additive `global_concurrency` preference defaults to 4 for old state; invalid stored values are clamped to 1–16. Relaunch converts unfinished saved jobs to cancelled/interrupted. There is no automatic resume or replay. History records initial Git metadata. Direct diffs query the registered working tree; isolated diffs query the retained checkout against its saved base commit.

## Dependencies and scope

- `eframe`/egui: native windowing and widgets. Version 0.36 is used with Rust 1.95 or newer. Linux/macOS explicitly use OpenGL; Windows enables wgpu with DX12/WGSL, allowing GPU or WARP software rendering without a VM OpenGL driver. `graphics` owns this platform setup. Windows-only `CODECONVOY_RENDERER=software` selects a surface-compatible CPU adapter; `auto` uses normal GPU-preferred selection and `opengl` retains the former renderer for troubleshooting. Wayland, X11, fonts, and accessibility are enabled. Graphics choices do not affect application persistence or process execution.
- Tokio: background process I/O, timers, task ownership, cancellation notifications.
- serde/serde_json: local state and backend-specific Codex/OpenCode/Claude JSON records.
- `directories`: conventional platform data paths.
- `tempfile`: atomic state replacement and isolated tests.
- `fs2`: cross-platform file locking.
- `sha2`: SHA-256 detects changed attachment contents without persisting them.
- `base64`: Claude's documented structured image input. Both additions use small, established crates already available in the dependency cache.
- `process-wrap`: Unix/Windows process-tree lifecycle.
- `anyhow`: actionable contextual errors at I/O boundaries.

No provider SDK, secret store, shell interpreter, database, plugin loading, Git hosting integration, or remote execution is included. Test-only fixtures exercise real local process and Git behavior without calling a model provider.

## v0.2 task context

Version-1 state gains additive, default-empty groups/templates and task
attachments. Groups reference existing canonical registration paths. Selection
is an explicit path set; group actions never alter jobs or create execution
units. Templates contain only name/prompt. Snapshot copies preserve tasks and
attachment metadata regardless of subsequent library edits. Missing group
memberships remain repairable. See [selection and lifecycle semantics](task-context.md).

Attachments are references plus size/digest/type, never persisted contents.
Background workers inspect and hash files; preflight validates them before Git
review. Once admitted, a job validates its references and builds stdin/arguments
on a blocking worker, then revalidates the Git baseline immediately before spawn.
Cancellation can drop this read-only worker wait without exposing repository
writes; the existing manager releases confirmed-safe capacity and leases.
All four backends choose their actual text/image transport without copying
files into repositories or adding directory grants. A stale add/reuse request
cannot overwrite a newer draft. Pending validation blocks preflight.
Reuse retains unavailable references and keeps an in-memory set of paths needing
attention; both the launch button and preflight entry point enforce it. Removing
a flagged reference explicitly resolves it, without modifying the historical
snapshot. A restarted draft still undergoes full file validation at preflight.

Group rendering builds one borrowed registration/availability index per frame,
then counts membership and selection without cloning paths or repeatedly scanning
registrations. Missing-path display strings are created only when hovered.
Attachment rendering uses stored metadata; no file reads or hashes run per frame.
CLI checks are event-driven as described above. Output copies/indexes are updated
when retained text changes, rather than once per frame. State writes require a
dirty flag and the existing save interval (or an explicit lifecycle save).

Activity is a backend-owned optional string drained after each input chunk.
It describes actual reported events; limited plain-text backends fall back to
normal messages. Raw output has independent bounded stream decoding, including
split UTF-8, and both logs share the session memory budget. Completion rules
remain independent from presentation. Session metadata is not newly persisted;
[Codex investigation](codex-sessions.md) retains ephemeral invocation and adds
no continuation actions.

The `test-support`-gated `v02_validation` example runs the real native UI with a
temporary Store, 24 disposable repositories and fixture executable settings for
all four backends. The fixture mode requires an explicit marker inside each
temporary repository's `.git` directory and records received text/image digests
there for assertions. It neither invokes installed coding agents nor reads their
authentication. `tests/workflows.rs` exercises this mode through the real runner
and scheduler; its separate authenticated Codex probe is ignored unless explicitly
requested. Neither helper is shipped as the application. No dependency was added
for Part 2.3.


## v0.3 Part 3.1A: worktree execution and lifecycle

`ExecutionMode::{Direct, IsolatedWorktree}` belongs to `TaskConfig` and each job
snapshot. Missing fields deserialize as Direct. Reuse copies the task setting,
never a job's worktree path. Groups remain path selection helpers and templates
remain task-only. The state version remains 1 with additive defaulted fields;
no backend settings, credentials or output are added to persistence.

Preflight captures canonical `git rev-parse --git-common-dir` identity (resolving
relative output against the checkout) and the source HEAD commit. Direct mode retains its
status/branch/HEAD recheck, dirty review and staged/unstaged diff. Isolated mode
uses that exact reviewed commit even if source HEAD subsequently moves; local
index, tracked edits and untracked files are not imported. An unborn or missing
base fails only that job. Source path and common identity are revalidated before
creation. There is no fallback to Direct.

Git's [detached worktree mechanism](https://git-scm.com/docs/git-worktree) provides
independent HEAD/index/checkout state while sharing repository administration.
CodeConvoy runs `git worktree add --detach --lock --reason ... -- <path> <commit>`
through `CommandSpec`, with OS-native argument paths, cleared inherited `GIT_*`
overrides and no shell. The lock prevents ordinary Git pruning of retained
worktree administration; it is not an execution mutex. No permanent branch or
private Git ref is needed. Preparation overrides `core.hooksPath` with the
attempt's empty hooks directory so `post-checkout` cannot run source hook actions.
On Windows, `git::path_argument` spells canonical drive/UNC paths without Rust's
verbatim prefix for Git's worktree and hooks arguments; stored identities remain
canonical and OS strings are preserved without lossy conversion.
Agent CLI configuration, permissions and Git content filters retain their normal
semantics; worktrees are filesystem separation, not a security sandbox. No
submodule initialization, dependency copying or agent authentication is added.

Storage is `<Store data directory>/worktrees/run-<id>-job-<index>-<random>/tree`.
Reservation checks the storage root with `symlink_metadata` before creating an
attempt or resolving its identity. Symlinks (including dangling links) and Windows
reparse points are rejected using the same resource checks as recovery.
The existing `tempfile` dependency atomically reserves collision-resistant
attempt directories **inside persistent application data**, then relinquishes
automatic deletion. Paths support spaces/Unicode. Storage inside the source tree
is refused. A synced sibling `owner.json` is written before Git starts, recording
an ownership format marker, run ID, job index, registered repository, canonical
common directory, exact checkout path, base commit and execution mode. The random
attempt path distinguishes repeated attempts without recycling old paths.
Inspection checks the record against job metadata and verifies canonical checkout
and common Git identity; a path prefix alone is insufficient. Part 3.1B strengthens
these checks for recovery and adds exact internal removal, described below. No
broad pruning, repair, unlock or raw recursive removal is performed.

Admission reserves both concurrency slots before **Preparing isolated worktree**.
A preparing job holds its slots through checkout, agent execution and final result
inspection; there is no unscheduled preparation pool. Round-robin admission and
per-convoy/global limits remain unchanged. Repository waiters consume no slots;
an admitted isolated job awaiting the short administration mutex does consume a
slot. This bounds all parallel preparation. The common-directory keyed async
mutex serializes CodeConvoy worktree creation, then releases before agent spawn.
Unconfirmed Git cleanup marks administration unhealthy; uncertain worker cleanup
also quarantines the repository/capacity under the existing policy.

The scheduler distinguishes repository access from checkout execution. Two
isolated jobs sharing a common directory can execute simultaneously in independently
reserved paths. A Direct lease is exclusive against both modes for that common
Git identity, including a separately registered linked worktree. Different Git
repositories at genuinely nested source paths still conflict. This provides an
exclusive protection boundary for a future Apply operation, without implementing
Apply or a new scheduling system. External Git applications and agent-initiated
shared-administration commands are outside CodeConvoy's coordination.

All four backends receive a `Repository` with the isolated checkout path for
command construction; their own working-directory flags, model/options, permissions,
stdin, attachments, decoders and completion criteria are unchanged. Attachment
capability checks and queued-job revalidation still run before agent spawn; files
are never copied into the worktree. Activity/Raw output stay per job.

Preparation and agent processes use the shared process-tree cancellation machinery.
Git checkout has a bounded five-minute timeout and awaits cancellation/cleanup;
ordinary inspection retains its 20-second command timeout. Every preparation
inspection receives the job cancellation token and awaits subprocess cleanup. The
final result inspection uses a separate token so cancelled changes can be observed;
a five-second inspection deadline cancels and awaits any remaining Git command.
Inspection cannot overwrite an earlier unconfirmed-cleanup flag. The administration
mutex is held until Git cleanup is confirmed or marked unsafe. Cancellation before
admission creates nothing. Cancellation while waiting for administration is prompt;
cancellation during filesystem reservation awaits that small worker so the manifest
can be associated with the job. Partial creation is retained, never recursively
deleted. Unix detached-session limitations remain the same as for agent execution.

`WorktreeResult` records last-observed existence and optional changed/unchanged
state independently of success/failure/cancellation. Inspection failure remains
explicitly unknown. Its session-only observation flag resets on deserialization:
saved existence/change values are historical information, never proof of restart
recovery. Git checks both `diff <base> --` and `diff --cached <base> --` using
`--quiet` (0 equal, 1 changed; other exits are errors), `--no-ext-diff`,
`--no-textconv` and `--ignore-submodules=none`. Either comparison differing, or any
nonignored untracked status entry, means changed. This includes staged content
when the working file has been restored to base, binary edits, renames, removals,
agent commits and Git-reported submodule differences. Ignored files alone do not
count. Comparison/status failures are unknown, never unchanged.

Isolated Diff renders both comparisons against the fixed base and lists untracked
names without reading their contents into a patch. Source HEAD changes do not
change the baseline. Results remain live files, so external edits can change them
and missing base objects produce an error. All results remain on disk, including
unchanged, failed, cancelled and partial checkouts; useful ignored data is not
removed. History cleanup only removes eligible metadata; unresolved ownership
records and their convoys are protected.

Activity prefixes application lifecycle messages with `[CodeConvoy]`; backend
decoders and Raw output are unchanged. Job rows use Preparing/Running/terminal
status; details display isolated result observations separately. Task & settings
uses short base IDs and a collapsed storage location. Inspection failures retain
expandable diagnostics without persisting error chains. The existing quit dialog
counts preparation separately, Cancel has no job side effects, and confirmed quit
closes admission before cancellation and persists terminal events after draining.
Completed retained results are not active work.

Part 3.1B builds restart reconciliation and internal safe cleanup on these persisted
fields, as described below. Preparing/Running/Queued history becomes interrupted
on load using the existing mechanism. Apply/Discard, Retry and convoy-wide Review
remain outside Part 3.1. See the historical [Part 3.1A validation](worktree-validation.md)
and current [Part 3.1B validation](worktree-recovery-validation.md).

`tests/worktrees.rs` uses real Git, disposable sources and fixture processes for
all four backends. It covers mode compatibility/reuse, fixed HEAD and dirty-source
exclusion, concurrent same-repository checkouts, ownership/collisions, failed and
cancelled retention, limits, attachments, linked identity, fixed-base diff, hook
suppression, and cancellation during Git checkout including a filter descendant.
The feature-gated `v03_worktree_validation` example opens the native UI with a dirty
source, clean source, gated preparation source, task-only fixture templates and
external Unicode text attachment. It uses fixture executable settings only and
disposes its temporary workspace on exit. It does not probe authenticated agents
or touch ordinary application state.

### Part 3.1A-1 core validation record — 2026-10-07

Validated on native macOS with the local Git installation and fixture CLIs:

- `cargo fmt --check` — passed.
- `cargo clippy --all-targets --all-features -- -D warnings` — passed.
- `cargo test --all-features` — passed (171 top-level tests; 4 existing optional
  installed/authenticated CLI probes ignored; subprocess self-tests also passed).
- `cargo build --release` — passed.
- `python3 -m unittest discover -s packaging -p 'test_*.py'` — 16 passed.
- Native window: launched an isolated fixture against a dirty source; confirmed
  the checkout contained committed content and the original dirty file remained
  byte-for-byte unchanged. Task & settings displayed the isolated mode, base,
  path and retained changes.
- Native window: launched two gated isolated convoys against that same source;
  both showed Running concurrently. Their distinct checkouts contained independent
  `native-a` / `native-b` edits and separate input/output records. Both succeeded
  after their gates were released; the source remained unchanged.
- Native window: switched to Current working tree, reviewed and ran a direct
  fixture on a clean disposable source. It succeeded, wrote its expected input
  file in the source and recorded no owned worktree.
- Native isolated Diff showed `-committed base` / `+native-b` against the recorded
  commit. Reuse restored isolated mode; the compact two-pane layout remained usable.

Linux/Windows native execution and authenticated provider runs were not performed
for this step. The cross-platform fixture tests and existing packaging tests remain
available for those platforms; no release or publication was performed.

## v0.3 Part 3.1B: retained result recovery and internal cleanup

The existing `owner.json` layout and application schema version (1) are unchanged.
Jobs gain a defaulted `ResultAvailability`: Unchecked, Available, Missing, Stale,
Invalid, CleanupPending, CleanupFailed or Cleaned. The last Git observation remains
separate, as does agent success/failure/cancellation/interruption. Session-only
validation flags reset on load; saved availability is not fresh evidence. Old
v0.1/v0.2 jobs default to Direct and require no ownership metadata. No repository
contents, attachments, output, diagnostics or credentials are newly persisted.

Startup first marks unfinished jobs interrupted, then inspects every retained job
on the Tokio runtime. Checks are sequential, with individual results delivered to
the UI promptly; neither Git nor filesystem scanning runs in rendering. The UI
applies identity-matching replies and saves each received batch deliberately. It
adds one CodeConvoy recovery Activity entry per job/session or availability
transition, never backend progress, and leaves Raw output intact. Quitting cancels
inspection subprocesses and waits for lifecycle leases alongside execution; no
agent is resumed or rerun and no checkout is recreated.

Available requires all of: the trusted Store storage boundary; exact sibling
manifest equality; canonical source/common-directory identity; usable original
base object; exact detached Git registration with CodeConvoy's ownership lock;
matching private Git administration/backlink; and successful fixed-base Git change
inspection. Missing means checkout/storage is absent. Stale means Git/source/base
inspection cannot currently establish availability. Invalid means an ownership,
layout, link or identity mismatch. CleanupFailed preserves incomplete/failed
removal evidence. Diagnostics distinguish these without rewriting repository data.
**Refresh diff** repeats validation and uses the original commit; source commits
cannot alter that base. Retained directories remain editable, not immutable archives.

All changed, unchanged, failed, cancelled and interrupted results are retained.
No age-based, startup or automatic unchanged-result deletion occurs. Any unresolved
metadata (including missing/invalid/failed cleanup) pins its whole convoy outside
the ordinary 30-run cap, Remove and Clear history. A saved Cleaned result is also
protected until revalidated in this session. Direct history retains its old policy.
Reuse copies only task/settings/context/repository selection, never resources,
base commits, observations or process state.

`RunManager::lifecycle` grants scoped result-operation leases through the same
scheduler repository boundary. Inspections serialize with other lifecycle operations
and CodeConvoy Git administration for that common repository. Cleanup refuses any
conflicting active preparation, agent, quarantined process state or result operation;
a queued instance of the same job is also protected. While a lease exists, new jobs
for that repository wait without consuming a job slot. Other repositories and
round-robin order are unaffected. Inspection may read a running isolated checkout;
cleanup cannot. RAII releases leases, and unconfirmed process termination quarantines
the repository rather than declaring it safe. This coordinates CodeConvoy operations,
not external Git tools, agents' own Git administration, or other data-directory instances.

`Store::cleanup_result` is an **internal** primitive with no user-facing action.
It rejects nonterminal jobs, identity mismatches and any currently registered source
inside/around the target. It atomically saves CleanupPending before requesting the
exclusive lease. The worker verifies ownership, writes and syncs a sibling
`cleanup.json` intent bound to the exact manifest, then rechecks immediately before
`git worktree remove --force --force -- <exact native path>`. The two force flags
are Git's supported removal of an intentionally locked, potentially dirty retained
result. The command is only reachable through this explicit internal transaction;
startup never calls it. No reset, clean, stash, branch deletion or broad prune is used.
Afterward, absence of both checkout and exact Git registration is verified, the
journal is atomically marked completed, and the Store saves Cleaned. All failures
preserve metadata and save CleanupFailed. A still-valid checkout remains inspectable
(including its diff) after a failed cleanup, while that failure state remains visible. Already-missing directories can be resolved
only with verified ownership and a reachable source; an unregistered directory that
still contains files is preserved. Small owner/hooks/journal tombstones remain;
there is no raw recursive filesystem removal fallback.

The Store's root is canonicalized once. Recovery rejects traversal, paths outside
that exact root/attempt/tree layout, symlinked directories or records, and Windows
reparse points/junctions. Git porcelain `-z`, native path bytes on Unix and Unicode
paths on Windows avoid quoted/lossy identity comparisons; paths are direct process
arguments. Canonicalization accounts for drive/extended-path spelling and platform
temporary-directory aliases. Git-backed removal failures (including open Windows
files) remain failures; ownership does not grant permission to guess a fallback.
Repeated verification narrows substitution races, but this is not a security sandbox
against hostile processes with the same filesystem permissions. Users must not race
external filesystem/Git replacement with cleanup.

Crash boundaries deliberately preserve data:

- Reserved/created before a run save: startup scans immediate storage entries and
  reports unreferenced or malformed attempts without deleting/importing them.
- Saved before checkout creation finishes: missing or inconsistent partial state
  remains represented, with no recreation or automatic removal.
- Cleanup intent saved before removal: interrupted cleanup becomes CleanupFailed.
- Git removal completed before final state/journal update: a matching durable intent
  plus verified absence of checkout and registration establishes Cleaned on restart;
  reconciliation finishes the CodeConvoy journal before releasing history protection.
- Removal partially failed or source unavailable: preserve state and diagnose;
  absence of a source is never deletion permission.
- Reconciliation interrupted before save: the next startup simply inspects again.

The orphan scan follows no directory links and reports up to 100 suspicious entries
(with a `100+` indication at the bound). Completed tombstones with absent checkouts
are excluded. Ambiguous resources are never purged. A data-directory lock and atomic
state replacement remain the persistence boundary. Abrupt exits still cannot promise
termination of Unix descendants that escaped their managed process group; persisted
PIDs are not used to kill potentially unrelated processes. Part 3.1B did not implement agent continuation, storage management, Apply, user-facing Discard or convoy-wide Review; Part 3.2 adds the explicit result actions below.


## v0.3 Part 3.2: Review, Apply and Discard

`review` builds Git-derived summaries on background workers. `ui/review` renders
one compact convoy grid containing every explicit launched job, aggregate counts,
selection and existing inspection tabs. The UI pump allows one review request at
a time, caches errors too, and recomputes on completion, restart, explicit refresh
and result operations. Rendering never spawns Git. Direct rows describe the current
mutable working tree; their old runs have no immutable result or resolution action.
An unrelated pending review does not disable the selected result's actions; only
its own inspection or an active result operation gates those controls. Manager
leases still reject real repository conflicts.

`Snapshot` imports the live index's entries into a fresh temporary index, runs Git add/write-tree
against working files (including tracked ignored additions and nonignored untracked
files), and derives numstat against the isolated original base or Direct current HEAD.
It imports paths, modes and object IDs through `ls-files --stage -z` and
`update-index -z --index-info`, without copying stat-cache data. Every working file
is reread, including same-size edits whose timestamps match the live index.
The real index is never edited. Git may create unreachable blobs/tree objects;
there are no commits, refs, branches or persisted repository copies. Paths in stats
use NUL records. Renames are deliberately represented as delete/add (two paths),
binary files have no numeric line counts, and modes can change with zero lines.
Staged contents differing from both base and working image are counted as separate
alternatives rather than double-counted into the working-file line totals. Their
original index diff remains inspectable; Apply rejects them. Submodule/sparse or
otherwise unrepresentable summaries report unavailability rather than no changes.
The session cache includes a tree identity so an externally changed result must be
reviewed again before Apply. Full patches are not serialized.

`worktrees/apply` uses the existing recovery verifier and lifecycle lease; it has
no alternate ownership path. Destination registration, canonical root/common Git
identity, original base, clean porcelain state and freshly read contents, hidden index flags, supported
attributes and modes are checked. It builds a full-index binary patch against the
fixed base and invokes `git apply --check` before applying without `--index`,
`--reject` or `--3way`. It repeats ownership/source-image/destination checks while
holding the lease, then verifies the destination image, unchanged HEAD and clean
real index. Standard patch rejection is all-or-nothing; OS write failures or
cancellation are not a filesystem transaction. Every error after the write attempt
is marked uncertain, never reported as Applied or rolled back with reset/clean/stash.
There is no automatic retry. The original isolated worktree always survives Apply.
Pending/uncertain Apply also blocks Discard and all cleanup entry points, preserving
the destination warning and comparison evidence across restart. History remains
pinned even if cleanup observations say the copy is absent. There is no in-app
uncertainty-acknowledgement flow in this implementation.

Conservative exclusions: submodules/nested repositories, sparse/unmerged or
assume-unchanged entries, divergent staged alternatives, content conversion
attributes (filter, encoding, text, eol, ident), autocrlf conversion, unsupported
executable/symlink representation and output/patch limits (32 MiB patch, existing
bounded Git inspection limits). Windows Git path conversion is reused. Unix
symlink/executable tests run only where supported; inability to create a symlink or
an unexpected OS write error is conservatively uncertain. External programs can
still race between checks; CodeConvoy leases only coordinate CodeConvoy operations.

Both Apply and Discard acquire exclusive maintenance access against all Direct and
isolated preparation/execution in the same common repository or a nested source,
and against other result operations, then hold the existing administration mutex.
They fail promptly when busy and consume no agent execution slot. Unrelated
repositories continue; pending jobs wait and undergo their normal baseline checks.
Discard removes only its selected checkout, preserving peer isolated worktrees.
Cancellation awaits owned process cleanup; uncertain process cleanup quarantines
the lease through the existing manager mechanism. Quit waits for the result
completion message to merge and save before exiting.

`persistence/results` splits the Store transaction into durable intent, background
operation, and completion merged into the current job (never a stale whole-state
snapshot). Internal cleanup now shares that transaction. The additive persisted
`ResultResolution` distinguishes Unresolved, ApplyPending, Applied, DiscardPending
and Discarded independently of `ResultAvailability` and agent status. Missing
fields default to Unresolved for Part 3.1 state. Only resolution and its timestamp
are newly persisted; no source text, patch, Activity or Raw logs are saved.

Applied stays Applied through reconciliation even if the copy becomes missing.
Applied copies retain Diff and ownership/history protection until the user confirms
**Clean up retained copy**. They are resolved, not unresolved results. Discard uses
Part 3.1's exact Git cleanup and journal, with safe-default confirmation. Failed
cleanup remains DiscardPending/CleanupFailed and can be retried. Completed cleanup
plus saved Discard intent recovers as Discarded after a crash. An ApplyPending
record after a crash remains uncertain (even if the transfer actually finished);
it never replays the patch or claims success from a later read. Retained metadata
cannot vanish through the history cap, Remove or Clear while cleanup is pending.
After verified cleanup, history removal again only removes application metadata.

Lifecycle Activity uses `[CodeConvoy]` prefixes; Raw remains unchanged. Native
keyboard/window scenarios and checks are recorded in
[Part 3.2 validation](review-apply-discard-validation.md). The continuation features added afterward are described below.


## v0.3 Part 3.3: Independent retries and follow-up drafts

`continuation` builds requests from immutable snapshots, copying only task/settings
and canonical registered source identities. It never copies jobs, logs or worktree
resources. `Job::retryable` includes terminal failed, cancelled and recovered
interrupted jobs; ordinary successes have no Retry action.

The existing scheduler identifies jobs by `(run_id, job_index)` and runs have fixed
repository snapshots. A retry is therefore a fresh one-repository convoy, with a
new run ID and job index 0. Optional `Provenance::Retry` records source run/index and
an ordinal (2 for the first retry, incremented when retrying a retry). Repeated
retries of the same original are sibling attempts with distinct run IDs, both
labelled attempt 2. There is no ancestry traversal, mutable attempt tree, hidden
session continuation or dependency scheduling. The source convoy's other jobs are
never duplicated. Provenance cannot change scheduling or result ownership.

Retry copies the original task and attachment references. Background preparation
checks current registration/Git, revalidates file identity/hash/readability and
backend support, then probes the original executable and builds the ordinary
`PreparedRun`. Normal review remains explicit, including dirty-tree acknowledgment.
Launch checks that registrations still exist, saves a new snapshot, adds a
`[CodeConvoy]` provenance Activity entry, and admits it through `RunManager::start`.
The current draft is preserved. Direct admission rechecks the newly reviewed
working tree; isolated preparation uses the newly captured committed HEAD and a
fresh owned path. Execution-time context revalidation, leases, fair admission,
limits, cancellation and shutdown remain shared. The prior result is never resolved
by Retry, even when the new attempt succeeds.

Review's follow-up checkbox set is separate from the inspected row and resets when
switching convoys. Every result can be selected, regardless of execution outcome,
resolution or missing retained worktree. `FollowUp` sorts/deduplicates source paths,
matches current registrations, checks availability in background and rechecks
registration when completing. Unavailable/unregistered paths produce visible
omissions. The editor is disabled during this short draft replacement operation.
Completion copies backend/options, mode and per-convoy concurrency, selects the
registered sources and focuses a blank task with no attachments. Groups, templates
and the global limit are untouched. Nothing is launched or registered implicitly.

Optional `Provenance::FollowUp` stores the source convoy and selected job indices on
the draft, then freezes them into the new run at launch. Both provenance forms are
small informational metadata; source history removal is permitted by the existing
ownership policy and cannot invalidate descendants. History, Task & settings and
the draft show plain origin text; the source-navigation button appears only while
the source still exists. Review selection itself is session-local, like ordinary
repository selection. Reuse convoy clears draft provenance and retains its existing
explicit full-task reuse behavior. Templates only edit text.

All fields are additive serde defaults. Old runs stay Direct without isolated
resources or artificial attempts. Output remains session-only and is never copied
or newly persisted. Appearance now saves System/Dark/Light; pre-existing versions
stored overrides only in memory, so no unsaved override can be recovered after an
upgrade. Missing appearance uses System. No dependencies or backend protocols change.
See [Part 3.3 validation and v0.3 readiness](retry-followup-validation.md).

## Post-v0.3: convoy and global Discard

`persistence/bulk_discard` snapshots the terminal convoys' unresolved isolated
result identities for confirmation. Applied/Discarded results and whole active
convoys are excluded. Uncertain Apply and unverifiable ownership remain visible
failures, never deletion permission. Before each target, the coordinator rechecks
terminal convoy state and exact identity, then calls the existing
`begin_result_operation(Discard)`, background `Operation::execute` and
`finish_result_operation`. There is no second Git deletion or locking path.
An outcome-save failure restores in-memory pending intent so history stays pinned.

`ui/bulk_discard` drives one target at a time through the ordinary result message
channel. It suspends new review requests during the batch and waits for an existing
review to finish. Completion merges into current state before advancing; unrelated
agent work continues under existing manager leases. A per-result failure does not
undo successful discards or stop other targets. A session summary shows counts and
diagnostics, with one Activity start/completion entry per affected convoy; Raw is
unchanged. Confirmed quit stops remaining targets and waits for the in-flight
operation's cleanup and state merge. A batch is never replayed after restart.

Confirmation freezes targets rather than expanding a global operation when another
convoy finishes. Cancel has default focus and Escape dismisses it. Discard does not
remove history; existing history protection stays authoritative, including Applied
copies still awaiting their separate explicit cleanup. No persistence schema,
backend contract, dependency or scheduler policy changes. See
[bulk discard validation](bulk-discard-validation.md).
