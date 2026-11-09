# CodeConvoy 1.3.4 — CLI compatibility guidance and dependency updates

## Changes since 1.3.3

### Antigravity CLI

- Permission help and review text now clarify that system command denylists and
  enterprise policies can still deny actions when `--dangerously-skip-permissions`
  is selected. Execution arguments, permission modes and authentication are
  unchanged. See the [Antigravity contract](../README.md#antigravity-cli-contract).

### OpenCode

- Added regression coverage for OpenCode 2.2.6 background-wait events and process
  termination: a stop event followed by background work does not finish a run
  before process exit, requested cancellation with exit code 143 stays cancelled,
  and an unsolicited exit code 143 fails. No adapter logic changed; both
  OpenCode 1 and OpenCode 2 remain supported.

### Dependencies and licensing

- Updated dependencies, including rfd 0.18.0 and zbus 6.2.1. The release inventory
  covers 32 version upgrades, two added packages and one removed package.
  It retains the new Unicode notice required by unicode-properties 0.1.5 and
  the notices for the added packages. See the
  [1.3.4 release refresh](release-1.3.4-license-validation.md).
- The recorded Microsoft/NSIS inputs, terms, runtime policy and Windows
  packaging configuration are unchanged, so the existing Windows licensing-risk
  decision applies without renewed acceptance.

**Windows Microsoft-component terms:** before using or redistributing Windows
downloads, review the [component terms](https://github.com/ChrisLauinger77/code-convoy/blob/v1.3.4/third-party/MICROSOFT-DISTRIBUTABLE-CODE.txt),
also included in `THIRD-PARTY-NOTICES.txt`. Permission for the Microsoft portions
is conditional on agreement to those terms; if you do not agree, do not use or
redistribute those portions. CodeConvoy's independent MIT license is unchanged.
Managed and downstream distributors must arrange recipient agreement; notice
delivery or silent installation alone does not establish it.

macOS remains ad-hoc signed without notarization, and Windows remains unsigned.
Native package and desktop verification remains a separate release step; see
the [platform matrix](platform-validation.md) and [release procedure](releasing.md).
