# Cursor CLI foundation — PR 1

Evidence reviewed on 2026-10-17. This is the first of three requested integration
PRs. Production event/lifecycle refinements belong to PR 2; final user guidance
and release readiness belong to PR 3. Cargo remains at 1.0.0.

## Evidence and version limits

No Cursor CLI is installed on this development host. The maintainer confirmed
that this integration is for other users and requested no installation. No
executable help, authenticated task, account status, or real permission behavior
was observed. The help and JSON fixtures are synthetic contracts derived from
official documentation, **not recordings of Cursor execution**.

The official installers referenced `2026.10.16-e3cb207` when inspected. This is
installer evidence, not a tested CLI version or minimum supported version.
Installed versions must pass CodeConvoy's runtime help check.

Primary sources:

- [Parameters](https://cursor.com/docs/cli/reference/parameters): print mode,
  positional prompt, workspace, model, output, force and trust flags.
- [Output format](https://cursor.com/docs/cli/reference/output-format): Cursor's
  own NDJSON events and terminal result contract.
- [Headless mode](https://cursor.com/docs/cli/headless): unattended execution and
  file modification behavior.
- [Permissions](https://cursor.com/docs/cli/reference/permissions): native allow
  and deny configuration and workspace-relative paths.
- [Authentication](https://cursor.com/docs/cli/reference/authentication): existing
  CLI login and status commands.
- [Installation](https://cursor.com/docs/cli/installation),
  [Unix installer source](https://cursor.com/install), and
  [Windows installer source](https://cursor.com/install?win32=true): launchers and
  supported installation locations. Installer scripts were read, never executed.

## Discovery and diagnostics

Cursor's primary name is `agent`; `cursor-agent` is the official legacy alias.
Automatic Cursor discovery considers both, preferring the primary name and
deduplicating canonical targets. Explicit names/paths remain exact. The existing
Find CLI chooser offers discovered candidates without executing them. Help must
identify Cursor and advertise the required flags and `stream-json`; a different
program called `agent` is rejected. Help/version probes use the existing bounded,
cancellable, per-backend check lifecycle.

Unix installers create both launchers in `~/.local/bin`, pointing into versioned
storage under `~/.local/share/cursor-agent`. Discovery uses stable launchers rather
than selecting arbitrary version directories. The Windows installer uses
`%LOCALAPPDATA%\cursor-agent`, copying native launchers when present. Discovery
adds this location and offers only `.exe` files. `.cmd`, `.bat` and `.ps1` launchers
are unsupported; CodeConvoy does not introduce a shell wrapper or a WSL bridge.
macOS uses the existing Unix locations. All platforms permit an explicit absolute
executable path.

Availability means executable compatibility, **not authentication**. CodeConvoy
does not run `agent status` automatically because its documented output includes
account information. It never reads authentication files, adds credential
options, or installs/logs into Cursor. Authentication failure remains a failed
CLI execution, with guidance to use `agent status` / `agent login` separately.
No account output is collected by availability checks.

## Command and permissions

The shell-free command shape is:

```text
agent agent --print --output-format stream-json --workspace=<assigned checkout>
      [--model=<identifier>] [--force] [--trust] -- <one prompt argument>
```

The process working directory and native workspace argument select the same
runner-assigned checkout. In isolated mode this is CodeConvoy's owned detached
worktree at the reviewed committed HEAD. Cursor's own `--worktree`, resume,
continuation and cloud workflows are not invoked. Inherited `GIT_*` overrides and
`PWD` are removed; existing Cursor authentication/configuration variables remain.
There are no scheduler, process, worktree, cleanup or Git safety changes.

The positional prompt transport follows the documented interface; stdin transport
is not assumed. An option terminator protects leading dashes in task text. Its
handling, like the actual CLI invocation, still needs an installed CLI smoke test.
The explicit `agent` subcommand keeps prompts such as `logout` out of root
management-command dispatch. Text attachments use the existing validated context encoding in that argument.
Prompts are visible to local process inspection. NUL bytes and prompts/context of
128 KiB or more are rejected; Windows also checks a conservative escaped command
line bound. Images are explicitly unsupported. No temporary prompt file, image
flag or new attachment protocol is invented.

Blank model preserves Cursor's default. A supplied model is one `--model=value`
argument; CodeConvoy does not invent a shared model list or query account access.

Permissions default to Cursor configuration without `--force`. Users may
explicitly select force, which Cursor documents as allowing operations unless
denied. Workspace trust defaults to saved Cursor decisions; users can explicitly
trust the assigned checkout with `--trust`, useful for fresh isolated paths. These
are separate choices. Neither is enabled by fallback/retry. No automatic MCP
approval or sandbox disabling is supplied. Existing rules/hooks/MCP configuration
still apply; workspace selection and tool permissions are not an OS sandbox.

The official headless guide says writes require force; the parameters/permissions
references describe print mode as having write/shell tools governed by policy.
PR 1 makes no stronger claim: without force, tasks may propose edits without
applying them or be rejected. No interactive approval host is present. Actual
allow/deny, trust, sandbox and organization behavior requires PR 2 validation with
a real installation. Git observations remain separate from agent success.

## Initial result contract

Cursor stdout is NDJSON, independent of stderr. The foundation displays event
records and terminal response text in Activity; the runner retains bounded original Raw output.
Detailed event summaries are deferred to PR 2. Records are bounded at 1 MiB;
oversized stdout or malformed records invalidate completion without treating
fragments as fresh JSON. This is a local limit, not a Cursor protocol maximum.

Success requires process exit zero and exactly one final `type: result` record
with `subtype: success`, boolean `is_error: false`, a string result, nonempty
session identifier, and nonnegative integer `duration_ms` / `duration_api_ms`.
The documented optional request identifier is not required. Explicit error
events, missing/malformed/duplicate results and events after a result fail closed.
An unterminated but complete final JSON record is accepted. Session identifiers
are parsed as protocol evidence only; no sessions are resumed. This confirms CLI
completion, not task fulfillment or file changes.

The existing supervisor owns both pipes, process-tree cancellation, shutdown and
repository lease release. This PR adds no Cursor-specific timeout policy. PR 2
must validate real error shapes, tool denials, session correlation, interruption,
timeouts, shutdown and event sequencing before calling Cursor production ready.

## Validation and platform coverage

Deterministic coverage includes discovery/alias deduplication, required help
flags, explicit permission/trust choices, argument boundaries/Unicode, credential
option rejection, byte-split NDJSON/stderr, malformed/incomplete/error/oversized
results, nonzero exits, missing executable/authentication fixtures, preference
and Store roundtrips, and image rejection. The managed isolated edit test verifies
detached HEAD, fixed base, assigned cwd/workspace, retained changes and preservation
of dirty/untracked source files. Existing all-backend worktree cancellation,
attachment, availability, compact UI and result tests include Cursor where its
capabilities apply. Image workflows retain the four existing transports.

Validation results are recorded in the draft PR. Linux fixture coverage is local;
the existing CI matrix runs Linux/macOS/Windows formatting, Clippy, Rust and
packaging checks. Real Cursor execution is unverified on **all three platforms**.
No authenticated acceptance claim, native macOS/Windows manual result, or release
readiness claim is made by this foundation.

Stop after the draft PR. PR 2 and PR 3 require explicit maintainer authorization.
