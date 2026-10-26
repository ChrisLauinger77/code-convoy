# Codex 0.163/0.164 audit captures

Captured on 2026-10-26 on Linux x86_64. [captures.json](captures.json) records the
binary provenance, scenario, stdout filename, exit code and expected decoder
outcome. The JSONL files are actual `codex exec --json` stdout, with no event
fields rewritten or invented. The help files are unmodified `codex --help` stdout.

The 0.163.0 binary was the installed standalone release. The 0.164.0 package was
downloaded from the official GitHub release and checked against its published
SHA-256. Both ran CodeConvoy's exact argument vector:

```text
codex --no-daemon --ask-for-approval never exec --json --color never --ephemeral --sandbox <mode> -
```

Each run used a fresh `CODEX_HOME` whose only configuration pointed
`openai_base_url` at a local deterministic Responses API HTTP/SSE fixture and
disabled update checks. A dummy key reached only that fixture; no hosted model,
account login or provider credential was used. Repositories had spaces and
Unicode in their paths. The prompt on stdin selected the fixture's scenario.

The fixture has no WebSocket endpoint, so every stream carries the CLI's real
`Reconnecting... n/5` `error` records and its HTTPS fallback item before the
turn result. Escalation and sandbox-denied streams come from real nested
`exec_command` calls: the escalation (`sandbox_permissions: require_escalated`)
was rejected by the approval policy and a read-only write failed in the Linux
sandbox. The model then answered. The failed-request stream is a real HTTP 400.

Cancellation streams are the stdout lines of CodeConvoy's Raw output from the
unchanged adapter, RunManager and process executor; labelled stderr lines were
dropped. `exit_code: null` means CodeConvoy stopped the process; the decoder
reports Failed while the runner owns the separate Cancelled result.

See [the audit](../../../docs/codex-0.164-audit.md) for results and limits. The
local provider, capture scripts, process observations and logs are not part of
this repository, so these files cannot be regenerated from it.
