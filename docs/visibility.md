# Visibility and review — 0.5.0

## History

The search field and compact status selector apply only to History. Active
convoys remain in their own selector section. Matching uses case-insensitive
substrings of convoy ID (with or without `#`), task text, repository name/path and
backend name. The saved task does not include a template name, so template names
are not searchable. Clearing filters restores the original history order.

Completed means every job succeeded. Failed and Cancelled follow the convoy's
existing aggregate status: a failure takes precedence over cancellation. Needs
Review is independent of execution outcome. Empty matches are explicit. Filtering
keeps the selected convoy and repository open, with a notice when they fall outside
the filter. Removing history continues to use the existing ownership protections.

## Comparison and review attention

Review shows total jobs, succeeded, failed, cancelled, changed, unchanged, unknown
and review-needed counts. Active/queued jobs can still be unknown; their presence
never creates a success result. Repository rows preserve execution order and show
execution outcome, completion change state, review attention and recorded duration.
All repositories / Changed / Failed / Needs Review narrow the rows. Repository
selection opens its details below, with the existing Activity, Diff, Raw output
and Task & settings actions. Follow-up selection remains independent.

Review is needed for terminal results with changed or unknown completion data,
or unresolved retained isolated copies, including unchanged retained copies that
still need an explicit disposition. Applied/Discarded results are resolved. This
indicator does not add a new execution status or a Direct “mark reviewed” action.
Failure alone is shown by the Failed outcome/filter. Unknown information never
means no changes.

New Direct jobs capture distinct changed paths against the HEAD reviewed for that
job, including the current committed HEAD, working files, index-only alternatives
and nonignored untracked files. Committing changes and then staging the reviewed
contents again still reports Changed. This includes pre-existing edits and committed
changes since that HEAD; it is not attribution of every edit to the agent. Renames count as deletion plus
addition. An unborn reviewed HEAD has an empty tracked baseline; a new first
commit remains visible even if every committed path is then staged for deletion.
New isolated jobs retain the runner's fixed-base changed/unchanged observation; a reliable
completion file count is not captured for them and is omitted.

Counts are observed after agent/process cleanup and before releasing the execution
lease. External tools are not locked out. Failed and cancelled agents can have
changes. Unstarted, interrupted, unsafe-cleanup, failed-inspection and timed-out
observations stay Unknown. The observation adds at most a five-second inspection
budget plus required subprocess cleanup to Direct completion.

## Historical compatibility

The optional `completion_changes` field stores only a change flag and optional
file count in the existing version-1 JSON. Older history loads without migration
and shows Unknown. Neither today's working tree nor older mutable retained-result
observations can reconstruct the missing completion state. Template metadata,
historical line statistics and historical Direct patches are not invented.

Live Review statistics and Diff remain current inspections, labelled separately.
Refresh, restart recovery, Apply, Discard and retained-copy cleanup do not rewrite
completion observations. Current health can be Clean while a saved result remains
Changed. Stable isolated Diff and all existing ownership/uncertainty protections
continue to apply. No dependency or backend contract is added.

Completion notifications use this saved observation too, including committed
Direct changes and recorded Unknown results. Later clean or unavailable working
trees cannot replace it. Only jobs without an observation use the older live
fallback; saved history never triggers a new notification on restart.

## Current repository health

Repositories in New Convoy show path, branch (including detached HEAD), Clean/Dirty,
and Available/Unavailable. They refresh when this main interface opens at startup
or with **Refresh state**. Up to four asynchronous checks run at once across all
refresh requests, including checks still cleaning up after cancellation. The draft
and result navigation remain usable. Newer refreshes cancel older checks and reject
late replies, as do unregistering and overlapping execution events. Superseded
queued checks cancel without waiting for capacity; new checks wait for running
checks to finish cleanup before reusing their slots.

Known active or nested repository work shows Unknown; refresh after it finishes.
Apply and Discard suppress only their source repository and overlapping paths.
Bulk discard uses the currently executing result's source; unrelated checks and
their pending replies remain available. Starting or finishing a result operation
invalidates overlapping cached state and late replies.
Read-only Git commands use the existing helpers with optional locks and fsmonitor
disabled. They acquire no scheduler capacity or repository execution locks. Missing
paths, invalid repositories, access failures and Git errors display Unknown with
an expandable diagnostic; failed checks are never guessed to be clean/detached.
This is a requested observation, not continuous monitoring or a preflight substitute.

## Validation

Deterministic coverage is in `tests/visibility.rs`, `tests/repository_health.rs`,
`src/git.rs` completion tests and `src/ui/visibility_tests.rs`. Real disposable Git
repositories verify status/counts without changing the real index, HEAD or files.
Runner tests verify completion observations precede terminal events for both
success and failure. UI tests cover stale results, selection and all three themes
at 780 × 560, 1180 × 820 and 1600 × 1000.

On 2026-10-09, macOS Apple Silicon source validation passed formatting, strict
all-target/all-feature Clippy, **320 Rust tests** (four optional installed-CLI probes
ignored), the optimized release build, **16 packaging tests**, and the `v0.5.0`
version/tag consistency check. The Rust harness additionally launches two existing
subprocess tests; those duplicate invocations are excluded from the total above.

A disposable native macOS fixture at 1180 × 820 verified rendered outcome/change
indicators, no-match search with selection preserved, Clear filters, Failed
repository filtering, repository selection and the existing Diff refresh showing
untracked changes. Clean/Dirty/Unavailable health states were also observed.
Automated headless egui layout checks cover all three sizes and System/Dark/Light;
full native resizing, light-theme and keyboard smoke passes at all sizes remain
before release. Native notification delivery was not tested in this fixture.

The native fixture is launched with:

```sh
cargo run --features test-support --example v05_visibility_validation -- 1180 820
```

It creates disposable repositories/state and never invokes installed coding agents.
Repeat with `780 560` and `1600 1000`. Check search/no-match/Clear, status and
comparison filters, selection, detail tabs, Refresh state, keyboard focus,
System/Dark/Light, and scroll reachability. Missing fixture CLIs are intentional.
Native Linux and Windows checks, macOS Intel execution, package installation and
the v0.4 native notification checklist remain required before publication.
