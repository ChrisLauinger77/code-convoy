# CodeConvoy 1.3.3 — Stricter Antigravity completion

## Changes since 1.3.2

### Antigravity CLI

- A run now fails when an Antigravity result reports a non-empty
  `finished_reason`, such as `print_timeout`, `interrupted` or `error`. agy
  1.4.11 and later add this optional field to identify a turn that was cut off,
  independently of the result status and exit code. Previously such a result
  could still complete as successful, including with exit code 0. The premature
  termination stays sticky, so a later successful result cannot erase it. Results
  without the field complete as before. See the
  [Antigravity contract](../README.md#antigravity-cli-contract).

### Claude Code

- No production change. New regression tests confirm that a Claude startup
  failure without output fails the run whether the CLI exits with 0 or, as
  Claude Code 2.1.314 and later do, with 1, and that a nonzero exit fails the
  run even after complete headless output.

### Dependencies and licensing

- Routine dependency updates, including zbus 6.2.0, AccessKit 0.25.2 and ICU4X
  2.4.0. No dependency was added or removed and every license expression is
  unchanged. The release notice inventory was refreshed for the 33 updated
  crate versions; see the
  [1.3.3 release refresh](release-1.3.3-license-validation.md).
- The recorded Microsoft/NSIS inputs, terms, runtime policy and Windows
  packaging configuration are unchanged, so the existing Windows licensing-risk
  decision applies without renewed acceptance.

**Windows Microsoft-component terms:** before using or redistributing Windows
downloads, review the [component terms](https://github.com/ChrisLauinger77/code-convoy/blob/v1.3.3/third-party/MICROSOFT-DISTRIBUTABLE-CODE.txt),
also included in `THIRD-PARTY-NOTICES.txt`. Permission for the Microsoft portions
is conditional on agreement to those terms; if you do not agree, do not use or
redistribute those portions. CodeConvoy's independent MIT license is unchanged.
Managed and downstream distributors must arrange recipient agreement; notice
delivery or silent installation alone does not establish it.

macOS remains ad-hoc signed without notarization, and Windows remains unsigned.
Native package and desktop verification remains a separate release step; see
the [platform matrix](platform-validation.md) and [release procedure](releasing.md).
