# CodeConvoy 0.5.0 — Visibility & Review

Unreleased source preparation. No release, tag, package publication or push is
part of this implementation.

## Changes since 0.4.0

- Case-insensitive History search by convoy ID, task, repository and backend,
  with All, Completed, Failed, Cancelled and Needs Review filters. Active convoys
  remain separate; filters preserve selection and history order.
- Compact Review comparison with execution totals, changed/unchanged/unknown
  counts, review attention, per-repository outcomes and recorded duration.
  Filter rows and select a repository to use the existing detailed views.
- New jobs persist completion-time change observations separately from current
  Review and repository health. Direct counts compare against the reviewed HEAD
  and include pre-existing edits. Isolated results retain their fixed-base change
  flag. Older entries show Unknown without reconstructing history from live Git.
- Responsive repository health with branch, Clean/Dirty and availability,
  a shared four-check limit across refreshes, cancellation and stale-reply rejection. Active
  repository work shows Unknown until an explicit refresh after completion;
  unrelated repositories remain available during Apply and single/bulk Discard.
- Review-needed notifications prefer saved completion observations, including
  committed Direct changes and Unknown results. Delivery filters, outcome priority
  and exactly-once tracking remain unchanged.
- Text-backed change and review indicators support System/Dark/Light themes.
  Narrow comparison rows wrap instead of requiring a wide table.

No new dependencies, agent backends, Git mutation actions or background monitoring.
Existing notification policy, scheduling, cancellation, repository safety and result
resolution remain intact. The optional completion field is compatible with existing
version-1 state. Template names are not persisted in historical tasks and are not
searchable. Direct Diff remains live, and isolated completion file counts are not
available. See [behavior and compatibility](visibility.md).

## Validation and remaining release work

macOS Apple Silicon validation passed formatting, strict Clippy, 320 Rust tests
(four optional CLI probes ignored), the release build, 16 packaging tests and
version/tag consistency. A disposable native macOS smoke pass verified search,
filtering, comparison selection and Diff navigation; headless layout checks cover
all requested sizes and themes. Detailed evidence and remaining native checks are
recorded in [visibility validation](visibility.md#validation).
Native Linux/Windows UI checks, macOS Intel execution, release packages and the
[native notification checklist](notifications.md#validation-and-native-smoke-checklist)
remain release checks. No authenticated backend E2E or native notification delivery
is claimed by this feature implementation.

Existing limits remain: external applications are outside CodeConvoy's leases;
unsupported Apply transfers are refused; uncertain Apply retains evidence and has
no in-app uncertainty-resolution flow. Review changes before committing. Package
signing, platform compatibility and installation requirements follow the
[release procedure](releasing.md) and existing validation records.
