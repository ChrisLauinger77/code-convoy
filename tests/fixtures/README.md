# Saved-state fixtures

Cursor's `cursor-help-contract.txt` and `cursor-success.jsonl` are synthetic
documentation-derived protocol fixtures, not captured help or authenticated CLI
output. See [Cursor validation](../../docs/cursor-validation.md) for sources,
version limits and remaining real-platform checks.

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
