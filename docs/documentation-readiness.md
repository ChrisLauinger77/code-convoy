# 1.0 readiness PR 4 — Documentation and project readiness

Audit date: 2026-10-15. Base: `f26d8bd` after readiness PR 3 and the Linux ABI
and Claude Auto compatibility changes. This is documentation/project guidance
only. Application version remains 0.10.0; no application, dependency, schema,
package payload or CI/release workflow changes are included.

## Coverage and corrections

| Area | Result |
| --- | --- |
| README | Added a guide index and accessible screenshot description; separated source documentation from published-package evidence; linked current toolchain/platform/CLI boundaries. Existing macOS screenshot still represents the two-pane workflow and is retained. |
| First use | Added [Getting started](getting-started.md): installation, external authentication/CLI checks, registration/groups, convoy review, execution, Git review, validation and history. |
| Groups, filtering and context | Cross-checked the existing guide against library/repository-selection controls. Hidden selections and overlapping-group deselection remain explicit; templates are task-only. |
| Worktrees and storage | Kept default/custom location, retained results, Apply/Discard and conservative Storage cleanup distinct. Corrected the history guide's obsolete claim that Storage Overview does not exist. Windows ordinary Storage cleanup remains unavailable. |
| Validation and reports | Linked one-command configuration, presets/bulk assignment, manual execution, session-only output, retention and metadata-only Markdown/JSON exports. No automatic validation or export-as-backup claim. |
| Keyboard and desktop | Cross-checked the shortcut binding table; added a stable README anchor. Notification guidance links newer hosted/private-bus evidence while retaining missing native delivery/click checks. |
| CLI evidence | Corrected the architecture overview's stale Claude authenticated-E2E claim. OpenCode authenticated E2E remains unverified. No new authenticated test is claimed. |
| Contributor setup | Added Linux/macOS/Windows prerequisites, exact checks, disposable UI fixture instructions, dependency review and best-effort support expectations. Documented manifest minimum Rust 1.95 separately from stable CI and release Rust 1.99.0. |
| Release instructions | Matched local Linux prerequisites to the existing release job, including EGL/libdbus/fontconfig. The manual candidate → annotated-tag publication architecture is unchanged. |
| OSS files | Reviewed MIT LICENSE and existing public issue forms; added CONTRIBUTING, SECURITY and a PR template. Clarified original icon licensing versus bundled application fonts. |
| Compatibility | Added intended 1.x settings/history, export, OS and external-CLI policy. Internal recovery formats can evolve; downgrade and backup limits remain explicit. |

Current controls and guards were checked in `src/ui/library.rs`,
`repository_sections.rs`, `shortcuts.rs`, `validation.rs`, `worktree_settings.rs`,
and the existing history/storage/worktree guides. Build guidance follows
`Cargo.toml`, `.github/workflows/ci.yml`, `.github/workflows/release.yml`,
`packaging/build.py` and the existing disposable examples. Historical platform
records retain their original dates, revisions and unperformed checks.

After maintainer approval on 2026-10-15, GitHub private vulnerability reporting
was enabled. A fresh repository API read confirmed `enabled: true`. SECURITY.md
now links the private advisory form instead of the temporary public contact-request
fallback. No vulnerability report or advisory was created.

## Validation

Environment: Linux (Debian forky/sid), Rust 1.96.1, Python 3.14.8, Git 2.53.0.
Locked dependencies were fetched after the recent merged dependency updates.

- `cargo fmt --check`: passed.
- `cargo clippy --locked --all-targets --all-features -- -D warnings`: passed.
- `cargo test --locked --all-features`: **602 passed, 0 failed, 10 ignored**.
  Seven nested subprocess summaries are excluded from that count. The first
  sandboxed attempt failed filesystem/worktree fixtures; the complete suite
  passed with the OS access those fixtures require. No test or guard was changed.
- Packaging unit tests: **38 passed**, including final-artifact ABI fixtures.
- `cargo build --locked --release --bin codeconvoy`: passed.
- Private-D-Bus unavailable-service check: **1 passed**, explicitly enabled with
  `CODECONVOY_PRIVATE_BUS_TEST=1`; no visible notification test is implied.
- The documented `v06_desktop_validation`/test-agent build: passed. The interactive
  fixture was not launched in this documentation audit.
- Local Markdown link/image targets and heading anchors: passed after the final
  edits. This checks local destinations, not external website availability.
- Locked Cargo metadata: 458 resolved packages, all declaring license expressions.
  This includes all target graphs, not 458 libraries shipped in every artifact.

No native macOS/Windows build or interactive desktop test was run in this
documentation change. No minimum-Rust-1.95 or Python-3.11 run was performed.
No authenticated provider invocation or fresh package candidate was requested or
produced. Markdown-only changes skip automatic CI under the existing path filter.

## Remaining readiness decisions

- **Third-party notices:** package scripts verify/distribute the project MIT
  license, but no complete third-party notice inventory is maintained. In
  particular, `epaint_default_fonts` 0.36.2 embeds fonts with separate license
  texts; the graph also includes Unicode terms and MPL-2.0 `option-ext`. Review
  actual redistributed dependencies/fonts/AppImage runtime and supply required
  notices before 1.0 publication. Metadata inspection is not a compliance audit.
  Packaging changes to carry that inventory are outside this documentation PR.
- **Runtime evidence:** native Ubuntu 24.04/Debian 13/Fedora desktop checks,
  Windows interaction, Intel macOS execution, notifications, clipboard and
  dialogs retain the gaps in the [platform matrix](platform-validation.md).
  Container/ABI evidence is not promoted to full desktop compatibility.

This handoff does not declare 1.0 release approval. No tag, publication, merge,
release candidate dispatch or readiness PR 5 is part of this work.
