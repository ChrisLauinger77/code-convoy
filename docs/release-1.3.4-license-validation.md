# 1.3.4 release license-evidence refresh

Reviewed on 2026-11-09 during CodeConvoy 1.3.4 release preparation, based on
`4bda030c95f5ce1dcc6ae5809196e27c764224aa`.

## Changed inputs

Dependency updates since 1.3.3 (#122, #123, #125, #126 and #127) left the
release snapshot behind Cargo.lock. The rfd requirement also changed, requiring
an updated dependency-input fingerprint. This release preparation changes no
dependency version or requirement.

The refresh records 32 version upgrades, two added packages and one removed
package. Each changed registry archive was verified against its Cargo.lock
SHA-256 checksum. Unchanged records retain their reviewed notices and decisions.

| Crate | Previous | Updated | Release targets |
| --- | --- | --- | --- |
| arc-swap | 1.9.3 | 1.9.4 | Linux |
| async-executor | 1.14.0 | 1.14.1 | Linux |
| async-lock | 3.4.2 | 3.5.0 | Linux |
| cc | 1.10.0 | 1.11.1 | Linux, Windows, macOS (build only) |
| crc32fast | 1.5.3 | 1.5.4 | Linux, Windows, macOS |
| flate2 | 1.1.10 | 1.1.11 | Linux, Windows, macOS |
| js-sys | 0.3.106 | 0.3.107 | excluded |
| log | 0.4.34 | 0.4.35 | Linux, Windows, macOS |
| objc2-uniform-type-identifiers | added | 0.3.2 | macOS |
| rfd | 0.17.2 | 0.18.0 | Linux, Windows, macOS |
| serde_spanned | 1.1.2 | 1.1.3 | Windows (build only) |
| smallvec | 1.16.4 | 1.16.5 | Linux, Windows, macOS |
| toml | 1.1.8+spec-1.1.0 | 1.2.1+spec-1.1.0 | Windows (build only) |
| toml_datetime | 1.1.2+spec-1.1.0 | 1.1.3+spec-1.1.0 | Linux, Windows (build only) |
| toml_edit | 0.25.17+spec-1.1.0 | 0.25.19+spec-1.1.0 | Linux (build only) |
| toml_writer | 1.1.3+spec-1.1.0 | 1.2.0+spec-1.1.0 | Windows (build only) |
| unicode-properties | 0.1.4 | 0.1.5 | Linux, Windows, macOS |
| ureq | 3.5.0 | 3.5.1 | Linux, Windows, macOS |
| ureq-proto | 0.6.5 | 0.7.1 | Linux, Windows, macOS |
| utf8_iter | 1.0.4 | 1.1.0 | Linux, Windows, macOS |
| utf_types | added | 1.0.0 | Linux, Windows, macOS |
| uuid | 1.29.0 | 1.29.1 | Linux, Windows, macOS |
| wasm-bindgen | 0.2.129 | 0.2.130 | excluded |
| wasm-bindgen-futures | 0.4.79 | 0.4.80 | excluded |
| wasm-bindgen-macro | 0.2.129 | 0.2.130 | excluded |
| wasm-bindgen-macro-support | 0.2.129 | 0.2.130 | excluded |
| wasm-bindgen-shared | 0.2.129 | 0.2.130 | excluded |
| web-sys | 0.3.106 | 0.3.107 | excluded |
| webbrowser | 1.2.5 | 1.2.6 | Linux, Windows, macOS |
| winnow | 1.0.4 | 1.0.5 | Linux, Windows |
| zbus | 6.2.0 | 6.2.1 | Linux |
| zbus_macros | 6.2.0 | 6.2.1 | Linux (build only) |
| zcheapstr | 1.1.0 | 1.2.0 | Linux |
| zruntime | 0.2.1 | 0.2.2 | Linux |

`pollster` 0.4.0 is removed; the already reviewed 0.5.0 remains.
`objc2-uniform-type-identifiers` 0.2.2 remains excluded alongside the newly
included 0.3.2.

## Notices and obligations

Archive notices were collected with `packaging/collect_licenses.py`. Existing
license text for updated versions is byte-identical to retained text, with the
following additions and metadata changes:

- `unicode-properties` 0.1.5 changes its declaration from MIT/Apache-2.0 to
  **(MIT OR Apache-2.0) AND Unicode-3.0**. Its added `LICENSE-UNICODE` retains
  the Unicode 1991–2026 copyright and permission notice alongside the existing
  MIT, Apache and COPYRIGHT texts. The cumulative Unicode notice obligation is
  covered by the existing supported policy and aggregate notice delivery.
- New `utf_types` 1.0.0 declares Apache-2.0 OR MIT. Both archive license texts
  already exist in the map; its Mozilla Foundation COPYRIGHT text is added.
  Its source headers use those terms and identify public-domain test sections.
- New `objc2-uniform-type-identifiers` 0.3.2 declares Zlib OR Apache-2.0 OR MIT.
  Its archive omits license files. The repository's combined `LICENSE.md` was
  recovered from exact VCS revision `7b1abfd750a2cacaea71d6a56ecfb83cb7de560b`,
  matching the retained text already used by other framework crates from that
  revision. The new record retains that exact origin.
- `uuid` 1.29.1 updates its authors metadata; its notice texts are unchanged.

The content-addressed text map grows from 402 to 404 entries. Changed crate
sources were inspected for embedded include assets. The cc compiler-detection
C/C++ inputs and rfd wasm CSS are byte-identical to their previous versions;
other include references cover crate documentation, examples, tests, existing
license text or user-provided wasm-bindgen input. They add no release payload
requiring separate notices. New crate source headers and nested notices were
reviewed. Existing font, MPL, Rust/compiler and native component decisions are
unchanged.

## Release-target coverage

`cargo metadata --locked --all-features --format-version 1 --filter-platform`
exports cover Linux x86-64, Windows x86-64, macOS arm64 and macOS x86-64.
Traversal excludes dev edges, retains build/proc-macro inputs conservatively,
and records normal dependency edges separately without proc-macro subtrees.
All unchanged-package coverage reproduces the prior inventory except
`windows-sys` 0.59.0, which returns to excluded after dependency resolution
moves the Windows path to the already included 0.61.2. Its retained evidence is
preserved.

The catalog covers all 492 registry packages: 417 included and 75 excluded.
Windows Microsoft/NSIS inputs, terms, runtime policy and distribution
configuration are unchanged, and the existing `windows-crt` scope hash still
matches. No renewed risk acceptance is needed. Source bundles, toolchain pins
and native provenance evidence are unchanged.

## Validation

On Linux with Rust 1.99.1 and Python 3.12.14, these passed:

- License integrity, current dependency policy, `prepare` and `release-ready`.
- All 120 packaging unit tests and `release.py check-tag v1.3.4`.
- `cargo fmt --check` and strict all-target/all-feature Clippy.
- `cargo test --locked --all-features`: 831 top-level tests passed, 15 ignored.
  Process tests ran under `tini` with a temporary directory outside protected
  workspace paths. The first build exhausted disk before tests could run;
  removing obsolete generated Cargo artifacts allowed the complete rerun.
- The documented Linux private-bus integration test: one additional test passed.
- `cargo build --locked --release --bin codeconvoy`.

These checks do not establish legal compliance or native package execution.

Native Windows/macOS checks, the fresh six-format candidate and desktop
verification remain separate release steps. The existing publication approval
gate is preserved.
