# 1.3.0 release license-evidence refresh

Reviewed on 2026-10-20 during CodeConvoy 1.3.0 release preparation, based on
`8f5a681a0b6354c892d276a5516b7ec47a9dea2a`.

## Why a refresh was needed

Since 1.2.0, the About update check (#89), eframe/egui 0.37.0 (#92) and zbus 6
(#93) changed Cargo.lock. Ordinary CI accepted their notice-only declarations
without catalog edits. #92 refreshed only `epaint_default_fonts`. The release
snapshot therefore no longer matched Cargo.lock: 56 locked crate versions were
missing, 24 catalog versions were stale, and the dependency-input fingerprint
differed. `licenses.py prepare` and `release-ready` refused the snapshot, so a
candidate could not be packaged.

## Changed inputs

Each new registry archive was fetched through locked Cargo metadata and
independently checked against its committed Cargo.lock checksum (57 archives,
including `semver`; all matched). Every declared license expression passes the
existing notice-only policy. Unchanged crate records keep their reviewed notices
and decisions; only their target coverage was recomputed.

Updated versions:

| Crate | Previous | Updated | Release targets |
| --- | --- | --- | --- |
| accesskit | 0.24.1 | 0.25.1 | Linux, Windows, macOS |
| accesskit_atspi_common | 0.18.1 | 0.21.0 | Linux |
| accesskit_consumer | 0.35.0, 0.36.0, 0.38.0 | 0.39.1 | Linux, Windows, macOS |
| accesskit_macos | 0.26.3 | 0.27.1 | macOS |
| accesskit_unix | 0.21.1 | 0.24.0 | Linux |
| accesskit_windows | 0.32.1 | 0.35.1 | Windows |
| accesskit_winit | 0.32.2 | 0.34.1 | Linux, Windows, macOS |
| ecolor, egui, egui-wgpu, egui-winit, egui_glow, emath, epaint, eframe | 0.36.2 | 0.37.0 | Linux, Windows, macOS |
| glifo | 0.2.0 | 0.3.0 | Linux, Windows, macOS |
| phf, phf_generator, phf_macros, phf_shared | 0.13.1 | 0.14.0 | Linux |
| vello_common, vello_cpu | 0.1.0 | 0.2.0 | Linux, Windows, macOS |

Added crates:

| Crate | Version | License | Release targets |
| --- | --- | --- | --- |
| ureq | 3.4.2 | MIT OR Apache-2.0 | Linux, Windows, macOS |
| ureq-proto | 0.6.5 | MIT OR Apache-2.0 | Linux, Windows, macOS |
| http | 1.5.0 | MIT OR Apache-2.0 | Linux, Windows, macOS |
| httparse | 1.10.1 | MIT OR Apache-2.0 | Linux, Windows, macOS |
| utf8-zero | 0.8.1 | MIT OR Apache-2.0 | Linux, Windows, macOS |
| rustls | 0.23.46 | Apache-2.0 OR ISC OR MIT | Linux, Windows, macOS |
| rustls-native-certs | 0.8.5 | Apache-2.0 OR ISC OR MIT | Linux, Windows, macOS |
| rustls-pki-types | 1.15.1 | MIT OR Apache-2.0 | Linux, Windows, macOS |
| rustls-webpki | 0.103.15 | ISC | Linux, Windows, macOS |
| ring | 0.17.14 | Apache-2.0 AND ISC | Linux, Windows, macOS |
| untrusted | 0.9.0 | ISC | Linux, Windows, macOS |
| subtle | 2.6.1 | BSD-3-Clause | Linux, Windows, macOS |
| zeroize | 1.9.1 | Apache-2.0 OR MIT | Linux, Windows, macOS |
| getrandom | 0.2.17 | MIT OR Apache-2.0 | Linux, Windows, macOS |
| unicode-properties | 0.1.4 | MIT/Apache-2.0 | Linux, Windows, macOS |
| openssl-probe | 0.2.1 | MIT OR Apache-2.0 | Linux |
| security-framework | 3.7.0 | MIT OR Apache-2.0 | macOS |
| security-framework-sys | 2.17.0 | MIT OR Apache-2.0 | macOS |
| core-foundation | 0.10.1 | MIT OR Apache-2.0 | macOS |
| schannel | 0.1.30 | MIT | Windows |
| zbus, zbus_macros, zbus_utils | 6.0.0 | MIT | Linux |
| zruntime | 0.2.0 | MIT | Linux |
| nom | 8.0.0 | MIT | Linux |
| wl-clipboard-rs | 0.9.4 | MIT OR Apache-2.0 | Linux |
| tree_magic_mini | 3.2.2 | MIT | Linux |
| petgraph | 0.8.3 | MIT OR Apache-2.0 | Linux |
| fixedbitset | 0.5.7 | MIT OR Apache-2.0 | Linux |
| hashbrown | 0.15.5 | MIT OR Apache-2.0 | Linux |
| foldhash | 0.1.5 | Zlib | Linux |
| os_pipe | 1.2.3 | MIT | Linux |
| accesskit_ios | 0.2.1 | MIT OR Apache-2.0 | Excluded |
| send_wrapper | 0.6.0 | MIT/Apache-2.0 | Excluded |

zbus 6 is added alongside the retained zbus 5 consumers. The stale entries
removed are the previous versions in the first table.

## Notices

Archive notices were collected with `packaging/collect_licenses.py`. Updated
crates keep the same license expressions, and their collected texts match the
previous versions, except that AccessKit now also retains `LICENSE.chromium`.

- **ring 0.17.14** retains `LICENSE`, `LICENSE-BoringSSL`,
  `LICENSE-other-bits`, `src/polyfill/once_cell/LICENSE-APACHE` and
  `-MIT`, and `third_party/fiat/LICENSE`. Its bundled C and assembly files carry
  Apache-2.0 headers (OpenSSL Project, BoringSSL, Intel, CloudFlare, ARM, Oracle
  and Google copyrights) or ISC headers. Seven ISC files name Intel (2014) or
  Google (2014, 2015, 2019) rather than Brian Smith, whose ISC text is in
  `LICENSE-other-bits`. ISC requires their copyright and permission notice in
  all copies, so their exact leading comment blocks are retained with
  `:: <path>#header` provenance. No NOTICE file, OpenSSL/SSLeay license or
  other copyleft term was found.
- **AccessKit 0.25.1 family** omits license files from its archives. The exact
  `LICENSE-MIT` and `LICENSE-APACHE` at `.cargo_vcs_info.json` revision
  `ce8164ba92995cfa86005b6259115e08c8244253` are byte-identical to the
  previously retained texts. Files in `accesskit`, `accesskit_atspi_common`,
  `accesskit_consumer`, `accesskit_macos` and `accesskit_windows` derive from
  Chromium and refer to `LICENSE.chromium`, a BSD-3-Clause text that requires
  reproducing its notice in binary distributions. The same references existed
  in the previously catalogued versions without that file. The exact
  [`LICENSE.chromium`](https://raw.githubusercontent.com/AccessKit/accesskit/ce8164ba92995cfa86005b6259115e08c8244253/LICENSE.chromium)
  is now retained for those five crates. `accesskit_ios` is excluded from every
  release target and, like other excluded crates without archive notices, has
  no retained notice.
- **egui 0.37.0 family** also omits license files. eframe records revision
  `3c6eb8261c3293ad9c7ea1ed58487e038860ebef`, and its upstream texts were
  fetched there. `ecolor`, `egui`, `egui-wgpu`, `egui-winit`, `egui_glow`,
  `emath` and `epaint` (and the already refreshed `epaint_default_fonts`)
  record `7c0b40402271e8d40cda69f87cf3c2617c2e4382`, which GitHub does not
  serve ("No commit found"). Their texts were therefore taken from the
  `0.37.0` release tag commit `841a328ba2dd8f6996beb7189ebb7fc00e668a2a`,
  four commits after eframe's revision, which change no license file. Upstream
  history shows `LICENSE-MIT` last changed in 2021 and `LICENSE-APACHE` in
  2020; all fetched texts are byte-identical to the retained 0.36.2 texts. The
  catalog origin names the tag commit, not the unavailable recorded revision.
- `semver` 1.0.28 is now a direct dependency on all targets. Its archive
  `LICENSE-APACHE` and `LICENSE-MIT` are retained.
- `send_wrapper` is excluded but its available archive notices are retained,
  as for other excluded crates whose archives include them.

Twenty-nine distinct texts are added, bringing the content-addressed map from
372 to 401 entries. All existing texts are preserved.

Embedded assets compiled into new or updated crates are unchanged in kind:
eframe's `data/icon.png` and vello_common's `assets/probe.rgba` were present in
the reviewed versions; glifo's and vello_cpu's font `include_bytes!` uses are
test-only; rustls-pki-types and rustls-webpki embed DER algorithm identifiers
under their crate licenses. petgraph and zruntime image files are documentation
assets and are not compiled.

## Release-target coverage

`cargo metadata --locked --all-features --format-version 1 --filter-platform`
exports cover Linux x86-64, Windows x86-64, macOS arm64 and macOS x86-64.
Traversal from `resolve.root` excludes dev-only edges while retaining build and
proc-macro inputs. Application coverage follows normal edges and excludes
proc-macro targets. Applied to the `v1.2.0` sources, the same traversal exactly
reproduces all 457 records of the `v1.2.0` catalog.

The catalog now covers all 489 registry packages. The conservative union rises
from 382 to 413 included packages, leaving 76 explicit exclusions. `ring`'s
build script brings `cc`, `find-msvc-tools`, `jobserver` and `shlex` to every
release target as build-only inputs; they were Linux-only before. `semver`
moves from excluded to every target. No other unchanged record's coverage
changed. The dependency-input fingerprint is refreshed for the 1.3.0 manifest.

Windows Microsoft/NSIS inputs, terms, runtime policy and distribution
configuration are unchanged, and the `windows-crt` scope binding still matches.
Source bundles, Rust/compiler pins, native component evidence and accepted
risks are unchanged.

## Regression checks

On macOS with Homebrew Rust 1.99.1: `python3 packaging/licenses.py check`,
`check-dependencies`, `prepare` and `release-ready` passed; all 110 packaging
tests passed (1 skipped). `cargo fmt --check`, strict all-target/all-feature
Clippy, `cargo test --all-features` (809 passed, 12 ignored) and
`cargo build --release` passed. Native Linux/Windows checks and the fresh
six-format release candidate remain separate validation; these metadata exports
do not claim native package execution.
