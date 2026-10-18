# Antigravity integration — release readiness

Scope: integration testing and release readiness for the 1.2.0 Google
Antigravity backend after merged
[foundation #81](https://github.com/ChrisLauinger77/code-convoy/pull/81),
[execution #82](https://github.com/ChrisLauinger77/code-convoy/pull/82),
[lifecycle #83](https://github.com/ChrisLauinger77/code-convoy/pull/83) and
[UX #84](https://github.com/ChrisLauinger77/code-convoy/pull/84).
Base: `e857de4`. Evidence date: 2026-10-18. This is a maintainer-review handoff,
not release approval. No version, dependency, state/export schema, package
format, license or release-publication change is part of this PR.

## Acceptance coverage

| Requirement | Implementation / evidence |
| --- | --- |
| Backend registration, discovery, settings | Sixth registered backend `antigravity`; per-backend settings persist; `agy` in the shared search directories (including `~/.local/bin`) and, on Windows, Google's `%LOCALAPPDATA%\agy\bin`. Keyboard UI regression and persistence tests. |
| Version and CLI contract | Minimum 1.2.16; help read from stderr; identity by `--print-timeout`/`--prompt-interactive`; required invocation flags. Real agy 1.4.2 passes Check CLI. |
| Headless execution and completion | One stdin `stream-json` turn, no `--print-timeout`; success requires init/`cwd`, consistent conversation, all results `SUCCESS`, no unfinished step, exit zero. Decoder fixtures and process-level scenarios. |
| Authentication failure | Real agy 1.4.2 output captured; jobs fail with sign-in guidance. Real end-to-end jobs without sign-in pass through the runner. |
| Permission modes | Configured by default; explicit `--dangerously-skip-permissions` and `--sandbox`; Windows sandbox gated at build 26100; Retry never broadens. All argv variants verified against real agy 1.4.2 through the authentication gate. |
| Lifecycle and safety | Stop, Quit, time limit, leftover processes, concurrency, leases and quarantine through the shared runner (fixture). Real auto-updater runs detached and does not delay completion (macOS). |
| Session continuation | Not supported; documented with the conversation-history limitation. |
| User-facing errors | Recovery advice for sign-in, model, quota, slash command; conversation ID and exit code or signal on every failure. UI summary regression. |
| Packaging and discovery | Package probe includes the sixth `agy` launcher fixture; ran directly on macOS (not inside an AppImage). Package formats, runtime and publication workflow unchanged. |
| Licensing | No new dependency or redistributed component; `packaging/licenses.py check` unchanged (457 crates, 0 unresolved obligations, 1 existing accepted risk). |

## Real CLI evidence

Isolated **agy 1.4.2** on macOS (Apple Silicon), installed from Google's
manifest with SHA-512 verification, temporary `HOME`/XDG under
`/tmp/agy-sandbox`, never signed in. Details: [installed CLI
check](antigravity-validation.md#installed-cli-check-agy-142) and
[argument syntax and end-to-end jobs](antigravity-validation.md#argument-syntax-and-end-to-end-jobs).

| Evidence | Linux | macOS | Windows |
| --- | --- | --- | --- |
| Check CLI (help on stderr, version, flag contract) | Not performed | Passed, 1.4.2 | Not performed |
| CodeConvoy argv variants through the authentication gate | Not performed | Passed, 1.4.2 | Not performed |
| Real jobs through the runner without sign-in, Direct and Isolated | Not performed | Passed, 1.4.2 (8 jobs) | Not performed |
| Authenticated task, real `stream-json` success, model access | Not performed | Not performed | Not performed |
| Real permission enforcement, `denied_actions`, rules loading | Not performed | Not performed | Not performed |
| Auto-updater process layout | Not performed | Detached own session; no pipe hold | Not performed (Job Object behavior unknown) |
| Deterministic process, lifecycle and UI suite | CI | Local and CI | CI |
| Fresh packaged interactive desktop walkthrough | Not performed | Not performed | Not performed |

## Remaining release checks

Before calling authenticated Antigravity execution release-verified, a
maintainer with a signed-in agy should explicitly authorize a disposable task
and run:

```sh
CODECONVOY_AGY=/path/to/agy CODECONVOY_AGY_AUTHENTICATED=1 cargo test --all-features --test antigravity authenticated_agy_completes_a_disposable_task -- --ignored --nocapture
```

It makes one model call that creates one file in a temporary repository and
fails if the repository shows any other change, including untracked files.
Record the agy version, platform, model, the native events in Raw output, the
file change and the result. Also check configured versus approved tool
behavior, `denied_actions`, `.agents/` customization loading, Stop and time
limit with a real run, and Windows (agy.exe discovery, Job Object behavior of
the auto-updater, sandbox gating). Model calls can consume quota or AI credits.
These runs were not authorized or performed. Antigravity remains labelled
experimental in user guidance.

## Validation

Local macOS validation at the PR head:

- `cargo fmt --check`: passed.
- `cargo clippy --locked --all-targets --all-features -- -D warnings`: passed.
- `cargo test --locked --all-features`: passed with 0 failures; opt-in tests
  ignored by default.
- `cargo build --locked --release --bin codeconvoy`: passed.
- `python3 -m unittest discover -s packaging -p 'test_*.py'`: passed (1 skipped).
- `python3 packaging/licenses.py check`: passed, unchanged inventory.
- Release `package_probe` (`--features test-support`) run directly with six
  disposable launchers: all six fixture lookups, disposable Git/worktree,
  cancellation and environment preservation passed.
- Opt-in against the real agy 1.4.2: `installed_agy_passes_check_cli` and
  `installed_agy_job_without_sign_in_fails_with_sign_in_guidance` passed.

Fresh Linux/macOS/Windows CI results are recorded in the PR. No authenticated
execution or full protocol compatibility is inferred from fixtures, help output
or unauthenticated runs.
