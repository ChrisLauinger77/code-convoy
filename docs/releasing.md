# Releasing CodeConvoy

## Maintainer procedure

`Cargo.toml` is the version authority. About, Windows resources, package
metadata, bundle versions and final filenames derive from it. For this release
it is `0.10.0`, with tag `v0.10.0`, without a prerelease suffix.

`docs/release-notes.md` contains only the current release and its changes since
the immediately previous version. Replace the previous release's content during
release preparation and remove older version sections. Historical release notes
remain in Git history; preserve separate historical validation records.

1. Review [release-notes.md](release-notes.md) and commit the release changes.
   Confirm applicable normal CI checks are green; Markdown-only changes skip CI.
2. Run **Release → Run workflow** on the reviewed branch. This
   builds and verifies packages and uploads the complete `release-assets`
   workflow artifact, including checksums. It never publishes a release.
3. Download those packages and complete the outstanding desktop checks in
   [quality-release-validation.md](quality-release-validation.md) and the native
   [platform checklist](platform-validation.md), particularly Linux and Windows.
4. When satisfied with the packages, copy the exact `git tag` and `git push`
   commands from the candidate run's summary or **Print the exact commands to
   publish this candidate** step. The annotated tag includes `Candidate-Run:`
   and points to the exact tested commit. Tag creation is a maintainer action.

Only manual runs build packages. They build Linux x86-64 on `ubuntu-24.04`,
Windows x86-64 on `windows-2025`, and both macOS architectures using Apple's SDK on `macos-26`. Rust 1.99.0,
Python 3.15 and the committed lockfile are used. Each platform runs formatting,
strict Clippy and all non-ignored tests; no authenticated agent tasks or provider
credentials are used.

A `v*` tag push runs only candidate verification, publication and package
repository update requests; it never rebuilds packages. The publisher requires
an annotated tag with exactly one `Candidate-Run: <run ID>` line and exact
Cargo version/tag equality. That run must be a successful, completed manual
run of `.github/workflows/release.yml` in this repository, with its source
commit equal to the tagged commit. It downloads that run's complete
`release-assets` artifact by its verified artifact ID. Missing, empty or expired
artifacts fail publication; there is no rebuild fallback. The final artifact
requests 30 days of retention, subject to repository limits.

Start a fresh manual run for each candidate instead of using **Re-run jobs**.
Reruns can replace artifacts under the same run ID, so candidates with
`run_attempt` greater than one are rejected. A manual run selected on a tag
can build a candidate but still cannot publish.

Only the final publishing job has `contents: write`; it also has `actions: read`
to inspect and download the selected candidate. All six packages must be
nonempty and have exactly the expected final filenames before checksums are
generated. The publisher requires exactly one original checksum for each of the
six assets and verifies every digest before publication. A partial, duplicate or
malformed manifest is rejected without regenerating it or creating a draft.
A single publisher creates a **draft**, uploads all seven assets,
resolves the draft's numeric release ID with `gh release view`, checks its
names/sizes and GitHub-provided digests through the release-ID API, then publishes
that same ID. The REST tag endpoint only returns published releases and cannot
verify a draft. A failed upload leaves a draft, never an intentionally partial
public release. Existing releases are not overwritten. If publication fails after
draft creation, inspect/delete that draft before rerunning the publisher; do
not move a published tag. A new release requires a new version/tag.

Normal CI runs checks/builds only. A manual release workflow run cannot publish,
even when its selected ref is a tag. The workflow never creates a Git tag.

## Homebrew and Scoop updates

CodeConvoy is also distributed through the maintainer's
[Homebrew tap](https://github.com/ChrisLauinger77/homebrew-cask/blob/main/Casks/code-convoy.rb)
and [Scoop bucket](https://github.com/ChrisLauinger77/scoop-bucket/blob/main/bucket/code-convoy.json).
The cask uses the Universal macOS DMG; the Scoop manifest uses the Windows x86-64
ZIP. Both reference published GitHub release assets and their SHA-256 values.
Keep their asset names compatible when changing packaging. User installation
and update commands are in the [README](../README.md#homebrew-on-macos).

After successful publication, the release workflow's **Request package repository
updates** job sends `repository_dispatch` events to both repositories:

- `update-cask` to `ChrisLauinger77/homebrew-cask`, with
  `client_payload.cask=code-convoy`.
- `update-scoop` to `ChrisLauinger77/scoop-bucket`.

Immediate dispatch requires the optional `PACKAGE_REPOSITORIES_TOKEN` repository
secret, authorized to send these events to both package repositories. Without
it, the job reports the scheduled fallback and succeeds. Each package repository
also checks daily: the Homebrew updater at 04:17 UTC and the Scoop updater at
00:00 UTC. Their workflows also support manual dispatch. These updates are
separate from publishing the CodeConvoy release and can finish later.

For a release published manually, or a failed dispatch, let the scheduled checks
run or manually run **Update casks** with `cask=code-convoy` and **Update Scoop
manifests** in the respective repositories. Confirm their update runs succeed
and the package definitions on `main` have the intended version, asset URL and
checksum before announcing package-manager availability. Check the
[Homebrew workflow](https://github.com/ChrisLauinger77/homebrew-cask/blob/main/.github/workflows/update-casks.yml)
and [Scoop workflow](https://github.com/ChrisLauinger77/scoop-bucket/blob/main/.github/workflows/update.yml)
when diagnosing delayed updates. They currently commit successful updates directly
to their package repositories' `main` branches.

## WinGet compatibility

No CodeConvoy manifest is maintained in this repository. The inspected WinGet
publisher directory contains other projects, not CodeConvoy. A future external
submission should use the existing x86-64 NSIS `setup.exe`, per-user scope,
`/S` silent installation, Cargo-derived DisplayVersion and the final asset's
SHA-256. Validate that exact installer with WinGet before claiming availability.
The portable ZIP remains compatible with the inspected Scoop manifest; adding
`LICENSE.txt` leaves its `CodeConvoy.exe` shortcut target unchanged. Homebrew's
Universal DMG name and `CodeConvoy.app` path are unchanged. This PR adds no
package-manager submission or automatic publication.

## CI and Renovate

[Normal CI](../.github/workflows/ci.yml) runs on pull requests targeting `main`
and manual dispatch, excluding changes consisting entirely of `**/*.md`
files. Mixed Markdown/code/configuration changes still run CI. Its matrix uses
`ubuntu-latest`, `macos-latest`, and `windows-latest`, stable Rust, and Python
3.15. Packaging unit tests, formatting, strict Clippy, all non-ignored Rust tests
and a locked release build run on each platform; Windows also runs the explicit
software graphics initialization test. Markdown-only edits have no new CI run.

[Release packaging](../.github/workflows/release.yml) has separate triggers:
`v*` tag pushes publish a previously tested candidate; manual dispatch builds
that candidate. It has no Markdown path filter and uses the explicit runners
and Rust version listed above for candidate builds. The source-build minimum
remains Rust 1.95 as declared in `Cargo.toml`; the release compiler pin does not
change that requirement.

[renovate.json](../renovate.json) inherits the shared
[CodeConvoy preset](https://github.com/ChrisLauinger77/ChrisLauinger77/blob/main/renovate-config/code-convoy.json)
and its [default preset](https://github.com/ChrisLauinger77/ChrisLauinger77/blob/main/renovate-config/default.json).
These disable automerge, pin GitHub Actions to full commits and group their
updates. Major updates, Rust 0.x minor/patch updates and
`dtolnay/rust-toolchain` action updates require Dependency Dashboard approval.
A local regex manager maintains `PACKAGER_VERSION` for cargo-packager in the
release workflow. Renovate updates also cover runner labels and the Rust/Python
versions selected by the workflows.

Review compiler and runner changes for their effect on package compatibility,
including Linux glibc requirements and the macOS deployment target. After such
changes, update these docs and run the non-publishing release workflow to
revalidate the packages before tagging. Update AppImage tool URLs and their
verified SHA-256 values together; mismatches fail before downloaded tools run.
Native runner SDKs, system package versions and package timestamps can vary;
this is a repeatable, locked build recipe, not a claim of byte-identical artifacts.
Keep the Linux build runner at Ubuntu 24.04 while glibc 2.39 is the intended
minimum supported ABI; a suggested runner upgrade is a compatibility decision,
not routine version maintenance. Validation, aggregation and publication runners
do not select the shipped binary's ABI. Never reuse compiled objects from a
newer sysroot in the baseline build.

## Packaging choice

| Tool | Role | Reason and limitations |
| --- | --- | --- |
| [cargo-packager 0.11.8](https://docs.rs/cargo-packager/0.11.8/cargo_packager/) | DEB, NSIS setup.exe, macOS app | Maintained Rust executable packager, independent of the UI framework. Shares identity, icons and metadata across three platforms. Does not support RPM. It is a build tool, not an application dependency. |
| [RPM's rpmbuild](https://rpm.org/docs/6.0.x/man/rpmbuild.8) | RPM | Native RPM spec and automatic ELF requirements; no conversion of Debian dependencies into RPM names. Uses the runner's distribution tool. |
| [linuxdeploy](https://github.com/linuxdeploy/linuxdeploy), [appimagetool](https://github.com/AppImage/appimagetool), [type2-runtime](https://github.com/AppImage/type2-runtime) | AppImage | Explicit deployment and image creation let us preserve the host environment. Each downloaded executable/runtime has a fixed release URL and SHA-256 in `appimage-tools.json`. |
| Apple's lipo, codesign, hdiutil | Universal binary, signing, DMG | Native Apple tools make both slices and signature verification explicit, including verification after mounting the finished image. |
| Python standard library | Metadata, ZIP, inventory, checksums, orchestration | Already available on CI, no pip dependencies or application framework. Requires Python 3.11+. |

The cargo-packager AppImage backend was not selected: its helper downloads and
default launcher provide less control over pins and inherited environment. Our
AppRun executes the binary directly. `linuxdeploy` places dependencies alongside
it using relative ELF library paths; AppRun does not alter `PATH` or
`LD_LIBRARY_PATH`. No filesystem sandbox or bundled Git/agent installations are
introduced. CI creates a disposable probe image using the same launcher/runtime
and CodeConvoy's real process/Git layer to verify access to a host repository,
host Git, disposable worktree creation/removal, all four CLI-name fixtures,
redirected-descendant cancellation and unchanged search paths. The probe is
never distributed and never contacts an agent service. It exercises the process
and Git layers, not an interactive task through the final GUI binary.

`cargo-packager` downloads its NSIS toolchain/helpers on Windows; its pinned
version fixes that tool selection. These are installer helpers, not a Tauri
runtime or WebView. No Node, Tauri, Electron or JavaScript application dependency
was added. Rust dependencies introduced by hardening are Windows-only
`winresource` (build-time icon/version resources) and an explicit dependency on
the already-locked `windows` crate (the typed `CREATE_NO_WINDOW` constant).
The existing process-wrap dependency supplies the creation-flags wrapper.

## Identity and payload

The crate and source executable remain `codeconvoy`. The visible application is
**CodeConvoy**. DEB/RPM package name is `code-convoy`; Linux desktop file is
`codeconvoy.desktop`, matching Wayland app ID `codeconvoy`, icon name `codeconvoy`,
category `Development`. Linux installs
under `/usr/bin` and `/usr/share`. There are no services or post-install agents.
The common bundle/installer identifier is
`io.github.chrislauinger77.code-convoy`. Windows staging names the executable
`CodeConvoy.exe`, without renaming the crate or backend executable settings.

The Windows setup is per-user (`currentUser`), requests no elevation, supplies
Start Menu integration and an uninstaller, and does not remove user state.
The ZIP contains `CodeConvoy.exe` and `LICENSE.txt`; the release build statically
links the MSVC CRT. Both use the same normal per-user state location. Release GUI builds
hide their console; debug builds keep it. A native startup error dialog preserves
visibility when state loading or application startup fails. Child processes use
`CreationFlags(CREATE_NO_WINDOW)` together with JobObject, preserving the wrapper's
suspended-spawn/assignment sequence and descendant cancellation.

macOS builds `aarch64-apple-darwin` and `x86_64-apple-darwin` independently with
deployment target 11.0. Packaging merges them, requires exactly `arm64 x86_64`,
assembles the app, sets both bundle version strings from Cargo, then signs the
**final** bundle with identity `-`. It verifies `--deep --strict
--all-architectures` and checks ad-hoc signature metadata. The app includes its license in
`Contents/Resources/LICENSE.txt` before signing; it is checked with the signed
app and again in the mounted image. Its icon and minimum-system metadata are
also checked. The DMG contains the app and `/Applications` link; the mounted copy
is verified again. Future
Developer ID signing, notarization and stapling belong at `sign_mac_app`, between
assembly and DMG creation. They are intentionally not implemented now.

Ad-hoc signing supplies integrity, not an Apple-verified publisher identity or
notarization. Gatekeeper acceptance is not guaranteed. Windows Authenticode
signing is also absent. See the README for first-launch guidance.

The project-owned icon's source and generated PNG/ICO/ICNS files are in `assets/`.
The application window and all package formats share this artwork. No icon
generator dependency is needed to build or package committed assets.

## Local packaging

Install the pinned packager (see `PACKAGER_VERSION` in the release workflow):

```sh
cargo install --locked --version 0.11.8 cargo-packager
python3 -m unittest discover -s packaging -p 'test_*.py'
```

`packaging/build.py` always uses `--locked`, remaps checkout/home/Cargo source
paths out of release diagnostics, and selects a static CRT on Windows. It does
not change global environment settings. Package assembly expects the repository's
normal `target/` directory; do not override `CARGO_TARGET_DIR` for these commands.

On Linux, install the README's source prerequisites plus packaging/test-only
packages: `binutils rpm desktop-file-utils file patchelf squashfs-tools xvfb xauth`.

```sh
python3 packaging/build.py
python3 packaging/build.py --probe
python3 packaging/release.py linux
xvfb-run -a python3 packaging/smoke.py dist/CodeConvoy-0.10.0-x86_64.AppImage
```

On Windows, from a development shell with Python, Rust MSVC and Windows SDK:

```powershell
python packaging/build.py
python packaging/release.py windows
```

`windows-smoke.ps1` is intended for a disposable CI user profile, not an existing
user installation. It checks metadata, silent per-user install, Start Menu,
portable identity and uninstall. The normal Rust tests exercise process-tree
cancellation on the native Windows runner. An interactive desktop check remains
separate; a hosted runner does not establish real desktop graphics behavior.

On macOS:

```sh
rustup target add aarch64-apple-darwin x86_64-apple-darwin
export MACOSX_DEPLOYMENT_TARGET=11.0
python3 packaging/build.py --target aarch64-apple-darwin
python3 packaging/build.py --target x86_64-apple-darwin
python3 packaging/release.py macos
```

Outputs go to ignored `dist/`, intermediates to `target/packaging/`. Assembly
recreates only its platform-specific intermediate directory. Do not put personal
files there. `python3 packaging/release.py aggregate` requires all six packages
and writes their final checksums. Do not run `publish.py` locally as a build test.

Linux runtime packages are separate from CI packaging tools: desktop graphics,
X11/Wayland libraries, fontconfig, libdbus, Git, URL-opening utilities and the
desktop's portal/backend. The Debian dependency list and RPM requirements use
their respective distribution names; RPM also derives ELF requirements.
The DEB is user-verified to install and run on Debian Forky; AppImage startup is
also user-verified there.
The Linux release build uses Ubuntu 24.04; **glibc 2.39 is the intended minimum
supported binary ABI** for Ubuntu 24.04 LTS, Debian 13 Stable and newer compatible
glibc distributions. Packaging reads the executable's required GLIBC symbol
versions with GNU `readelf` and sets the DEB's `libc6` minimum from that
measurement; it verifies the generated control field. RPM derives its own ELF
requirements.

After all three Linux packages are assembled, `linux_abi.py` independently
extracts the final DEB with `dpkg-deb`, the RPM with `rpm2cpio`/`cpio`, and the
AppImage with `unsquashfs` at the final runtime's reported offset. It examines
every regular ELF in those payloads and the outer AppImage runtime. Internal
symlink aliases are covered by their targets; broken/escaping links fail.
`readelf --version-info --wide` runs in the C locale. Only the Version needs
section contributes requirements: exported definitions do not raise the minimum.
Version tuples are compared numerically, including weak requirements. Unknown or
private GLIBC ABI names, incomplete need records, command failures and readelf
diagnostics fail validation. Dynamic loader tags must agree with the version
needs table, so missing section headers cannot hide requirements. Any requirement above 2.39 fails the candidate before
upload. The static runtime may legitimately have no versioned GLIBC requirements;
the application's executable may not silently lose its measurement.

`target/packaging/linux/glibc-requirements.txt` and the candidate summary record
each final artifact's SHA-256, every inspected ELF and its highest required GLIBC
version. Aggregation checksums and tag/candidate provenance verification are
unchanged. No new public format, Linux variant or rebuild-at-publication path is
introduced.

The Linux candidate job also installs the same packages in disposable Ubuntu
24.04, Debian 13 and Fedora 44 containers using normal APT/DNF dependency
resolution, starts the installed application and AppImage extraction fallback
on Xvfb, requests normal window close, runs the disposable process/Git probe,
and removes the package. The Ubuntu runner checks mounted AppImage startup when
`/dev/fuse` exists; a missing device is explicitly reported. These are userspace
and virtual-display checks, not native Wayland/XWayland or interactive desktop
certification. Verify graphics, decorations, clipboard, portals and shutdown on
the actual target desktops with the exact candidate before claiming complete
distribution compatibility. Record missing evidence in the
[platform matrix](platform-validation.md#linux-glibc-239-baseline).

The X11 backend loads `libxkbcommon-x11.so.0` with `dlopen`, so ELF dependency
scanning does not discover it. CI and DEB explicitly require
`libxkbcommon-x11-0`; RPM requires `libxkbcommon-x11`. AppImage uses this host
desktop library too. Keep it in the source prerequisites and release runner's
package list: compilation and unit tests can pass without it, but X11 startup
(including the Xvfb smoke check) cannot.
