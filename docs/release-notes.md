# CodeConvoy 1.0.0 — Stable release candidate preparation

## Changes since 0.10.0

These notes prepare the first stable release. **Publication is not approved:**
third-party licensing obligations and required native checks remain open in the
[readiness report](release-readiness.md). No release or RC tag is needed to build
a candidate through the existing first-stage workflow.

### Repository and process safety

- Preserve saved state when history-retention settings are malformed instead of
  accepting values that could silently alter retention behavior.
- Extend isolated-job repository protection to the actual checkout. Cancel
  Activity sends and binding replies without allowing an abandoned bind to
  authorize Git creation.
- Keep Unix process identity anchored while supervising descendants, including
  launchers that exit before children or children that close their output
  streams. Failed or uncertain termination retains repository protection.
  Linux and macOS regressions cover cancellation, shutdown and group identity.
- Expand upgrade fixtures and native process/path tests, including Windows
  locked files and junctions. Historical unfinished work remains interrupted;
  CodeConvoy does not resume old agent processes.

### Backend and desktop compatibility

- Add Claude Code **Auto** permission mode using `--permission-mode auto`.
  Existing saved modes retain their behavior. Unsupported versions and policy
  restrictions remain explicit; CodeConvoy never substitutes unrestricted
  permissions. See [the authenticated Linux evidence](claude-validation.md#auto-permission-mode--2026-10-14)
  and its account/model/organization limitations.
- Accept Claude 2.1.300's specific post-result UI notification while continuing
  to reject trailing execution/error records and unknown completion shapes.
- Find macOS validation toolchains from desktop launches and use a monochrome
  status-area tray glyph with theme-appropriate contrast.

### Packaging, documentation and licensing

- Build Linux candidates on Ubuntu 24.04. The intended minimum binary ABI is
  glibc **2.39**, for Ubuntu 24.04 LTS, Debian 13 Stable and newer compatible
  distributions. Every final DEB/RPM/AppImage ELF, including the AppImage
  runtime, is checked. This alone does not certify desktop compatibility.
- Preserve the six existing package formats and source-commit/checksum-based
  two-stage release process. Candidate builds do not publish or create tags.
- Include offline third-party notices, an inventory, Rust library notices and
  available source archives in each package. The exact MPL `option-ext` source
  is included without changing CodeConvoy's MIT license. Final-package checks
  reject missing notices and unreviewed native components. Publication remains
  blocked on the documented unresolved licensing obligations.
- Add first-use, contribution, security-reporting and compatibility guidance.
  GitHub private vulnerability reporting is enabled. Existing data/worktree
  ownership protections and explicit cleanup remain unchanged.

The source-build minimum remains Rust 1.95; candidate builds use pinned Rust
1.99.0. macOS remains ad-hoc signed without notarization, and Windows remains
unsigned. Ordinary Windows Storage cleanup still refuses deletion without the
required stable filesystem identity proof. Native desktop, notifications and
screen-reader limitations remain documented in the [platform matrix](platform-validation.md).
