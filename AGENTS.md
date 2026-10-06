# AGENTS.md

## Project

CodeConvoy is a native Rust/egui desktop application for running one task
across multiple local Git repositories using a selected coding-agent CLI.

Supported backends:

- OpenAI Codex CLI
- GitHub Copilot CLI
- OpenCode
- Claude Code

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

Do not introduce:

- Tauri
- Electron
- Node.js
- JavaScript / TypeScript
- a web frontend

without an explicit architectural decision.

Prefer conservative dependencies and justify new ones.

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

The backend abstraction is feature-frozen for the v0.1.x cycle except for
bug fixes or changes required by real backend behavior.

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

CodeConvoy operates directly on repositories explicitly registered by the
user.

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

The `NEW CONVOY` UI is a draft. Editing it must never mutate an existing run.

Multiple convoys may execute concurrently.

Preserve both:

- per-convoy concurrency limits
- the global job limit

Scheduling should remain fair. The current scheduler uses deterministic
round-robin admission.

Blocked repository jobs must not consume execution slots.

Cancelling one convoy must not affect unrelated convoys.

## Task context

Repository groups are selection helpers, never scheduling or execution units.
Task templates contain reusable task text, not backend settings or workflows.
Keep launched task and attachment snapshots independent of later draft/library
edits. Missing context must remain explicit; reuse must not silently omit it.

Backends own attachment capabilities and transport. Inspect attachment files off
the UI thread and persist only references/metadata, never contents. Activity
summaries must describe actual backend events and keep bounded Raw output
available for diagnostics.

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

## Scope

Before adding a new feature, check whether it belongs in CodeConvoy's core
purpose:

> Run one task across multiple repositories using the coding agent of your
> choice.

For the v0.1.x cycle, do not add new coding-agent backends unless explicitly
requested.

The initial backend set is intentionally limited to:

- Codex
- GitHub Copilot
- OpenCode
- Claude Code

Features such as pipelines, worktrees, automatic commits, automatic pull
requests, embedded terminals, cloud execution, and agent-to-agent workflows
are outside the current scope unless explicitly requested.

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
   `docs/release-notes.md` to describe the changes since the previous release.
   Preserve historical versions in validation records, past-release evidence,
   dependency versions, and CLI compatibility documentation; do not perform a
   repository-wide version replacement.
4. Run all checks under **Required validation**, plus:

   ```sh
   python3 -m unittest discover -s packaging -p 'test_*.py'
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
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo build --release
```
