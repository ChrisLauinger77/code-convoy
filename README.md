# CodeConvoy

A local, native desktop task runner for coding-agent CLIs.

**Write one task → choose an agent → select local Git repositories → run with a concurrency limit → inspect each result.**

CodeConvoy 0.1 is a native Rust/egui implementation with four independent coding-agent backends. There is no web frontend, provider API integration, built-in terminal, or cloud service.

| Backend | Status | Authenticated E2E status |
| --- | --- | --- |
| Codex CLI | Supported | User-verified on Linux with two concurrent repositories |
| GitHub Copilot CLI | Supported | User-verified on Linux with two concurrent repositories |
| OpenCode | Supported | Unverified; official-source contract and deterministic fixtures tested, CLI unavailable locally |
| Claude Code | Supported | Unverified; official documentation/source and deterministic fixtures tested, CLI unavailable locally |

## Downloads and installation

The `v0.1.0` release workflow produces the following assets on the
[GitHub Releases page](https://github.com/ChrisLauinger77/code-convoy/releases).
Packages are available after the maintainer publishes the release tag.

| Platform | Package | Installation |
| --- | --- | --- |
| Linux x86-64, Debian/Ubuntu | `code-convoy_0.1.0_amd64.deb` | `sudo apt install ./code-convoy_0.1.0_amd64.deb` |
| Linux x86-64, Fedora/RPM | `code-convoy-0.1.0-1.x86_64.rpm` | `sudo dnf install ./code-convoy-0.1.0-1.x86_64.rpm` |
| Linux x86-64, portable | `CodeConvoy-0.1.0-x86_64.AppImage` | Make executable and run; no installation or sandbox |
| Windows x86-64 | `CodeConvoy-0.1.0-windows-x86_64-setup.exe` | Run the per-user installer; Start Menu and uninstall support included |
| Windows x86-64, portable | `CodeConvoy-0.1.0-windows-x86_64.zip` | Extract and run `CodeConvoy.exe` |
| macOS, Apple Silicon and Intel | `CodeConvoy-0.1.0-macos-universal.dmg` | Drag CodeConvoy.app onto Applications |

`SHA256SUMS` covers the six final downloads. On Linux use `sha256sum -c
SHA256SUMS` with the downloaded files; on macOS use `shasum -a 256`; on Windows
use `Get-FileHash -Algorithm SHA256` and compare the corresponding entry.

Linux packages target glibc 2.35 or later (Ubuntu 22.04 build baseline), with a
working X11 or Wayland desktop and OpenGL/EGL driver. The folder picker needs
`libdbus` and an XDG Desktop Portal with a FileChooser-capable backend appropriate
to your desktop; `zenity` is its fallback. Manual path entry remains available.
Git and agent CLIs run from the host. If AppImage mounting is unavailable, run
`./CodeConvoy-0.1.0-x86_64.AppImage --appimage-extract-and-run`.

The macOS app targets macOS 11 or later and is **ad-hoc signed, not Developer ID
signed or notarized**. Gatekeeper may block the first launch. After verifying the
download and attempting to open it, use **System Settings → Privacy & Security →
Open Anyway** for CodeConvoy if you trust the source. See
[Apple's per-application guidance](https://support.apple.com/en-us/102445).
Do not disable Gatekeeper globally. Windows packages are not Authenticode signed;
Windows may show an unknown-publisher warning.

No package installs Git, coding-agent CLIs, credentials, services, or an updater.
Install and authenticate the agents separately. Desktop launchers may have a
different `PATH` from your terminal, particularly Finder on macOS; provide the
agent's absolute executable path in CodeConvoy if needed, and ensure Git is on the
application's `PATH`. The portable ZIP uses the same per-user application data
location as the installed Windows app; its executable is portable, its state is
not stored beside it.

See [release procedure and packaging design](docs/releasing.md) and the
[release validation record](docs/release-validation.md) for tested behavior and
remaining platform checks.

## Build and run

Requirements:

- Rust 1.95 or later and Cargo.
- Git on `PATH`.
- A working desktop graphics environment.
- The CLI for the agent you want to use, already installed and authenticated using its own login flow. Its executable must be on the desktop application's `PATH`, or supply its absolute path in the UI. No agent CLI is required for CodeConvoy to start; **Check CLI** checks only the selected agent.

On Debian/Ubuntu, the native source-build prerequisites are:

```sh
sudo apt install build-essential pkg-config libx11-dev libxkbcommon-dev libwayland-dev libgl1-mesa-dev
cargo run --locked
```

For a distributable local binary:

```sh
cargo build --locked --release
```

The executable is `target/release/codeconvoy` (`codeconvoy.exe` on Windows).
Windows source builds need the MSVC C++ build tools and Windows SDK; macOS needs
Xcode Command Line Tools. Packaging additionally needs the tools listed in the
[release procedure](docs/releasing.md); those are not application runtime
requirements. Native macOS UI and Universal DMG checks have passed; authenticated
macOS execution and Windows interactive runtime verification remain outstanding.

**Browse…** uses the desktop's folder picker: an XDG Desktop Portal on Linux, an NSOpenPanel sheet on macOS, and the native Windows file dialog in folder mode. Linux needs a FileChooser-capable portal backend (such as GTK/GNOME/KDE) and runtime `libdbus`; `zenity` is the fallback when the portal is unavailable. Manual path entry remains available. See [folder-picker implementation and validation](docs/folder-picker-validation.md).

## First run

1. Enter a task and select Codex, GitHub Copilot CLI, OpenCode, or Claude Code.
2. Configure the settings shown for that backend. Codex offers **Read-only** or **Workspace-write** sandboxes. Copilot offers tool approvals and temporary-directory access; these are not Codex sandbox modes. OpenCode offers its own model, agent, variant and permission controls. Claude offers model, effort, turn limit and tool permissions. Model and reasoning support depends on the selected CLI/model.
3. Enter an existing repository **root directory**, or use **Browse…** to fill the path field, then choose **Add repository**. Cancelling the picker preserves the field; choosing a folder never registers it automatically. Select the repository checkboxes for this task.
4. Set **This convoy** (its job limit) and **Global job limit** (shared by every convoy), then **Run Convoy**.
5. Review branches and existing changes. Dirty working trees require acknowledgment before **Start convoy**.
6. Select a job to read its live output. Use **Stop** for one job or **Stop Convoy** for the selected convoy. Other convoys continue.
7. Open **Diff** and use **Refresh diff** to load the staged and unstaged changes. Untracked filenames are listed, but their contents are not included in Git's diff.

**NEW CONVOY is a draft.** Run Convoy captures its prompt, selected backend/options, and repositories for review. Start convoy saves an independent run snapshot and queues its jobs. After an accepted launch, the prompt and repository checkboxes clear for your next task. Backend settings, both concurrency limits, and registered repositories stay in place. A failed launch leaves the draft intact. Later edits never alter existing runs. Each backend retains its own draft preferences.

Each convoy has its own concurrency limit. **Global job limit** defaults to **4**, is saved as a user preference, and caps agent jobs across all convoys. Both limits must allow a start. Changing the global limit affects queued work; lowering it lets existing jobs finish before starting more. Eligible convoys take turns receiving slots, and a repository waiter does not occupy a slot.

CodeConvoy serializes jobs targeting the same canonical repository path (including overlapping parent/child paths). The next job waits, then rechecks Git state. If an earlier convoy changed branch, HEAD, or status since review, the waiting job fails safely and needs a fresh review. These are application-level leases, not locks against external tools or separate instances with different data directories.

Use the bounded, scrollable run selector's **ACTIVE** and **HISTORY** groups to switch convoys without affecting execution. It shows convoy number, agent, state, finished/total progress, duration, and a task summary; selected-run details include elapsed time and its original settings. Any failed job makes the completed convoy **Failed**; otherwise any cancelled job makes it **Cancelled** (or **Interrupted** when recovered after application exit). All jobs must succeed for **Succeeded**. While work remains, state is Running (a job is running) or Queued, and the convoy stays in ACTIVE.

**Stop Convoy** cancels only the selected convoy's running and queued jobs. **Stop job** cancels one job. **All convoys → Stop All Convoys** is the distinct emergency stop. Closing CodeConvoy stops all convoys; restarting marks unfinished jobs cancelled/interrupted and never resumes agent processes. Partial repository edits remain.

History retains all active convoys plus the latest 30 completed convoys, including snapshots, repository paths, initial Git state, timestamps, exit codes, and statuses. Logs remain session-only. Existing version-1 state loads with the default global limit. **History cleanup → Remove convoy #… from history** removes the selected terminal convoy; **History cleanup → Clear history** removes all terminal convoys, including failed, cancelled, and interrupted runs. These actions save immediately and only remove CodeConvoy metadata and session output. Repository files, Git changes, registered repositories, and active convoys are untouched.

**Reuse convoy** copies the selected convoy's prompt, backend settings, per-convoy limit, and still-registered repository selection into NEW CONVOY, replacing the current draft. It leaves the global limit unchanged and does not start jobs. Unregistered repositories are skipped with a message; add them again if needed. Review the draft and use Run Convoy for a fresh Git check.

The execution controls stay visible while the task and repository pane scrolls. **Appearance** follows the system theme by default and offers dark/light overrides for the current session. Output and diffs render only visible lines, with Copy actions for the full retained text. Diff headers, additions, and removals have distinct styling; wide lines scroll horizontally.

Keyboard navigation uses egui's Tab / Shift+Tab focus traversal, with task, backend settings, repositories, then execution controls in order. Use arrows within menus and Enter / Space to activate controls. Numeric fields become editable when focused. Launch review and About keep focus inside their dialog; Escape closes them. Path entry supports paste. No file chooser is required.

**Select all / Select none** changes repository selection without hiding Git state or skipping launch review. **Check CLI** reports Unchecked, Available, Unavailable (executable not found), or Invalid configuration for the selected settings only. Detailed diagnostics can be expanded and copied. Availability confirms CLI compatibility, not authentication or model access.

**Task & settings** shows friendly option labels, repository paths, the original per-convoy limit, and UTC timestamps. The live global limit is not part of a historical snapshot. **About**, in the footer, shows version, build-time base Git commit when available, project link, and license without a network request. See the [second UX pass validation](docs/ui-ux-validation.md) for native checks and remaining platform limitations.

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

## OpenCode contract

The backend uses the official [OpenCode CLI](https://opencode.ai/docs/cli/#run), checked against the released [v1.18.34 source](https://github.com/anomalyco/opencode/blob/v1.18.34/packages/opencode/src/cli/cmd/run.ts). No OpenCode installation was available on the development Mac, and no authenticated task was run. [Validation details and limitations](docs/opencode-validation.md) distinguish fixtures, native UI checks, and outstanding Linux/E2E checks.

Default invocation:

```text
opencode run --format json --dir <absolute-repository-path>
```

CodeConvoy spawns the executable directly with separate arguments, sets its working directory to the selected repository, writes the exact prompt to stdin and closes the pipe. No shell interpolation, positional prompt, `-` prompt sentinel, shared server, or session resume is used. Each job owns a fresh local process. Inherited `PWD` and `GIT_*` overrides are removed so repository discovery stays with that job.

| Control | Behavior |
| --- | --- |
| Executable | `opencode` on PATH or an absolute path |
| Model | **OpenCode default** when blank; otherwise `--model=provider/model`. Use `opencode models` separately to discover configured models. No fixed catalog or provider entitlement checks. |
| Agent | OpenCode default when blank; otherwise `--agent=NAME`. Choose a primary agent from `opencode agent list`. |
| Variant | OpenCode default when blank; otherwise `--variant=NAME`. Provider/model-specific setting, including reasoning effort; not a shared Codex/Copilot scale. |
| Permissions | **Existing rules; reject asks** by default. **Auto-approve; keep denies** adds `--auto`. |

OpenCode owns login, credentials, providers, model availability and configuration. CodeConvoy does not ask for or store keys, inspect credential files, edit OpenCode configuration, or create accounts. Existing configuration/environment, repository instructions, plugins, MCP tools and session retention remain under OpenCode's control. This includes automatic sharing if the user has configured it; CodeConvoy does not add `--share` or disable the user's configuration.

[OpenCode permissions](https://opencode.ai/docs/permissions/) are tool rules, **not an OS sandbox**. Existing rules may already permit shell commands and file writes. In unattended run mode, requests that require approval are rejected; auto mode approves such requests while explicit denies remain enforced. There is no interactive approval UI. `--thinking` only affects displayed thinking blocks, so it is not offered as a reasoning control. Agent-specific rules still apply. OpenCode may warn and fall back to its default agent when a requested agent is unknown or is a subagent; review the output.

`Check CLI` inspects `run --help` for the required capabilities and reports `--version` where available. No version-number ordering or startup detection is imposed. Older versions missing a required flag are rejected with an actionable message. Authentication is exercised only by an actual job.

OpenCode's output handler reads its own JSON event stream. Assistant text is readable; tool records, step metadata, errors and unknown events stay visible as JSON, and stderr is labelled. A job succeeds only with exit zero, a final `step_finish` whose reason is `stop`, no session error events, and no malformed/oversized JSON records. Tool errors can be recovered by the agent and remain visible; success does not prove the requested change was made. Token-limit stops, missing completion and nonzero/signal exits fail conservatively. Stop uses the existing process-tree cancellation and retains partial edits. Output remains session-only; no Activity view is introduced.

OpenCode settings participate in draft persistence, launch snapshots, history and **Reuse convoy**. Old Codex/Copilot version-1 state still loads. Concurrency, round-robin scheduling, canonical repository locks, dirty-tree review and baseline revalidation are unchanged.

### Safe manual OpenCode E2E test

1. Independently install/authenticate OpenCode and check `opencode --version`, `opencode run --help`, `opencode models` and `opencode agent list`. Use an existing configured primary agent with file-edit permission; inspect its permissions yourself.
2. Prepare two **disposable**, clean Git repositories. Each should contain a tracked README and a tracked `CODECONVOY_OPENCODE_SMOKE.md` with a placeholder heading. Do not use important working repositories.
3. Select **OpenCode**, keep model/variant at **OpenCode default**, choose the verified primary agent, keep **Existing rules; reject asks**, and set both concurrency limits to **2**. Run **Check CLI**, select both repositories, then review/start this task:

   > Edit only CODECONVOY_OPENCODE_SMOKE.md. Replace its placeholder with a heading “CodeConvoy OpenCode smoke test”, this repository's directory name, and a short summary of its README. Do not modify any other files, run shell commands, or perform Git operations. Report the file you edited. If permissions prevent editing, report that and stop.

4. Verify both jobs succeed, each response identifies its own repository, and each Diff contains only its tracked smoke-test file. Check for denied tools or fallback-agent warnings even on success.
5. Use **Reuse convoy**: confirm settings and both repository selections are restored without execution. For cancellation, start a separate read-only inspection task in these disposable repositories and stop it while active; inspect retained edits and remaining processes. A real completed authenticated task is required before updating OpenCode's E2E status.

Optional help-only compatibility probe (never invokes a model):

```sh
cargo test --all-features --test opencode installed_opencode_accepts_generated_arguments -- --ignored --nocapture
```

## Claude Code contract

Claude is the fourth independent backend. It uses the documented [print interface](https://code.claude.com/docs/en/headless) and [CLI flags](https://code.claude.com/docs/en/cli-reference). No Claude installation or authenticated execution was available on the development Mac. See [validation evidence and limitations](docs/claude-validation.md).

Default invocation:

```text
claude --print --input-format text --output-format stream-json --verbose \
  --no-session-persistence --permission-mode dontAsk
```

CodeConvoy passes separate arguments directly, sets the selected repository as the working directory, writes the exact task bytes to stdin and closes it. It removes inherited `GIT_*`/`PWD` overrides and inherits Claude authentication, provider environment and configuration. Each job is a fresh invocation; CodeConvoy does not log in, request keys, change Claude settings, attach to a server or resume a session.

| Control | Behavior |
| --- | --- |
| Executable | `claude` on PATH or an absolute executable path; native `claude.exe` on Windows |
| Model | **Claude default** when blank; otherwise `--model=ALIAS_OR_ID`, without a fixed model catalog |
| Effort | Claude default, or `--effort=low/medium/high/xhigh/max`; CLI/model support applies |
| Permissions | **Existing approvals; deny asks** (`dontAsk`, default), or **Accept edits / file commands** (`acceptEdits`) |
| Max turns | Blank preserves Claude's default; otherwise `--max-turns=POSITIVE_INTEGER`. Reaching it fails completion. This is not a time limit. |

These are [Claude tool permissions](https://code.claude.com/docs/en/permissions), **not an OS sandbox**. Existing approvals can allow writes and shell commands. `acceptEdits` additionally auto-approves edits and common filesystem commands such as `mkdir`, `touch`, `mv` and `cp`. Unresolved permission requests are denied because CodeConvoy supplies no interactive permission host. Existing permission hooks still apply. No additional directories or blanket bypass are enabled by CodeConvoy; configured access remains Claude's responsibility.

Use trusted repositories: print mode skips workspace trust prompts and can load repository hooks/MCP configuration. Disabling session persistence does not disable all Claude logging, configuration or network activity.

`Check CLI` checks the core help interface and displays the version where available, without contacting a model. Some documented flags are hidden from help, so this is a compatibility check, not proof that every optional setting works on an older CLI. Missing Claude is a normal state and does not affect other backends.

Output retains Claude's JSON records for assistant/tool activity and metadata, extracts final response text, and labels stderr. Success requires exit zero and one valid final `result` with `subtype=success`, `is_error=false`, required result fields, and no explicit execution/protocol error or reported interruption. Errors, missing results, malformed/oversized records and activity after the result fail conservatively. Recovered tool failures and permission denials remain visible; review the response and diff even after success. Assembly allows 1 MiB per record, following the official SDK's default; existing display/log caps still apply.

Scheduling, repository safeguards, cancellation, draft clearing, persisted options, snapshots, history and **Reuse convoy** use the shared lifecycle unchanged. No state migration is required, and reuse never starts work automatically.

### Safe manual Claude E2E test

1. Use an already installed/authenticated Claude Code. Check its version/help independently and inspect existing permissions, hooks and MCP settings. Do not use important repositories.
2. Prepare two **trusted, disposable**, clean Git repositories, each with a tracked README and a tracked `CODECONVOY_CLAUDE_SMOKE.md` placeholder.
3. Select **Claude Code**, leave model/effort at **Claude default**, set **Max turns** to **10** and both concurrency limits to **2**. Keep **Existing approvals; deny asks** if your existing rules allow editing the smoke file. Otherwise explicitly select **Accept edits / file commands**, understanding its broader filesystem-command approval. Run **Check CLI**, select both repositories, review and start:

   > Edit only CODECONVOY_CLAUDE_SMOKE.md. Replace its placeholder with a heading “CodeConvoy Claude smoke test”, this repository's directory name, and a short summary of its README. Do not modify other files, run shell commands, or perform Git operations. Report the file you edited. If permissions prevent editing, report that and stop.

4. Confirm independent output and expected tracked-file diffs in both repositories. Inspect permission denials even on success. Prompt instructions are not an access boundary. Turn-limit failures are not successful E2E results.
5. Select **Reuse convoy** and verify all settings/selections return without execution. Separately start an inspection task, stop it while active, and check process cleanup and retained edits. Record the real CLI version, platform, options and outcomes before claiming authenticated E2E success.

An optional installed-CLI probe runs help/version only:

```sh
cargo test --all-features --test claude installed_claude_accepts_generated_arguments -- --ignored --nocapture
```

## Repository safety and data

CodeConvoy's Git operations only inspect working trees. Registration/removal never clones, deletes, resets, stashes, checks out, commits, or pushes. External diff/textconv and filesystem-monitor commands are disabled for inspection. Duplicate and nested repository selections are rejected. Each queued job rechecks branch, HEAD, and porcelain status immediately before starting; detected state changes fail that job without affecting other jobs.

Codex Workspace-write, Copilot file-write/tool approvals, and OpenCode/Claude permissions can permit the **agent** to modify files. CodeConvoy is an orchestrator, not an extra security sandbox. Stop terminates processes and retains partial edits; it cannot roll changes back. Review the diff before committing. Git checks are not filesystem locks: another application can still edit a repository, and changes within an already-dirty file may leave the same porcelain status.

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

The `test-support` feature builds a deterministic local test executable. It is never used as an application backend and requires no account or network. Integration tests use temporary Git repositories and handshake-controlled fixture processes to check simultaneous Codex/Copilot/OpenCode/Claude convoys, both concurrency limits, draft snapshots, repository leases and release, cancellation/failure isolation, shutdown, process descendants, output capture, and Git rechecks. Pure scheduler tests check round-robin fairness without timing dependencies. Core tests cover all four backends' command arguments, output handling, exit interpretation, backend preferences and old-state compatibility, persistence, history recovery, and Git inspection. Copilot integration tests use its real backend with a local fixture executable, not a provider. An optional installed-CLI check runs only help/version commands:

```sh
cargo test --all-features --test copilot installed_copilot_accepts_the_exact_command_flags -- --ignored --nocapture
```

Read [the architecture](docs/architecture.md) for module boundaries and implementation tradeoffs. Claude is the fourth and final backend for the v0.1.0 cycle. The backend abstraction is feature-frozen except for bug fixes. Authenticated Claude/OpenCode E2E and Windows runtime verification remain outstanding.

## License

MIT. See [LICENSE](LICENSE).
