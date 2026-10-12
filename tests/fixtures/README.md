# Saved-state fixtures

`state-v0.1.json` is the existing minimal legacy fixture. `state-v0.9.json` is a
synthetic, hand-authored representative save checked against the types at tag
`v0.9.0` (state format 1). No real user's paths, tasks or data were copied.

The latter covers repository registrations, a group with a missing member,
templates, all four backend option maps, draft and historical attachments,
appearance/concurrency, notification and tray preferences, history retention,
custom worktree location, validation configuration and results, completion
observations, retained ownership and an uncertain ApplyPending result. A second
run contains unfinished agent and validation work.

`tests/upgrade.rs` checks every supplied field through Store load, atomic save and
reopen. Only the documented interrupted-job/validation recovery fields change;
load itself leaves the input file untouched. Additional default fields are allowed.
These nonexistent fixture paths are never accessed and saved ownership remains
unverified. This is a serialization/upgrade test; existing worktree recovery tests
separately verify actual manifests, Git backlinks, leases and retained contents.
