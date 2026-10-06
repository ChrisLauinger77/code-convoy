# Codex session investigation

Investigated on **2026-10-06**, with installed **Codex CLI 0.160.1**. No
user-facing Resume/Open action is implemented for CodeConvoy jobs.

CodeConvoy launches independent `codex exec --json --ephemeral` processes with
each selected repository as cwd. A `thread.started` identifier is an event from
that invocation; it does not establish a saved, resumable session. The public
[CLI reference](https://developers.openai.com/codex/cli/reference/) says
`--ephemeral` does not save session rollout files. The existing command contract
is retained. A completed ephemeral job therefore has no persisted rollout for
normal CLI resume; cancellation does not create one either. This conclusion is
based on the documented flag behavior, rather than a private-state inspection.

For persisted CLI sessions, the public
[noninteractive guide](https://developers.openai.com/codex/noninteractive/)
documents `codex exec resume SESSION_ID` and `--last`. Installed `exec resume
--help` accepts an identifier or name; installed interactive `resume --help`
also offers noninteractive-session inclusion and cwd filtering. These interfaces
do not make an ephemeral identifier resumable. CodeConvoy must not run an
unrelated `--last` session on the user's behalf or infer a repository from it.

The current [desktop features documentation](https://developers.openai.com/codex/app/features/)
describes CLI `/app` continuation of the current session. It does not establish
a supported way to open an arbitrary completed CodeConvoy ephemeral job by its
captured identifier. There is no documented desktop link used by CodeConvoy.

| Case | Current finding |
| --- | --- |
| Successful completed job | Its events can contain an identifier; `--ephemeral` prevents a saved resumable rollout. |
| Cancelled job | It may stop before any identifier arrives. The same ephemeral persistence policy applies; retained edits are independent. |
| Simultaneous jobs | Separate per-job decoders and `(run ID, job index)` event routing associate output with the snapshotted repository/cwd; no shared session selection is used. |
| Correct repository on continuation | Execution cwd is explicit today. A future continuation feature would need a persisted session plus verified repository association. |
| Completed/cancelled jobs in desktop | Not established. Documentation does not promise desktop visibility for ephemeral external exec jobs. |
| CLI resume with captured identifiers | Public resume exists for saved sessions. Help-only inspection cannot prove resume of an ephemeral CodeConvoy identifier. |

Validation performed: public `--version`, `exec --help`, `exec resume --help`
and `resume --help`; review of official documentation; deterministic concurrent
job/output/cancellation tests. No authenticated session-creation/resume or
desktop-visibility experiment was run in this change, so those observations are
not claimed. Existing manual Codex execution evidence remains separate.

No authentication files, app databases, private session formats or undocumented
deep links were inspected or manipulated. Changing ephemeral execution to
persist sessions would be a distinct privacy/lifecycle decision and require
successful, cancelled and concurrent authenticated experiments before adding
any continuation buttons.
