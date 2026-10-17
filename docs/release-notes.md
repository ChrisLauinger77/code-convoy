# CodeConvoy 1.1.0 — Cursor CLI and desktop refinements

## Changes since 1.0.0

### Experimental Cursor CLI support

- Add Cursor CLI as a fifth backend, using the locally installed `agent` or
  legacy `cursor-agent` launcher and its existing sign-in. CodeConvoy does not
  install Cursor, manage credentials or perform sign-in.
- Add Cursor-specific model, permissions, workspace trust and optional time-limit
  settings, CLI discovery, sign-in guidance, Activity summaries and Raw output.
  History, comparison, reuse, text attachments and repository validation retain
  the original launch settings and existing repository protections.
- Supervise Cursor process trees through cancellation, timeout and shutdown;
  retain isolated results and fail conservatively on incomplete or inconsistent
  output. Repository rules remain the CLI's responsibility.
- Cursor remains **experimental**. Linux CLI `2026.10.16-e3cb207` argument syntax
  was verified through the authentication gate. Authenticated execution, actual
  permissions, trust, rule loading and full protocol compatibility remain
  unverified on every platform. See the [Cursor guide](cursor.md) and
  [validation record](cursor-validation.md).

### Desktop and packaging

- Reduce the macOS menu bar glyph by 20% with transparent padding, retaining
  Retina resolution. Linux and Windows tray sizing is unchanged.
- Replace custom Windows installer helper DLLs with native Windows API handling.
  The new installer requires only the application's x64 Visual C++ runtime;
  its uninstaller needs no Visual C++ runtime. Upgrades invoking the released
  1.0.0 uninstaller still require that uninstaller's x86 runtime. Process checks,
  per-user installation and failure handling remain enforced.
- Replace the custom AppImage runtime build with the pinned official upstream
  runtime. Preserve final-byte verification, the glibc 2.39 ABI gate, source and
  notice delivery, and the existing six package formats. The existing upstream
  extraction-fallback cleanup defect still leaves `appimage_extracted_*` in
  `TMPDIR`; see the [runtime verification record](appimage-upstream-runtime-validation.md#accepted-upstream-extraction-defect).
- Set package publisher and Windows company metadata to `ChrisLauinger77`.

**Windows Microsoft-component terms:** before using or redistributing Windows
downloads, review the [component terms](https://github.com/ChrisLauinger77/code-convoy/blob/v1.1.0/third-party/MICROSOFT-DISTRIBUTABLE-CODE.txt),
also included in `THIRD-PARTY-NOTICES.txt`. Permission for the Microsoft portions
is conditional on agreement to those terms; if you do not agree, do not use or
redistribute those portions. CodeConvoy's independent MIT license is unchanged.
Managed and downstream distributors must arrange recipient agreement; notice
delivery or silent installation alone does not establish it.

macOS remains ad-hoc signed without notarization, and Windows remains unsigned.
Native package and desktop verification remains a separate release step; see
the [platform matrix](platform-validation.md) and [release procedure](releasing.md).
