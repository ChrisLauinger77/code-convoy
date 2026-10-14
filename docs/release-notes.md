# CodeConvoy 0.10.0 — Quality & Polish

## Changes since 0.9.0

This release refines existing workflows, keyboard operation, responsiveness and
release packaging. It adds no agent backend or scheduling feature. State format 1,
repository safeguards, retained worktrees and explicit validation remain intact.

### Claude Code compatibility

- Claude's permission selector now includes **Auto**, passed as
  `--permission-mode auto`. Existing permissions and the `dontAsk` default are
  unchanged. Activity reports Claude's actual mode; Manual fallback is also
  recorded in the final job summary for Auto jobs. Auto availability diagnostics
  follow each job's saved mode and do not appear on legacy-mode jobs.
  Unsupported invocations retain their CLI
  error, with no automatic retry or unrestricted-permission substitution. Auto
  remains subject to CLI, account/model and organization support; see the
  [authenticated Linux evidence and limitations](claude-validation.md#auto-permission-mode--2026-10-14).
- Accept the specific UI-render notification emitted after a successful result
  by Claude 2.1.300. Trailing execution records, errors and unknown notification
  shapes still fail completion.

### Interface and keyboard use

- Settings and library dialogs fit the minimum 780×560 window, including long
  names and diagnostics. Toolbars wrap and destructive confirmations keep safe
  initial focus.
- Empty Diff and output states distinguish unchanged, unknown and unavailable
  results. Review errors remain actionable; agent outcome stays separate from Git
  changes and human result resolution.
- Modal focus stays within the active dialog and returns to its opener. Escape
  respects popup priority; held or modified activation keys cannot accidentally
  confirm destructive actions.
- Populated fields retain accessible names and focus is more visible. Tests cover
  three themes and 100%, 150% and 200% scaling. Orca, VoiceOver and NVDA operation
  has not been certified; native screen-reader testing remains necessary.

### Responsiveness and platforms

- Background messages wake the UI directly; idle repaint polling is removed and
  saves have an explicit deadline. Repository busy-state checks are shared within
  each frame without changing authoritative execution locks.
- Evicted session logs release their allocated buffers. On the recorded Linux
  fixture, full-window layout fell from 4.808 to 0.957 ms and retained log-buffer
  capacity from about 128 MiB to 32 MiB. These are local measurements, not hardware
  guarantees; startup and idle memory were effectively unchanged.
- Wayland uses the existing native Adwaita frame. The performance fixture now
  initializes macOS Quit handling consistently with the regular application.
- New native regressions cover literal arguments and Unicode paths, process-tree
  cleanup on cancellation/future drop, Windows locked files and junctions, and
  missing Linux tray/notification services on a private D-Bus session.

### Packaging and upgrades

- Linux candidates now build on Ubuntu 24.04, with glibc 2.39 as the intended
  minimum supported binary ABI for Ubuntu 24.04 LTS, Debian 13 Stable and newer
  compatible glibc distributions. Final DEB, RPM and AppImage ELF requirements,
  including the AppImage runtime, are measured and rejected above 2.39. DEB
  dependencies still use the executable's measured minimum. Older published
  packages are unchanged; native distribution compatibility remains subject to
  the exact-candidate evidence in the platform matrix. Packaged startup tests
  await the window manager's completed startup callback to avoid launching
  during its initialization.
- Windows installed/portable packages and the macOS bundle include the license.
  Packaging verifies archive contents, bundle metadata and icons. Artifact names
  and package-manager executable/app paths remain unchanged.
- Publication rejects incomplete, duplicate, malformed or incorrect checksum
  manifests before creating a draft; it never repairs downloaded checksums.
- Legacy 0.1 and representative 0.9 saves are tested through load/save/reload.
  Repository groups, templates, backend options, validation, notifications, tray,
  history retention and retained ownership metadata survive. Unfinished work is
  interrupted on restart and never resumed automatically.

macOS remains ad-hoc signed without notarization; Windows remains unsigned.
Ordinary Storage cleanup on Windows still refuses removal without stable
filesystem identity proof. Desktop activation, tray hosts and notification
presentation remain subject to platform policies.

See the [quality-release validation record](quality-release-validation.md),
[performance measurements](performance-validation.md),
[accessibility limits](accessibility-validation.md) and
[native platform checklist](platform-validation.md). These notes prepare 0.10.0;
publication requires a separately approved, tested release candidate.
