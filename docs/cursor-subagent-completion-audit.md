# Cursor CLI subagent completion audit

Reviewed on 2026-10-26 against CodeConvoy `a83386d` and Cursor's official
v2026.10.22 release notes. No runtime incompatibility was confirmed. This audit
adds regression coverage and documentation only; the adapter, parser, permissions,
process supervision, dependencies and release version are unchanged.

## Upstream evidence

Cursor's [official release notes](https://prod.cursor.com/docs/release-notes)
list CLI **v2026.10.22**, dated October 22, 2026. The relevant changes are:

- Waiting on a failed or stopped subagent now exposes the failure to the parent
  instead of treating the child as finished.
- Tool display marks edit, shell and task calls started while arguments are
  still being generated.
- Shell commands Cursor executes in Windows PowerShell now use UTF-8.

The notes describe parent-agent behavior. They do not specify a new subagent
NDJSON schema, promise that every failed child forces a failed parent run, or
define new process exit codes. Inferring any of those would exceed the evidence.
The [output reference](https://cursor.com/docs/cli/reference/output-format)
still documents initialization, messages, tool start/completion records and a
successful terminal result. The
[headless guide](https://cursor.com/docs/cli/headless) still documents print mode
and structured streaming. These are rolling references checked on the audit
date, not a captured v2026.10.22 execution.

The earlier [permission audit](cursor-permission-audit.md) already covers risky
deletes, unresolved approvals, finite deadlines and cleanup. Its implementation
and regressions are reused here; no additional permission mechanism is added.

## Actual adapter exposure

[`Cursor::build`](../src/agents/cursor.rs) launches one local parent invocation:

```text
<executable> agent --print --output-format stream-json --workspace=<assigned checkout> [existing options] -- <prompt>
```

The prompt remains a positional argument, stdin is closed, and the process cwd
is the assigned checkout. CodeConvoy does not launch, address, resume or schedule
Cursor's individual subagents, read their transcript files, or consume hooks,
ACP, SDK or worker APIs.

[`CursorOutput`](../src/agents/cursor/output.rs) consumes the parent's stdout
and stderr. It observes generic tool records, correlating call IDs and tool
names. If Cursor exposes a subagent operation through a supported generic tool
envelope, its start/completion is visible as an ordinary tool call. There is no
child-state machine, per-child job status, or special interpretation of nested
`status`, agent IDs, task output or assistant prose. Nested tool payloads remain
opaque in the bounded Raw output. An explicit generic tool `result.error` is already a
sticky failure, regardless of the tool name or subsequent parent success.

Thus CodeConvoy uses the parent process and its validated structured stream,
with existing generic tool-error checks. It does **not** use exit code alone.
Success confirms CLI completion, not task fulfillment or file changes.
No subagent-specific parsing is necessary on the available evidence.

## Classification and terminal handling

| Observable evidence | Existing CodeConvoy classification |
| --- | --- |
| Valid initialization/session/workspace, correlated completed tools, exactly one valid successful result, exit 0, no diagnostics or invalid records | Succeeded |
| Child failure/stop reported as a generic tool error, even followed by successful parent result and exit 0 | Failed; failed-tool diagnostic |
| Explicit parent error, unsuccessful result, or `is_error: true`, including exit 0 | Failed |
| Parent exits nonzero, including 7 or 130, even after a valid success result | Failed; process exit code preserved |
| Child state appears only inside an opaque success payload or assistant text, followed by otherwise valid parent success and exit 0 | Succeeded; no independent child inference |
| Missing, malformed, duplicate or out-of-order terminal evidence, unfinished tools, or session conflict | Failed |
| CodeConvoy Stop/shutdown wins supervision | Cancelled after confirmed process cleanup; decoded success cannot override it |
| CodeConvoy deadline wins supervision | Failed with timeout detail after cleanup; a later Stop cannot relabel it |
| Cleanup cannot be confirmed | Existing failure/quarantine protects capacity, leases and retained evidence |

A child being stopped does not itself cancel the CodeConvoy job. A parent
process reporting interruption without a CodeConvoy cancellation also fails;
130 is not interpreted as a cancellation request. Only the local supervision
outcome supplies Cancelled. Signals without a numeric exit code still fail
`ExitStatus::success()`.

A terminal JSON record does not complete a running process. The
[`runner`](../src/runner.rs) drains/finishes decoding after
[`process::execute_until`](../src/process.rs) has awaited exit and owned cleanup.
The final complete record may omit its newline. Events after a terminal result,
including another terminal result, invalidate completion. Stderr cannot supply
terminal JSON evidence. Cancellation/timeout classification takes precedence
over the decoder's result.

## Cancellation boundary and Windows UTF-8

Cursor retains `Containment::Group`. On Unix its process group is terminated and
confirmed absent before repository release, including attached descendants that
closed stdout/stderr. On Windows its private Job Object owns the suspended
launch and descendants; native termination and empty-job confirmation are
required. Both normal parent exit and cancellation use owned cleanup. The new
regression cancels after a success record while the parent and a real descendant
remain live, then checks descendant lock release, successor admission ordering
and retained edits in Direct and IsolatedWorktree execution.

The existing [Unix containment limitation](architecture.md#scheduling-and-shutdown)
still applies: a descendant that deliberately escapes into a new session/group
is not contained by this Cursor execution path. No claim is made that real
v2026.10.22 subagents were observed to stay inside it. No detached-group tracking
or process-tree redesign is justified without evidence of such behavior.

CodeConvoy spawns Cursor with native arguments and pipes; it does not use a
PowerShell command string or select a Windows output code page. Windows Cursor
launchers must be native `.exe` files; `.ps1`, `.cmd` and `.bat` are rejected.
However, a supported native Cursor process can itself run PowerShell tools, so
the upstream change can improve non-ASCII tool output arriving through that
parent. This is an indirect supported path, not a reason to add shell wrappers.
Existing stdout JSON, independent stderr and Raw decoding already handle UTF-8
split across reads. The new parser regression also checks Unicode with CRLF and
one-byte chunks. Native Windows PowerShell behavior was not executed here.

## Tests and remaining limits

All new child/task/error/stop payloads are explicitly **synthetic**, built from
the adapter's existing generic envelopes. Their function names, nested fields,
non-success subtypes and messages are not presented as captured Cursor output.
No actual subagent or provider task is launched by these fixtures.

| Coverage | Test evidence |
| --- | --- |
| Successful headless command and assigned cwd/prompt transport | Existing real-process fixture and Direct/isolated manager cases |
| Failed/stopped child exposed as generic error, parent success cannot hide it | New decoder cases and expanded manager matrix in both modes |
| Opaque child success/failure/stop/cancel state and assistant prose | New decoder matrix pins the parent-result boundary without child parsing |
| Parent failure at exit 0; success result with exit 7; exit 130 | Expanded manager matrix asserts status and exact exit codes |
| Structured streaming, terminal requirements, session/tool correlation, bounds | Existing parser tests plus CRLF/Unicode/one-byte new cases |
| Success record before live-parent cancellation | New manager test in both modes, real descendant OS lock and queued successor |
| Cancellation/shutdown/deadline, peers/nested repos, cleanup uncertainty | Existing Cursor lifecycle and native process suites |
| Destructive-operation permission handling | Existing rejection, pending-confirmation and timeout regressions rerun unchanged |

Local validation used Linux with Rust/Cargo 1.97.1:

- `cargo fmt --check` — passed.
- `cargo clippy --all-targets --all-features -- -D warnings` — passed.
- `TMPDIR=/var/tmp cargo test --all-features --test cursor --test platform_process`
  — passed: 40 Cursor tests and 11 native process tests, no failures or ignores.
- `TMPDIR=/var/tmp cargo test --all-features` — passed: 823 tests, zero failures,
  15 existing opt-in tests ignored; nested subprocess reruns excluded from this
  count. This includes the existing permission-handling regressions.
- `cargo build --release` — passed.

PR preparation also passed `cargo clippy --locked --all-targets --all-features
-- -D warnings`, `cargo build --locked --release --bin codeconvoy`, and
`python3 -m unittest discover -s packaging -p 'test_*.py'` (120 tests).
`TMPDIR=/var/tmp cargo test --locked --all-features` also passed with the same
823 passed / 15 ignored totals.

The initial focused run inside the sandbox passed 31 Cursor tests and failed
nine isolated cases at the existing worktree-location safety check: the
sandbox's synthetic `/tmp` Git ancestor prevents an outside-repository worktree
location. The successful rerun used `/var/tmp` outside the sandbox, matching
earlier validation records; no application safety check was weakened.

Neither `agent` nor `cursor-agent` was found on PATH or in the checked local
installation directories. No installation, login, credential inspection or
authenticated model execution was performed. The successful headless,
failed/stopped-child and parent-failure tests therefore establish CodeConvoy's
fixture-driven behavior, not real Cursor v2026.10.22 propagation or exit codes.
The earlier `2026.10.16-e3cb207` syntax/authentication-gate capture remains
historical evidence only. Native macOS/Windows execution and real Cursor
subagent cleanup remain unverified.

Residual risks are the existing opaque-payload boundary (a parent may recover
or still report success), conservative rejection of any explicit tool error even
if the parent recovers, strict rejection of future event variants, and Unix
escaped descendants. None is evidence of a newly confirmed v2026.10.22
incompatibility. No runtime fix or release bump is warranted by this audit.
