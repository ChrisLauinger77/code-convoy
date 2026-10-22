# 1.3.1 release license-evidence refresh

Reviewed on 2026-10-22 during CodeConvoy 1.3.1 release preparation, based on
`fa96a0f8f0aea07710d0b57064e7cd8b00dc0009`.

## Why a refresh was needed

Since 1.3.0, routine dependency updates (#99, #104, #105, #106, #107) changed
Cargo.lock. Ordinary CI accepted their notice-only declarations without catalog
edits, so the release snapshot no longer matched Cargo.lock: 10 locked crate
versions were missing and 8 catalog versions were stale. `licenses.py prepare`
and `release-ready` refused the snapshot. The Cargo.toml dependency
declarations are unchanged, so the dependency-input fingerprint still matches.

## Changed inputs

Registry archives were fetched with `cargo fetch --locked`, which verifies each
archive against its committed Cargo.lock checksum. Every declared license
expression passes the existing notice-only policy. Unchanged crate records keep
their reviewed notices and decisions.

Updated versions:

| Crate | Previous | Updated | Release targets |
| --- | --- | --- | --- |
| cc | 1.8.0 | 1.9.0 | Linux, Windows, macOS (build only) |
| crc32fast | 1.5.2 | 1.5.3 | Linux, Windows, macOS |
| find-msvc-tools | 0.1.14 | 0.1.15 | Linux, Windows, macOS (build only) |
| ksni | 0.3.6 | 0.3.7 | Linux |
| memmap2 | 0.9.11 | 0.9.12 | Linux |
| rustls-webpki | 0.103.15 | 0.103.16 | Linux, Windows, macOS |
| tokio | 1.53.3 | 1.54.1 | Linux, Windows, macOS |
| ureq | 3.4.2 | 3.5.0 | Linux, Windows, macOS |

Added crates, both new dependencies of ksni 0.3.7:

| Crate | Version | License | Release targets |
| --- | --- | --- | --- |
| arc-swap | 1.9.3 | MIT OR Apache-2.0 | Linux |
| crossbeam-queue | 0.3.14 | MIT OR Apache-2.0 | Linux |

## Notices

Archive notices were collected with `packaging/collect_licenses.py`. Every
updated crate keeps its license expression, and its archive texts are
byte-identical to the previously retained texts. ksni 0.3.7's archive includes
`UNLICENSE`, identical to the upstream text retained for 0.3.6.

- **arc-swap 1.9.3** retains its archive `LICENSE-APACHE` (an existing text)
  and `LICENSE-MIT` with `Copyright (c) 2017 arc-swap developers`, the one new
  text.
- **crossbeam-queue 0.3.14** retains its archive `LICENSE-APACHE` and
  `LICENSE-MIT`, identical to those of the other catalogued crossbeam crates.
- **rustversion 1.0.23** was already catalogued as excluded without retained
  notices. arc-swap uses this procedural macro, so it is now a Linux build-only
  input. Its archive `LICENSE-APACHE` and `LICENSE-MIT`, both existing texts,
  are now retained.

One distinct text is added, bringing the content-addressed map from 401 to
402 entries. All existing texts are preserved. None of the added crates
embeds assets with `include_bytes!` or `include_str!`.

## Release-target coverage

`cargo metadata --locked --all-features --format-version 1 --filter-platform`
exports cover Linux x86-64, Windows x86-64, macOS arm64 and macOS x86-64. They
use the same traversal as the 1.3.0 refresh. Every record that was unchanged
since 1.3.0 reproduced its catalogued coverage exactly, except `rustversion`.

The catalog now covers all 491 registry packages. The conservative union rises
from 413 to 416 included packages, leaving 75 explicit exclusions.

Windows Microsoft/NSIS inputs, terms, runtime policy and distribution
configuration are unchanged, and the `windows-crt` scope binding still matches.
Source bundles, Rust/compiler pins, native component evidence and accepted
risks are unchanged.

## Regression checks

On macOS with Homebrew Rust 1.99.1 and Python 3.15, these passed:

- `python3 packaging/licenses.py check`, `check-dependencies`, `prepare` and
  `release-ready`
- the packaging unit tests
- `cargo fmt --check`, strict all-target/all-feature Clippy,
  `cargo test --all-features` and `cargo build --release`

Native Linux/Windows checks and the fresh six-format release candidate remain
separate validation; these metadata exports do not claim native package
execution.
