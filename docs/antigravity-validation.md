# Antigravity CLI feasibility and validation

Assessed on **2026-10-17** from Google's public documentation, the public
`google-antigravity/antigravity-cli` changelog and issue tracker. No Antigravity
CLI was installed on the assessment machine; nothing below was executed,
authenticated or observed locally unless stated.

## Interface decision

Antigravity has three surfaces: the desktop app, the Python SDK and the
terminal CLI `agy`. Only `agy`'s documented headless (print) mode fits
CodeConvoy's execution model: one local child process per repository, shell-free
arguments, stdin/stdout pipes and process-tree cancellation. The desktop app is
not automated. The SDK would be a custom Google agent implementation. Gemini CLI
is not a substitute.

| Requirement | Documented capability | Source |
| --- | --- | --- |
| CLI / automation | `agy -p` / `--print` runs one prompt and exits | [Headless mode](https://antigravity.google/docs/cli/headless/) |
| Prompt transport | Flag argument, or `--input-format stream-json` over stdin (requires `--output-format stream-json`) | Headless mode |
| Structured output | `json` envelope or `stream-json` NDJSON: `init`, `step_update`, one `result` | Headless mode |
| Completion | `result.status` is `SUCCESS`, `ERROR`, `CANCELED`, `INTERRUPTED`, `INVALID`, `WAITING` or `RUNNING`; exit codes 0, 1, 2 and 3 (`AGY_ERROR`, since 1.2.6/1.2.10) | Headless mode, changelog |
| Working directory | Process cwd; `init.cwd` reports the effective workspace; no headless workspace flag documented | Headless mode, [conversations](https://antigravity.google/docs/cli/conversations/) |
| Permissions | Settings rules (`permissions.allow/deny/ask`), soft-deny in headless, `--dangerously-skip-permissions`, `--sandbox`, `--mode` | Headless mode, [permissions](https://antigravity.google/docs/permissions?tab=cli), [modes](https://antigravity.google/docs/cli/modes/) |
| Session continuation | `--continue`, `--conversation <id>`; conversations scoped to cwd | Headless mode, conversations |
| Authentication | Google sign-in (OS keyring), Gemini Enterprise, ADC (`AGY_ADC_AUTH=true`), Gemini API key with `modelProvider=gemini`; headless reuses cached credentials and fails with `authentication required` without a terminal | [Install and auth](https://antigravity.google/docs/install?tab=cli), headless mode |
| Platforms | macOS, Linux and Windows releases (changelog fixes for each); Auto mode is Linux/macOS only | Changelog |

Third-party orchestration of `agy` with account sign-in is described as
supported only by an unofficial forum answer; no official terms statement was
found. Invoking the binary through pipes is the boundary; extracting tokens or
calling backend endpoints is out of scope and never done.

## Minimum version

CodeConvoy requires **agy 1.2.16 or newer**. Check CLI fails closed when
`agy --version` fails or has no `MAJOR.MINOR.PATCH`. The floor is the newest
changelog fix the planned protocol depends on:

| Version | Dependency |
| --- | --- |
| 1.1.1 | Server errors in print mode go to stderr with non-zero exit (issue #76: silent exit 0 with empty stdout) |
| 1.1.3 | Tools needing approval are soft-denied instead of hanging or silently approved |
| 1.1.8 / 1.1.15 | `--output-format stream-json`; `--input-format stream-json` |
| 1.1.27 | Refused actions reported as `denied_actions` |
| 1.2.10 | Streamed runs ending in error exit 3 with the partial response |
| 1.2.16 | Headless runs no longer exit before answering a finished background command |

The version gate is a backend hook (`AgentBackend::check_version`); other
backends keep version output informational.

## Planned execution constraints

These are review conditions for the execution milestone, not implemented
behavior:

- Exit 0 alone is never success. Success requires exactly one terminal
  `result` with `status` `SUCCESS`, matching the exit status.
- Soft-denied tools exit 0. `denied_actions` must remain visible and must not be
  reported as a clean result.
- `--print-timeout` returned partial output with exit 0 in 1.1.28, and the
  documented default differs between the guide (5 minutes) and changelog
  (unlimited since 1.2.6). Expiry must not become success.
- `init.cwd` must match the assigned checkout.
- Antigravity's own Git operations, `/fork` and remote control are not used.

## Unverified

- Actual `agy --help` text: `tests/fixtures/antigravity-help-contract.txt` is a
  synthetic documentation contract, including the `Antigravity` identity marker.
- `agy --version` output format.
- Installation locations; discovery uses the shared generic directories only.
- Concurrent headless instances sharing one sign-in and conversation store.
- Process-tree cancellation leaving no daemon (fixed in 1.2.9 according to the
  changelog).
- Authenticated execution, model access and permission enforcement on every
  platform.
