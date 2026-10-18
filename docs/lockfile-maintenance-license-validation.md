# Lock-file maintenance license-evidence validation

Reviewed on 2026-10-18 for PR #86, based on
`93ce1bd778ac33843d112e06caec81aa360fa39f`.

## Changed inputs

The lock-file update replaces 27 crate versions. Each new registry archive was
fetched through locked Cargo metadata and independently checked against its
committed Cargo.lock checksum. The declared license expression is unchanged
for every updated crate. Source metadata, authors, upstream revisions, notice
provenance and checksums now describe those exact versions. Comparing the
updated source headers found no added copyright/SPDX/license markers, and
comparing source-file inventories found no added binary or font assets.

| Crate | Previous | Updated | Registry archive SHA-256 |
| --- | --- | --- | --- |
| arboard | 3.6.1 | 3.7.0 | `aeb9553e62312c8756a5979254fc6d7da8bbeb5986a1e17037fab932f7037917` |
| cc | 1.6.0 | 1.8.0 | `6651c9ed80effdc7db0ff72512157f901af5e3549e341e24b1dd4887d836d838` |
| combine | 4.6.8 | 4.6.9 | `4356eba6976435498d05a311ee9fc486da3bc81c200d3f0d7dd9703b41913f92` |
| deranged | 0.5.8 | 0.5.9 | `9e9de72ce2ad1f90dc62fa25f0f430ef85eb4b0d8fa0be4f30373bc40a21d28e` |
| either | 1.18.0 | 1.19.0 | `0e9c71c2167ca323c882b99918929403426e2373ea17242ff5653e0d5e1058be` |
| equivalent | 1.0.2 | 1.0.3 | `00d174d5400e5e8fd687ad1049e2f578285fa914201b1af7e8b112a4546bd826` |
| font-types | 0.12.6 | 0.12.7 | `835ae9d781464a79026cde387f9cbc650c9a4909d7cf7e658b127e3c35d2f35d` |
| libc | 0.2.190 | 0.2.191 | `65cd87c9a6d60536580b316ad985726104730a5151c99845f2490428ef91240a` |
| litrs | 1.0.0 | 1.0.1 | `4744e383959f0db86ede514b809b1c53251889093803c05267acc7d4e7030d70` |
| objc2 | 0.5.2 | 0.5.3 | `19d5490aaf8f1d7cf7688dfa9b0ce07900e168852c45cd2c03f534dfd27cfd0b` |
| range-alloc | 0.1.4 | 0.1.5 | `ca45419789ae5a7899559e9512e58ca889e41f04f1f2445e9f4b290ceccd1d08` |
| redox_syscall | 0.9.4 | 0.9.5 | `580c875845b41ecb1564ad1ae4703044cef774145e3d7a94dc2b3fa7c97868fd` |
| serde_spanned | 1.1.1 | 1.1.2 | `4e7523beb55eece201a2356bee0bbca0d1ab466c14c07703b2e0ee6d42cb0c2c` |
| smallvec | 1.16.2 | 1.16.3 | `5b3dc8af474f516a851ff4bd12db780f948b9250ad37211e4eec0bccea54e01b` |
| syn | 3.0.6 | 3.0.9 | `d78c8dee4c7bf0e14673097256fed6142ce9d3b85a408189d07482442145823b` |
| toml | 1.1.6+spec-1.1.0 | 1.1.8+spec-1.1.0 | `20489e00e4d8741d6be680764cc12e270655e375a20d1011e844a9c3379e678d` |
| toml_datetime | 1.1.1+spec-1.1.0 | 1.1.2+spec-1.1.0 | `2b86d767906c6c42421dcba507eb9d203e779497710a47782a224bb871653053` |
| toml_edit | 0.25.15+spec-1.1.0 | 0.25.17+spec-1.1.0 | `e3641d5bbb5349a79e1020a242d251efbc546ad8048d133958323ce9c40a9c9c` |
| toml_parser | 1.1.3+spec-1.1.0 | 1.1.5+spec-1.1.0 | `baa693a8032d7e1cada7d0041e96126df243179ff061456783ac7f12bda4744c` |
| toml_writer | 1.1.2+spec-1.1.0 | 1.1.3+spec-1.1.0 | `06bdbd8cfc056b8d2e2e85f29b56a3bdbecb527cef81eb39e3e7b98af4652770` |
| unicode-ident | 1.0.26 | 1.0.27 | `a2c754d6c33795a1c324727428e5a7dedb5b06195f9890bdbcba760d3e246563` |
| uuid | 1.26.1 | 1.28.0 | `7cc1186384beb7dd8eedea376413fd654937285ea6c9cfbb928dc3043ea4b606` |
| zbus_names | 4.3.4 | 4.3.5 | `812a12be4ead7a59e18ea8f047d08add0066b73bc62fc1467066a2472ae088dd` |
| zerocopy | 0.8.59 | 0.8.63 | `e5fe1f8f1b06191a00962174c61aa5005e0bb391a6d80d07e24d115c01a92ed8` |
| zerocopy-derive | 0.8.59 | 0.8.63 | `863ad3ac83293fb4d740aedbfdc9240dd8d1a50c1099acd76ce80ce7c7230c7f` |
| zvariant | 5.15.0 | 5.15.1 | `80bf6d6cd6a4b601acb64cfb8284cdc643fcfa75ef26ae3c46ec8aa4e8a1fbb7` |
| zvariant_derive | 5.15.0 | 5.15.1 | `ef2672456ba1cb54b5eaec136e63135b3c7495fb938e53b28a62e633496d486d` |

The six updated TOML-family crates (`serde_spanned`, `toml`, `toml_datetime`,
`toml_edit`, `toml_parser`, `toml_writer`) replace the Apache appendix's copyright
placeholder with `Copyright Individual contributors`. Their exact new text is
retained. Other collected changed-crate notice texts match the previous source
collection. The new catalog also includes the available notices for the
excluded `combine`, `deranged` and `redox_syscall` crates, which the previous
catalog omitted. Five additional distinct texts are retained, bringing the
content-addressed map from 366 to 371 entries; all existing texts are preserved.

`objc2` 0.5.3 omits its license from the registry archive. Its `.cargo_vcs_info.json`
identifies revision `5237cbf081d04ef75a1f6a207b4018e7c1ae7438`; the exact upstream
[`LICENSE.txt`](https://raw.githubusercontent.com/madsmtm/objc2/5237cbf081d04ef75a1f6a207b4018e7c1ae7438/LICENSE.txt)
was fetched and compared with the previously retained 0.5.2 license. Those bytes
match, so the content-addressed text is reused with updated provenance. No
generic replacement notice is used.

## Release-target coverage

Before/after `cargo metadata --locked --format-version 1 --filter-platform`
exports cover Linux x86-64, Windows x86-64, macOS arm64 and macOS x86-64. Traversal
from `resolve.root` excludes dev-only edges while retaining build and proc-macro
inputs. Application coverage follows normal edges and excludes proc-macro
targets. The baseline traversal exactly reproduces the existing catalog.

The catalog still covers all 457 registry packages. The conservative union
decreases from 383 to 382 included packages, leaving 75 explicit exclusions:
Windows no longer reaches `windows-sys` 0.59.0. Existing Windows 0.52/0.59 to
0.61.2 edge changes are reflected in the fresh target traversal. No new crate
name is introduced. Unchanged crate records retain their reviewed notices and
decisions. Source bundles, Rust/compiler pins, native component evidence,
accepted risks and publication checks are preserved.

## Regression checks

`python3 packaging/licenses.py check` and all 98 packaging tests passed locally.
With verified Rust 1.99.0 on Linux, `cargo fmt --check`, strict all-target/all-feature
Clippy, `cargo test --locked --all-features` (700 passed, 11 ignored) and
`cargo build --locked --release` all passed. Native Windows/macOS checks and the fresh six-format release candidate remain separate
validation; these metadata exports do not claim native package execution.
