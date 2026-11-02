# 1.3.3 release license-evidence refresh

Reviewed on 2026-11-02 during CodeConvoy 1.3.3 release preparation, based on
`a0aeab8f295a6d4dc1fe2f2ac817d7909ab733ff`.

## Why a refresh was needed

Since 1.3.2, routine dependency updates (#115, #116, #118) changed Cargo.lock.
Ordinary CI accepted their notice-only declarations without catalog edits, so
the release snapshot no longer matched Cargo.lock: 33 catalog versions were
stale. `licenses.py prepare` and `release-ready` refused the snapshot. The
Cargo.toml dependency declarations are unchanged, so the dependency-input
fingerprint still matches.

## Changed inputs

Registry archives were fetched with `cargo fetch --locked`, which verifies each
archive against its committed Cargo.lock checksum. No crate was added or
removed. Every updated crate keeps its license expression, authors and
repository, and passes the existing notice-only policy. Unchanged crate records
keep their reviewed notices and decisions.

Updated versions:

| Crate | Previous | Updated | Release targets |
| --- | --- | --- | --- |
| accesskit | 0.25.1 | 0.25.2 | Linux, Windows, macOS |
| accesskit_atspi_common | 0.21.0 | 0.21.1 | Linux |
| accesskit_consumer | 0.39.1 | 0.39.2 | Linux, Windows, macOS |
| accesskit_ios | 0.2.1 | 0.2.2 | excluded |
| accesskit_macos | 0.27.1 | 0.27.2 | macOS |
| accesskit_unix | 0.24.0 | 0.24.1 | Linux |
| accesskit_windows | 0.35.1 | 0.35.2 | Windows |
| accesskit_winit | 0.34.1 | 0.34.2 | Linux, Windows, macOS |
| cc | 1.9.0 | 1.10.0 | Linux, Windows, macOS (build only) |
| clipboard-win | 5.4.1 | 5.4.3 | Windows |
| icu_collections | 2.3.0 | 2.4.0 | Linux, Windows, macOS |
| icu_locale_core | 2.3.0 | 2.4.0 | Linux, Windows, macOS |
| icu_normalizer | 2.3.0 | 2.4.0 | Linux, Windows, macOS |
| icu_normalizer_data | 2.3.0 | 2.4.0 | Linux, Windows, macOS |
| icu_properties | 2.3.0 | 2.4.0 | Linux, Windows, macOS |
| icu_properties_data | 2.3.0 | 2.4.0 | Linux, Windows, macOS |
| icu_provider | 2.3.1 | 2.4.0 | Linux, Windows, macOS |
| openssl-probe | 0.2.1 | 0.2.2 | Linux |
| ordered-float | 5.5.0 | 5.5.1 | Linux, Windows, macOS |
| rustix | 1.1.5 | 1.1.6 | Linux, macOS |
| smallvec | 1.16.3 | 1.16.4 | Linux, Windows, macOS |
| tinystr | 0.8.4 | 0.8.5 | Linux, Windows, macOS |
| unicode-width | 0.2.2 | 0.2.3 | Linux, Windows, macOS |
| uuid | 1.28.0 | 1.29.0 | Linux, Windows, macOS |
| webbrowser | 1.2.4 | 1.2.5 | Linux, Windows, macOS |
| wl-clipboard-rs | 0.9.4 | 0.9.5 | Linux |
| yoke | 0.8.3 | 0.8.4 | Linux, Windows, macOS |
| zbus | 6.0.0 | 6.2.0 | Linux |
| zbus_macros | 6.0.0 | 6.2.0 | Linux (build only) |
| zbus_utils | 6.0.0 | 6.1.0 | Linux |
| zerovec | 0.11.8 | 0.11.9 | Linux, Windows, macOS |
| zerovec-derive | 0.11.6 | 0.11.7 | Linux, Windows, macOS (build only) |
| zruntime | 0.2.0 | 0.2.1 | Linux |

## Notices

Archive notices were collected with `packaging/collect_licenses.py`. For every
updated crate that ships license files, the archive texts are byte-identical to
the previously retained texts.

The AccessKit crates and clipboard-win ship no license files in their registry
archives. Their texts were recovered again from the exact upstream revisions
recorded in `.cargo_vcs_info.json`: AccessKit
`f3026222559c7f5f46d8f25cb7404de68eb149f0` (`LICENSE-MIT`, `LICENSE-APACHE`,
`LICENSE.chromium`) and clipboard-win
`ed27008ae5d1f10cfbb266de039f0fb35fd05a56` (`LICENSE`). All are byte-identical
to the retained texts, and the origin URLs now name those revisions. The AccessKit
crates that contain Chromium-derived files are the same as before, so
`LICENSE.chromium` remains retained for the same records.

No distinct text is added or removed; the content-addressed map keeps 402
entries. None of the updated crates adds an `include_bytes!` or `include_str!`
asset.

## Release-target coverage

`cargo metadata --locked --all-features --format-version 1 --filter-platform`
exports cover Linux x86-64, Windows x86-64, macOS arm64 and macOS x86-64. They
use the same traversal as the 1.3.0 refresh. Every unchanged record reproduced
its catalogued coverage exactly, except `windows-sys` 0.59.0.

The zbus updates (#115, #118) re-resolve `dirs-sys` 0.5.0 to `windows-sys`
0.59.0 instead of 0.61.2, so `windows-sys` 0.59.0 moves from excluded to
Windows. Its record already retains its archive `license-apache-2.0` and
`license-mit`, both existing texts, and its license expression is unchanged.

The catalog covers all 491 registry packages. The conservative union rises
from 416 to 417 included packages, leaving 74 explicit exclusions.

Windows Microsoft/NSIS inputs, terms, runtime policy and distribution
configuration are unchanged, and the `windows-crt` scope binding still matches.
Source bundles, Rust/compiler pins, native component evidence and accepted
risks are unchanged.

## Regression checks

On Linux with Rust 1.97.1 and Python 3.14, these passed:

- `python3 packaging/licenses.py check`, `check-dependencies` and
  `release-ready`
- all 120 packaging unit tests
- `cargo fmt --check`, strict all-target/all-feature Clippy,
  `cargo test --all-features` and `cargo build --release`

`licenses.py prepare` passed its release snapshot check but stopped at its
compiler check: it requires the audited Rust 1.99.1 compiler, which the Release
workflow uses. Native
Windows/macOS checks and the fresh six-format release candidate remain separate
validation; these metadata exports do not claim native package execution.
