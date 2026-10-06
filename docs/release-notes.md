# CodeConvoy 0.2.0

CodeConvoy 0.2.0 makes repeated maintenance across local Git repositories easier
with repository groups, task templates, file attachments and readable activity
views. It continues to use your installed Codex CLI, GitHub Copilot CLI, OpenCode
or Claude Code, with each backend's own settings and permissions.

## Changes since 0.1.0

- Save and edit named repository groups. Group selection updates the explicit
  repository list, deduplicates overlapping members and keeps unavailable
  memberships visible for repair.
- Save, load, edit and delete task-text templates. Loading a template preserves
  repository selection, attachments and backend settings, and never starts work.
- Attach Markdown, text, JSON, YAML, PNG, JPEG and WebP files using the native
  picker or attachment drop area. Each backend supplies text and images through
  its own supported CLI interface; unsupported input fails explicitly.
- Check attachments before launch and again when queued jobs start. History
  retains file references and metadata rather than file contents. Reusing a
  convoy keeps missing or changed files visible and blocks launch until they
  are explicitly removed or restored and re-added.
- Inspect backend events in **Activity** alongside **Diff**, **Raw output** and
  **Task & settings**. Activity and raw logs remain bounded and session-only.
- Check installed agent CLIs automatically in the background at startup and
  after executable changes. Missing agents do not prevent editing tasks or
  viewing repositories and history.
- Improve keyboard navigation, accessible controls, compact-window layouts,
  group-selection performance and the About dialog's project link.
- Document Linux installation commands and Homebrew/Scoop installation and
  updates. The release publisher now verifies drafts by numeric release ID and
  requests package-repository updates after publication when configured.

Existing state remains compatible. Convoys keep immutable task/settings/context
snapshots, independent results, fair bounded scheduling, repository conflict
protection, Git baseline review and process-tree cancellation. Agent CLIs and
authentication remain under your control. No provider credentials, agent CLIs,
updater, service or background agent are bundled.

## Downloads and platform notes

Downloads include Linux x86-64 DEB/RPM/AppImage, Windows x86-64 per-user setup and
portable ZIP, and one macOS Universal DMG with Apple Silicon and Intel slices.
`SHA256SUMS` covers the six packages. Linux packages are built on Ubuntu 26.04.
Earlier DEB and AppImage packages have user-verified installation/startup
coverage on Debian Forky; those checks have not been repeated for 0.2.0 packages.
The minimum glibc version and compatibility with older distributions still need
verification after the runner update. A working desktop graphics environment is
required; the native picker uses your desktop portal.

On macOS, drag CodeConvoy to Applications. The app is **ad-hoc signed, not
Developer ID signed or notarized**. After verifying the source and attempting a
first launch, you may need the CodeConvoy-specific **Open Anyway** option in
System Settings → Privacy & Security. Follow
[Apple's guidance](https://support.apple.com/en-us/102445); do not disable
Gatekeeper globally. Windows packages are not Authenticode signed and may display
an unknown-publisher warning.

Install Git and your chosen coding-agent CLI separately. Desktop applications
may inherit a different PATH from your shell. Use an absolute agent executable
path where necessary; Git must also be available to the application.

## Validation and remaining limits

The v0.2 workflows have deterministic coverage for all four backends, including
text/image delivery to two disposable repositories and independent results.
Native macOS fixture checks cover groups, templates, file selection, concurrent
convoys, history reuse, themes, keyboard navigation and compact/enlarged windows.
See the [Part 2.3 completion report](https://github.com/ChrisLauinger77/code-convoy/blob/v0.2.0/docs/v0.2-completion.md)
for detailed evidence.

Codex and Copilot have earlier user-verified authenticated E2E coverage on Linux
and macOS. Authenticated attachment/model acceptance remains unverified for all
four backends; OpenCode and Claude authenticated E2E remain unverified overall.
Native Linux/Windows file-picker and OS drag-and-drop checks, Intel macOS runtime
checks and fresh 0.2.0 package installation checks remain outstanding. Codex
session continuation remains deferred. Consult the repository's validation
documents for platform-specific evidence and limitations.

CodeConvoy works directly in registered repositories. Stopping work does not
undo partial edits. Review changes before committing them. Removing history
only removes CodeConvoy metadata, never repository contents.
