# OpenCode 2.1 audit captures

Captured on 2026-10-26 with official Linux x86_64 npm platform binaries.
[captures.json](captures.json) records package URLs, verified SHA-512 integrity,
CLI version, scenario, stdout filename, exit code and expected decoder outcome.
The JSONL files are actual CLI stdout, with no event fields rewritten or invented.

The provider was a local deterministic HTTP/SSE fixture using the supported
OpenAI-compatible provider package. It produced text, read-tool calls, a 401 and
a quota 429 with `Retry-After: 2`. The read tools actually accessed the disposable
repository, including a missing file. Reported input usage of 25,000 tokens
triggered automatic compaction; `*-compaction-export.json` independently records
the completed compaction. An invalid summary produced the compaction-failure
stream. The local stdio MCP server requested input, timed out after two seconds,
and returned a real MCP tool error. Cancellation captures came through the
unchanged CodeConvoy adapter and process executor with a live shell tool.

The V2 invocation was:

```text
opencode run --standalone --format json --model=audit/audit-model
```

The V1 invocation replaced `--standalone` with `--dir <absolute-repository>`.
Continuation probes added `--session <previous-session-ID>`. Process probes and
MCP probes added `--auto`. Prompt input was `AUDIT:<scenario>` followed by a
newline on stdin. Help files came directly from `opencode run --help`.

Each version used separate XDG directories, disabled update/model/LSP downloads,
and inherited no credentials created for this audit. Repositories had spaces and
Unicode in their paths. No hosted model, account login or provider key was used.
These are real CLI captures with a synthetic provider, not authenticated provider
E2E evidence. The older 1.19.3/2.0.32 free-model fixtures remain unchanged.

`expected: Failed` for the cancellation stdout reflects the decoder's refusal to
report success on exit 130. The runner owns the separate Cancelled result.
Successful tool-error/MCP-input streams mean the model continued after the tool
failure; they do not assert that the requested tool operation succeeded.

See [the audit](../../../docs/opencode-2.1-audit.md) for results, source links and
the separate cleanup findings. Capture scripts, process observations and command
logs are retained in the audit workspace at `/workspace/work/opencode-audit`.
