# Antigravity CLI

CodeConvoy 1.2.0 introduces Antigravity CLI as an experimental backend.
Packages older than 1.2.0 do not include it. CodeConvoy uses your local CLI
installation and existing sign-in; it does not install Antigravity or manage
accounts. Its headless protocol is implemented from Google's documentation and
deterministic fixtures. An installed **agy 1.4.2** on macOS passed Check CLI,
accepted every argument form CodeConvoy uses and failed real jobs at the sign-in
gate as expected; **agy 1.4.7** on Linux repeated Check CLI and the real
signed-out jobs with the same result. Authenticated tasks have not been tested.
See the [feasibility and validation record](antigravity-validation.md),
[release readiness](antigravity-readiness.md) and the
[1.4.3–1.4.7 audit](antigravity-1.4-audit.md).

CodeConvoy integrates the official command-line agent `agy` only. It does not
automate the Antigravity desktop app or IDE, use private APIs, use the Python SDK
or substitute Gemini CLI.

## Installation and sign-in

Follow [Google's installation guide](https://antigravity.google/docs/install?tab=cli).
Install separately and inspect any downloaded installer before executing it.
Use `agy --version` to record the installed version. CodeConvoy requires
**agy 1.2.16 or newer** (see [minimum version](antigravity-validation.md#minimum-version)).
Desktop launchers can have a different PATH from your terminal.

Select **Antigravity CLI** under **NEW CONVOY → Agent**. **Find CLI** searches for
`agy`, including `~/.local/bin` (the Linux/macOS installer default) and, on
Windows, `%LOCALAPPDATA%\agy\bin`. Choose **Use and check**, or enter an absolute path to the executable. On
Windows choose a native `agy.exe`; `.cmd`, `.bat`, `.ps1`, WSL wrappers and shell
command strings are unsupported.

**Check CLI** runs `agy --help` and `agy --version` only; `agy` prints its help to
stderr, which CodeConvoy reads for this backend. Neither command writes files or
starts other processes. **Available** means the
required headless flags were advertised and the version is supported, not that
you are signed in or can use a model. Google's
[headless guide](https://antigravity.google/docs/cli/headless/) requires one
interactive `agy` sign-in before headless use; run it yourself in a terminal
with the same installation and user environment. Google sign-in tokens are kept
in the operating system keyring by `agy`. Application Default Credentials and
Gemini API key setups are configured in your own environment and `agy` settings.
CodeConvoy never reads, changes or stores credentials, keyring entries or
`~/.gemini` configuration, and never sets authentication variables. A job without
a usable sign-in fails with an authentication message.

Upstream fixes that affect headless sign-in: agy 1.4.6 keeps enterprise sign-ins
across restarts on macOS and Windows, which earlier releases could lose because
the credential exceeded the keychain's size limit. agy 1.4.7 loads skills, rules,
custom agents, plugins, plugin MCP servers and hooks when you sign in with
Application Default Credentials (`AGY_ADC_AUTH`); earlier releases skipped them.
On Linux, an open upstream report says that inherited `SSH_CONNECTION`,
`SSH_CLIENT` or `SSH_TTY` variables, for example in a tmux session that was once
attached over SSH, make agy skip the desktop keyring and ask you to sign in.
CodeConvoy passes its environment to agy unchanged apart from Git overrides and
`PWD`. If jobs fail at sign-in while `agy` works in a plain terminal, start
CodeConvoy from a session without those variables.

## Settings

| Control | Behavior |
| --- | --- |
| Executable | `agy` or an absolute path; native `.exe` required on Windows |
| Model | Blank keeps Antigravity's default; otherwise a slug from `agy models`, passed as `--model`. Unknown slugs fail rather than fall back |
| Effort | Blank keeps Antigravity's configuration; Low/Medium/High map to `--effort` |
| Permissions | **Antigravity configuration** passes no permission flag; **Approve all tool requests** explicitly passes `--dangerously-skip-permissions` |
| Terminal sandbox | **Antigravity configuration** keeps your setting; **Enable for this job** passes `--sandbox`. On Windows it requires Windows 11 24H2 (build 26100) or newer and is rejected at review otherwise, because Antigravity turns the sandbox off on older Windows. See [Terminal sandbox](#terminal-sandbox) for Linux and credential directories |
| Time limit (seconds) | Blank is unlimited; 1–86400 limits execution after spawn, excluding queueing/preparation |

Headless `agy` runs have no approval prompt. With **Antigravity configuration**,
rules in your Antigravity settings apply; tools that would ask (shell commands by
default) are soft-denied while the run continues. **Approve all tool requests**
approves file writes and command execution without review; it is never enabled
automatically. Neither choice is an OS sandbox provided by CodeConvoy.

If your Antigravity settings select Auto mode (Linux and macOS since agy 1.3.5,
Windows 11 24H2 or newer for personal Google Accounts since 1.4.4), Antigravity's
own reviewer may approve commands instead of soft-denying them. CodeConvoy passes
no `--mode`; **Activity** shows the permission mode that agy reports at start.

### Terminal sandbox

The sandbox is Antigravity's, and CodeConvoy only passes `--sandbox` when asked:

- On Linux hosts and containers that block unprivileged user namespaces,
  Google's [sandbox guide](https://antigravity.google/docs/sandbox?tab=cli)
  states that agy 1.4.4 and newer ignore `--sandbox` and
  `enableTerminalSandbox`. Commands then run without the sandbox wherever your
  permission rules or **Approve all tool requests** allow them. CodeConvoy cannot
  see this from agy's output; check the sandbox in an interactive `agy` session on
  the same host before relying on it.
- Since agy 1.4.6, sandboxed commands cannot read or modify `~/.docker`,
  `~/.kube` or `~/_netrc` unless your Antigravity settings grant access. Tasks
  that use Docker registry or Kubernetes credentials inside the sandbox fail
  there. CodeConvoy adds no grants and does not relax the sandbox. The failed
  commands appear in **Activity**. A request to run them outside the sandbox
  needs approval, which a headless run soft-denies.

## Execution

Each job runs, with each argument passed directly and no shell:

```text
agy --input-format stream-json --output-format stream-json [--model=SLUG] [--effort=LEVEL] [--dangerously-skip-permissions] [--sandbox]
```

The process directory is the assigned checkout. The task and UTF-8 text context
are written to stdin as one `{"event":"user","message":{"content":…}}` line, and
stdin is closed, so the session ends after that turn. The prompt never appears in
the process arguments. Image attachments are unsupported. CodeConvoy passes no
`--print-timeout`: headless runs have no Antigravity deadline by default since
1.2.6, so a blank time limit stays unlimited. A configured CodeConvoy time limit
stops the process tree and fails the job.

A job **succeeds** only when all of these hold:

- the stream starts with one `init` event whose `cwd` is the assigned checkout;
- every event uses the same conversation ID;
- the stream ends with a `result` whose status is `SUCCESS`, and every earlier
  result is also `SUCCESS`;
- no step of any type (tool, streamed response or other) reported as `ACTIVE`
  is left without its `DONE` update when a `SUCCESS` result arrives;
- every record is valid JSON within 1 MiB;
- the process exits zero, and stderr reports no `AGY_ERROR` or authentication
  failure.

Results with `ERROR`, `CANCELED`, `INTERRUPTED`, `INVALID`, `WAITING` or
`RUNNING`, malformed or unknown events, a missing result and a different working
directory fail the job. A successful result confirms CLI completion, not that the
task was fulfilled or files changed.

Tools refused by policy (`denied_actions`) do not fail the job, because the run
completed as Antigravity intends. The result detail and **Activity** state how
many actions were refused, since the task may be incomplete. Recoverable tool
errors are shown in **Activity** without failing the job. Review **Activity**,
**Raw output** and **Diff**.

Tasks that start with a slash command answered by the CLI, such as `/model`,
end with an `ERROR` result in this transport.

### Failure messages

A failed job's detail starts with what to do. Recognized advice comes before
Antigravity's own error text, so the compact summary keeps it even when that text
contains a colon. Every failure ends with the exit code (or, on Unix, the
terminating signal such as `SIGKILL` or `SIGSEGV`, and whether a core was dumped)
and, when Antigravity reported one, the conversation ID. CodeConvoy adds advice only when Antigravity's
own error text indicates a known cause; the classification is unchanged and the
original output stays in **Activity** and **Raw output**.

| Antigravity reports | Advice in the job detail |
| --- | --- |
| `authentication required`, `authentication failed`, `Authentication expired` | Run `agy` once in a terminal with the same installation and user account to sign in, then retry |
| `invalid model selection`, model not recognized | Choose a slug listed by `agy models`, or leave **Model** blank |
| Quota, `RESOURCE_EXHAUSTED`, AI credits balance | Check `agy -p /usage` in a terminal before retrying |
| Slash command answered by the CLI itself | Start the task with ordinary text |
| `WAITING` | The run waited for input a headless run cannot provide |
| `CANCELED`, `INTERRUPTED` | The run was stopped before it completed |

### Known upstream issues

Open in Google's tracker on 2026-10-26; details in the
[1.4.3–1.4.7 audit](antigravity-1.4-audit.md):

- **Stalled turn after a refused tool (agy 1.4.6 and newer).** When the model
  requests several tools at once and one is soft-denied, agy can leave the others
  waiting and never send a `result`. The report uses `-p`; agy 1.4.7 runs
  CodeConvoy's stdin transport through the same print mode, but the hang has not
  been reproduced here. CodeConvoy cannot tell it from a long tool, so the job
  stays running. **Stop** or a **Time limit** ends it and stops
  the process tree; the job is Cancelled or Failed and keeps its edits. Set a
  time limit for unattended Antigravity convoys that use **Antigravity
  configuration** permissions.
- **Sandboxed Windows runs on exFAT drives.** With `--sandbox`, agy exits with
  code 2 and no `result` at the first command when the checkout is on an exFAT
  volume. CodeConvoy fails the job with exit code 2.

## Repository customizations

`agy` loads its own workspace customizations from the assigned checkout, such as
rules, skills and hooks in `.agents/` directories between the checkout and the
project root. CodeConvoy does not read, copy or translate them. Direct mode
exposes the registered checkout including uncommitted customizations; isolated
mode contains only files committed at the reviewed commit. Existing user-level
Antigravity configuration in `~/.gemini` applies in both modes.

## Session continuation and conversation history

CodeConvoy does not continue Antigravity sessions. `agy` documents `--continue`
and `--conversation <id>`, but CodeConvoy has no resume model for any backend:
**Retry** and follow-ups start a fresh convoy with the original task and settings,
and recovery never resumes an agent. Emulating continuation would require reusing
Antigravity's directory-scoped conversation state, which CodeConvoy does not do.

`agy` itself keeps every job's conversation, including the task text and tool
output, in its local history below `~/.gemini/antigravity-cli`. There is no
documented opt-out comparable to Codex's ephemeral mode or Claude Code's
disabled session persistence. The conversation ID appears in the job detail for
reference; CodeConvoy stores no other Antigravity session data and never deletes
Antigravity's history.

## Lifecycle and safety

Antigravity jobs use the same scheduler, repository leases, worktrees and
process supervision as every other backend; nothing is Antigravity-specific.

- **Stop** and the optional time limit terminate the whole owned `agy` process
  tree, including commands and background tasks it started. The job is Cancelled
  or Failed; edits are retained, nothing is rolled back.
- When `agy` exits, CodeConvoy terminates any process it left behind in the owned
  tree (for example a background server) before the job finishes.
- **Quit** cancels all running Antigravity jobs the same way and retains isolated
  worktrees.
- Once process cleanup is confirmed, every outcome (success, reported failure,
  authentication failure, malformed output, interruption, time limit) releases
  its execution slot and repository lease. A failure in one job does not affect
  other jobs or convoys.
- If cleanup **cannot be confirmed** (supervision or termination fails, or a
  descendant may still be alive), the job still ends, but its repository access
  and execution slot **stay reserved** and its history is protected. The job
  detail says so; inspect remaining processes before restarting CodeConvoy. This
  is the shared quarantine behavior, not specific to Antigravity.
- Isolated jobs on the same repository run concurrently in separate checkouts;
  direct access waits for them. Nested repositories never run concurrently.
- **Retry** reuses the original permission and sandbox choices; CodeConvoy never
  broadens them.

Processes that deliberately leave the owned process tree (a new Unix session or
process group) cannot be stopped by CodeConvoy; this is a general platform
limitation. Antigravity's background auto-updater is such a process: on macOS,
agy 1.4.2 starts `agy --bg-updater` at most every 15 minutes in its own session.
It is not stopped when the job ends, does not hold the job's output pipes, so it
does not delay completion, and exits on its own. It may replace the installed
`agy` binary; no headless opt-out is documented. agy 1.4.7 on Linux behaved the
same way: its updater ran in its own session under the user's service manager
and exited on its own. Windows behavior is unverified.
Antigravity's own subagent worktrees are managed by Antigravity; CodeConvoy
neither creates nor removes them.

## Current limitations

- Authenticated runs are untested; successful-run stream fixtures are
  documentation-derived. Check CLI and the unauthenticated failure were verified
  with agy 1.4.2 on macOS and agy 1.4.7 on Linux. Other 1.4.3–1.4.7 changes were
  reviewed from Google's changelog, documentation and issue tracker.
- Session continuation (`--continue`, `--conversation`) is not supported; see
  [Session continuation](#session-continuation-and-conversation-history).
- Antigravity keeps local conversation history for every job; CodeConvoy cannot
  disable it.
- Lifecycle behavior is verified with a fixture executable; with real `agy` only
  the process layout of an unauthenticated run was observed. See
  [Lifecycle evidence](antigravity-validation.md#lifecycle-and-safety).
