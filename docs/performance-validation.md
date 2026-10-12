# v0.10 PR 3: performance and resource efficiency

## Environment and method

Measured 2026-10-12 on Linux 7.2.9 / Debian, AMD Ryzen 7 9800X3D,
16 logical CPUs, rustc 1.96.1, release profile with thin LTO. Baseline:
`a45365b` (merged PRs 1 and 2 plus dependency updates), with only the probes
added. No installed agent, provider account or user repository was used.
These are observations on this machine, not timing thresholds in CI.

Native runs used the GNOME desktop through XWayland with software OpenGL,
1180×820 at scale 1. Each fixture creates temporary state, saves it and reloads
it, configures missing CLI paths, then delegates to the real App lifecycle.
Historical Git review is prefilled with a synthetic diagnostic so these runs
measure startup/rendering/idle rather than thousands of Git subprocesses.
Fixture generation is excluded from startup. Loading the state, creating the
runtime/window and completing the first UI layout are included; GPU presentation
and compositor latency are not measured. Filesystem pages are warm from fixture
creation. The first fresh-process run and two subsequent runs are recorded;
this is not a cold OS-cache/boot measurement. No cache flushing was attempted.

Headless layout uses the actual UI at 1180×820, 10 warmup frames and 30 sampled
frames. Input is synthetic; the numbers exclude native event delivery and GPU
rendering. The full-window fixture has 128 registrations, 5,000 convoys with
four jobs each, four active convoy snapshots, and large selected Raw output.
The 512-registration case deliberately exceeds the supported limit of 128;
the product limit is unchanged. Validation output is approximately 64 KiB,
Raw output reaches its 512 KiB retention limit. No live agent is launched by
the active-convoy fixture.

## Measured bottlenecks and changes

1. Each repository row scanned all historical jobs to determine whether current
   work overlapped its path. Collecting busy paths once per render removes this
   repeated scan. The derived flags exist only during rendering and include
   queued/preparing/running jobs, running validation and result operations.
   Nested-path semantics and authoritative manager leases are unchanged.
2. The idle app requested a repaint every second. Background message senders now
   wake egui after enqueueing, and the unconditional idle timer is removed.
   Saves have an explicit deadline, also armed after UI edits. Pending saves or
   exports do not spin while Quit waits for acknowledgement. Active execution,
   validation, result operations and shutdown keep their existing 100 ms polling;
   animations and transient Copy feedback retain their own deadlines.
3. Session log eviction used String::clear, retaining every evicted allocation.
   Eviction now drops that allocation. The same oldest-first retention, UTF-8
   text, truncation marker and ability to append subsequent output are preserved.

### Layout (milliseconds, median / p95)

| Fixture | Before | After |
| --- | ---: | ---: |
| Repository picker, 128 | 0.581 / 0.591 | 0.574 / 0.585 |
| Repository picker, 512 stress | 2.281 / 2.297 | 2.257 / 2.267 |
| Open history selector, 5,000 | 0.222 / 0.233 | 0.228 / 0.233 |
| Results, 128 jobs, Review | 0.690 / 0.696 | 0.693 / 0.706 |
| Results, 128 jobs, Raw 512 KiB | 0.640 / 0.650 | 0.648 / 0.660 |
| Results, 128 jobs, Validation ~64 KiB | 0.637 / 0.648 | 0.636 / 0.639 |
| Results, 128 jobs, Task & settings | 0.724 / 0.735 | 0.728 / 0.732 |
| Full window, 5,000 history, 4 active | 4.808 / 4.882 | 0.957 / 0.964 |
| Full window with History settings | 5.006 / 5.044 | 0.967 / 0.972 |

The demonstrated improvement is about 5× in the full window. Isolated result,
output and history rendering were already inexpensive in these fixtures; no
additional persistent cache or list virtualization was introduced.

### Native startup and idle

Three fresh processes per cell; CPU/RSS report the median. CPU sampling spans
12 seconds after startup settles. At this host's 100 ticks/second, the resolution
is about 0.083% of one core. `0.000` means no measurable tick delta in that
interval, not a claim that the app can never use CPU while idle. The frame ranges
include repaint requests from egui and native window/compositor events.

| Fixture / measurement | Before | After |
| --- | ---: | ---: |
| Empty, first process startup ms | 27.602 | 25.575 |
| Empty, subsequent startups ms | 25.269, 25.463 | 25.439, 27.254 |
| Empty, idle CPU % | 1.417 | 0.083 |
| Empty, idle RSS KiB | 159,360 | 159,736 |
| Empty, UI calls / 20 s | 35–40 | 13–16 |
| 5,000 history, first process startup ms | 73.242 | 65.029 |
| 5,000 history, subsequent startups ms | 66.187, 64.597 | 62.679, 63.662 |
| 5,000 history, idle CPU % | 1.667 | 0.000 |
| 5,000 history, idle RSS KiB | 243,224 | 243,736 |
| 5,000 history, UI calls / 20 s | 29–30 | 7–11 |

Startup and idle RSS are effectively unchanged; no improvement is claimed for
either. Native state has no retained session logs, so the buffer fix below is
measured separately. All six native runs after the change completed ordinary
close/shutdown and released their temporary stores.

### Buffer retention

The probe appends four 128 KiB chunks to both Activity and Raw for each of 128
jobs, invoking the ordinary global eviction after each job. Retained text is
33,554,432 bytes in both builds. Sum of String capacities falls from 134,224,896
to 33,556,224 bytes (about 128 MiB to 32 MiB). This is live buffer allocation,
not total process RSS: allocator arenas, viewer copies and graphics resources
are separate, and freeing a String need not immediately return pages to the OS.

### Background storage and persistence

The existing history probe measures five storage scans over freshly saved local
state with an empty ownership inventory. Scanning still runs off the UI thread.
Owned-worktree traversal and Git cleanup eligibility are covered by the existing
[storage](storage-validation.md) and [history reliability](history-reliability-validation.md)
fixtures; this table does not represent thousands of real worktrees.

| Convoys | State JSON bytes | Storage before ms | Storage after ms | Load before ms | Load after ms |
| ---: | ---: | ---: | ---: | ---: | ---: |
| 100 | 525,057 | 0.783 | 0.789 | 0.646 | 0.574 |
| 1,000 | 5,248,485 | 7.945 | 7.724 | 6.770 | 5.635 |
| 5,000 | 26,248,946 | 40.274 | 39.509 | 35.198 | 29.407 |

Storage/persistence code is unchanged; differences are run-to-run observations,
not an optimization claim. At 5,000 convoys, warm metadata search remained
0.026 ms and save measured 25.354 / 23.656 ms before/after. The scan completion
now wakes the UI directly instead of relying on its polling timer.

## Reproduction

```sh
cargo test --release --all-features --lib desktop_layout_performance -- --ignored --nocapture
cargo test --release --all-features --lib output_retention_performance -- --ignored --nocapture
cargo run --release --features test-support --example history_performance
cargo build --release --features test-support --example desktop_performance
env -u WAYLAND_DISPLAY LIBGL_ALWAYS_SOFTWARE=1 WINIT_X11_SCALE_FACTOR=1 \
  target/release/examples/desktop_performance 5000 20
```

The native probe prints its PID, first-layout time and number of UI calls in a
20-second idle interval, beginning five seconds after App construction. Its only
own repaint deadline is the eventual close; it closes through the normal app
lifecycle. Repeat with history count `0` for the empty state. For Linux CPU/RSS,
sample `/proc/PID/stat` user+system ticks and `/proc/PID/statm` resident pages at
7 and 19 seconds after the PID line. CPU percent is tick delta divided by
`SC_CLK_TCK` and elapsed wall time, multiplied by 100 (one core = 100%). RSS KiB
is resident pages × `SC_PAGE_SIZE` / 1024. Do not build or run competing benchmarks
during these idle samples. Run three fresh processes per size and revision.

## Resource and lifecycle review

- Scheduling remains event-driven in the existing Tokio manager. The global and
  per-convoy limits, round-robin admission, repository leases and cancellation
  paths are unchanged. UI polling while work is active is retained deliberately.
- Agent events still use the bounded 256-entry channel and visible dropped-output
  diagnostics. Validation still coalesces its bounded 64 KiB record in a watch
  channel, with at most four active operations. No extra thread, relay queue or
  output copy was added by the repaint change. Metadata messages use the existing
  channel with a wake-on-send wrapper.
- Storage still has one running generation and one coalesced replacement request,
  cancellation and stale-result rejection. Review/Git, CLI probing and attachment
  inspection stay on workers. Rendering does not add filesystem or Git operations.
- History and output retain their existing visible-row rendering. Task/settings
  formatting and metadata snapshots remain unchanged; large output is not copied
  into persistence snapshots. Saves still serialize under the Store reservation.
- Regression checks cover enqueue-before-wake, idle/save/quit timer policy,
  nested repository busy-state changes, freeing both evicted output buffers and
  appending new Unicode output after eviction. Existing process-tree, shutdown,
  multiple-convoy, ownership and atomic-save tests remain required.

## Limits

Native macOS, Windows, hardware OpenGL and native Wayland were not measured here.
Long single-line text and arbitrary slow/network filesystems are not represented
by the multiline output/local-storage fixtures. Startup parsing and explicit
durable transactions remain synchronous; this PR makes no startup speed claim.
No authenticated backend run, OS-wide memory guarantee, or proof against thread
leaks in external drivers is implied. No dependency, persistence schema, backend
command, scheduling policy, version, release tag or package publication changes.

## Final validation

- `cargo fmt --check`: passed.
- `cargo clippy --all-targets --all-features -- -D warnings`: passed.
- `cargo test --all-features`: 566 passed, zero failed, eight intentionally
  ignored (four installed-backend probes and four manual performance checks).
  Counts exclude filtered subprocess re-executions. Both new performance probes
  were run separately, as were the existing history example and the native probe.
- `cargo build --release`: passed.
- `git diff --check`: passed.

The full filesystem/process suite used `TMPDIR=/var/tmp` outside the Codex
sandbox, which otherwise injects a protected `/tmp/.git` boundary into disposable
Git fixtures. Production repository and storage safety checks were not relaxed.

PR review follow-up removes the redundant dispatch repaint and the context
arguments that only forwarded it, uses an inclusive save deadline, and shares
the path-overlap predicate between single-repository checks and the per-frame
projection. The message sender reports delivery as a boolean; its ordering test
also verifies that a closed receiver returns false without requesting repaint.
