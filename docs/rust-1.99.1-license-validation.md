# Rust 1.99.1 license-evidence validation

Reviewed on 2026-10-18 for PR #85, based on
`852878b901c4bb3d0f745a7bb43f6cc1c9f5b54d`.

The release compiler moves from Rust 1.99.0 to 1.99.1. The ordinary CI Windows
CRT diagnostic pin follows the release compiler. Existing installer, Microsoft
terms, runtime policy and maintainer decisions remain in force.

## Verified upstream inputs

The official [versioned release manifest](https://static.rust-lang.org/dist/channel-rust-1.99.1.toml)
identifies the release date as 2026-10-15. Its downloaded SHA-256 sidecar matches
`fb152be80975c194aa5fc684b2aec3a6e14e2b855efe2faa4e5f8ae6e590f463`. Both archives below match the manifest's `xz_hash`:

| Input | SHA-256 |
| --- | --- |
| [Rust compiler archive](https://static.rust-lang.org/dist/2026-10-15/rustc-1.99.1-x86_64-unknown-linux-gnu.tar.xz) | `2ff05ff9c08a90861e7f6e4065b40a8b87463b4576186fd613871c3e37f2091f` |
| [Rust source archive](https://static.rust-lang.org/dist/2026-10-15/rust-src-1.99.1.tar.xz) | `e485a88c45db546b73571df7bebfcb4454ba4fb552c492e25fc5b00eeb5bf22d` |

The 1.99.1 compiler's `COPYRIGHT`, `LICENSE-MIT`, `LICENSE-APACHE` and exact
`COPYRIGHT-library.html` all match the prior reviewed bytes. The retained HTML's
SHA-256 remains `5647be074c8edf7339fd863055923a8fc80bc5610a8d4661ec3b767b9d392c27`.
Every one of the catalog's 76 referenced source-archive notices was compared
with the matching path in verified 1.99.1 `rust-src`; none changed.

The source archive's separate `library/Cargo.lock` is also byte-identical:
`53d9ccff0c906de434b65af90d5c30063894b940fc839261f1fb06ef2e69e3d3`.
Its 30 registry entries, checksums, existing target exclusions and covered
notice texts remain unchanged. The generated report's 141 entries remain
unchanged too. The source ZIP now names this lockfile for Rust 1.99.1; the
catalog records the new verified origins and both archive digests. Existing
content-addressed notice texts are reused only after these comparisons.

## Regression checks

The tool-change regression now derives the Rust and cargo-packager versions
from the reviewed catalog. It still changes each workflow input to a different
value and requires the gate to reject it. Neither inventory gate is weakened.

`python3 packaging/licenses.py check` and all 98 packaging tests passed locally.
With verified Rust 1.99.1 on Linux, `cargo fmt --check`, strict all-target/all-feature
Clippy, `cargo test --locked --all-features` (700 passed, 11 ignored) and
`cargo build --locked --release` all passed.

Native Windows/macOS checks and a fresh six-format candidate remain separate
validation. The existing historical Windows evidence is retained as historical;
this source/notice comparison does not claim a new native linker capture or
final-package inspection.
