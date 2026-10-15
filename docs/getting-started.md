# Getting started

CodeConvoy runs one task across selected local Git repositories using one of
your installed coding-agent CLIs. Start with a disposable repository or a small
change you can inspect. CodeConvoy does not install agents, manage accounts or
provide an additional security sandbox.

## 1. Install CodeConvoy and an agent

Follow the [installation instructions](../README.md#installation) for your OS.
Check the exact release's requirements and SHA-256 checksums. The
[platform matrix](platform-validation.md) distinguishes package checks from
native desktop testing; Linux's glibc 2.39 target alone is not desktop certification.

Install Git and at least one supported CLI separately: Codex, GitHub Copilot CLI,
OpenCode or Claude Code. Authenticate using that CLI's own instructions, then
confirm it works in a terminal. Do not enter credentials in CodeConvoy tasks.

Launch CodeConvoy and select the agent in **NEW CONVOY**. Availability checks run
in the background. If discovery fails, use **Find CLI → Use and check**, or enter
the absolute executable path and choose **Check CLI**. Desktop and terminal PATHs
can differ. On Windows use a native executable; shell wrappers are not supported.
**Available** confirms the CLI contract, not authentication, account entitlement
or access to a particular model.

Review the selected backend's options. Permissions are backend-specific; they
are not interchangeable sandbox guarantees. See the contracts for
[Codex](../README.md#codex-contract), [Copilot](../README.md#copilot-contract),
[OpenCode](../README.md#opencode-contract) and
[Claude Code](../README.md#claude-code-contract). Claude Auto requires CLI/account
support and can be disabled by policy; inspect its reported effective mode.

## 2. Register and group repositories

Enter an existing Git repository's root path, or select it with **Browse…**, then
choose **Add repository**. Selecting a folder alone does not register it.
Registration does not clone or modify the repository. Select its checkbox for
the next convoy.

Use **Manage groups…** to save named collections. A repository can belong to more
than one group but receives only one job per convoy. Selecting a group adds its
available members; deselecting it removes those members even if another group
overlaps. Check the final selection count.

**Filter repositories…** searches names and paths. Hidden selections remain
selected. **Select all / Select none** and group checkboxes still affect their
full sets, including hidden repositories. See
[groups, filtering and templates](task-context.md).

## 3. Prepare and start a convoy

Write the task once; every selected repository receives that task. **Templates**
saves and loads task text only. **Add files…** supplies optional context according
to the chosen backend's capabilities. Files must remain available and unchanged
through launch; history retains references, not attachment contents.

Choose the execution mode:

- **Current working tree** (default): the agent works directly in each registered
  checkout, including its existing edits.
- **Isolated worktree**: a fresh detached checkout starts at the committed HEAD
  captured during review. Local staged, unstaged and untracked edits are excluded.
  Results stay in the retained checkout until an explicit result action. Configure
  new locations under **Settings → Worktree location**; existing attempts keep
  their original paths. See [worktree settings](desktop-workflow.md#worktree-location).

Set **This convoy** and **Global job limit**. Both limits apply; conflicting
repository jobs wait without consuming an execution slot. Choose **Run Convoy**,
review the exact repository list, branch/HEAD and existing changes, and acknowledge
dirty trees where required. **Start convoy** confirms execution. Git and context
are checked again before each job starts; stale review data can fail a queued job.

The accepted run is an immutable snapshot. Editing the next draft does not change
it. After launch, task text and repository selections clear for the next task;
backend preferences and concurrency settings remain.

## 4. Inspect execution and Git changes

In **RUNS / RESULTS**, select a convoy and repository. **Activity** summarizes
events, **Raw output** shows CLI details, and **Task & settings** shows the original
request. **Stop job** or **Stop Convoy** cancels that work, waits for owned process
cleanup and retains partial edits. Cancellation is not undo.

Open **Review** and **Diff**. Agent success, Git changes and validation are separate
observations: a failed agent can leave useful edits, and a successful agent can
leave mistakes. Direct diffs include pre-existing changes; isolated diffs use
the captured base. Use **Refresh review / Refresh diff** after external edits.
Review the actual Git changes before committing through your normal Git tools.

For an isolated result, **Apply result** can copy reviewed changes to the original
checkout only when ownership is verified and the destination is clean at the
original base. It leaves unstaged changes, without committing or pushing.
Unsupported or uncertain changes stay blocked with diagnostics. **Discard result**
deletes a verified retained checkout after confirmation. Applied copies remain
until **Clean up retained copy** is explicitly confirmed. These result actions
are distinct from conservative Storage cleanup; Direct mode has no Apply/Discard.
See [result safety](../README.md#first-run) and [cleanup rules](worktree-cleanup.md).

## 5. Run manual validation

In **Repositories**, choose **Configure validation…**, select a preset or enter
an executable and a JSON argument array, then **Save command**. For example,
**Rust Tests** configures `cargo` with `["test"]`. **Assign Validation Preset…**
can configure several repositories/groups; it preserves existing commands unless
you explicitly confirm overwrite. Configuration never executes a command.

Select a finished repository job and choose **Run Validation** in Review or the
**Validation** tab. Check the displayed directory and command: Direct uses the
recorded source, isolated uses the retained worktree. Applied or unavailable
results cannot validate. Inspect the output and use **Cancel Validation** if
needed. Commands can change files; a pass records that attempt, not a guarantee
about later edits. See [validation and platform requirements](validation.md).

## 6. Revisit history and manage storage

Completed convoys appear under **HISTORY**. Search/filter does not change the
selected result. Reopening the app never resumes an agent; interrupted work is
labelled accordingly, and retained worktrees need fresh verification.
**Reuse convoy** restores a draft for a new review, not a running session.

**Copy Summary / Copy Repository Result** copy bounded Markdown reports.
**Export… / Export History…** save Markdown or JSON reports of recorded metadata.
Review them before sharing; names, task text and validation arguments can be
sensitive. Reports omit logs, diffs and ownership records and are not backups.
Output logs and validation output are available only during the current session.

History is unlimited by default. **Settings → History Retention…** configures
optional limits; **History cleanup → Clean Up History…** previews metadata removal.
Protected work and retained ownership records stay protected. Removing history
never deletes a checkout. See [history](history.md) and [export](history-export.md).

**Settings → Storage…** measures known history/worktree storage. **Manage Worktrees**
offers separately verified, confirmed cleanup of eligible entries. Unknown,
modified, active and unresolved entries remain protected. Ordinary Storage cleanup
is unavailable on Windows; measurement and inspection still work. See
[Storage Overview](storage.md) and [safe cleanup](worktree-cleanup.md).

## Desktop controls

Use **Settings → Keyboard Shortcuts…** for the in-app reference, or the
[shortcut table](../README.md#keyboard-shortcuts). **Settings → Desktop notifications**
controls completion alerts. Tray support is optional and off by default;
close-to-tray needs a working tray. Explicit Quit confirms active work and waits
for cleanup. See [desktop behavior](desktop-workflow.md) and
[notification limits](notifications.md), including Wayland focus restrictions.
