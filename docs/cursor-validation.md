# Cursor CLI evidence and reliability — PR 2

Evidence reviewed on 2026-10-17. PR 1 supplied the backend foundation; PR 2 adds
structured Activity, completion validation and supervised lifecycle coverage.
Cargo remains at 1.0.0. Final user guidance and release readiness belong to PR 3.

## Evidence and limits

The maintainer accepted an isolated **Linux x86_64 syntax smoke test with Cursor
CLI `2026.10.16-e3cb207`**. The PR 1 invocation is unchanged. This proves argument
parsing through the authentication gate, not authenticated execution, structured
protocol compatibility, model access or permission enforcement. No paid agent
task, login or API key was used. No minimum supported CLI version is inferred.

Structured success, tool, permission and session fixtures remain synthetic,
derived from Cursor's documentation. Only the saved help, unauthenticated error
and syntax probe records below came from an installed Cursor CLI. A real
successful task and native allow/deny/trust behavior remain unverified on every
platform. CodeConvoy checks installed help at runtime.

Primary sources:

- [Parameters](https://cursor.com/docs/cli/reference/parameters): print mode,
  positional prompt, workspace, model, output, force and trust flags.
- [Overview](https://cursor.com/docs/cli/overview#non-interactive-mode): the
  alternative root print-mode form; both forms passed syntax parsing.
- [Output format](https://cursor.com/docs/cli/reference/output-format): Cursor's
  NDJSON initialization, messages, tool calls and terminal result.
- [Headless mode](https://cursor.com/docs/cli/headless) and
  [permissions](https://cursor.com/docs/cli/reference/permissions): unattended
  behavior and native allow/deny configuration.
- [Authentication](https://cursor.com/docs/cli/reference/authentication): external
  CLI login/status, separate from executable availability.
- [Installation](https://cursor.com/docs/cli/installation),
  [Unix installer](https://cursor.com/install), and
  [Windows installer](https://cursor.com/install?win32=true): official launchers
  and supported installation locations.

## Accepted isolated syntax test

The official Unix installer was downloaded and inspected in full before
execution. Its SHA-256 was
`9b70b142b7bbd56d5d75c2575279b63c884178f8f8e6862b4f41c09afcf6c8a2`.
It fetched the official [versioned Linux package](https://downloads.cursor.com/lab/2026.10.16-e3cb207/linux/x64/agent-cli-package.tar.gz).
The installation lived under a disposable `/tmp` directory with isolated HOME,
XDG configuration/cache/data/state/runtime directories and a child-only PATH.
The child environment was cleared, no credentials were supplied, and the
version's inspected file credential-store setting kept probes inside that home.
There was no sudo, system installation, real HOME/profile modification or
permanent PATH change. The temporary installation and root were removed and
removal verified. The CodeConvoy repository was unchanged by that test.

The checked-in [probe record](../tests/fixtures/cursor-2026.10.16-syntax.json)
contains all 14 argument arrays, working directories, exit codes and relevant
output. Only its disposable root is normalized to `<TEMP>`. Full captured
[root help](../tests/fixtures/cursor-2026.10.16-help.txt) and the captured
[authentication diagnostic](../tests/fixtures/cursor-2026.10.16-authentication.stderr.txt)
are separate fixtures. In this table `agent` denotes the temporary launcher and
`W` the temporary workspace containing spaces; prompts are single arguments.

| Command / probe | Exit | Observed output |
| --- | --- | --- |
| `agent --version` | 0 | `2026.10.16-e3cb207` |
| `agent --help` | 0 | Cursor help, required flags, `agent [prompt...]` |
| `agent agent --help` | 0 | `Usage: agent agent [options] [prompt...]` |
| `agent agent --print --output-format stream-json --workspace=W --model=gpt-5 --force --trust --help` | 0 | Subcommand help; global flags after subcommand accepted |
| `agent agent --print --codeconvoy-invalid-option` | 1 | Unknown option: syntax error (negative control) |
| `agent agent --workspace` | 1 | Missing workspace argument: syntax error (negative control) |
| `agent status` | 0 | `Not logged in` |
| `agent agent --print --output-format stream-json --workspace=W -- PROMPT` | 1 | Authentication required; exact default CodeConvoy shape |
| Same with `--model=gpt-5 --force --trust` before `--` | 1 | Authentication required |
| Same default with prompt `--codeconvoy-invalid-option` after `--` | 1 | Authentication required; literal prompt accepted |
| Same default with prompt `logout` after `--` | 1 | Authentication required; no management-command dispatch |
| `agent --print --output-format stream-json --workspace=W -- PROMPT` | 1 | Authentication required; overview form accepted |
| `agent agent --print --output-format stream-json --workspace W -- PROMPT` | 1 | Authentication required; separate workspace value accepted |
| `cursor-agent agent --print --output-format stream-json --workspace=W -- PROMPT` | 1 | Authentication required; legacy launcher accepted |

No probe timed out. The valid invocation paths returned this stderr diagnostic,
with empty stdout, rather than either negative control's parser error:

```text
Error: Authentication required. Please run 'agent login' first, or set CURSOR_API_KEY environment variable.
```

Thus authentication failures are distinguished from syntax failures. The smoke
test did not observe any authenticated NDJSON, native tools or file edits.

## Discovery and invocation

Cursor's primary executable is `agent`; `cursor-agent` is the official legacy
alias. Automatic discovery prefers the primary name and deduplicates canonical
targets; explicit names/paths remain exact. Unix stable launchers are under
`~/.local/bin`, backed by versioned `~/.local/share/cursor-agent` storage. Windows
adds `%LOCALAPPDATA%\cursor-agent` and offers only native `.exe` launchers;
`.cmd`, `.bat`, `.ps1`, shell wrappers and WSL bridges are unsupported. macOS uses
the existing Unix locations. All platforms allow an absolute executable path.
Discovery does not execute candidates or install anything.

Help must identify Cursor, advertise `agent [prompt...]`, and contain the
required flags and `stream-json`. Bounded cancellable help/version probes verify
executable compatibility, not authentication. CodeConvoy does not automatically
run `agent status`, collect account output, read credential files, add credential
options or initiate login.

The unchanged shell-free invocation is:

```text
agent agent --print --output-format stream-json --workspace=<assigned checkout>
      [--model=<identifier>] [--force] [--trust] -- <one prompt argument>
```

The process cwd and workspace argument select the same runner-assigned checkout.
In isolated mode this is the owned detached worktree at the reviewed committed
HEAD. Cursor's own worktree/resume/continuation/cloud workflows are unused.
Inherited `GIT_*` overrides and `PWD` are removed; existing Cursor auth/config
variables remain inherited without being copied into application diagnostics or
persisted options. Session IDs are protocol evidence, never resumption handles.

The positional prompt and validated text context are visible to local process
inspection. NUL bytes and prompts/context of 128 KiB or more are rejected; Windows
also has a conservative escaped command-line bound. Images are unsupported.
Blank model preserves Cursor's default; a supplied model is one `--model=value`
argument. CodeConvoy does not query model/account access.

## Permissions and headless behavior

Configured permissions omit `--force` and preserve Cursor's native global
`~/.cursor/cli-config.json` and project `.cursor/cli.json` allow/deny settings.
Explicit force adds only the documented `--force` (allow unless denied).
Workspace trust is separate: configured preserves saved decisions; explicit
trust adds `--trust` for the assigned checkout. Neither setting is enabled by
fallback, cancellation, timeout or retry. No interactive approval host, automatic
MCP approval or sandbox-disabling flag is added. Native rules/hooks/MCP behavior
continues to belong to Cursor; PR 3 must investigate rules acceptance.

The official headless guide says writes require force; the parameters/permissions
references describe print-mode tools governed by policy. Without force, a task
may propose changes, wait or fail when approval is needed. CodeConvoy preserves
this distinction and reports recognized rejection/approval diagnostics without
retrying with broader permissions. Real native denials/trust errors have not
been captured, so those regressions use explicitly synthetic cases.

Workspace selection and permissions are not an OS sandbox. CodeConvoy verifies
the reported initialization cwd against the canonical assigned checkout and
fails completion on a mismatch or unavailable identity. This detects conflicting
protocol evidence; it cannot confine a trusted native CLI's filesystem access.
Git observations remain separate from agent success.

## Structured Activity and completion

Stdout is bounded NDJSON; stderr is independently streamed as labelled text with
split UTF-8 handling. Stderr cannot supply JSON completion evidence. Activity
summarizes initialization (session/model/reported permission mode), task context,
assistant text, correlated tool start/completion and the final response. Tool
summaries show native names and reported paths, not entire tool arguments, file
contents or environment dumps. Bounded Raw output retains original backend
bytes separately. Application-generated Activity messages use `[CodeConvoy]`.

Success requires all of the following, rather than exit zero alone:

- Exactly one initialization with a nonempty typed session ID and assigned cwd.
- Consistent session IDs on messages, tools and the terminal result.
- Every observed tool start matched by one completion with the same native tool
  and call ID; no unfinished calls, explicit errors or recognized rejections.
- Exactly one terminal `type: result`, `subtype: success`, boolean `is_error:
  false`, string `result`, and nonnegative integer `duration_ms` /
  `duration_api_ms`, followed by no events, plus process exit zero.

The optional request ID is not required. A complete final record without newline
is accepted. Unknown fields are ignored; unknown event types, unsupported content
or tool-result shapes, invalid/missing/duplicate events, conflicting sessions,
and malformed/incomplete output fail closed. Records are capped at 1 MiB and
unfinished tool tracking at 128 calls. Oversized fragments are discarded through
the next newline and never reparsed as new records. These are local limits.

Explicit tool errors remain failures even if Cursor subsequently reports success;
this conservative policy does not infer that an assistant recovered the task.
Recognized authentication, approval/permission and trust diagnostics from stderr
or explicit error payloads produce actionable failure details. Other nonzero
exits, invalid output and missing completion have distinct failure details.
Diagnostic matching is conservative text matching, not proof of a stable native
error schema. Recognized stderr phrases fail closed even in an otherwise harmless
tool warning followed by exit zero and a valid result; stderr does not identify
its emitter. Regression cases explicitly pin this tradeoff. Assistant prose is
never searched for these diagnostic phrases.
Unrecognized errors remain visible in Activity/Raw and cannot turn a nonzero exit
into success. A successful result confirms CLI completion, not task fulfillment
or file changes. The strict documented contract may reject future event variants;
no full protocol compatibility claim is made without authenticated validation.

## Cancellation, time limits and repository safety

The existing shared process group / Windows Job Object owns Cursor and its
attached descendants. Stop and shutdown take precedence over decoding; they
await cleanup before releasing scheduler capacity or repository leases. Existing
Unix escaped-descendant/crash limitations still apply (see architecture).

Cursor adds an optional **CodeConvoy** time limit of 1–86400 seconds; blank is
unlimited. It starts after spawn, excluding queueing and worktree preparation,
and changes no CLI arguments. Supervision starts concurrently with Started/Running
publication: blocked delivery cannot postpone pipe draining/reaping or relabel an
already completed process as a timeout. Output delivery preserves Started ordering
and bounded omission accounting while decoding every byte. The supervisor uses
the same process-tree cleanup as cancellation under UI backpressure. Confirmed
expiry fails the job and retains all edits/results. Cancellation wins when both
signals are ready and interrupts blocked Started/Running publication. A closed
Started receiver signals job cancellation; a non-fail-fast join awaits execution
and owned cleanup before returning the publication error. Cleanup uncertainty
takes precedence over that error. A cleanup error preserves the existing quarantined lease,
capacity and history protection; later observations do not retroactively declare
cleanup safe. Existing backends retain unlimited execution by default.
The cause selected by execution remains authoritative through cleanup and result
inspection: Stop/shutdown arriving after timeout expiry cannot turn that timeout
into Cancelled, in either Direct or isolated mode.

Scheduler fairness, global/per-convoy limits, canonical/nested repository locks,
Git baseline revalidation, worktree ownership, explicit cleanup and result
transactions are unchanged. Success, ordinary failure, cancellation, timeout and
shutdown retain isolated checkouts and manifests. No automatic delete/prune,
Direct fallback, source edits, permission escalation or agent restart is added.
Concurrent isolated results can run independently; Direct access waits until all
conflicting work is safely finished.

### Windows cleanup confirmation

CI on `4d8792c` passed all 31 Cursor tests but failed the process deadline test:
the expired-deadline test observed a descendant's OS lock still held immediately
after execution returned. A retained file lock alone does not prove a live process
on Windows: Microsoft's [LockFileEx documentation](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-lockfileex)
permits lock release to lag process termination. Independent inspection of process-wrap 10.0.1 found that its
Windows helper accepts any job completion-port notification and caches the
leader exit before confirming the job is empty.

The Windows regression now checks native process termination immediately when
execution returns, using a read-only process-status probe of the PID recorded by
the fixture before its readiness handshake. An absent PID or a signaled process
is accepted; other access errors fail the test. The probe never signals a PID,
and PID reuse can only produce a conservative failure, since the original fixture
must already have terminated. Test handles are not retained across execution.
Only the separate filesystem lock-release observation has a bounded wait. Unix
retains its immediate lock assertion. This follows Microsoft's [process termination
semantics](https://learn.microsoft.com/en-us/windows/win32/procthread/terminating-a-process).

`process/windows` now creates an unnamed, non-inheritable private outer job and
assigns the suspended child before the existing wrapper resumes it. Supervision
waits through the owned native child handle. It snapshots up to 4096 private-job
members (including nested jobs), opens read-only wait handles and rechecks their
membership to handle PID reuse. It always terminates the private job, awaits the
members' native process signals, releases the wait handles, and requires the
private job's active-process count to reach zero within the cleanup deadline.
An incomplete/oversized list fails closed. Cancellation targets that same private job.
Termination/query errors and timeout retain repository/history protection;
Drop/kill-on-close remain unconfirmed fallbacks. Normal leader exit with a
closed-stream descendant is now tested on Windows too. No inherited host job,
persisted PID or unrelated process is targeted.

On `6cbda50`, Windows passed the expired-deadline/cancellation-precedence test
but failed the new normal leader-exit test's immediate native descendant check.
The follow-up removes the accounting-only early return and adds the explicit
member-handle wait above. Its native CI result is recorded in the PR.

This follows Microsoft's [nested-job accounting and termination](https://learn.microsoft.com/en-us/windows/win32/procthread/nested-jobs)
and [active-process accounting](https://learn.microsoft.com/en-us/windows/win32/api/winnt/ns-winnt-jobobject_basic_accounting_information).
[Job member lists](https://learn.microsoft.com/en-us/windows/win32/api/winnt/ns-winnt-jobobject_basic_process_id_list)
include nested jobs; [membership checks](https://learn.microsoft.com/en-us/windows/win32/api/jobapi/nf-jobapi-isprocessinjob)
require limited query access in addition to synchronization access for waiting.
[Completion-port messages](https://learn.microsoft.com/en-us/windows/win32/api/winnt/ns-winnt-jobobject_associate_completion_port)
are notifications, not unconditional evidence that all members have exited.
The existing `windows` dependency's API features are explicit in Cargo; versions,
resolved features and dependency edges remain unchanged for Linux, both macOS
architectures and Windows. The reviewed inventory fingerprint is updated; no
new license/source payload is introduced. Native validation results belong in
the PR, separately from the unauthenticated Cursor syntax evidence.

## Deterministic validation and platform coverage

The Cursor tests exercise the production manager, supervisor and decoder using
local fixture processes and disposable real Git repositories, without providers:

- Success, nonzero failure, executable loss after review, captured missing-auth
  diagnostic, malformed JSON, missing result, interruption and permission denial.
- Byte-split stdout/stderr, Unicode, session/workspace conflicts, tool correlation,
  bounded records/tracking, extra fields and errors despite a successful result.
- Direct and isolated failures retain edits, release confirmed-safe slots/leases
  and permit a later job; isolated results preserve fixed bases and dirty sources.
- Cancellation stops descendants before nested repository admission; unrelated
  jobs and concurrent isolated results remain independent.
- Shutdown, timeouts, a full Started-event queue and unconfirmed cleanup use OS
  file-lock handshakes. The Windows platform deadline and normal-leader-exit
  regressions additionally require immediate native descendant termination;
  file-lock release may lag on that platform as documented above.
- An already-expired deadline cleans up a live descendant, and a ready cancellation
  wins the deadline race. Permission/trust defaults and the verified argument
  arrays remain unchanged, including when a time limit is configured.
- A controlled cleanup gate admits Stop/shutdown only after the deadline has
  won. All four Direct/isolated and Stop/shutdown combinations remain Failed
  with the timeout diagnostic, retain results, and release confirmed-safe leases.
- A completed process (both successful and nonzero exits) retains its real result
  while Started delivery stays blocked beyond the deadline. OS exit is established
  by a fixture lock released by process termination. This regression first
  reproduced the review finding as an exit-zero job misclassified as timed out.
- Closing a blocked Started receiver requests cancellation, awaits descendants
  and releases confirmed-safe history/lease protection. It never drops a running
  supervision future through a publication error. This regression first reproduced
  the fail-fast join's unsafe drop/quarantine path. Explicit cancellation also
  interrupts Started publication without emitting a delayed Started event.

Existing all-backend process, convoy, worktree, attachment, persistence, compact
UI, results and packaging suites remain relevant. On 2026-10-17, local formatting,
strict Clippy, all-feature Rust tests and the release build passed, including
32 focused Cursor tests and 98 packaging tests. Rust integration tests used
`TMPDIR=/var/tmp` to avoid the execution sandbox's synthetic `/tmp` Git boundary.
Commands and CI results are also recorded in the PR. Local process/Git coverage
is Linux. The existing CI matrix
runs Linux/macOS/Windows checks with native fixture processes; it is not real
Cursor execution. macOS/Windows Cursor smoke tests and authenticated acceptance
remain unperformed. No release-readiness claim is made.

Stop after the PR 2 draft for maintainer review. Do not start PR 3 or merge
without explicit authorization.
