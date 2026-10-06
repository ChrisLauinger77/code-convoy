# Quit confirmation

## Cause and implementation

The previous root-window close handler immediately called `cancel_all()` and
waited for cleanup. No confirmation preceded cancellation. On macOS, AppKit's
`terminate:` action (Cmd+Q/application Quit) could also bypass that handler and
reach the unconditional cleanup in `on_exit`.

`src/ui/quit.rs` now handles the quit decision for every platform. Active means
either unfinished manager controls or any nonterminal job in convoy metadata;
queued jobs and repository checks are included. Counts update while the modal
is open. Jobs and scheduling continue until the user confirms.

- Cancel, Escape and backdrop dismissal only clear the pending decision. They do
  not signal tokens, alter jobs or touch locks. Cancel initially receives keyboard
  focus, so Enter is safe. Tab moves to the explicit **Stop jobs and quit** action.
- Repeated requests reuse the same decision. Once confirmed, UI launch methods
  and manager admission reject new work; shutdown is idempotent.
- Confirmation sets a shared stopping flag before invoking the existing Stop All
  tokens. The scheduler checks before admission and after awaited event delivery;
  workers recheck after building a command and immediately before spawning it.
- The normal event loop continues draining while the existing process-group/Job
  Object cleanup runs. After the manager finishes, the last lifecycle events are
  applied, any interrupted metadata is recovered, state is saved and the close
  request is allowed. Existing cleanup quarantine semantics remain unchanged.
- With no active work, exit proceeds immediately without a modal.

The handler runs in eframe's `App::logic`, which also runs for minimized/hidden
windows. A pending quit brings the root window forward to show the decision.

On macOS, an application-owned `NSApplication` subclass is created before eframe
initializes AppKit. It redirects `terminate:` to an egui root-window close request.
It does not replace winit's delegate, swizzle methods, change menu shortcuts or
replace the native About panel. This also covers application-menu/Dock Quit via
the native action. Linux and Windows use the shared window-close handler.
No new dependencies or backend command changes were needed.

References: [AppKit application lifecycle](https://developer.apple.com/documentation/appkit/nsapplication),
[native termination](https://developer.apple.com/documentation/appkit/nsapplication/terminate(_:)).
The integration was checked against the locally resolved winit 0.30.13 and
eframe/egui-winit 0.36.2 sources.

## Automated coverage

New UI tests cover idle exit, running/queued confirmation, repeated requests,
side-effect-free Cancel (including unsignalled manager cancellation tokens),
manager work ahead of UI metadata, launch rejection after confirmation, draining
and persisted terminal states, unchanged completed history, Escape/default focus,
and cancellation of close events without a rendered UI pass.

The convoy integration test now checks repeated shutdown, rejection of new
convoys and absence of queued launches. An additional bounded-channel test
confirms shutdown while admission is suspended on lifecycle event delivery.
Existing fixture tests continue covering process-tree cancellation, capacity and
repository-lease release for all four backends. Tests do not use authenticated CLIs.

Required checks: `cargo fmt --check`,
`cargo clippy --all-targets --all-features -- -D warnings`,
`cargo test --all-features`, and `cargo build --release`.
The Windows all-target/all-feature cross-compilation check was also run locally.
The unchanged CI matrix runs checks, tests and release builds on macOS, Linux
and Windows.

## Native macOS validation

Validated on Apple Silicon using an ad-hoc-signed application bundle containing
the release binary, a separate application home, two disposable Git repositories
and a local CLI fixture. The fixture starts a parent and child process, each
writing a heartbeat outside the repositories. Per-convoy/global concurrency of
one leaves the second job queued. No provider or authentication was used.

- Idle Cmd+Q exited immediately.
- Window close showed one modal with one running and one queued job. Repeating
  close and Cmd+Q did not stack dialogs or cancel anything.
- Enter activated the initially focused Cancel. Both process IDs were unchanged
  and both heartbeats advanced; running/queued states remained visible.
- A subsequent window close followed by **Stop jobs and quit** exited the app.
  Both fixture PIDs were gone. Both jobs were saved as cancelled, with the queued
  job's start time still absent.
- Restart showed cancelled history with no running/resumable work.
- Repeated with Cmd+Q: Escape left both processes alive with advancing heartbeats.
  Cmd+Q from a minimized window restored the confirmation. Repeated Cmd+Q reused
  it; Tab then Enter selected the explicit stop action and completed cleanup.
- Previously cancelled history remained byte-for-byte equivalent as JSON data.
  The user's original application state hash was unchanged.

Linux and Windows native GUI behavior was not exercised on this macOS host;
their shared logic is covered by deterministic tests and the CI matrix. Force
Quit, crashes and hard OS termination cannot offer a confirmation; existing Unix
limitations for deliberately detached descendants remain unchanged.
