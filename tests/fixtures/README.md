# Saved-state fixtures

`opencode-1.19.3-run-help.txt` (printed to stderr), `opencode-2.0.32-run-help.txt`
(stdout), `opencode-1.19.3-run.jsonl` and `opencode-2.0.32-run.jsonl` are unmodified
captures from a sandboxed, unauthenticated run of the official macOS arm64 builds:
`run --help` and a read-tool task in a disposable repository. The test agent behaves
as OpenCode 2 when it is hard-linked as `opencode2`. See [OpenCode validation](../../docs/opencode-validation.md#opencode-2-compatibility).

Cursor's `cursor-help-contract.txt`, `cursor-success.jsonl` and
`cursor-activity.jsonl` are synthetic documentation-derived fixtures, not
authenticated CLI output. Runtime fixture processes emit additional synthetic
failures, denials and sessions for lifecycle coverage.

`cursor-2026.10.16-help.txt`, `cursor-2026.10.16-authentication.stderr.txt` and
`cursor-2026.10.16-syntax.json` contain actual unauthenticated Linux smoke-test
evidence from CLI `2026.10.16-e3cb207`. Only the disposable root is normalized to
`<TEMP>` in the command record. The official temporary installation was removed;
no login, API key or paid task was used. These prove invocation syntax through
the authentication gate, not structured protocol or native permission behavior.
See [Cursor validation](../../docs/cursor-validation.md) for sources, commands,
exit codes, isolation and platform limits.

`antigravity-help-contract.txt` keeps the layout and exact flag set of the
installed agy 1.4.2 help (`Usage of agy:` and a double-dash flag column, printed
to stderr) with placeholder descriptions instead of Google's text.
`antigravity-1.4.2-authentication.stdout.jsonl` and
`antigravity-1.4.2-authentication.stderr.txt` are the unmodified output of
CodeConvoy's invocation of agy 1.4.2 without sign-in; no credentials, URLs or
paths appear in them. `antigravity-success.jsonl`
reproduces the headless guide's recorded stream with sanitized values;
`antigravity-activity.jsonl`, `antigravity-error.jsonl` and
`antigravity-interrupted.jsonl` are synthetic variants of the documented shapes.
None is authenticated CLI output. See
[Antigravity validation](../../docs/antigravity-validation.md#fixtures).

These are synthetic representative saves with fields checked against the local
release tags, not captured user data or recordings produced by old binaries.
All use internal state format 1. No real user's paths, tasks or data were copied.

| Fixture | Release boundary covered |
| --- | --- |
| `state-v0.1.json` | Existing minimal fixture: registrations, per-backend options, draft, concurrency and history |
| `state-v0.2.json` | `v0.2.0`: groups, templates and attachment references |
| `state-v0.3.json` | `v0.3.0`: isolated ownership/results, Applied resolution, appearance and retry/follow-up provenance |
| `state-v0.6.json` | `v0.6.0`: notifications (0.4), completion observations (0.5), tray/close preferences and custom worktree base |
| `state-v0.7.json` | `v0.7.0`: validation commands, completed results and interrupted validation |
| `state-v0.9.json` | `v0.9.0`: explicit retention settings and uncertain ApplyPending result |

The intermediate fixtures project the 0.9 synthetic sample onto the corresponding
tag's `AppState`, `TaskConfig` and `Job` fields, with historical ownership paths and
provenance examples. The 0.7-to-0.8 preset addition did not change those types:
presets save ordinary command values, not preset identities. The 0.9-to-0.10
changes did not change their serialized fields either, so duplicate fixtures for
those releases are unnecessary.

The 0.9 fixture covers repository registrations, a group with a missing member,
templates, all four backend option maps, draft and historical attachments,
appearance/concurrency, notification and tray preferences, history retention,
custom worktree location, validation configuration and results, completion
observations, retained ownership and an uncertain ApplyPending result. A second
run contains unfinished agent and validation work.

`tests/upgrade.rs` checks every supplied field through Store load, atomic save and
reopen. Only the documented interrupted-job/validation recovery fields change;
load itself leaves the input file untouched. Additional default fields are allowed.
These nonexistent fixture paths are never accessed by `tests/upgrade.rs` and saved
ownership remains unverified. `tests/worktree_locations.rs` additionally rebinds
the 0.3/0.6 ownership fixtures to disposable Git repositories and verifies actual
manifests, original-location bindings and retained contents across save/reopen and
fresh reconciliation. No old application or installed agent is executed.
