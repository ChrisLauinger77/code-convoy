# CodeConvoy 1.3.0 — Open in Git Client and OpenCode 2

## Changes since 1.2.0

### Open in Git Client

- Add a global **Settings › Git client…** preference with presets for
  GitKraken, SourceGit, AngKorGit, Sublime Merge and Fork, plus a Custom
  executable whose arguments contain `{repo}`. Presets use only launch methods
  verified from each client's documentation, release notes or source; a
  platform without one shows the preset as unavailable (GitKraken on macOS,
  Fork on Linux). See [External Git clients](git-clients.md).
- Add **⬈ Open in Git Client** to registered repository rows and to the
  selected repository in current and historical convoy results. The directory
  comes only from recorded context: Direct jobs open the saved repository path
  and isolated jobs open their own retained worktree, never the source
  repository. A missing, moved or damaged target is reported instead of
  falling back to another directory.
- The client starts without a shell, with separate arguments, null stdio and
  no `GIT_*` variables. Target verification reads Git's metadata files on a
  worker thread; opening runs no Git, takes no repository lease and changes no
  repository. It is disabled while Apply, Discard or cleanup may change that
  worktree.
- Quit CodeConvoy moves to the bottom of the Settings menu.

### OpenCode 2 support

- Add an OpenCode version setting (default 1). OpenCode 1 keeps
  `run --format json --dir <path>`. OpenCode 2 runs
  `run --standalone --format json` in the exact repository or isolated
  worktree and sends a variant as `provider/model#variant`. Check CLI and
  launch preflight verify the selected major version and name the other one on
  a mismatch.
- Read OpenCode's help from both output streams: OpenCode 1.19.3 prints
  `run --help` to stderr, which previously made Check CLI reject it.
- Accept OpenCode 2's real event stream, which omits the final `step_finish`.
  An OpenCode 2 job succeeds only with exit status 0, a final step that answers
  on its own, no error events and valid bounded JSON. OpenCode 1 keeps its
  final-stop rule. Activity now shows OpenCode 2's relative tool paths.
- On macOS and Linux, OpenCode 2 detaches its standalone server into its own
  process group. Stop, time limits and quitting now send SIGTERM first with a
  10-second grace, and the repository is released only after every recorded
  detached process group has exited. A remaining group or incomplete record
  keeps the repository reserved. Windows continues to use its Job Object.
- Real OpenCode 1.19.3 and 2.0.32 binaries passed an unauthenticated smoke test
  on macOS arm64. Signed-in provider tasks remain unverified; see the
  [OpenCode validation record](opencode-validation.md#opencode-2-compatibility).

### Manual update check

- Add **Check for updates** to the About dialog. Each click sends one HTTPS
  request to GitHub's latest-release API and reports whether a newer release
  exists, with a link to its release page. There is no startup, background or
  scheduled check, and nothing is downloaded or installed. Homebrew and Scoop
  installations are shown the matching update command. See
  [update check](desktop-workflow.md#update-check).

### Dependencies and release validation

- Update eframe/egui to 0.37.0 and zbus to 6. Add `ureq` with `rustls` on
  `ring` and the operating system's trust roots for the update check, avoiding
  OpenSSL and `webpki-roots`.
- Refresh the third-party notice inventory for 56 added and 24 removed crate
  versions and their release-target coverage. Packages now also retain `ring`'s
  BoringSSL, fiat and ISC source-header notices and AccessKit's Chromium
  license. See the [1.3.0 license refresh](release-1.3.0-license-validation.md).
- The recorded Microsoft/NSIS inputs, terms, runtime policy and Windows
  packaging configuration are unchanged, so the existing Windows
  licensing-risk decision applies without renewed acceptance.

**Windows Microsoft-component terms:** before using or redistributing Windows
downloads, review the [component terms](https://github.com/ChrisLauinger77/code-convoy/blob/v1.3.0/third-party/MICROSOFT-DISTRIBUTABLE-CODE.txt),
also included in `THIRD-PARTY-NOTICES.txt`. Permission for the Microsoft portions
is conditional on agreement to those terms; if you do not agree, do not use or
redistribute those portions. CodeConvoy's independent MIT license is unchanged.
Managed and downstream distributors must arrange recipient agreement; notice
delivery or silent installation alone does not establish it.

macOS remains ad-hoc signed without notarization, and Windows remains unsigned.
Native package and desktop verification remains a separate release step; see
the [platform matrix](platform-validation.md) and [release procedure](releasing.md).
