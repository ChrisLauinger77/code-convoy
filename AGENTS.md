# AGENTS.md

## Project

CodeConvoy is a native Rust/egui desktop application for running one task
across multiple local Git repositories using a selected coding-agent CLI.

Agent backends:

- OpenAI Codex CLI
- GitHub Copilot CLI
- OpenCode
- Claude Code
- Cursor CLI (experimental)
- Antigravity CLI (experimental)

See `README.md` for current support status and exact validation evidence.

CodeConvoy orchestrates existing local CLI installations. It does not manage
provider accounts, API keys, or authentication.

## Project principles

Keep CodeConvoy:

- local-first
- agent-neutral
- native Rust
- compact and desktop-oriented
- safe around user repositories
- explicit about backend-specific behavior

Do not turn CodeConvoy into an IDE, chat application, cloud service, workflow
engine, or general multi-agent framework.

## Technology

- Rust
- egui / eframe
- Tokio where asynchronous work is required
- Git CLI for repository operations
- serde for persisted state

Keep the desktop application native Rust/egui. Do not introduce into it:

- Tauri
- Electron
- Node.js
- JavaScript / TypeScript
- a web frontend

without an explicit architectural decision.

The separate project homepage in `website/` uses HTML/CSS. Keep it static and
independent of the desktop application; follow `website/README.md` for its build
and preview workflow.

Prefer conservative dependencies and justify new ones.
See `CONTRIBUTING.md` for toolchain requirements, platform prerequisites and
dependency/license checks. Preserve `Cargo.lock`; do not run `cargo update` just
to build.

## Architecture

Keep these concerns separated:

- domain/application state
- agent backends
- process execution
- Git operations
- convoy scheduling
- persistence
- UI

Do not move execution or Git logic into egui rendering code.

Read `docs/architecture.md` before making significant architectural changes.

## Agent backends

Evolve backend interfaces only for concrete CLI behavior or explicitly requested
features. Follow `docs/compatibility.md` for the current 1.x compatibility policy.

Each backend owns its own:

- executable detection
- configuration/capabilities
- command construction
- prompt transport
- output decoding
- completion criteria
- backend-specific errors

Do not force different agents into artificial common semantics.

In particular, do not assume that concepts such as model, reasoning effort,
permissions, sandboxing, agents, variants, or tool approval mean the same
thing across backends.

Do not change another backend while implementing or fixing one unless the
shared behavior genuinely requires it.

Never guess CLI flags. Verify behavior against the installed CLI or current
authoritative documentation/source.

## Authentication and secrets

CodeConvoy relies on each coding-agent CLI's existing authentication.

Never:

- request provider API keys
- persist provider credentials
- modify agent authentication files
- implement provider login flows
- print secrets into logs

Do not make CodeConvoy a provider/account manager.

## Repository safety

User repositories are important data.

Never silently:

- reset Git state
- clean untracked files
- stash changes
- discard modifications
- switch branches
- create commits
- push
- force-push
- delete branches
- delete repositories

CodeConvoy operates on repositories explicitly registered by the user, either
directly or in an explicitly selected CodeConvoy-owned isolated worktree.

Preserve:

- dirty-tree review
- Git baseline revalidation before execution
- canonical repository locking
- nested-repository conflict protection

Two active jobs must never write to the same or nested working tree
concurrently.

History removal must only remove CodeConvoy metadata. It must never modify
repository contents.

## Convoys and scheduling

A convoy is an immutable snapshot of:

- task
- selected backend
- backend-specific settings
- selected repositories
- per-convoy concurrency
- execution mode (Direct by default, or IsolatedWorktree)

The `NEW CONVOY` UI is a draft. Editing it must never mutate an existing run.

Multiple convoys may execute concurrently.

Preserve both:

- per-convoy concurrency limits
- the global job limit

Scheduling should remain fair. The current scheduler uses deterministic
round-robin admission.

Blocked repository jobs must not consume execution slots.

Cancelling one convoy must not affect unrelated convoys.

## Isolated execution

Isolated jobs use the exact committed HEAD captured at review. Never import local
uncommitted changes or fall back to Direct after preparation failure. Preparation
consumes both scheduler slots; serialize CodeConvoy Git administration by canonical
common-directory identity, while allowing independent isolated checkouts to run
concurrently. Direct access remains exclusive and real nested repositories conflict.

Keep ownership records outside the checkout, written before creation. A path prefix
alone is not ownership. Retain successful, failed, cancelled and partial worktrees;
no automatic deletion or pruning. Isolated diffs always compare with the saved base
commit. Reuse restores mode and creates a fresh path. Keep agent status distinct
from result observations. Saved observations are not recovery validation. Determine
changes with Git against the fixed base (working tree, index and nonignored untracked
files), never from agent status. Retain unchanged worktrees too until explicit cleanup. Cancellation must await Git/process cleanup before releasing leases; do not
turn unconfirmed cleanup into a safe result through a later inspection. Prefix
application Activity events with `[CodeConvoy]` and leave backend Raw output intact.
Reconcile persisted results off the UI thread before claiming availability. Never
resume agents, recreate missing worktrees, or infer deletion permission from an
unavailable repository. Pin unresolved ownership metadata through history trimming,
Remove and Clear history. Recovery must not purge changed or unchanged results.

Internal cleanup must use the Store transaction and manager lifecycle lease:
persist intent first, verify the trusted storage boundary, sibling manifest,
canonical Git identity, detached ownership lock and Git backlink, then remove only
the exact owned worktree through Git. Reject symlinks/reparse points and uncertain
ownership. Coordinate cleanup exclusively against active work in that repository;
never use broad prune or recursive deletion as a fallback. Preserve failure state
and crash evidence. Apply is explicit and requires a terminal owned result, a still-registered canonical
source, clean destination at the fixed base, reviewed-result revalidation and an
exclusive manager lifecycle lease. Use binary Git patch preflight; never silently
flatten staged alternatives or unsupported changes. Preserve the real indexes,
HEAD and retained worktree; never reset/clean/stash after failure. An uncertain
write remains pending across restart and cannot retry, discard or clean up its
retained evidence without a separate explicit uncertainty-resolution flow. Applied is
resolved independently of physical availability; retain its stable Diff until
explicit verified cleanup. Discard uses the same Store intent/outcome transaction
and cleanup primitive, requires safe-default confirmation, and changes no source
files. Pin ownership metadata until physical cleanup is verified, even for Applied
results. Review caches Git statistics off the UI thread, never agent prose; Direct
statistics are current observations, with no destructive Apply/Discard actions.

## Retry and follow-up

Retry creates a new one-repository convoy using the original immutable task and
settings. Revalidate registration, current Git, backend capabilities and attachment
references through normal review/admission. Isolated retries use a fresh worktree
from the newly reviewed HEAD; never mutate or resolve the previous attempt/result.
Use the ordinary scheduler, leases, cancellation and quit lifecycle.

New convoy from selected only replaces the draft's explicit registered repository
selection and settings. Clear task and attachments, preserve the global preference
and libraries, report missing/unregistered omissions, and never auto-launch.
Provenance is optional informational metadata, never a scheduling dependency or a
reason to pin source history beyond the existing ownership protections.

## Task context

Repository groups are selection helpers, never scheduling or execution units.
Task templates contain reusable task text, not backend settings or workflows.
Keep launched task and attachment snapshots independent of later draft/library
edits. Missing context must remain explicit; reuse must not silently omit it.

Backends own attachment capabilities and transport. Inspect attachment files off
the UI thread and persist only references/metadata, never contents. Activity
summaries must describe actual backend events and keep bounded Raw output
available for diagnostics.

## Manual validation

Validation is an explicit user action, separate from agent execution and result
resolution. Preserve shell-free commands, repository leases and process-tree
cancellation. Preset assignment only configures commands; it must never run them.
Follow `docs/validation.md` for configuration, execution and persistence rules.

## Process management

Agent processes must never block the egui UI thread.

Preserve process-tree cancellation.

Every terminal path must release:

- scheduler capacity
- repository locks
- process resources

A failure in one repository or convoy must not terminate unrelated work.

Avoid shell interpolation. Prefer direct process arguments and stdin for
prompt transport.

## Persistence

Maintain backward compatibility with existing persisted state whenever
reasonably possible.

Never persist secrets.

Do not claim that running jobs can resume after CodeConvoy itself exits.

On restart, unfinished historical runs must be represented truthfully rather
than appearing to still be running.

Run IDs must remain stable and must not be reused after history cleanup.

Follow `docs/compatibility.md` for settings/history compatibility and
`docs/history-export.md` for the public export contract. Application versions,
internal state versions and export schema versions are independent. Exports are
reports, not backups or an import format.

## UI

Keep the existing compact two-pane native desktop design.

Preserve:

- `NEW CONVOY` on the left
- `RUNS / RESULTS` on the right
- system/dark/light themes
- keyboard navigation
- visible focus
- status communication that does not rely only on color

Avoid:

- web-dashboard styling
- excessive cards
- decorative animation
- IDE-style feature creep
- chat-oriented UI

Backend-specific settings should only appear for the selected backend.

Do not perform expensive subprocess or Git work every egui frame.

Keep update checks explicit and inform-only: no automatic requests, downloads or
installation. Follow `docs/desktop-workflow.md#update-check` for the existing
network and privacy boundaries.

## Scope

Before adding a new feature, check whether it belongs in CodeConvoy's core
purpose:

> Run one task across multiple repositories using the coding agent of your
> choice.

Do not add new coding-agent backends unless explicitly requested. Maintain the
existing backends listed under **Project** with their backend-specific contracts.

Direct execution and CodeConvoy-owned isolated worktrees are within the current
scope, including explicit result review, Apply, Discard and verified cleanup.
Follow the safety requirements above and `docs/worktree-cleanup.md`.

Features such as pipelines, automatic commits, automatic pull requests, embedded
terminals, cloud execution, and agent-to-agent workflows are outside the current
scope unless explicitly requested.

## Code quality

Use idiomatic Rust.

Prefer:

- small focused modules
- explicit types
- actionable errors
- deterministic tests
- clear ownership of responsibilities

Avoid `unwrap()` and `expect()` in normal runtime paths unless an invariant is
genuinely guaranteed and documented.

Do not add abstractions for hypothetical future requirements.

## Maintainer release requests

When the user asks `create release X.Y.Z`, prepare that CodeConvoy release and
create a local release commit. This request explicitly authorizes the version
updates, release documentation changes, validation, and commit; do not ask for
another confirmation to create that commit.

Follow this workflow:

1. Read `docs/releasing.md` and inspect the working tree and index. Confirm the
   requested version is a normal `X.Y.Z` version supported by the packaging
   scripts and has not already been tagged or released. Preserve unrelated
   staged and unstaged changes.
2. Update `[package].version` in `Cargo.toml` and the `codeconvoy` package entry
   in `Cargo.lock`. Keep dependency versions unchanged. Application and package
   metadata already derive from Cargo; retain that single version authority.
3. Search for current-release version references and update them where needed,
   including README download filenames/install commands and the release version,
   tag and packaging examples in `docs/releasing.md`. Update
   `docs/release-notes.md` to contain only the current release and its changes
   since the immediately previous version. Remove older release sections when
   preparing a new version; keep historical release notes in Git history.
   Preserve historical versions in validation records, past-release evidence,
   dependency versions, and CLI compatibility documentation; do not perform a
   repository-wide version replacement.
4. Run all checks under **Required validation**, plus:

   ```sh
   python3 packaging/release.py check-tag vX.Y.Z
   ```

   Substitute the requested version in the tag check. Resolve failures before
   committing and report platform checks that were not performed truthfully.
5. Review the final diff and create a commit containing only the release
   preparation changes. Preserve unrelated work, including already-staged
   changes. Use the Conventional Commit message:
   `chore(release): prepare vX.Y.Z`.
6. Report the version, commit ID, and validation results.

This workflow ends with the local release commit. Create or push a tag, push
commits, dispatch a workflow, or publish a release only when the user explicitly
requests those actions. Never move or overwrite an existing release tag.

## Required validation

Before considering a change complete, run:

```sh
python3 -m unittest discover -s packaging -p 'test_*.py'
cargo fmt --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-features
cargo build --locked --release --bin codeconvoy
```

See `CONTRIBUTING.md` and `.github/workflows/ci.yml` for platform-specific checks
and dependency/license validation. Deterministic tests need no provider
credentials. Run ignored or authenticated tests only for their documented purpose,
and report native platform or authenticated checks that were not performed.
