# Cursor CLI

Cursor CLI is an experimental backend in the current source. Published packages
may predate it. CodeConvoy uses your local CLI installation and existing sign-in;
it does not install Cursor or manage accounts. The accepted Linux smoke test
with **2026.10.16-e3cb207** establishes syntax through the authentication gate.
Authenticated tasks, native permission enforcement and full protocol compatibility
remain unverified on every platform. See the [validation record](cursor-validation.md).

## Installation and sign-in

Follow [Cursor's official installation guide](https://cursor.com/docs/cli/installation)
for Linux, macOS or native Windows. Install separately and inspect any downloaded
installer before executing it. Use `agent --version` to record the installed
version. Desktop launchers can have a different PATH from your terminal.

Select **Cursor CLI** under **NEW CONVOY → Agent**. **Find CLI** searches for
`agent` and the legacy `cursor-agent` launcher. Choose **Use and check**, or enter
an absolute path to the launcher. A similarly named executable must still pass
Cursor's help contract. On Windows choose a native `agent.exe` or
`cursor-agent.exe`; `.cmd`, `.bat`, `.ps1`, WSL wrappers and shell command strings
are unsupported by this backend.

**Check CLI** runs help/version only. **Available** means the required interface
was advertised, not that you are signed in or can use a model. For
[authentication](https://cursor.com/docs/cli/reference/authentication), run
`agent status` and, when needed, `agent login` yourself in a terminal using the
same installation and user environment. The app's **Cursor sign-in help** link
opens documentation only. Never put credentials in tasks, model fields or
executable settings. Existing CLI authentication is inherited; CodeConvoy does
not read, change or store credential files.

## Settings and permissions

The [README control table](../README.md#cursor-cli-contract) lists every exposed
setting. Blank **Model** keeps Cursor's default. If an explicit model is useful,
choose an identifier supported by your installed CLI/account; `agent models`
lists account models. The app does not fetch models, hardcode a catalog or check
entitlements. `--model` syntax passed the unauthenticated smoke test; model access
was not tested. See [Cursor parameters](https://cursor.com/docs/cli/reference/parameters).

**Cursor configuration** is the default for both permissions and workspace trust.
It omits `--force` and `--trust`. Existing Cursor policy applies. Headless tasks
may propose changes without applying them, or fail when approval is required;
CodeConvoy provides no interactive approval host. The
[headless guide](https://cursor.com/docs/cli/headless) describes `--force` for
applying edits, while the [permission reference](https://cursor.com/docs/cli/reference/permissions)
also documents configured allow/deny rules for print-mode tools. Effective
behavior depends on the installed version and policy; do not assume a successful
response means edits were applied.

**Force allow unless denied** explicitly passes `--force`, allowing commands and
edits unless denied by Cursor. **Trust assigned workspace** separately passes
`--trust`, bypassing the headless workspace trust prompt. Review repository rules,
hooks and MCP configuration before opting in. A fresh isolated checkout may have
no saved trust decision. CodeConvoy never broadens permissions or trust on retry.
Neither setting, nor `--workspace`, provides an OS sandbox. Existing user and
organization configuration remains Cursor's responsibility.

**Time limit (seconds)** accepts a whole number from 1 to 86400. Blank or missing
saved values use 3600 seconds, including older drafts that previously ran without
a limit. This bounds unresolved confirmation and other unattended waits without
granting permission. The application starts this timer after process spawn;
queueing and isolated-worktree preparation are excluded. Expiry fails the job,
stops its owned process tree and retains edits. Stop cancels the job with the
same cleanup guarantees. Neither operation rolls changes back.

Cursor's v2026.10.08 Auto-review change can affect risky deletion requests when
that native policy is active. A new print-mode incompatibility or confirmation
hang has not been reproduced. See the [permission audit](cursor-permission-audit.md)
for verified behavior, mocked failure coverage and remaining limitations.

## Repository rules

[Cursor's CLI rules documentation](https://cursor.com/docs/cli/using#rules) says
it loads `.cursor/rules` and root `AGENTS.md`/`CLAUDE.md`. The
[rules reference](https://cursor.com/docs/rules) describes `.mdc` configuration
and nested instructions. Those are Cursor's semantics, not a CodeConvoy rules
engine or a guarantee about every installed CLI version.

CodeConvoy sets both the process working directory and `--workspace` to the
assigned checkout. It does not read, concatenate, translate or generate these
rules. Direct mode exposes the registered checkout, including its local rule
edits. Isolated mode contains only tracked files at the exact reviewed commit;
uncommitted, ignored or untracked rules from the source are not copied. Commit
needed project rules before reviewing an isolated convoy. Existing CLI-level
configuration is inherited rather than isolated by CodeConvoy.

Deterministic tests verify the assigned directory, unchanged prompt, retained
rule files and committed-versus-dirty source separation. They do **not** prove
that Cursor loaded or obeyed a rule. Authenticated rule-loading verification
remains outstanding; the syntax smoke test cannot establish it.

## Output, review and troubleshooting

**Activity** summarizes initialization, assistant text and tool events. **Raw
output** retains bounded CLI stdout/stderr for the current session. **Task &
settings** shows the immutable launch settings even after the next draft changes.
Success requires a complete, valid terminal success result and zero exit status.
Unknown, malformed or inconsistent streams fail conservatively. Diagnostic
matching also fails on authentication/permission/trust messages on stderr, even
alongside a success result; inspect Raw output for a possible incidental tool
warning. Fixtures cover this tradeoff, not every future Cursor event shape.

| Symptom | Next step |
| --- | --- |
| Unavailable | Install separately, use Find CLI or enter an absolute launcher path. |
| Invalid configuration / missing help flags | Check the chosen executable and settings; use a compatible CLI and Check CLI again. |
| Authentication required | Run `agent status` / `agent login` separately, then launch a fresh convoy. |
| Permission denial / approval required | Inspect the requested operation and Cursor policy. Select broader permissions only after review. |
| Workspace trust required | Inspect the assigned checkout, rules, hooks and MCP settings, then explicitly choose trust if appropriate. |
| Unknown/incomplete event stream | Inspect Activity/Raw output and record the exact CLI version; report the unsupported event shape without secrets. |
| Time limit expired | Inspect retained edits; review the limit before a fresh attempt. |
| Success with no changes | Inspect the response and Git diff; configured print mode can produce proposals only. |

**Review** and **Diff** use Git, independently of agent status. Existing result
comparison, copying, history and **Reuse convoy** work through the shared model;
reuse prepares a new draft and never resumes a Cursor session. Manual validation
presets are repository commands, independent of the selected agent. Run them
explicitly after reviewing the result.

Text attachments are included in the single prompt argument, subject to platform
command-line limits and local process inspection. Images are unsupported. Prompts
and settings are saved with history, while output logs are session-only. Cursor
may maintain its own logs/configuration. No ACP server, CLI-managed worktree,
cloud execution, session resumption, automatic Git commit or push is enabled by
this integration.
