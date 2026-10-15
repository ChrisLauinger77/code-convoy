# CodeConvoy 1.0.0 — Final readiness report

**Recommendation: Not Ready.** Metadata is prepared for 1.0.0 and the existing
first-stage workflow can build candidates without tags. Known third-party
licensing obligations and missing native desktop evidence prevent release
approval. No tag, publication or external package-repository update is part of
this PR. PR 5 remains a draft for maintainer review.

## Five-PR summary

| Readiness step | Evidence and outcome |
| --- | --- |
| [PR 1: upgrade/data integrity (#57)](https://github.com/ChrisLauinger77/code-convoy/pull/57) | Reject malformed retention policy without discarding persisted state; merged. |
| [PR 2: execution/worktree safety (#58)](https://github.com/ChrisLauinger77/code-convoy/pull/58) | Protect actual isolated checkout, cancel blocked lifecycle waits, supervise Unix launcher/descendant lifetime conservatively; merged. |
| [PR 3: cross-platform verification (#59)](https://github.com/ChrisLauinger77/code-convoy/pull/59) | Expand upgrade fixtures, native path/process regressions and platform evidence; merged. Follow-up #60 establishes the glibc 2.39 build/gate; #62 adds Claude Auto compatibility. |
| [PR 4: documentation (#67)](https://github.com/ChrisLauinger77/code-convoy/pull/67) | First-use, contribution, security, compatibility and release guidance; private vulnerability reporting enabled; merged. |
| [PR 5: final readiness (#68)](https://github.com/ChrisLauinger77/code-convoy/pull/68) | Prepare 1.0.0 metadata, audit redistributed components, ship known notices/source and add omission/publication gates. Licensing blockers remain explicit. |

## Metadata and distribution audit

Cargo.toml and only CodeConvoy's Cargo.lock entry are changed to **1.0.0**.
Application/About and Windows version resources continue to derive from Cargo;
package and macOS bundle versions use the same authority. Identifier
`io.github.chrislauinger77.code-convoy`, desktop identity, icons and the six
artifact naming patterns are unchanged. Current release notes describe changes
since 0.10.0; historical evidence retains its original versions.

Homebrew's current cask uses `CodeConvoy-<version>-macos-universal.dmg` and
`CodeConvoy.app`. Scoop uses `CodeConvoy-<version>-windows-x86_64.zip` and the
root `CodeConvoy.exe`. Their current 0.10.0 definitions were read again during
this audit; the naming/path contracts are preserved. They have not been edited.
No WinGet definition is maintained here; a future submission still needs exact
installer validation, a new version and its final checksum. No package-manager
publication automation was added.

The first stage retains all supported runners, compiler/Python/packaging tool
versions, formats, installation tests, GLIBC gate and aggregate checksum checks.
The second stage still downloads the successful first-attempt candidate by
verified workflow/run/artifact identity and exact source commit, without
rebuilding. It additionally refuses publication while the license inventory
records unresolved obligations. A changed commit requires a fresh candidate.

## Upgrade and workflow evidence

Existing disposable fixtures cover releases 0.1, 0.2, 0.3, 0.6, 0.7 and 0.9;
0.10 used the same serialized fields. Tests preserve every supplied setting,
registration, group, template, backend option, validation command/result,
history/retention value, desktop preference and ownership field through
load/save/reopen, except the documented interruption recovery. The fixtures
never touch their historical paths or the maintainer's real store. Separate
tests rebind ownership to disposable real Git worktrees and verify retained
contents/manifests after reopening and reconciliation. This is fixture-based
upgrade evidence, not an old-binary installer upgrade or real-user data trial.

| Critical workflow | Automated evidence; limits |
| --- | --- |
| Registration, groups and selection | Canonical/nested path checks, overlapping-group selection and missing members; no fresh manual registration UI walkthrough claimed. |
| Execution and multiple convoys | Backend fixtures, fair admission, per-convoy/global limits and repository exclusions. |
| Cancellation and Quit | Process-tree, blocked Activity/binding, launcher-exit, shutdown and uncertain-cleanup regressions. |
| Results, changes and validation | Fixed-baseline Git inspection, explicit Apply/Discard, validation commands/presets and outcome separation. |
| History/export/retention | Metadata-only exports, deterministic retention preview, pinned ownership and atomic persistence. |
| Storage and eligible cleanup | Disposable ownership/manifests/backlinks and failure-preserving cleanup tests; Windows conservative refusal remains documented. |
| Desktop restore and notifications | Existing private-bus/backend lifecycle fixtures. Actual delivery, tray host, restore and accessibility need native target checks. |

No new authenticated provider task was performed for PR 5. Existing CLI evidence
keeps its date/version and scope; OpenCode authenticated E2E remains unverified.

## Blockers and outstanding evidence

- **Blocker — license/source obligations:** the [license audit](third-party-licenses.md)
  records exact AppImage static-library provenance and LGPL relinking gaps,
  missing `dispatch` attribution evidence, installer plugin provenance and
  Windows static CRT redistribution evidence. Passing tests does not resolve
  these legal/source obligations.
- **Release approval gap — native target evidence:** Ubuntu 24.04/Debian 13
  hardware-backed Wayland/XWayland, rendering/decorations, clipboard/dialogs,
  task cancellation and normal Quit; Windows interactive desktop behavior;
  Intel macOS execution; notification/tray activation and screen readers retain
  the gaps in the [platform matrix](platform-validation.md). Container/Xvfb or
  ABI inspection must not be promoted to full desktop compatibility.
- **Documented limitations:** macOS ad-hoc signing and Windows unsigned packages,
  separately installed/authenticated agent CLIs, unavailable ordinary Windows
  Storage cleanup, platform-dependent tray/notification presentation. These
  remain explicit capabilities/constraints rather than silent fallbacks.

No new confirmed data-loss, worktree-deletion or core execution defect was found
in this infrastructure change. That statement does not certify missing native
tests. The known licensing blockers alone require **Not Ready**.

## Final verification

Local 1.0.0 validation on Debian forky/sid, Rust 1.96.1 and Python 3.14.8:

- Formatting and strict all-target/all-feature Clippy: passed.
- Locked all-feature Rust tests: **602 passed, 0 failed, 10 ignored**; nested
  subprocess summaries excluded. Includes all upgrade fixtures above.
- Locked release application build: passed.
- Packaging regression suite: **58 passed**; license evidence and prospective
  `v1.0.0` tag/version checks passed. No tag was created.
- Existing remote `v1.0.0` tag and release checks returned absent.
- Opt-in private-D-Bus unavailable-service regression: **1 passed** with its
  test-support feature enabled; no visible notification or tray interaction is
  implied.

Fresh candidate and exact final-artifact results will be recorded after the
platform package build completes. Earlier 0.10.0 package references are
diagnostic evidence only. The handoff stops at this draft PR and waits for
maintainer approval.
