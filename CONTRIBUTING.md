# Contributing to CodeConvoy

CodeConvoy is a compact native desktop application for running one task across
local Git repositories with an existing coding-agent CLI. Read
[AGENTS.md](AGENTS.md) for repository rules and [architecture](docs/architecture.md)
for module boundaries. Propose substantial features in an issue before building
them; keep fixes focused and preserve repository/worktree safety.

## Issues and support

Use the [bug report or feature request forms](https://github.com/ChrisLauinger77/code-convoy/issues/new/choose).
For bugs, include the About version/revision, OS/architecture, desktop/session,
package type, agent CLI version and minimal reproduction. Label whether evidence
comes from a fixture, source build or exact release package. Sanitize diagnostics:
prompts, paths, repository names and CLI output can contain private information.
Use [SECURITY.md](SECURITY.md) for suspected vulnerabilities.

Support and review are best effort. There is no guaranteed response time, release
schedule or long-term support branch. Report current-version problems where
practical; do not destroy a failing state or retained result merely to upgrade.
Keep a backup first. The [compatibility policy](docs/compatibility.md) describes
the intended 1.x boundaries without promising support for every CLI or desktop.

## Build prerequisites

`Cargo.toml` declares Rust **1.95** as the minimum. Normal CI uses current stable
Rust; the release workflow pins **1.99.0**. There is no separate minimum-toolchain
CI job, so the manifest floor is not evidence that every dependency update has
been tested on 1.95. Use stable Rust with `rustfmt` and `clippy` for development,
and the release pin when reproducing candidate builds. Keep `Cargo.lock` intact.

Git must be on PATH. No agent installation, authentication or provider account is
needed to compile or run deterministic tests. An actual graphical desktop is
needed for interactive UI checks. Python **3.11 or newer** is needed for the
standard-library packaging scripts (`tomllib`); CI/release use **3.15**.

### Linux

On Debian/Ubuntu, install the same source prerequisites used by CI:

```sh
sudo apt-get update
sudo apt-get install -y build-essential pkg-config libx11-dev libxkbcommon-dev libxkbcommon-x11-0 libwayland-dev libgl1-mesa-dev dbus
```

To run the UI, also provide a working X11/Wayland desktop and OpenGL/EGL driver.
Linux file dialogs use the desktop portal with a FileChooser-capable backend and
runtime libdbus; `zenity` is the fallback. The tray is optional and needs a
StatusNotifier host. Source-build success does not validate desktop services.

The Linux release baseline is Ubuntu 24.04/glibc 2.39. Building on a newer host
can create a binary that the release ABI gate correctly rejects. See
[local packaging](docs/releasing.md#local-packaging) for additional package tools.

### macOS

Install Xcode Command Line Tools (`xcode-select --install`), Git, and native Rust
through rustup. Source builds use the host architecture. A Universal release
requires both Rust targets and Apple's SDK, as described in
[local packaging](docs/releasing.md#local-packaging). Packaged notification tests
need an app bundle and OS permission; a bare `cargo run` binary is insufficient.

### Windows

Install Visual Studio Build Tools with **Desktop development with C++**, its MSVC
toolchain and Windows SDK, Git, and Rust's `x86_64-pc-windows-msvc` toolchain. Use a
development terminal with those tools on PATH. Packaging checks also need Python.
The ordinary application uses Direct3D 12 with a WARP software fallback; see
[Windows graphics](README.md#windows-vms-and-graphics) for troubleshooting.

### Checkout and run

```sh
git clone https://github.com/ChrisLauinger77/code-convoy.git
cd code-convoy
cargo fetch --locked
cargo run --locked
```

Run `cargo fetch --locked` again after pulling a changed `Cargo.lock`. Do not run
`cargo update` just to build. The application uses the normal per-user state
directory; use the disposable fixtures below for UI development that must not
touch personal registrations/history. A release executable is produced by
`cargo build --locked --release --bin codeconvoy` under `target/release/`.

## Required checks

Run from the repository root before requesting review:

```sh
python3 -m unittest discover -s packaging -p 'test_*.py'
cargo fmt --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-features
cargo build --locked --release --bin codeconvoy
```

Use `python` instead of `python3` on Windows where appropriate. The `test-support`
feature builds local fixture processes, not a production backend. Tests use
temporary repositories and do not need provider credentials. Ignored tests have
explicit environment/desktop/authentication requirements; do not run all ignored
tests indiscriminately. Authenticated probes must be intentional and reported
separately from deterministic tests.

On Linux, CI also exercises unavailable desktop services on a private bus:

```sh
CODECONVOY_PRIVATE_BUS_TEST=1 dbus-run-session -- cargo test --locked --all-features --test linux_desktop -- --ignored --nocapture
```

On Windows, CI checks the software graphics adapter:

```powershell
cargo test --locked --bin codeconvoy software_adapter_initializes_egui_renderer -- --ignored --nocapture
```

[CI](.github/workflows/ci.yml) runs on Linux, macOS and Windows. Markdown-only
changes skip automatic CI; this does not turn unrun checks into passes. Record
local results and any missing platform evidence in the PR.

## Native UI verification

Build and open the existing disposable desktop fixture:

```sh
cargo build --locked --features test-support --bin codeconvoy-test-agent --example v06_desktop_validation
cargo run --locked --features test-support --example v06_desktop_validation
```

It uses a temporary Store/repositories, configures only the local test agent and
disables notifications. It does not prove provider or notification behavior.
Check 780 × 560, 1180 × 820 and 1600 × 1000 layouts in System/Dark/Light themes,
keyboard focus/modal cancellation, independent convoys and Stop/Quit behavior.
Follow the [native checklist](docs/platform-validation.md#native-smoke-checklist)
for Wayland/XWayland, clipboard, dialogs, tray and platform-specific checks.
Record OS/session, architecture, build revision, fixture/package and results.
Headless egui tests and container startup are not full desktop validation.

## Dependencies and licensing

The project and original icon artwork use [MIT](LICENSE); see
[asset provenance](assets/README.md). Contributions must be compatible with that
license. Keep dependencies conservative and explain additions or feature changes.
Do not assume dependencies or packaged tools inherit CodeConvoy's MIT license.

For dependency changes, inspect the resolved `Cargo.lock` graph with
`cargo metadata --locked --format-version 1` and review each new crate's license
expression, license files and bundled assets in its actual source. Include
target-specific/transitive dependencies, fonts, native libraries and packaging
tools. Preserve required notices when redistributing them. The AppImage runtime
and `linuxdeploy` pins live in `packaging/appimage-tools.json`; their upstream
licenses need review when pins or payloads change. The project has no automated
license-compliance gate; passing Rust or packaging tests is not a license audit.

The current graph includes `epaint_default_fonts` with OFL/Ubuntu font license
terms in addition to MIT/Apache, Unicode-licensed crates, and `option-ext` under
MPL-2.0. The font crate embeds its font bytes in the application. Packaging
currently installs the project `LICENSE`; that is not a complete third-party
notice inventory. Before 1.0 publication, review the actual redistributed
dependencies/fonts/runtime and carry their required notices into the packages.
This documentation change does not certify license compliance or alter payloads.

## Pull requests and releases

Use a focused branch and describe the concrete problem, resulting behavior,
validation and remaining limitations in the PR template. Add meaningful regression
tests for behavior changes. Documentation should match current controls; keep
dated validation records labelled as historical evidence instead of rewriting
their outcomes. Never claim platform or authenticated tests you did not perform.

Use Conventional Commit subjects such as `fix: preserve cancelled job state` or
`docs: clarify history export`. Avoid unrelated formatting, dependency updates or
architecture changes. Backend-specific work belongs in that backend; verify flags
against the installed CLI or authoritative documentation.

The [release procedure](docs/releasing.md) preserves two stages: a manual candidate
run builds/verifies artifacts without publishing; a maintainer's annotated tag
selects those exact bytes for verification and publication without rebuilding.
Ordinary PR validation needs no tag, package publication or release workflow
dispatch. Do not run publishing scripts as tests.
