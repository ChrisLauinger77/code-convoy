# History export — Markdown and JSON

Use **Export…** beside **Copy Summary** for a terminal convoy, or **Export
History…** in RUNS / RESULTS. Choose a scope and format, then **Export…** opens
the native save dialog. Suggested names are `codeconvoy-convoy-42.md` or
`codeconvoy-history-2026-10-11.json` (the history filename uses the UTC date).
The selected format controls the contents; retain the suggested extension when
naming files. The exact chosen destination is used.

## Scopes and snapshots

- **Selected convoy #…**: the convoy selected when the export dialog opened,
  including one outside the history filters. CodeConvoy has single-convoy
  selection; this PR does not add multi-selection.
- **Filtered history**: all terminal entries matching the current history search
  and status filter when Export is confirmed. Repository comparison filters do
  not restrict the report's repository rows.
- **All retained history**: every terminal entry, including entries hidden by
  search/filtering or protected from retention because they own retained results.

Active convoys are excluded. Empty scopes cannot export. Removed selected IDs
produce an error; selection never falls back to another convoy. Failed, cancelled,
interrupted and legacy records can be exported, including empty legacy records.
Convoys keep the history's existing order, and repositories keep original job order.
Repeated IDs are included once. Each operation produces one file.

Confirming Export captures a consistent, metadata-only snapshot **before** opening
the save dialog. Changes to history, search, validation records or result resolution
after that point do not change the report. Export never reads current Git state,
starts validation, modifies history or triggers retention. It can run alongside
ordinary saves and history cleanup. Previously exported files remain independent
of Remove, Clear and retention; there is no automatic export before cleanup.

## Markdown

Single exports use the same formatter as **Copy Summary**. Multiple exports add a
History heading, total count and separated convoy sections. Reports include the
convoy ID, outcome, backend, bounded task summary, recorded times, duration,
repository count, success/failure/cancellation/change/review counts, validation
counts and a compact repository table. Review resolution and agent outcome remain
separate. Validation is the latest saved result, not a certification of current files.
Unknown observations are labelled Unknown; partial change totals are labelled known.

## JSON schema 1

JSON is an explicit report model, independent of the internal state-file format.
It is UTF-8 with stable field names and deterministic array order. There is no
export-time field, so identical snapshots/options produce identical output.

- `schema_version`: integer `1`.
- `export_type`: `single_convoy` for a selected convoy; `history` for filtered/all
  scopes (even when they contain one convoy).
- `convoys`: ordered array of convoy records. `id` is the existing numeric convoy ID.
- `status`: `succeeded`, `failed`, `cancelled`, `interrupted`, or `no_jobs`.
  Failed takes precedence over cancelled, matching the UI.
- `backend`: `codex`, `copilot`, `opencode`, or `claude`.
- `task_summary`: first nonblank task line, bounded to 160 characters plus ellipsis.
- `created_at`, `started_at`, `completed_at`: RFC 3339 UTC strings with second
  precision, or `null` for missing/out-of-range dates. Start is the first recorded
  job start. Completion requires a finish for every job. Convoy `duration_ms` is
  creation-to-completion time (including queue time), not summed execution time.
- `summary`: repository, outcome, interrupted, change and review counts. Interrupted
  is a subset of cancelled in ordinary recovered records.
- `repositories`: original job order, with bounded name, status, `interrupted`,
  start/finish/duration, `changes`, `review` and `validation`.
- Repository `status`: `succeeded`, `failed`, or `cancelled`; `interrupted` is a
  separate boolean. Repository duration uses recorded job start and finish.
- `changes.changed`: boolean or `null`; `changes.changed_files`: integer or `null`.
  These are independently optional saved completion observations. Unknown is never
  changed to zero or reconstructed from live Review or mutable worktree observations.
- `review`: `required`, `not_required`, `applied`, `discarded`, `apply_pending`,
  `discard_pending`, or `unavailable`, using the existing Copy Results rules.
- `validation`: `null` means not run / not recorded (not “not configured”). Otherwise
  includes `status` (`passed`, `failed`, `cancelled`, `unavailable`, `running`), a
  bounded display-only `command`, `started_at`, `completed_at`, recorded
  `duration_ms` and `exit_code`. Running validation can exist on a terminal convoy;
  its duration is only the latest saved observation. Commands are not executable
  shell scripts. Missing exit codes and finish dates are `null`.

All optional fields are present with `null` for missing information. Recorded zero
counts/durations/exit codes stay zero. Durations are unavailable when timestamps
are absent, out of range or reversed. No current-time estimate is invented.

Small example (one successful repository with no recorded validation):

```json
{
  "schema_version": 1,
  "export_type": "single_convoy",
  "convoys": [
    {
      "id": 42,
      "status": "succeeded",
      "backend": "codex",
      "task_summary": "Update documentation",
      "created_at": "2026-10-10T12:00:00Z",
      "started_at": "2026-10-10T12:00:10Z",
      "completed_at": "2026-10-10T12:04:32Z",
      "duration_ms": 272000,
      "summary": {
        "repositories": 1,
        "succeeded": 1,
        "failed": 0,
        "cancelled": 0,
        "interrupted": 0,
        "changed": 1,
        "unchanged": 0,
        "unknown_changes": 0,
        "needs_review": 1
      },
      "repositories": [
        {
          "name": "example-project",
          "status": "succeeded",
          "interrupted": false,
          "started_at": "2026-10-10T12:00:10Z",
          "completed_at": "2026-10-10T12:04:32Z",
          "duration_ms": 262000,
          "changes": { "changed": true, "changed_files": 8 },
          "review": "required",
          "validation": null
        }
      ]
    }
  ]
}
```

## Privacy

Both formats include task/name metadata, IDs, backend, execution/validation
outcomes, times, saved change counts and review resolution. JSON also includes
per-job times and the latest validation's bounded command, exit code and timing.
Markdown's convoy table includes validation status only; Copy Repository Result
continues to include the command and timing.

Neither format includes raw agent output, Activity logs, diagnostics, diffs,
validation logs, environment variables, backend settings, attachment references or
contents, repository/worktree directories, ownership records, Git identities or
internal recovery metadata. Export does not read repository or attachment files.

The existing Copy Results policy is shared: repository names are bounded to 80
characters; task summaries to 160. Command display uses an executable bounded to
80 characters, at most 24 arguments of 160 characters each and a final 320-character
limit (plus ellipsis). Recognizable absolute/home-relative paths in free-text fields
use the existing `[local path omitted]` marker; absolute command executables use
the basename. No new secret-redaction rules are introduced.

**Review before sharing.** Names, task text and command arguments may contain
sensitive information or credentials. Omission of structural secret/log fields and
recognizable paths is not a guarantee that the report contains no sensitive text.

## File handling and platforms

Formatting and writing run on a worker. JSON streams directly; Markdown builds one
convoy section at a time. No large logs or diffs are cloned. One export can run at
a time, with inline progress and Cancel. Cancel/Escape in either dialog writes
nothing. Cancellation during writing removes the temporary file before publication;
a cancellation arriving after the final replacement cannot undo the saved file.

Output is written to a temporary file in the destination directory, flushed and
synced, then published. Failed writes (including full disks), failed replacements
and cancellations clean up temporary files during normal error handling. Existing
files require **Replace file** confirmation, with Cancel initially focused, even
if the native picker already prompted. Changed destinations fail with a retry
message. New files use no-clobber publication. Symlink/reparse-point targets,
directories and CodeConvoy's own application-data directory are rejected. Errors
leave the existing destination intact. Unexpected process termination can leave
an orphan temporary file; there is no directory scan/cleanup framework.

The existing rfd native dialog is reused: XDG desktop portal on Linux (including
GNOME/Wayland), AppKit on macOS, and the Windows file dialog. Native extension and
overwrite prompts vary, so the application performs its own replacement review.
Native picker cancellation may also be reported by the library when a platform
picker is unavailable; the report is not written. Cancel export does not forcibly
close an already-open OS dialog; dismiss it to finish cancellation and quit.

Exports are portable reports, **not full-fidelity backups**. There is no import,
restore, scheduled export, synchronization, log/diff export or storage management.
See [validation coverage and remaining native checks](history-export-validation.md).
