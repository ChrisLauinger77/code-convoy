CodeConvoy's first release is a native desktop application for applying one task
across local Git repositories using Codex CLI, GitHub Copilot CLI, OpenCode, or
Claude Code.

It includes independent concurrent convoys, bounded scheduling, repository
conflict prevention, Git baseline review, process-tree cancellation, local
history, result/diff inspection, native folder selection and keyboard navigation.
Agent CLIs and authentication remain under your control. No provider credentials,
agent CLIs, updater, service or background agent are bundled.

Downloads include Linux x86-64 DEB/RPM/AppImage, Windows x86-64 per-user setup and
portable ZIP, and one macOS Universal DMG with Apple Silicon and Intel slices.
`SHA256SUMS` covers the six packages. Linux packages are built on Ubuntu 26.04.
The DEB is user-verified to install and run on Debian Forky, and AppImage
startup is also user-verified there. The minimum glibc version and compatibility
with older distributions still need verification
after the runner update. A working desktop graphics environment is required;
the native picker uses your desktop portal.

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

Codex and Copilot have user-verified authenticated E2E coverage on Linux and
macOS. OpenCode and Claude Code have deterministic protocol/execution tests;
authenticated E2E execution remains unverified. Consult the repository's
validation documents for platform-specific evidence and limitations.

CodeConvoy works directly in registered repositories. Stopping work does not
undo partial edits. Review changes before committing them. Removing history
only removes CodeConvoy metadata, never repository contents.
