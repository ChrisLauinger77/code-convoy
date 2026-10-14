# 0.10.0 PR 5 — Packaging, release quality and stabilization

## Scope and findings

The package version and lockfile now prepare 0.10.0. Application/About and native
resources continue to derive their versions from Cargo. No dependency version,
state schema, backend command, scheduler or repository-safety behavior changed.
The current release notes replace 0.9.0's notes; historical validation records
remain intact. No tag, public release or package-manager submission is created.

| Finding | Change |
| --- | --- |
| DEB still promised glibc 2.35 after the build runner changed | Derive its minimum from the built executable's required GLIBC versions, verify the actual control field, and report deployed AppImage requirements. |
| Windows ZIP and macOS bundle did not ship the license; NSIS license display did not install a copy | Include LICENSE.txt as an installed/bundle resource and ZIP member, with content checks and installer uninstall coverage. |
| A partial SHA256SUMS could pass sha256sum and then be silently regenerated | Verify exact six-asset coverage, unique entries, syntax and every digest without modifying the manifest, before any draft is created. |
| Upgrade coverage was spread across feature tests | Add a representative 0.9 fixture alongside 0.1 and compare every supplied field through load/save/reopen. |
| Current release docs still described push CI and an old workflow name | Match pull-request/manual CI and the Release workflow; update install/removal guidance and release checklist. |

Packaging retains the six public asset names and the separate SHA256SUMS. Cargo's
lockfile, release compiler, packaging tool and checksum-pinned AppImage tools
remain the repeatable build inputs. Runner SDKs, system libraries and archive
timestamps can vary; byte-identical rebuilding is not claimed.

## Local verification, 2026-10-12

Debian forky/sid, GNOME 50.5 Wayland, x86-64, rustc 1.96.1, Python 3.14 and
cargo-packager 0.11.8. The release workflow uses its separately pinned toolchain.
Only disposable repositories, state and install roots were used. No real coding
agent was run, and no installed CodeConvoy or user state was replaced.

| Check | Result |
| --- | --- |
| Formatting, strict Clippy, full Rust tests, release build | Passed; 569 tests, zero failures, nine optional tests ignored; excludes filtered subprocess reruns. |
| Packaging unit tests | 23 passed, covering candidate/publication gates, exact checksums, ZIP license/payload, resource inclusion, measured glibc and Cargo/notes version consistency. |
| Legacy upgrade | 0.1 and representative 0.9 saves survive load/save/reopen, with no input-file rewrite on load. |
| Native Linux package assembly | DEB, RPM and AppImage built successfully; identities, desktop entry, icon/license payload and ELF requirements checked. |
| AppImage environment probe | Host Git, disposable repository access, all four CLI-name fixtures and search-path preservation passed. |
| AppImage startup | Passed on native Wayland and X11 through XWayland, using software OpenGL, empty temporary state and no agent installations. |
| DEB installation/removal | Real dpkg transaction in an unprivileged disposable root, installed binary startup and removal passed; unrelated state remained. Dependencies were deliberately not resolved in that empty root. |
| RPM installation/removal | Real rpm transaction in a disposable user namespace with host filesystem read-only; payload/license installed and removed, unrelated state remained. Dependency resolution, plugins and scripts were disabled; the package has no maintainer scripts. |
| Version/tag consistency | `python3 packaging/release.py check-tag v0.10.0` passed; this command creates no tag. |
| macOS/Windows packages | Source/contract audit and portable package tests only on this host; native package assembly and installer/DMG checks remain candidate gates. |

The local DEB and AppImage payload require glibc **2.43**. This is a measurement
of these locally built artifacts, not the Ubuntu release runner's output or a
promise that every older desktop works. RPM's generated dependencies include its
ELF requirements. AppImage still uses host glibc and graphics/session services.
The report is written to `target/packaging/linux/glibc-requirements.txt` and the
candidate run summary; it is not an extra public release asset. As of PR 60 it
measures every final DEB, RPM and AppImage ELF, including the AppImage runtime,
and records per-artifact maxima and checksums. The 2.43 measurement above is
historical; see the [current 2.39 candidate evidence](platform-validation.md#linux-glibc-239-baseline).

The RPM transaction first failed without chroot capability. It passed with only
CAP_SYS_CHROOT added inside an unprivileged user namespace. This is package-file
transaction evidence on Debian, not native Fedora/DNF dependency compatibility.
The DEB test similarly proves payload install/remove behavior, not apt dependency
resolution. Neither probe is a substitute for the intended distribution checks.

Startup smoke checks prove liveness and fresh-state initialization, then terminate
their disposable process group. They do not certify interactive close, native
dialogs, screen readers, notifications or GPU rendering. The separate normal-quit
and retained-worktree probes from PR 4 remain recorded in
[platform-validation.md](platform-validation.md).

## Release preparation recheck, 2026-10-12

Rechecked the merged release source at `a8621dbb4f62` on Linux x86-64 with
Rust/Cargo 1.96.1 and Python 3.14.8. Cargo.toml, the codeconvoy lockfile entry,
README examples, releasing instructions and current-only release notes already
use 0.10.0. The notes were reviewed against changes since `v0.9.0`; dependency
versions and historical validation records were preserved. Neither a local nor
remote `v0.10.0` tag nor a GitHub release for that version existed at this check.

| Check | Result |
| --- | --- |
| `cargo fmt --check` | Passed |
| `cargo clippy --all-targets --all-features -- -D warnings` | Passed |
| `cargo test --all-features` | 569 passed, zero failures, nine optional tests ignored; excludes three filtered subprocess reruns. |
| `cargo build --release` | Passed |
| `python3 -m unittest discover -s packaging -p 'test_*.py'` | 23 passed |
| `python3 packaging/release.py check-tag v0.10.0` | Passed |

The first sandboxed Rust test run could not use its temporary worktree fixtures:
the sandbox exposed `/tmp/.git`, so the existing storage-location safety checks
rejected them. It also denied a Unix socket bind. The complete suite passed when
rerun outside that sandbox, without changing application code or tests.

[PR #55 CI](https://github.com/ChrisLauinger77/code-convoy/actions/runs/38286780101)
passed on Linux, macOS and Windows before merge. This preparation did not rebuild
native release packages or repeat interactive desktop checks. The exact-candidate
package and desktop gates below remain outstanding. Preparation ends with the
local release commit; it does not create or push a tag, dispatch a candidate
workflow or publish a release.

## Upgrade compatibility

The fixture preserves repository paths and missing group members, templates,
backend-specific options, draft/history attachment references, theme, concurrency,
validation commands and results, notifications, tray preferences, retention,
custom worktree location, completion observations and ownership metadata. Pending
Apply stays pending. Running agent/validation records become interrupted/cancelled;
no task resumes and no saved availability becomes a fresh ownership proof.

The fixture uses the unchanged v0.9 serialized types, checked against the release
tag. It does not invent a migration or rewrite actual checkout/ownership files.
Existing worktree recovery, storage, interrupted-operation and retention tests
cover the corresponding real filesystem safety boundaries. See the
[fixture provenance](../tests/fixtures/README.md).

## Package-manager audit

Inspected on 2026-10-12:

- [Homebrew cask](https://github.com/ChrisLauinger77/homebrew-cask/blob/main/Casks/code-convoy.rb): Universal DMG name, CodeConvoy.app path and SHA-256 remain compatible. Its quarantine handling is external to this PR.
- [Scoop manifest](https://github.com/ChrisLauinger77/scoop-bucket/blob/main/bucket/code-convoy.json): Windows ZIP name and CodeConvoy.exe shortcut target remain compatible; the additional license does not change either.
- [WinGet publisher directory](https://github.com/microsoft/winget-pkgs/tree/master/manifests/c/ChrisLauinger77): no CodeConvoy entry was present there. No WinGet availability is claimed. Any future submission must validate the per-user NSIS installer, version and SHA-256 externally.

Existing downstream publication/update behavior is unchanged. No package-manager
automation or new CI pipeline was added.

## Five-PR handoff and final release gates

| Phase | Delivered evidence |
| --- | --- |
| PR 1 — UI polish | Compact minimum-window layouts, consistent confirmations, truthful empty/error states; [record](ui-polish-validation.md). |
| PR 2 — Accessibility | Modal focus/restoration, keyboard activation guards, accessible field names and theme/scale tests; screen-reader certification remains open; [record](accessibility-validation.md). |
| PR 3 — Performance | Recorded full-window layout 4.808 → 0.957 ms; log capacity about 128 → 32 MiB. Empty idle CPU 1.417% → 0.083%, large-history idle 1.667% → below sample resolution; [method and limits](performance-validation.md). |
| PR 4 — Platforms | Native OS regressions and Linux Wayland/X11 smoke checks; [record and manual matrix](platform-validation.md). |
| PR 5 — Packaging | Measured ABI declarations, license/payload checks, complete checksum verification, legacy upgrade tests and current release documentation. |

After PR 5 is integrated, build a fresh candidate from the merged commit using
the existing non-publishing Release workflow. Only that run's exact artifacts can
be approved for a release tag. Before publishing 0.10.0 or declaring 1.0 readiness:

1. Require all three native CI and package jobs, complete six-asset aggregation
   and unchanged downloaded checksums. Recheck compiler/runner-derived ABI results.
2. Install, launch, upgrade and remove the exact DEB/RPM on intended distributions
   with dependency resolution; test AppImage on the oldest intended host.
3. Verify Windows installer/portable metadata, license, icon, Start Menu, uninstall
   and user-state preservation on Windows. Run the existing Windows smoke script
   only in a disposable user profile.
4. Verify both macOS slices, final ad-hoc signature, bundle versions/icon/license,
   mounted DMG and Applications link; test native Intel and the oldest supported
   macOS. Check actual downloaded-file quarantine/first launch. Signing remains
   ad-hoc; no notarization infrastructure is added.
5. Complete the outstanding [native desktop checklist](platform-validation.md),
   including real clipboard/dialogs, notification/tray interaction, shutdown and
   assistive technologies. Preserve existing Windows Storage cleanup restrictions.
6. Back up representative real state plus ownership records and retained custom
   worktrees before an exact-artifact upgrade rehearsal. Verify unknown/missing
   resources remain explicit and no work automatically resumes.

These are remaining release-validation gates, not claims of newly discovered
runtime regressions. Authenticated OpenCode/Claude E2E, Intel runtime and native
screen-reader evidence remain absent from the recorded release-wide validation.
No tag or publication should imply those checks were completed.
