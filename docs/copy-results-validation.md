# Copy Results validation — 0.8.0 PR 2

Scope: Markdown clipboard copying in the existing convoy and repository result
views. No new export screen, persistence fields, dependencies, backend behavior,
scheduling changes, release tag or package publication.

## Automated coverage

Sixteen pure formatting tests in `src/ui/result_markdown_tests.rs` cover successful,
mixed, failed, cancelled and interrupted results; known/unknown change counts;
review/resolution states; independent validation outcomes; empty/partial history;
missing or reversed timestamps; all backends; persisted round-trip equivalence;
launch ordering; Unicode, pipes, Markdown/HTML and multiline escaping; bounded
metadata; omitted logs, diagnostics, backend options, attachments and paths.
Live Review, pre-run Git statistics and mutable worktree observations cannot
replace the recorded completion metadata.

The PR review regression reproduces an absolute path leaking from a list-valued
argument such as `--inputs=src,/home/alice/private`. It failed before the fix and
passes with comma, semicolon, pipe and ampersand boundaries recognized across
task summaries, repository names and validation arguments. Relative list items
such as `src,tests/unit` remain visible.

A second review regression reproduces named-user home paths such as
`~alice/private` and `--config=~alice/private` being copied. The test failed before
the fix and now covers slash/backslash forms, quoted and list-delimited tokens,
Unicode and qualified usernames, and literal embedded tildes that must stay
visible. Recognition uses text alone, without expanding homes or looking up users.

The control-character regression failed for `"\0/home/alice/private"` before the
fix. It exercises every non-whitespace C0/C1 control across task summaries,
repository names and validation arguments, including controls within named-user
home prefixes. Stripped controls establish path boundaries and cannot break an
existing home prefix; ordinary relative metadata remains visible. The test also
confirms that current validation arguments allow these controls except NUL.

The attached-short-option regression failed for `-I/home/alice/private` before
the fix. It covers single-letter options with absolute and home-relative values,
quoted/embedded option tokens, and stripped controls within the option prefix.
Relative option values, ordinary embedded hyphens and numeric fractions stay
visible. Detection recognizes the textual `-<letter><value>` form without
interpreting a specific CLI's options.

The input-redirection regression failed for `Read </home/alice/private` before
the fix. It covers absolute and home-relative paths after `<` in task summaries,
repository names and validation arguments, including redirection without spaces.
Path-containing fields and arguments are omitted in full, while relative paths
and ordinary HTML-like text retain their Markdown escaping.

The at-marker regression failed for `@/home/alice/private` before the fix. It
covers plain and option-prefixed markers in task summaries, repository names
and validation arguments, with absolute and home-relative paths omitted in full.
Qualified usernames such as `~user@domain/private` remain recognized. Relative
response-file paths, scoped package names and ordinary email addresses stay visible.

Three egui tests in `src/ui/copy_results_tests.rs` exercise the real result views
through Tab/Enter and check emitted native clipboard commands and unchanged
persisted state. Coverage includes both actions, every existing detail tab,
selection outside history/comparison filters, a finished job in an active convoy,
absent actions for active/missing results, expiring feedback and compact result
pane widths in System/Dark/Light themes. The shared keyboard harness now captures
platform output commands for assertions.

## Checks — Linux x86-64, 2026-10-10

- `cargo fmt --check`: passed.
- `cargo clippy --all-targets --all-features -- -D warnings`: passed.
- `cargo test --all-features`: passed, including all 19 new tests. Existing
  optional authenticated/installed CLI probes remain ignored.
- `cargo build --release`: passed.
- `git diff --check`: passed.

The full suite used a disposable `TMPDIR` under `/var/tmp`, because this
execution environment places a `.git` marker in `/tmp`. Existing worktree storage
safety checks correctly reject that ancestry; no safety logic was changed.

## Native clipboard coverage

The existing `v05_visibility_validation` example was built with `--all-features`
and launched at 1180 × 820 using the Linux X11 backend on the available Wayland
session (XWayland). It uses disposable state and repository fixtures with no
installed agent execution. Both copy controls were visually present in Review.

**Copy Summary passed**: after explicitly focusing the fixture and using separate
mouse press/release events, the system clipboard contained convoy #6, its four
ordered repository rows, accurate mixed outcomes and unknown change count, and
no temporary repository paths. Clipboard contents were checked using a test-only
clipboard reader; the application itself uses only egui's clipboard integration.

Synthetic repository-button activation did not reliably reach the fixture, so a
native **Copy Repository Result** round trip is not claimed. Its real widget,
keyboard activation and exact clipboard payload pass the headless egui tests.
Native feedback timing/keyboard interaction, Linux native Wayland, macOS and
Windows remain manual smoke checks. No cross-platform runtime claim is inferred
from the Linux build or shared clipboard API.

Manual follow-up: select a terminal convoy, activate each copy action and paste
into a plain-text editor. Check feedback, headings, repository order and Unicode
names; switch result tabs and select partial history. Confirm unknown metadata
stays explicit and no logs/diffs/paths are added. Repeat with Tab/Enter and the
supported themes on each platform.

## Known metadata limits

Validation without a saved record is “Not run / not recorded”; current
configuration cannot establish its historical setup. Isolated completion records
usually have only a changed/unchanged flag, so file counts remain absent. Missing
completion timestamps do not produce zero durations. Recognizable paths in free
text are conservatively omitted, sometimes hiding useful text. Copied task/name/
command metadata still needs review before sharing and is not guaranteed free
of sensitive information.

## Deterministic generated Markdown

This is the exact golden output asserted by
`successful_convoy_has_stable_complete_markdown`:

```markdown
# Convoy #42 — Succeeded

Backend: OpenAI Codex CLI

Task summary: Improve result inspection

First recorded start: 2023-11-14 22:13:30 UTC

Created: 2023-11-14 22:13:20 UTC

Completed: 2023-11-14 22:14:54 UTC

Duration: 1:34

Repository jobs: 2 recorded

## Summary

- Succeeded: 2
- Failed: 0
- Cancelled: 0
- Changed: 1
- Unchanged: 1
- Unknown changes: 0
- Needs review: 1

## Validation

- Passed: 1
- Not run / not recorded: 1

## Repository Results

| Repository | Result | Changes | Review | Validation |
|---|---|---|---|---|
| alpha | Succeeded | Changed · 8 files | Required | Passed |
| beta | Succeeded | Unchanged | Not required | Not run / not recorded |

Changes are saved completion observations; validation is the latest recorded result.
```
