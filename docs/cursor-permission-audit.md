# Cursor unattended permission audit — 2026-10-20

## Assessment

The upstream change can affect deletion tasks **when Cursor's Auto-review policy
is active**. It does not establish that CodeConvoy's direct print invocation is
incompatible, that every recursive deletion needs approval, or that print mode
waits for interactive confirmation. No such incompatibility was reproduced.

The concrete CodeConvoy gap was an unlimited execution time when Cursor's saved
limit was blank or absent. A CLI or helper that failed to resolve confirmation
could occupy the job until Stop or application shutdown. Cursor now defaults to
3600 seconds through the existing supervisor, matching the application's existing
Copilot time limit. This is application policy, not a Cursor approval rule.
Permissions, command arguments, output parsing and process cleanup are unchanged.

## Upstream evidence and applicability

Cursor's [v2026.10.08 changelog](https://cursor.com/docs/cli/changelog) describes
stricter Auto-review of risky recursive, forced, wildcard and mirror deletions
with uncertain targets, including unset variables and path spacing mistakes.
This concerns the native approval decision, rather than a new CLI transport or
an announced incompatible event format. The same changelog's v2026.06.22 entry
describes Auto-review as a CLI mode selected through a flag or native settings.

The [run-mode documentation](https://cursor.com/docs/agent/security/run-modes)
describes allowlist, sandbox and classifier stages. It does not promise that a
particular deletion will be allowed or that all deletions will be denied.

CodeConvoy's existing invocation is:

```text
agent agent --print --output-format stream-json --workspace=CHECKOUT
      [--model=ID] [--force] [--trust] -- PROMPT
```

The default permission setting omits `--force` and preserves native configuration.
It does not explicitly select Auto-review. Existing user/team policy and CLI
version determine the effective behavior; CodeConvoy cannot infer that policy
from its permission choice. An isolated worktree does not remove native policy
or act as an OS sandbox. The captured
[2026.10.16 help](../tests/fixtures/cursor-2026.10.16-help.txt) advertises
Auto-review, but that historical syntax-only probe did not exercise it.

The [parameter reference](https://cursor.com/docs/cli/reference/parameters)
defines explicit force as allowing commands unless denied, and workspace trust
as a separate choice. CodeConvoy still passes either flag only when requested
in the immutable task settings. The audit adds no permission mode, broad grant,
policy-file change, approval response, or automatic retry.

The [headless guide](https://cursor.com/docs/cli/headless) describes print mode
as noninteractive and says unforced tasks can propose edits instead of applying
them. The [permission reference](https://cursor.com/docs/cli/reference/permissions)
also describes configured allow/deny rules for print-mode tools, with deny taking
precedence. These sources do not specify the exact print-mode outcome of this
new classifier decision for every version and policy.

## Waiting and process ownership

Cursor receives a positional prompt, with null stdin and piped stdout/stderr.
These streams provide no interactive terminal or approval host. A prompt reading
stdin gets EOF, not an automatic affirmative answer. That does not prove a CLI/helper
cannot wait elsewhere or fail to handle EOF.

The [ACP reference](https://cursor.com/docs/cli/acp) explicitly describes a
permission request/response exchange that can block without a response.
CodeConvoy does not use ACP, so its request schema and cancellation RPC cannot
be assumed to apply to direct print mode.

The existing `timeout_seconds` setting now uses 3600 for missing, blank and
whitespace-only values, including old saved drafts and reused settings. Explicit
whole numbers from 1 to 86400 retain their meaning. This intentionally removes
unlimited Cursor execution: legitimate tasks exceeding an hour need a larger
reviewed limit. No CLI-version gate, dependency or persistence migration is needed.

The deadline begins after spawn and excludes queueing/worktree preparation. It
uses the same owned process-tree termination and awaited cleanup as Stop. A
pending approval can remain pending until that deadline or cancellation; it is
not automatically granted. Shared scheduler admission, repository leases,
retained worktrees and cleanup-uncertainty quarantine are unchanged.

## Completion and permission classification

The [output reference](https://cursor.com/docs/cli/reference/output-format)
documents a final successful result and stderr/nonzero-exit failures, possibly
without a terminal event. It does not document a dedicated direct-print approval
RPC or a complete classifier error-code catalog.

The existing decoder already handles the tested cases conservatively:

| Observed evidence | CodeConvoy outcome |
| --- | --- |
| Valid correlated init/tools/result and exit zero | `Succeeded` (UI completion), independently of Git changes |
| Recognized permission denial/approval requirement on stderr or in an accepted error envelope | `Failed`, with permission guidance, even after a successful terminal result |
| Ordinary tool error, explicit agent error or nonzero exit | `Failed`, with the corresponding failure detail |
| Unfinished tool, unknown event, malformed/incomplete stream | `Failed`; unknown permission shapes are not asserted to have a known schema |
| Deadline expires while waiting; cleanup confirmed | `Failed`, timeout cause retained; approval evidence remains in Raw/Activity |
| User cancels while waiting; cleanup confirmed | `Cancelled`, even if permission diagnostics were already decoded |
| Cleanup cannot be confirmed | Failure with repository protection retained |

No parser change was necessary for the supported rejection cases. Recognized
diagnostics remain sticky. Successful tool output, command arguments and
assistant/result prose are not scanned as permission errors. Stderr matching
remains conservative and can classify incidental tool warnings as failures.
Conversely, a refusal expressed only in assistant prose followed by valid success
evidence is CLI completion; CodeConvoy cannot establish task fulfillment from it.
Review the response and Git diff. The audit does not claim universal detection of
future or undocumented denial wording.

## Regression evidence and limits

`cargo test --all-features --test cursor` passed all **37 tests** on macOS.
The same 37 Cursor tests passed within the full suite after the host reboot.

| Required validation | Result |
| --- | --- |
| `cargo fmt --check` | Passed |
| `cargo clippy --all-targets --all-features -- -D warnings` | Passed |
| `cargo test --all-features` | Passed; existing opt-in ignored tests remained ignored |
| `cargo build --release` | Passed |

The full suite ran outside the execution sandbox because an existing storage
test creates a temporary Unix socket, which the sandbox blocks. The first
restricted run failed at that unrelated operation; the post-reboot unrestricted
run passed. No test or application permission policy was weakened.

The added cases cover ordinary successful shell output containing quoted denial
text, recursive deletion rejection, unset-variable/wildcard and spaced-path
ambiguity, mirror deletion, split NDJSON/stderr, stderr-only rejection, unfinished
tools, unknown permission events and denial followed by a success result.

Manager tests cover terminal permission failures and pending confirmation in
both Direct and IsolatedWorktree execution. Pending fixtures have closed stdin,
a live descendant holding an OS lock and retained partial edits. Tests verify
timeout versus cancellation status, descendant termination before successor
admission, released capacity/leases and retained isolated evidence. Existing
shutdown, nested-repository, backpressure and cleanup-failure tests also pass.

**All deletion command strings and rejection payloads are synthetic data.** They
are never executed and do not test Cursor's classifier. Existing parser error
envelopes/codes are exercised defensively, not presented as newly captured
v2026.10.08 output. Only disposable repositories contain test writes.

No `agent` or `cursor-agent` executable was found on PATH or CodeConvoy's usual
macOS discovery paths during this audit. No CLI was installed, credentials read,
login initiated or paid model task run. Real Auto-review decisions, exact denial
wording, EOF behavior and descendant behavior therefore remain unverified. Linux
and Windows were not run for this audit. The earlier Linux syntax smoke test
remains historical evidence only; it does not verify authenticated permissions.
