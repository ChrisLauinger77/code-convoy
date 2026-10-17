# Cursor integration — PR 3 readiness

Scope: UI integration, documentation and release readiness after merged
[foundation #78](https://github.com/ChrisLauinger77/code-convoy/pull/78) and
[lifecycle #79](https://github.com/ChrisLauinger77/code-convoy/pull/79).
Base: `152860199014a3b838a7d2a5a62f7515aaa17a3a`. Evidence date: 2026-10-17.
This is a maintainer-review handoff, not release approval. No version, dependency,
state/export schema, package format, license or release-publication change is
part of this PR. The accepted PR 1 invocation is unchanged.

## Acceptance coverage

| Requirement | Implementation / evidence |
| --- | --- |
| Agent selection and settings | Existing registry and option renderer include Cursor. Real-widget keyboard regression selects Cursor, edits model/permissions/trust/time limit, switches backends and restores only Cursor preferences. |
| Availability and sign-in | Visible help/version-versus-sign-in guidance and official installation/authentication links. Keyboard tests verify exact link destinations; no login/status or model call is dispatched. Existing availability tests exercise discovery, missing/wrong CLI, stale checks and per-backend results. |
| Progress and terminal result | Headless native-widget test applies Started/Output/Finished events and renders Cursor Activity, original Raw JSON, task settings and Git diff after the draft switches to another backend. PR 2 tests own process supervision and completion parsing. |
| Errors | Existing backend diagnostics distinguish authentication, permission, trust, process failure, timeout and malformed/incomplete streams; the result-view regression retains actionable external sign-in guidance. [Troubleshooting](cursor.md#output-review-and-troubleshooting) explains recovery. |
| History / comparison / reuse | Cursor result summaries retain distinct success/failure counts. Save/load and reuse preserve original settings; reuse restores registered selections without executing. Existing old-state/upgrade and result-comparison checks remain in the full suite. |
| Validation presets | Regression configures a repository preset under every backend, including unavailable/invalid CLI settings, switches the backend and verifies commands remain independent. Assignment never launches validation or creates repositories. |
| Rules | Official CLI documentation checked on 2026-10-17. Deterministic process/worktree tests pin rule-file preservation, unchanged positional prompt, assigned directory and committed-only isolated rules. No custom rules loading or engine. Actual authenticated loading/obedience remains unverified. |
| Packaging boundary | Existing host-environment probe now includes the fifth `agent` launcher fixture. Package formats, runtime, NSIS and publication workflow are unchanged. This is executable lookup evidence, not Cursor installation or authentication. |

## Verified syntax and remaining release checks

The accepted **Linux x86_64 Cursor CLI `2026.10.16-e3cb207`** smoke test used an
inspected official installer and isolated disposable HOME/XDG directories. No
authentication, API key or paid task was used; the installation was removed.
The exact command, `agent agent`, global flags after the subcommand, `--`, model,
permissions, trust and workspace forms were accepted through the authentication
gate. Invalid options and missing option values produced distinct syntax errors.
All commands, exit codes and relevant output are retained in the
[validation record](cursor-validation.md#accepted-isolated-syntax-test).

| Evidence | Linux | macOS | Windows |
| --- | --- | --- | --- |
| Actual Cursor version/help/argument syntax | Passed, version above, unauthenticated | Not performed | Not performed |
| Authenticated task / structured protocol / model access | Not performed | Not performed | Not performed |
| Actual Cursor permissions / workspace trust / rules loading | Not performed | Not performed | Not performed |
| Native CodeConvoy deterministic process, headless UI and regression suite | Local checks passed; fresh CI recorded in PR | Fresh CI recorded in PR | Fresh CI recorded in PR |
| Fresh packaged interactive Cursor desktop walkthrough | Not performed | Not performed | Not performed |

Before calling authenticated Cursor execution release-verified, a maintainer with
an installed CLI and existing sign-in should explicitly authorize a disposable
task and record CLI version, platform, requested/effective settings, native events,
file changes and completion. Check configured and denied tool behavior, trust,
AGENTS.md/.cursor/rules loading, Stop/timeout and retained diffs. Model calls can
incur provider charges. These runs were not authorized or performed for this PR.
The existing [platform](platform-validation.md) and [release](releasing.md)
requirements still apply. Cursor remains labelled experimental in user guidance.

## Validation

Local Linux validation:

- `cargo fmt --check`: passed.
- `cargo clippy --locked --all-targets --all-features -- -D warnings`: passed.
- `cargo test --locked --all-features`: passed (662 passed, 10 ignored;
  filtered nested subprocess summaries excluded). Includes both new real-widget
  keyboard/results regressions, rules transport and backend-independent presets.
- `cargo build --locked --release`: passed.
- `python3 -m unittest discover -s packaging -p 'test_*.py'`: 98 passed.
- `python3 packaging/licenses.py check`: passed; 457 locked crates, zero unresolved
  release obligations and one existing maintainer-accepted licensing risk. No
  licensing finding or new publication approval is implied.
- Release `package_probe` built with `test-support` and executed directly using
  disposable launchers: exit 0; all five fixture lookups, disposable Git/worktree
  operations, cancellation and environment-preservation checks passed. This was
  outside an AppImage; no new final-artifact packaging run is claimed.
- Documentation local-link targets and `git diff --check`: passed.

The full suite used `TMPDIR=/var/tmp` and normal OS access because this host's
sandbox `/tmp` Git boundary interferes with disposable worktree fixtures.
Fresh Linux/macOS/Windows CI results are recorded in the PR description.
No authenticated execution or full protocol compatibility is inferred from
fixtures, native process tests, help output or the accepted syntax smoke test.
