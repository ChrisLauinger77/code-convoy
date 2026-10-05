# CodeConvoy

A local, native desktop task runner for coding-agent CLIs.

**Write one task → choose an agent → select local Git repositories → run with a concurrency limit → inspect each result.**

CodeConvoy 0.1 is an initial Rust/egui implementation. Codex and GitHub Copilot CLI are independent, implemented backends. Both have been user-verified end-to-end on Linux with two real repositories running concurrently. Claude Code and OpenCode are planned next, in that order. There is no web frontend, provider API integration, built-in terminal, or cloud service.

## Build and run

Requirements:

- Rust 1.88 or later and Cargo.
- Git on `PATH`.
- A working desktop graphics environment.
- The CLI for the agent you want to use, already installed and authenticated using its own login flow. Its executable must be on the desktop application's `PATH`, or supply its absolute path in the UI. Neither CLI is required for CodeConvoy to start; **Check CLI** checks only the selected agent.

On Debian/GNOME, the usual native development prerequisites are:

```sh
sudo apt install build-essential pkg-config libx11-dev libxkbcommon-dev libwayland-dev libgl1-mesa-dev
cargo run --locked
```

For a distributable local binary:

```sh
cargo build --locked --release
```

The executable is `target/release/codeconvoy` (`codeconvoy.exe` on Windows). Installers and desktop integration are not yet provided. macOS and Windows are architectural targets; they need native runtime verification. A CI matrix is included for all three platforms.

## First run

1. Enter a task and select Codex or GitHub Copilot CLI.
2. Configure the settings shown for that backend. Codex offers **Read-only** or **Workspace-write** sandboxes. Copilot offers tool approvals and temporary-directory access; these are not Codex sandbox modes. Model and reasoning support depends on the selected CLI/model.
3. Register existing repository **root directories**. Select the checkboxes for this task.
4. Set **This convoy** (its job limit) and **Global job limit** (shared by every convoy), then **Run Convoy**.
5. Review branches and existing changes. Dirty working trees require acknowledgment before **Start convoy**.
6. Select a job to read its live output. Use **Stop** for one job or **Stop Convoy** for the selected convoy. Other convoys continue.
7. Open **Diff** and use **Refresh diff** to load the staged and unstaged changes. Untracked filenames are listed, but their contents are not included in Git's diff.

**NEW CONVOY is a draft.** Run Convoy captures its prompt, selected backend/options, and repositories for review. Start convoy saves an independent run snapshot and queues its jobs. The draft stays populated: immediately edit it and launch another convoy, including one using a different agent. Later edits never alter existing runs. Each backend retains its own draft preferences.

Each convoy has its own concurrency limit. **Global job limit** defaults to **4**, is saved as a user preference, and caps agent jobs across all convoys. Both limits must allow a start. Changing the global limit affects queued work; lowering it lets existing jobs finish before starting more. Eligible convoys take turns receiving slots, and a repository waiter does not occupy a slot.

CodeConvoy serializes jobs targeting the same canonical repository path (including overlapping parent/child paths). The next job waits, then rechecks Git state. If an earlier convoy changed branch, HEAD, or status since review, the waiting job fails safely and needs a fresh review. These are application-level leases, not locks against external tools or separate instances with different data directories.

Use the run selector to switch between active and historical convoys without affecting execution. It shows agent, state, finished/total progress, and a task summary; selected-run details include elapsed time and its original settings. Any failed job makes the completed convoy **Failed**; otherwise any cancelled job makes it **Cancelled**. All jobs must succeed for **Succeeded**. While work remains, state is Running (a job is running) or Queued.

**Stop Convoy** cancels only the selected convoy's running and queued jobs. Individual Stop remains available. **All convoys → Stop All Convoys** is the distinct emergency stop. Closing CodeConvoy stops all convoys; restarting marks unfinished jobs cancelled/interrupted and never resumes agent processes. Partial repository edits remain.

History retains all active convoys plus the latest 30 completed convoys, including snapshots, repository paths, initial Git state, timestamps, exit codes, and statuses. Logs remain session-only. Existing version-1 state loads with the default global limit. **Use this task again** copies a historical task into the draft.

The execution controls stay visible while the task and repository pane scrolls. **Appearance** follows the system theme by default and offers dark/light overrides for the current session. Output and diffs render only visible lines, with Copy actions for the full retained text. Diff headers, additions, and removals have distinct styling; wide lines scroll horizontally.

Keyboard navigation uses egui's Tab / Shift+Tab focus traversal; buttons and checkboxes support keyboard activation. Path entry supports paste. No file chooser is required.

## Codex contract

The installed CLI interface was checked against **codex-cli 0.160.0**. Detection requires `--no-daemon` so cancellation can target a dedicated process. Older CLIs without this flag are rejected with an explanation. Model and reasoning availability remains the CLI's responsibility.

Invocation is equivalent to:

```sh
codex --no-daemon --ask-for-approval never exec \
  --json --color never --ephemeral --sandbox read-only -
```

The application sends the prompt through stdin and sets the working directory to the selected repository. Optional model and reasoning overrides are separate arguments. It never constructs a shell command string, manages API keys, or reads CLI credential files. Sandbox bypass is not exposed. Existing CLI configuration, repository instructions, hooks, tools, authentication, and network behavior remain under Codex's control. `--ephemeral` disables Codex session history for these invocations; resume is deferred.

See the official [Codex noninteractive documentation](https://learn.chatgpt.com/docs/non-interactive-mode). The user has successfully verified authenticated execution against two real repositories concurrently, including independent changes, output capture, and Git diff inspection. This behavior and the Codex invocation are preserved when adding Copilot.

## Copilot contract

Copilot has been user-verified end-to-end on Linux with two real repositories running concurrently. Its installed help/version output and generated CLI flags have also been checked locally.

This installation has two versions: the normal launcher reports **1.0.91**, while `copilot --no-auto-update --version` reports the bundled **1.0.65**. CodeConvoy passes `--no-auto-update` to both detection and execution, so they consistently use the bundled executable and do not initiate CLI updates. **Check CLI** reports that effective version. If your bundled CLI is too old, it reports the missing capability; updating Copilot is a separate user action.

The default command shape is:

```sh
copilot --no-auto-update --no-ask-user --no-color --plain-diff \
  --output-format text --stream on --no-remote --no-remote-export \
  --allow-tool=write --deny-tool=shell
```

CodeConvoy sets the selected repository as the process working directory and writes the exact task to **stdin**, then closes it. Copilot documents piped input as noninteractive prompt mode. There is no shell pipeline in the implementation, no `exec` subcommand, and no `-p -` argument: that would be a literal prompt in Copilot. Each job starts a fresh local invocation; there is no resume, remote connection, fleet mode, session sharing, or worktree creation.

Available controls:

| Control | CLI behavior |
| --- | --- |
| Executable | `copilot` on PATH or an absolute executable path |
| Model | Blank uses the CLI default; otherwise `--model VALUE` (including `auto`) |
| Reasoning effort | Blank uses the CLI default; otherwise `--reasoning-effort none/low/medium/high/xhigh/max`, as supported by bundled 1.0.65 |
| File edits; shell denied (default) | `--allow-tool=write --deny-tool=shell` |
| Existing CLI approvals | No tool-approval override; unapproved actions may fail in unattended mode |
| Allow all tools, including shell | `--allow-all-tools`; this is an explicit permission to run shell tools automatically |
| Temporary directory | CLI default access, or `--disallow-temp-dir` |

Copilot path checks and its existing configured permissions apply. CodeConvoy never enables `--allow-all-paths`, `--allow-all-urls`, or `--yolo`. It removes an inherited `COPILOT_ALLOW_ALL` blanket grant from the child environment so it does not bypass the UI choice. Authentication variables, `COPILOT_HOME`, and the user's configuration/authentication files are not changed by CodeConvoy. Existing MCP servers, instructions, hooks, and other CLI settings remain the CLI's responsibility. Tool approvals are **not an OS sandbox**; file-edit mode can edit files, and all-tools mode can run shell commands with the user's permissions.

Copilot streams plain stdout and labelled stderr directly to the job log, including partial lines. CodeConvoy does not normalize or pretty-print Copilot events. Exit zero means the CLI succeeded; any nonzero or signal exit is a failure, and a CodeConvoy-requested stop is cancelled. This does not prove the agent fulfilled the task: review its response and diff, particularly when permissions denied an action. Codex continues to use its existing JSONL completion criteria. Copilot may keep its own session history/logs; no ephemeral flag was exposed by the inspected CLI. CodeConvoy itself still persists only task/run metadata and preferences.

References: [GitHub's programmatic execution guide](https://docs.github.com/en/copilot/how-tos/copilot-cli/automate-copilot-cli/run-cli-programmatically) and the installed `copilot --no-auto-update --help` / `copilot --no-auto-update help permissions`. The captured help used by tests is in `tests/fixtures/copilot-1.0.65-help.txt`.

### Safe manual Copilot E2E test

Use two disposable repositories, each containing a **tracked**, clean `CODECONVOY_COPILOT_SMOKE.md` with a placeholder heading and a small README. Select Copilot, **File edits; shell denied**, concurrency **2**, and this task:

> Edit only CODECONVOY_COPILOT_SMOKE.md. Replace its placeholder with a heading “CodeConvoy Copilot smoke test”, the current repository's directory name, and a short summary of its README. Do not create or modify any other files, run shell commands, or perform Git operations. Report which file you edited.

Verify both jobs succeed, each output identifies its own repository, and each current Git diff contains only the expected tracked file. Using tracked files makes the result visible in CodeConvoy's staged/unstaged diff viewer; untracked files would only appear in its status list. This test is for the user to run manually.

### Manual multi-convoy E2E check

Use four disposable, clean repositories with tracked smoke-test files. Set global limit **2**. Launch Codex with **Workspace-write** in the first pair with per-convoy limit **1**, asking it to edit only the smoke-test file. Immediately change the draft to Copilot with **File edits; shell denied**, select the other pair, and launch with limit **1** and a different smoke-test instruction. Switch between runs: confirm their original agent/settings, independent logs/diffs, progress, and at most two running jobs. Stop one convoy while it is active and verify the other continues. Inspect retained partial edits.

For repository queuing, launch a second convoy against a repository still in use by the first. It must wait. If the first changes its Git status, expect the waiting job's safety recheck to fail before an agent starts; review the new state and launch a fresh convoy. No reset, stash, branch change, commit, or push is part of this check.

## Repository safety and data

CodeConvoy's Git operations only inspect working trees. Registration/removal never clones, deletes, resets, stashes, checks out, commits, or pushes. External diff/textconv and filesystem-monitor commands are disabled for inspection. Duplicate and nested repository selections are rejected. Each queued job rechecks branch, HEAD, and porcelain status immediately before starting; detected state changes fail that job without affecting other jobs.

Codex Workspace-write and Copilot file-write/tool approvals permit the **agent** to modify files. CodeConvoy is an orchestrator, not an extra security sandbox. Stop terminates processes and retains partial edits; it cannot roll changes back. Review the diff before committing. Git checks are not filesystem locks: another application can still edit a repository, and changes within an already-dirty file may leave the same porcelain status.

State is saved atomically in the conventional per-user application data directory, available from the footer’s Local session tooltip:

- Linux: `$XDG_DATA_HOME/codeconvoy`, normally `~/.local/share/codeconvoy`.
- macOS: `~/Library/Application Support/CodeConvoy`.
- Windows: `%LOCALAPPDATA%\CodeConvoy\data`.

A lock prevents two instances sharing the same state directory. On Unix the directory is owner-only and state files are created with mode `0600`. Invalid or newer-format state fails startup with an actionable error and is preserved. Unfinished saved jobs are marked cancelled/interrupted at next startup; they are never silently resumed.

**Prompts and metadata are saved, so do not paste credentials into tasks.** CLI stdout/stderr, diagnostic messages, and diffs are never persisted by CodeConvoy. Output is bounded to the latest 512 KiB per job (32 MiB total, oldest job logs evicted first) and retained only for the application session. The UI reports truncated/dropped output. Diffs are live views, including pre-existing changes; they are not historical patch snapshots. The agent CLIs may maintain their own operational logs according to their configuration.

## Development

```sh
cargo fmt --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-features
```

The `test-support` feature builds a deterministic local test executable. It is never used as an application backend and requires no account or network. Integration tests use temporary Git repositories and handshake-controlled fixture processes to check simultaneous Codex/Copilot convoys, both concurrency limits, draft snapshots, repository leases and release, cancellation/failure isolation, shutdown, process descendants, output capture, and Git rechecks. Pure scheduler tests check round-robin fairness without timing dependencies. Core tests cover both backends' command arguments, output handling, exit interpretation, backend preferences and old-state compatibility, persistence, history recovery, and Git inspection. Copilot integration tests use its real backend with a local fixture executable, not a provider. An optional installed-CLI check runs only help/version commands:

```sh
cargo test --all-features --test copilot installed_copilot_accepts_the_exact_command_flags -- --ignored --nocapture
```

Read [the architecture](docs/architecture.md) for module boundaries and implementation tradeoffs. Claude Code and then OpenCode are the next planned backends. Before a public release, verify native behavior on macOS/Windows.

## License

MIT. See [LICENSE](LICENSE).
