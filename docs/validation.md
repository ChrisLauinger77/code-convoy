# Manual repository validation

## Configure a repository

In Repositories, choose **Configure validation…**. Each registration starts
unconfigured. Select a built-in **Preset** to populate the executable and JSON
argument fields, or keep **Custom**. Inspect the preview and edit either field
before **Save command**. Manual editing changes the draft to Custom. Selecting
another preset deliberately replaces both draft fields; choosing Custom keeps
them. Cancel/Escape leaves saved settings unchanged. **Remove command** clears
the repository configuration.

Enter an executable name on the desktop application's PATH, or an absolute path.
Every string in the argument array becomes one literal argument, including spaces,
quotes, Unicode and empty strings. The preview shows simple commands as
`npm run lint`; complex values use JSON quoting, for example
`cargo test "two words" ""`. This is display notation, not a portable shell command.

Configuration lives in CodeConvoy's existing local state, keyed by registered
canonical repository path. Only executable and arguments are saved; reopening an
editor shows Custom, without remembering a preset association. Unregistering removes
that configuration, while saved executions remain in history. Repository files
and agent output cannot configure or start validation. Configuration, preset
selection, bulk assignment, discovery and health inspection never execute it.

## Built-in presets

| Preset | Executable | Arguments |
| --- | --- | --- |
| npm Lint | `npm` | `["run", "lint"]` |
| Rust Tests | `cargo` | `["test"]` |
| Rust Check | `cargo` | `["check"]` |
| Rust Clippy | `cargo` | `["clippy", "--all-targets"]` |
| npm Test | `npm` | `["test"]` |
| npm Build | `npm` | `["run", "build"]` |
| Python pytest | `pytest` | `[]` |
| CMake Build | `cmake` | `["--build", "build"]` |
| Make Test | `make` | `["test"]` |

These are static templates. CodeConvoy does not detect project types, download
presets, install tools or maintain a user-defined preset catalog.

## Assign to several repositories

Choose **Assign Validation Preset…** in Repositories. Select individual repositories,
one group or several groups; group selection edits this dialog's own explicit
repository set. Overlapping memberships count once. Deselecting a group removes
its members from this set, including those selected through another group.
Individual checkboxes can adjust the result. The main convoy selection and group
membership remain unchanged; the repository filter does not limit these targets.

Choose a preset and review the target count and existing configuration count.
**Only repositories without validation** is the default. To replace commands,
choose **Overwrite … existing configurations**. **Review assignment…** shows the
command and target list, followed by **Confirm assignment** or **Confirm overwrite**.
Cancel is focused by default; Cancel/Escape saves nothing. Assignment changes
configuration only, even while agents or manually started validations are active.
Their existing execution snapshots are unchanged.

The result lists updated, preserved and failed repositories. Unregistered or
removed targets fail explicitly without being re-created. Registered but unavailable
paths can still receive configuration: assignment does not inspect Git, create
worktrees or require the folder to be accessible. Execution keeps its existing
availability checks. Commands changed after an overwrite review fail that target
and must be reviewed again. With preservation selected, newly configured targets
are skipped.

Eligible updates use one existing atomic state-file replacement. A save failure
reports all proposed updates as failed and leaves live configuration unchanged;
there is no per-repository save loop. If replacement succeeded but final disk sync
failed, durability is uncertain: the error asks you to fix state-directory access
and retry or reload to inspect saved state. Existing configuration serialization
and state version remain unchanged.

## One command per repository

For multiple checks, configure a project-owned script or command. For example,
a project's `package.json` can define `"validate": "npm run lint && npm run test"`;
configure executable `npm` and arguments `["run", "validate"]`. A Rust project's
own validation script can run fmt, Clippy and tests; configure its absolute executable
path (or a deliberately chosen interpreter and literal script argument).
CodeConvoy never generates those scripts or creates command chains, pipelines,
automatic post-convoy checks or scheduled validation.

## Execution and platform requirements

No shell is inserted. `&&`, `$VAR`, pipes, glob characters and quotes in an argument
are literal data; JSON escaping only expresses the argument strings. Relative
executable paths are rejected to avoid ambiguous directory resolution. On Windows,
`.cmd`/`.bat` wrappers are rejected: for npm use an absolute `node.exe` and arguments
such as `["C:\\tools\\nodejs\\node_modules\\npm\\bin\\npm-cli.js", "test"]`, adjusted to
your installation. A deliberately configured interpreter executable can execute
its own arguments; CodeConvoy adds no shell or scripting language. Tools must be
installed separately and usable without interactive stdin. GUI PATH may differ
from terminal PATH, so an absolute executable is useful when discovery fails.

The child inherits the process environment, except `PWD` and `GIT_*` overrides,
which are removed to keep repository targeting local to this operation. Key matching
is case-insensitive on Windows and case-sensitive on Unix. CodeConvoy does not dump
environment values. These commands run with your account's access;
they can write files or produce artifacts. Avoid credentials in arguments: the
command is persisted locally. Captured output and diagnostics stay in memory for
the current session and are never written to CodeConvoy's state file.

## Run, inspect and cancel

Select a finished repository job in Runs / Results. Review shows a separate
Validation indicator and **Run Validation** action; the **Validation** tab includes
the latest captured output. The current configured command and exact directory
appear before execution. The saved executed command remains visible even if the
configuration changes later. Cancel is available while a validation runs. Activity,
Raw output, Diff, and agent success/failure keep their existing meaning.

Direct results target the original recorded canonical working tree and Git common
directory, not whichever registration happens to be selected now. Isolated results
use the exact retained worktree and recheck its ownership manifest, original storage
location, Git backlink, base availability and identity. Applied results cannot run
validation: their retained copy and Diff stay unchanged until explicit cleanup.
Previous validation output remains readable in the current session. Missing,
cleaned, mismatched, pending/uncertain, discarded or unrecorded results are
unavailable. CodeConvoy neither recreates a worktree nor falls back to the
registered source. Legacy Direct entries
without recorded Git identity cannot be validated safely.

Validation runs asynchronously using the existing process owner: Unix process
groups and Windows Job Objects. Cancellation and Quit await owned-process cleanup.
Unconfirmed cleanup keeps repository access blocked. Intentionally detached Unix
processes are outside a process group's control. External applications can still
edit files; these leases coordinate CodeConvoy operations, not all OS activity.

Up to four explicitly requested validations can be active. There is no queue,
retry loop or automatic post-convoy action. Validation acquires the existing
exclusive repository lifecycle lease, including source and retained/nested paths.
It blocks conflicting agent preparation/execution, validation, Apply and cleanup
without consuming agent concurrency slots. Unrelated convoys and result actions
continue normally. Bulk discard checks each result independently: conflicting
results stay retained with an explanation while unrelated results can proceed.
If conflicting work already holds access, validation reports Unavailable; run it
again after that work finishes. Stop Convoy controls agents; Cancel Validation
controls validation. Explicit Quit stops both.

## Interpret the result

- **Not configured**: the registration has no command and no saved execution.
- **Not run**: a command is configured, but this job has no execution record,
  including historical 0.6.0 entries.
- **Running**: preparation or process execution is active; elapsed time is shown.
- **Passed / Failed**: the process completed with zero / nonzero or signal exit.
- **Cancelled**: the operation was stopped, or recovered as interrupted after exit.
- **Unavailable**: identity/access verification or process launch/cleanup failed.

Each job saves only its latest attempt: executed command, directory, UTC start/end
when known, measured duration, and exit code when available. Diagnostics and the
last 64 KiB of combined stdout/stderr remain available only in memory (stderr is
labelled, truncation is disclosed). Output is UTF-8 decoded with split characters
preserved. A fresh attempt replaces the previous record. Persistence uses the
existing atomic state file, not another database. Older saved output and diagnostics
are ignored on load and omitted on the next save. Active validation protects its
history entry from removal. Restart never restores output or resumes commands;
a saved Running record becomes Cancelled with interruption text.

A pass is an observation at its timestamp, **not certification of current files**.
The UI repeats that qualification; no file watcher or continuous revalidation is
added. Commands may change source, index, build output or Git state. Cached Review,
Diff and repository health are invalidated on validation start/completion; refreshed
Git inspection observes actual files. Original agent status, completion observations,
reviewed baseline and worktree ownership/resolution are never overwritten by
validation. CodeConvoy does not roll back command side effects.

## Verification

Deterministic tests use temporary Git repositories and the local native test
executable, without provider accounts or project toolchain installations. They
cover configuration/defaults/compatibility, literal arguments, output bounds,
pass/failure, missing executables/directories, retained-worktree targeting,
cancellation and descendant cleanup, conflicting operations, agent independence,
shutdown, timestamps/duration and history. Headless egui checks exercise required
780 × 560, 1180 × 820 and 1600 × 1000 layouts in System/Dark/Light modes.

Native acceptance remains separate: configure/run/edit/remove a command, cancel
an active validation, inspect output, refresh a changed diff, restart the app to
read history, and Quit during validation on Linux, macOS and Windows. Check keyboard
focus and modal dismissal at all three sizes. Platform execution evidence for this
implementation is recorded in the release notes; no package publication is implied.


Preset configuration coverage and native smoke evidence are recorded in
[Validation Presets validation](validation-presets-validation.md).
