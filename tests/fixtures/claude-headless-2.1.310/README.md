# Claude Code 2.1.310 headless audit captures

Captured on Linux on 2026-10-26 using the real Claude command builder,
`process::execute_until`, existing authentication, and detached Git worktrees
of disposable repositories. The official native binary was downloaded into
temporary storage; its size and SHA-256 matched the official release manifest:
`b73676430af12f97316a798b8371f81d640f90a58bdcb8f3ce56a835bb47df56`.
The installed CLI (2.1.303) was not updated.

These are reduced, sanitized replays. They retain initialization, tool calls,
tool results, permission metadata and terminal fields in their original order.
Session/tool identifiers and response prose are replaced. Thinking/signatures,
account/plugin/server inventories, usage, UI notifications and rate-limit events
are omitted. The lengthy generic permission-denial advice is shortened equally
in both tool-result fields. They contain no credentials or original local paths.

| Capture | Actual process outcome | Relevant observation |
| --- | --- | --- |
| `normal.jsonl` | Exit 0 | Successful no-tools response |
| `denied.jsonl` | Exit 0 | Denied Bash operation; success result with one permission denial |
| `mcp-confirm.jsonl` | Exit 0 | Human-only local MCP tool refused despite an exact tool allow; server never called |
| `mcp-elicit.jsonl` | Exit 0 | Form elicitation cancelled by the headless client; server deliberately remains pending and times out; success result with no permission denials |
| `auth.jsonl` | Exit 1 | Empty temporary configuration/no injected credentials; success subtype with `is_error=true` and `terminal_reason=api_error` |
| `cancel.jsonl` | Signal termination; `cancelled=true` | Cancellation after the MCP server sends its elicitation; no terminal result; server's OS lock released before supervision returns |

The local server is `tests/support/claude_mcp.rs`. Only its harmless test tool is
allowed through a test-only `--allowedTools` argument; production arguments and
permission policies are unchanged. The form's observed response was
`{"jsonrpc":"2.0","id":"audit-question","result":{"action":"cancel"}}`.
Consequently the live timeout capture has the ordinary MCP timeout diagnostic,
not the new unanswered-question wording from 2.1.310. That wording is separately
tested using a source-backed synthetic event; it was found in the verified
official binary and described by the official release notes.

Reproduce with an existing Claude login (uses model calls):

```sh
CODECONVOY_CLAUDE_AUDIT_EXECUTABLE=/absolute/path/to/claude \
  cargo test --all-features --test claude real_headless_permission_audit \
  -- --ignored --nocapture
```

Optional test-only `CODECONVOY_CLAUDE_AUDIT_CASES` selects comma-separated cases;
`CODECONVOY_CLAUDE_AUDIT_CAPTURE` saves original streams to a private audit
directory for manual sanitization. Original streams can contain account and
configuration metadata and should not be committed. The normal suite only
replays these checked-in fixtures; it does not call Claude or authenticate.
