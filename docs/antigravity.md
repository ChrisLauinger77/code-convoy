# Antigravity CLI

Antigravity CLI is an experimental backend in the current source. Published
packages do not include it. CodeConvoy uses your local CLI installation and
existing sign-in; it does not install Antigravity or manage accounts. Its
headless protocol is implemented from Google's documentation and deterministic
fixtures; no installed or authenticated `agy` has been tested yet. See the
[feasibility and validation record](antigravity-validation.md).

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
`agy`. Choose **Use and check**, or enter an absolute path to the executable. On
Windows choose a native `agy.exe`; `.cmd`, `.bat`, `.ps1`, WSL wrappers and shell
command strings are unsupported.

**Check CLI** runs `agy --help` and `agy --version` only. **Available** means the
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

## Settings

| Control | Behavior |
| --- | --- |
| Executable | `agy` or an absolute path; native `.exe` required on Windows |
| Model | Blank keeps Antigravity's default; otherwise a slug from `agy models`, passed as `--model`. Unknown slugs fail rather than fall back |
| Effort | Blank keeps Antigravity's configuration; Low/Medium/High map to `--effort` |
| Permissions | **Antigravity configuration** passes no permission flag; **Approve all tool requests** explicitly passes `--dangerously-skip-permissions` |
| Terminal sandbox | **Antigravity configuration** keeps your setting; **Enable for this job** passes `--sandbox` |
| Time limit (seconds) | Blank is unlimited; 1–86400 limits execution after spawn, excluding queueing/preparation |

Headless `agy` runs have no approval prompt. With **Antigravity configuration**,
rules in your Antigravity settings apply; tools that would ask (shell commands by
default) are soft-denied while the run continues. **Approve all tool requests**
approves file writes and command execution without review; it is never enabled
automatically. Neither choice is an OS sandbox provided by CodeConvoy.

## Execution

Each job runs, with each argument passed directly and no shell:

```text
agy --input-format stream-json --output-format stream-json --print-timeout=1500m [--model=SLUG] [--effort=LEVEL] [--dangerously-skip-permissions] [--sandbox]
```

The process directory is the assigned checkout. The task and UTF-8 text context
are written to stdin as one `{"event":"user","message":{"content":…}}` line, and
stdin is closed, so the session ends after that turn. The prompt never appears in
the process arguments. Image attachments are unsupported. Antigravity's own
headless deadline is set to 25 hours, above CodeConvoy's maximum time limit,
because the guide documents a five-minute default after which partial output can
exit successfully.

A job **succeeds** only when all of these hold:

- the stream starts with one `init` event whose `cwd` is the assigned checkout;
- every event uses the same conversation ID;
- the stream ends with a `result` whose status is `SUCCESS`, and every earlier
  result is also `SUCCESS`;
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

## Current limitations

- Captured `agy --help` / `--version` output and authenticated runs are still
  missing; the protocol fixtures are documentation-derived.
- Session continuation (`--continue`, `--conversation`) is not used. CodeConvoy
  retries and follow-ups start fresh convoys.
- Background "daemon" tasks that Antigravity deliberately leaves running are
  covered by CodeConvoy's process-tree cleanup only if they stay in the owned
  process tree; lifecycle verification is pending.
