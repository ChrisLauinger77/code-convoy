# Antigravity CLI

Antigravity CLI is an in-development backend in the current source. This
milestone adds discovery, version checks and settings only: **jobs fail before
any Antigravity process starts**. Published packages do not include it.
CodeConvoy uses your local CLI installation and existing sign-in; it does not
install Antigravity or manage accounts. See the
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
`~/.gemini` configuration, and never sets authentication variables.

## Settings

| Control | Behavior when execution is available |
| --- | --- |
| Executable | `agy` or an absolute path; native `.exe` required on Windows |
| Model | Blank keeps Antigravity's default; otherwise a slug from `agy models`. Unknown slugs fail rather than fall back |
| Effort | Blank keeps Antigravity's configuration; Low/Medium/High map to `--effort` |
| Permissions | **Antigravity configuration** passes no permission flag; **Approve all tool requests** explicitly passes `--dangerously-skip-permissions` |
| Terminal sandbox | **Antigravity configuration** keeps your setting; **Enable for this job** passes `--sandbox` |
| Time limit (seconds) | Blank is unlimited; 1–86400 limits execution after spawn, excluding queueing/preparation |

Headless `agy` runs have no approval prompt. With **Antigravity configuration**,
rules in your Antigravity settings apply; tools that would ask (shell commands by
default) are soft-denied while the run continues. **Approve all tool requests**
approves file writes and command execution without review; it is never enabled
automatically. Neither choice is an OS sandbox provided by CodeConvoy.

## Current limitations

- Execution, prompt delivery, structured output decoding and completion
  classification are not implemented yet.
- Attachments are not offered.
- Session continuation (`--continue`, `--conversation`) is not used. CodeConvoy
  retries and follow-ups start fresh convoys.
- The `agy --help` contract is checked against a documentation-derived fixture;
  captured help from an installed release is still required.
