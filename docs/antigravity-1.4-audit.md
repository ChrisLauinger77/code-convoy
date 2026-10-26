# Antigravity CLI 1.4.3–1.4.7 compatibility audit

Recorded 2026-10-26 against CodeConvoy **1.3.1**, commit
`d64ba7808c2aaffb1d9efbb0bde6b3fd86fd6f1c`, on Linux x86_64. Sources are Google's
[antigravity-cli changelog](https://github.com/google-antigravity/antigravity-cli/blob/bf1e6e6d4a05803d21dbe9af5ffba04d8b4fc0ff/CHANGELOG.md)
at its 1.4.7 commit, the
[headless guide](https://antigravity.google/docs/cli/headless/), the
[sandbox guide](https://antigravity.google/docs/sandbox?tab=cli) and the public
issue tracker. The official Linux **agy 1.4.7**, current in Google's manifest,
was then installed in isolation and run without sign-in; see
[Real CLI evidence](#real-cli-evidence-agy-147-linux). Nothing was signed in.

**No incompatibility was found between agy 1.4.3–1.4.7 and the `stream-json`
protocol CodeConvoy consumes.** Two documented sandbox changes made the setting
help and review summary inaccurate; those texts and the guide are corrected.
Invocation, flags, the version minimum, classification and defaults are
unchanged.

## Integrated backend

The Google backend is the Antigravity CLI `agy`
([`src/agents/antigravity.rs`](../src/agents/antigravity.rs)), not Gemini CLI.
Gemini CLI changes were not considered.

| Aspect | Implementation |
| --- | --- |
| Executable | `agy` on PATH or an absolute path; native `agy.exe` on Windows |
| Detection | `agy --help`, read from stderr; identity `--print-timeout`, `--prompt-interactive`; requires `--input-format`, `--output-format`, `--effort`, `--model`, `--dangerously-skip-permissions`, `--sandbox` |
| Version | `agy --version`; 1.2.16 or newer |
| Invocation | `agy --input-format stream-json --output-format stream-json [--model=…] [--effort=…] [--dangerously-skip-permissions] [--sandbox]`, one stdin `user` event, stdin closed; no `-p`, `--print-timeout`, `--mode` or continuation flags |
| Execution modes | Direct and Isolated worktree, through the shared scheduler and process supervision |
| Not supported | Session continuation, image attachments |

## Confirmed upstream changes and CodeConvoy exposure

None of these releases changes the `stream-json` events, the stdin transport,
exit codes, `AGY_ERROR`, `denied_actions` or a flag CodeConvoy requires. On
2026-10-26 the headless guide still documents `init`, `step_update` and
`result`, step states `ACTIVE`/`DONE`, the seven result statuses, the stdin exit
codes and `authentication required` for a missing sign-in. Those match the
decoder.

| Version | Confirmed change | CodeConvoy exposure |
| --- | --- | --- |
| 1.4.3 | Sandbox options are no longer offered where the sandbox cannot run (Linux hosts blocking unprivileged user namespaces, Windows before 11 24H2). Empty-file creation and blank artifact details fixed. A failing `PostToolUse` hook no longer fails its tool. | Interactive options only. Tool fixes can change task results, not the protocol. |
| 1.4.4 | Auto mode on Windows 11 24H2 for personal accounts. The sandbox guide labels its fallback "CLI v1.4.4+": on Linux hosts and containers that block unprivileged user namespaces, `--sandbox` and `enableTerminalSandbox` have no effect. | **Help and summary corrected.** **Enable for this job** said commands use the sandbox; on such hosts they do not. CodeConvoy passes no `--mode`; a persisted Auto mode applies and `init.permission_mode` appears in Activity. |
| 1.4.5 | Fixed the agent retrying without end when content safety filters blocked a response. | A job could previously run until Stop or its time limit. The resulting failure shape is undocumented; non-`SUCCESS` results already fail. |
| 1.4.6 | Sandboxed commands can no longer read or modify `~/.docker`, `~/.kube` or `~/_netrc` without an explicit grant. Enterprise sign-ins persist on macOS and Windows. | **Documented.** Sandboxed Docker/Kubernetes commands fail unless the user's Antigravity settings grant access; CodeConvoy adds no grants. Lost enterprise sign-ins already failed with sign-in advice. |
| 1.4.7 | Headless `-p` runs use less CPU and skip status-line and window-title commands. Customizations load with ADC (`AGY_ADC_AUTH`). Plugin MCP OAuth tokens belong to the signed-in user and may need a new sign-in. | Documented the ADC fix. Plugin MCP re-sign-in cannot happen headless; how agy reports it is unverified. |

Earlier reports that 1.4.4–1.4.7 concern sandbox access, headless execution,
permissions and authentication are confirmed. The credential-directory change
belongs to 1.4.6. The documented Linux `--sandbox` no-op is labelled 1.4.4.

## Open upstream issues

| Issue | Relevance | CodeConvoy behavior |
| --- | --- | --- |
| [#1328](https://github.com/google-antigravity/antigravity-cli/issues/1328), regression in 1.4.6 | A headless turn hangs when the model batches tool calls and one is soft-denied: a sibling stays pending and no `result` follows. Reported with `-p --mode accept-edits` on macOS. Its analysis places the wait in print-mode completion polling. agy 1.4.7 logs CodeConvoy's stdin invocation as print mode (`printmode.go`), so the same code runs; the hang itself needs a signed-in run and was not reproduced. Not mentioned in the 1.4.7 changelog. | The job stays running until **Stop** or the **Time limit**, which stop the process tree. Indistinguishable from a long tool, so no watchdog was added. The guide recommends a time limit. |
| [#1296](https://github.com/google-antigravity/antigravity-cli/issues/1296), still on 1.4.7 | Windows `--sandbox` on an exFAT volume exits 2 at the first command, with a tool step left `ACTIVE` and no `result`. | Failed with exit code 2 and "stream ended without a final result". |
| [#1336](https://github.com/google-antigravity/antigravity-cli/issues/1336) | On Linux, inherited `SSH_*` variables (stale tmux sessions) make agy skip the desktop keyring and ask for sign-in. Observed with `agy models`. | Documented as a workaround. No marker was added for its `Please sign in` text without a headless capture; such a run still fails. |
| [#1286](https://github.com/google-antigravity/antigravity-cli/issues/1286) | A sandboxed run could stop another run's processes; the reporter confirms 1.4.7 fixes this for sandboxed Windows runs. | CodeConvoy runs agy jobs concurrently. Unsandboxed agents can still stop other same-user processes, as with every backend. |

The sample records in #1328 use `type`/`Turn_Result` names and a step state
`ERROR`, unlike the documented format and the capture in #1296. They were not
used as fixtures. If agy did emit a step state other than `ACTIVE` or `DONE`,
CodeConvoy would fail the job as an unrecognized step state.

## Requested checks

| Check | Supported by the backend | Evidence |
| --- | --- | --- |
| Normal headless execution | Yes | Existing documented-shape fixtures, byte-boundary decoding and process-level execution tests. No authenticated run. |
| Permission-denied operation | Yes (soft-deny, `denied_actions`) | Existing refused-action tests; new `a_turn_stalled_after_a_refused_tool_never_succeeds` and `a_run_stalled_after_a_refused_tool_ends_only_by_time_limit_or_stop` for the #1328 shape. |
| Missing sandbox access to configuration | Yes (`--sandbox`) | New `sandbox_credential_denials_stay_tool_outcomes_rather_than_sign_in_failures`: a registry's `authentication required` in tool output or the answer stays a tool outcome, not a sign-in failure. |
| Authentication failure | Yes | Real agy 1.4.7 signed-out output is byte-identical to the 1.4.2 capture its test uses; 8 real signed-out jobs failed with sign-in guidance. |
| Structured output parsing | Yes | Existing decoder suite; no format change found. |
| Cancellation and process cleanup | Yes | Existing Stop, Quit, time-limit and leftover-process lifecycle tests; the new stalled-run test covers time limit (Failed after confirmed cleanup) and Stop (Cancelled). |
| Session continuation | Not used by CodeConvoy | Not tested; see the [guide](antigravity.md#session-continuation-and-conversation-history). |

Synthetic fixtures assume that a soft-denied tool is a `DONE` step with
`tool_info.error`; the denial shape is not documented.

## Real CLI evidence (agy 1.4.7, Linux)

Performed on 2026-10-26 on Debian Linux x86_64 after explicit approval. Google's
manifest `manifests/linux_amd64.json` named version `1.4.7` and
`antigravity-cli/1.4.7-5900205422411776/linux-x64/cli_linux_x64.tar.gz`
(65,681,429 bytes). The archive matched the manifest's SHA-512 and contains one
file, `antigravity`, copied to a scratch `bin/agy`. The installer and
`agy install` were not run. Every command used `env -i` with a temporary
`HOME`/XDG directory, `PATH=/usr/bin:/bin` and no D-Bus, SSH or Google variables.
agy therefore used file-based secret storage below the temporary home and never
reached the desktop keyring.

| Check | Observation | CodeConvoy consequence |
| --- | --- | --- |
| `agy --version` | Exit 0, stdout `1.4.7` | Version gate parses it |
| `agy --help`, `-h`, `help` | Exit 0, stdout empty, help on stderr; the same 21 flags as the 1.4.2 contract fixture | Detection unchanged |
| Side effects of help/version | No files, no processes | Check CLI stays side-effect free |
| CodeConvoy invocation, signed out | Exit 1 after 0.5 s; stdout and stderr byte-identical to the 1.4.2 capture | Existing fixture test still represents 1.4.7 |
| Mode of the stdin transport | agy's log reports `Print mode: starting` for `--input-format stream-json` | Print-mode issues such as #1328 apply to CodeConvoy's transport |
| Auto-updater | First run started `agy --bg-updater` in its own session, reparented to the user's systemd service manager; it exited on its own | Same as 1.4.2 on macOS; outside CodeConvoy's process tree |
| Files written | Only below `~/.gemini` in the temporary home; the repository stayed clean | No repository side effects |

The opt-in tests passed with this binary:

```sh
env -i HOME=<temporary home> XDG_CONFIG_HOME=… XDG_DATA_HOME=… XDG_CACHE_HOME=… XDG_STATE_HOME=… PATH=/usr/bin:/bin LANG=C.UTF-8 CODECONVOY_AGY=<scratch>/bin/agy <antigravity test binary> --ignored --exact installed_agy_passes_check_cli
env -i … CODECONVOY_AGY_SIGNED_OUT=1 <antigravity test binary> --ignored --exact installed::installed_agy_job_without_sign_in_fails_with_sign_in_guidance
```

Check CLI reported `1.4.7 detected`. The second test ran **8 real jobs** through
`runner::prepare` and `RunManager`: Direct and Isolated, each with default
settings, model and effort, **Approve all tool requests** and **Enable for this
job** (the sandbox). All failed with exit code 1 and the sign-in guidance; the
checkout stayed clean and no agy process remained afterwards.

## Changes

- [`src/agents/antigravity.rs`](../src/agents/antigravity.rs): the **Terminal
  sandbox** help names the Linux no-op and the 1.4.6 credential directories; the
  review summary no longer claims the sandbox applies unconditionally.
- Tests: two decoder tests in
  [`tests/antigravity/output.rs`](../tests/antigravity/output.rs), one
  process-level test in
  [`tests/antigravity/execution.rs`](../tests/antigravity/execution.rs) with a
  stalled fixture mode, and a summary assertion.
- Guidance: [Antigravity guide](antigravity.md), the
  [validation record](antigravity-validation.md) and the README.

Deliberately unchanged:

- **No Linux gate for `--sandbox`.** Windows below 24H2 is rejected at review.
  On Linux only `user.max_user_namespaces=0` and
  `kernel.unprivileged_userns_clone=0` prove the block. AppArmor's restriction
  (Ubuntu 23.10 and newer) and container seccomp policies cannot be read that
  way, and agy's own detection is undocumented. A partial gate would reject
  existing jobs; it is left to a maintainer decision.
- No idle watchdog, no new sign-in marker, no minimum-version change and no
  sandbox grants or relaxation.

## Validation and remaining risks

The required checks were run on Linux x86_64 with this audit's changes:

- `cargo fmt --check`: passed.
- `cargo clippy --all-targets --all-features -- -D warnings`: passed.
- `cargo test --all-features`: **823 passed, 0 failed, 14 ignored**; the
  Antigravity target has 33 passed and its 3 opt-in real-CLI tests ignored.
- `cargo build --release`: passed.

The opt-in real-CLI tests passed separately with agy 1.4.7, as described above.

Unverified: 1.4.3–1.4.6 binaries; 1.4.7 on macOS and Windows; whether the #1328
hang occurs with the stdin transport; the stream-json shape of a soft-denied tool
and of a plugin MCP sign-in failure; the 1.4.5 content-safety failure shape;
authenticated execution on every platform.
