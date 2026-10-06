# CLI availability lifecycle — v0.1 fix

Implementation and Linux automated verification: 2026-10-06. Native packaged
macOS verification of this change is **pending**; this session runs on Linux
and has no macOS test host. Earlier macOS CLI execution/UI results do not verify
this new startup lifecycle.

## Design

Application construction loads its existing state and schedules independent
checks for Codex, Copilot, OpenCode and Claude on the existing Tokio runtime.
It awaits no discovery or CLI process. Repository refresh, history, task editing
and convoy events keep their existing lifecycles. Each backend reports Checking…
until it settles into Available (with existing help/version details), Unavailable
or Invalid configuration. Optional missing CLIs never set the application notice.

Filesystem discovery runs on blocking workers, using the existing PATH/common
installation folders, including Homebrew and user launcher directories. Manual
absolute paths are authoritative. An explicit launcher name resolves only that
name, not another backend-default executable. Resolved launcher/symlink paths
are adopted into the normal per-agent preferences so later jobs use the checked
executable. No PATH changes, shell profiles, authentication checks or agent tasks
are introduced. Script launchers still need their interpreter in the GUI PATH.

Executable changes invalidate the previous result immediately. The old probe
is cancelled and reaped before the newest queued selection starts. A 300 ms
delay coalesces typed edits. Generation IDs reject old successes, errors and
discovery results, including when a user switches back to a previously checked
path. Other job options are validated at preflight and do not respawn availability
probes. Check CLI remains a manual refresh; repeats while a backend is already
checking do nothing. Unchanged selections never continuously rediscover.

Changing the Agent selector explicitly requests validation of the newly restored
executable. A cached result (including an unavailable/error result) or an in-flight
check for that same backend/executable is reused. An unchecked backend or a
changed executable starts its check automatically. Switching away leaves other
backend checks and results intact. Check CLI bypasses the session result and
forces fresh validation when the backend is idle.

Run Convoy waits for the selected backend's check to settle. The preflight
entry point also reconciles executable edits before allowing a snapshot, so a
pending discovery cannot leave preflight using the old launcher name. The
resolved path is adopted before review becomes available. Checks for other
backends do not gate review, and task editing and repository/history loading
remain usable.

There is at most one availability check per backend. A per-backend async mutex
also serializes the existing fresh preflight probe with startup/manual probes.
Successful help checks retain the existing optional version probe: Codex uses
one help command; Copilot, OpenCode and Claude use help then version. Missing
automatic discovery performs no CLI spawn, and failed help skips version.
Agent execution command construction and authentication remain unchanged.

Availability results are in memory only and all backends are checked anew on
restart. Confirmed quit cancels checks without treating them as active convoy
work. Application drop aborts async probes through the existing process-tree
cleanup guard and shuts the runtime down in the background, avoiding an exit
wait for a stuck filesystem worker. The convoy manager retains its normal
confirmed shutdown and final event/state handling.

## Automated coverage

`tests/availability.rs` uses the existing native fixture executable with gated
help/version probes and process-level overlap detection. It covers:

- All four startup checks, successful results and a fresh check after restart.
- Backend selection checking an unchecked executable, reusing an unchanged
  executable without additional probes, validating changed preferences and
  rejecting stale results across backend/executable switches. Fixture process
  counts also prove manual refresh bypasses the cache.
- Missing paths, wrong executables and invalid relative paths.
- Manual refresh, repeated refresh suppression and backend failure isolation.
- Executable changes, cancellation, coalescing and no overlapping processes.
- Stale queued success and A → B → A selection changes.
- Automatic discovery with an empty GUI-style PATH and a common-location override
  in an isolated child test process; exact manual launcher-name precedence.
- Stale automatic discovery after a manual path selection, and adoption of a
  discovered path without a duplicate probe.
- Shutdown during a gated version probe and suppression of further checks.
- Startup and existing launch preflight sharing one backend probe at a time.

UI tests verify the actual Agent selection handler's immediate unchecked-backend
trigger, cached-result reuse and changed-executable handling, plus startup
scheduling, backend-local failure presentation, manual
refresh, editable task input while Checking…, unchanged history/repository work,
candidate selection during existing checks, compact forms in both themes and
nonblocking exit with a deliberately blocked filesystem worker. Existing backend,
Git, persistence, scheduler and cancellation tests remain required.

Preflight regression tests verify that startup checks, executable edits before
the next poll, and manual refreshes prevent an early snapshot. An isolated Unix
test uses a gated help-only fixture outside the GUI-style PATH and a temporary
Git repository: repository loading completes during the check, early review is
refused, and review then succeeds with the adopted absolute executable path.
A different backend's pending check does not prevent that review.

Required commands:

```sh
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo build --release
```

Results on this Linux host, including the Agent selection and preflight follow-ups:
all four commands passed; **124 tests passed**,
3 optional installed-CLI probes were ignored, and no tests failed. The release
binary was built successfully. The GUI-style PATH test and headless egui input
tests are automated evidence; they do not establish native packaged macOS
behavior. The maintainer will perform the macOS checklist below.

## Native packaged macOS checklist — pending

Build/package the revised source using the macOS procedure in
[releasing](releasing.md), install the resulting app in Applications, and launch
it from Finder rather than a terminal. Do not use an older published v0.1.0 DMG
to verify this change.

1. Launch without pressing Check CLI. Switch among the four backends and confirm
   installed agents move from Checking… to Available. Inspect the resolved path
   and hover details/version where that backend supports it. A fast probe may
   settle before the first visible frame.
2. Restart normally from Finder and confirm availability is checked again.
3. Confirm absent agents settle into Unavailable with local guidance, without an
   application-wide error or modal.
4. Choose a different executable using Find CLI → Use and check, then also try
   editing the path manually. Confirm both automatically recheck; invalid paths
   settle into the existing unavailable/configuration state.
5. Change paths while a check is pending and switch backends. Confirm an old
   result never describes the newer selection. Switching back to an unchanged,
   already checked backend should retain its result; Check CLI should refresh it.
6. Edit a task, inspect restored history/repositories and close the app while
   checks run. Confirm the UI remains responsive and CLI probes do not hold exit
   open. Confirm Run Convoy becomes available after the selected CLI check settles
   and review uses its resolved executable. Record macOS/app build versions,
   launcher paths and observed results.
