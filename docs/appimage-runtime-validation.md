# AppImage runtime provenance and relinking — 2026-10-16

This is a focused follow-up to the two AppImage publication blockers. It does
not repeat the Rust/Windows license audit or change CodeConvoy's MIT license.
The changes are based on `17bebd91dc1230dfb9637fe81d6b89e3092f0196`.
Validation was completed locally before preparing the pull request. No release
tag or release publication was created.

## Decision and obligations

The original checksum-pinned upstream 20251108 binary could not be retained
with demonstrably sufficient evidence. Existing sources/notices identify the
upstream versions and libfuse mount patch, but not the exact Alpine patches,
startup objects or a usable corresponding rebuild. Re-running its mutable
Docker recipe would identify a new binary, not recover the original's inputs.
No smaller sufficient alternative was established. Its earlier evidence is
preserved as `native_provenance.appimage.previous_runtime` and in the historical
[blocker record](license-blocker-evidence.md).

The distinctions used for this decision are:

| Category | Requirement or finding |
| --- | --- |
| License obligations | Preserve applicable notices; provide corresponding libfuse changes and usable source/object materials for modifying and relinking the combined launcher; permit modification and debugging reverse engineering. Modified library files carry dated notices. |
| Missing provenance | The original binary's exact Alpine inputs were unknown. That is not automatically a license violation, but prevented a demonstrated source/binary correspondence here. |
| Internal reproducibility requirement | Pin the build environment and inputs, reject unknown output, retain link evidence and compare two fresh builds byte for byte. These are engineering acceptance requirements, not a claim that LGPL itself demands identical rebuilds. |
| Optional improvements | Rebuilding the compiler bootstrap chain, independent-host attestations and external legal review are not represented as completed. They are distinct from identifying the compiler package used. |

The applicable combination is the standalone launcher plus static libfuse,
separate from the CodeConvoy executable in SquashFS. The source-based path in
[LGPL-2.1 section 6(a)](https://github.com/libfuse/libfuse/blob/fuse-3.15.0/LGPL2.txt)
is used: exact library/launcher sources, patches, scripts and other required
materials accompany the binary. System compiler tools are downloadable through
an exact checksummed manifest. No written offer or shared-library alternative
is claimed. This is a documented compliance interpretation, not authoritative
legal approval. Reproducibility and a successful demonstration support the
engineering evidence; neither independently establishes legal compliance.

## Replacement and exact inputs

The replacement keeps AppImage Type 2 and upstream runtime revision
`dd6cebedcbddde9c82f89b011e8e1d40b6e43868`. Its launcher C source is unchanged;
the build-time version label is `dd6cebed-codeconvoy1`. All runtime input pins
are in [runtime-inputs.json](../third-party/runtime/runtime-inputs.json).

| Component | Input and changes |
| --- | --- |
| type2-runtime | 20251108, revision above; original source archive SHA-256 `f5fec23be76e50e2445ed2d018bac49b367490fc483c62c4637e99ec705d27ba` |
| libfuse | 3.15.0; upstream `mount.c.diff`, then CodeConvoy's dated modification notice |
| squashfuse | 0.5.2, source-built; gzip/zstd enabled, xz/lzo/lz4 explicitly disabled, matching existing image compression support |
| musl | 1.2.5, source-built without downstream patches; supplies libc and rcrt1/crti/crtn |
| zstd | 1.5.6, source-built; BSD alternative and embedded source notices retained |
| zlib | 1.3.1, source-built |
| mimalloc | 2.1.7, existing source-only repack; four prebuilt Windows executables remain excluded |
| Linux UAPI headers | Exact Alpine `linux-headers-6.6-r1.apk`; inline byte-order code/types and mmap flags are additional identified inputs. Exact headers, package metadata, Alpine recipe and GPLv2/Linux-syscall-note texts are supplied in `linux-uapi-6.6-r1.tar.gz`. No kernel binary is bundled. |
| Build image | Alpine minirootfs 3.21.3 x86-64, SHA-256 `1a694899e406ce55d32334c47ac0b2efb6c06d7e878102d1840892ad44cd5239`, reconstructed with 59 exact APK inputs |
| Compiler/linker | Alpine GCC 14.2.0-r4, binutils 2.43.1-r3; package and executable hashes retained |
| Other build tools | GNU Make 4.4.1-r2, Meson 1.6.1-r0, CMake 3.31.1-r0, Python 3.12.15-r0, plus the complete transitive package closure in the manifest |

The kernel header exception expressly addresses inclusion in userspace programs;
see [Linux v6.6's exception](https://raw.githubusercontent.com/torvalds/linux/v6.6/LICENSES/exceptions/Linux-syscall-note).
All per-header source notices remain intact. This was a focused check of the
replacement runtime's actual source graph, not an assumption that the initial
list of seven projects was exhaustive.

The applied upstream libfuse patch has SHA-256
`1c7fd9e26717545a476b226b083a9f9d05676c180edbd71a04bbd8a73599dc44`.
The added notice patch has SHA-256
`af58e8224e51e855cea68b86d36c0e579f21514bd40740cf4188f8dca0326cfa`.
The optional recipient demonstration patch has SHA-256
`c8ffbf679fcf59f75d6b9c821299395aa8791339249c86b766ccd07ee8adb156`.
It is never applied to the stock runtime.

The exact GCC startup inputs are `crtbeginS.o`
(`7f13ed58a696945c2a1f4de29450485c59664886606b55b5eb7701d32cd6bad1`)
and `crtendS.o`
(`801b38e1001ac23947848c28a74bcf739516c253173b5641644fedcb459233b4`).
They come from the pinned GCC APK, whose source recipe/patches are retained at
Alpine revision `1a03a2a9c9e77f1a07d48b6f6805e72c8c63c03f`. None of those
patches changes `libgcc/crtstuff.c`; that source, GPLv3 and the Runtime Library
Exception 3.1 are supplied. Its full GCC source URL and checksum are also
recorded. The exception interpretation rests on C compilation with GCC, without
plugins, LTO or proprietary intermediate-code processing. It does not relicense
CodeConvoy or certify every upstream contributor's title.

The [stock link audit](../third-party/evidence/appimage-runtime-build.json)
records every linked object/archive, selected member, tool hash and musl CRT
hash. `libgcc.a`, `libgcc_eh.a` and `libssp_nonshared.a` are searched but supply
no selected members; newly selected code from them fails the build for review.
The resulting ELF has neither an interpreter nor a `DT_NEEDED` shared library.

## Reproducibility and recipient demonstration

The [standalone recipe](../third-party/runtime/build-runtime.py) uses a fresh
root filesystem for every build. Only `fetch` accesses the network. Building
installs already verified APK files with `--no-network`, clears the environment
and runs in a network-disabled bubblewrap namespace. Mutable tags and package
resolution are absent from the build path. Host bubblewrap and Python operate
the sandbox; compiler/build-tool execution happens inside the pinned image.

`SOURCE_DATE_EPOCH=1762560000`, UTC/C locale, fixed internal paths, prefix maps,
deterministic archives and fixed generic x86-64 flags normalize the output.
The existing GCC `sprintf` format-overflow warning in upstream runtime.c is
retained as a warning; the function was not changed. No other warning policy
is relaxed.

Two independent clean final builds matched in all four files: stripped runtime,
unstripped runtime, link map and JSON audit. One used the extracted distributed
source ZIP. They used separate root/build directories and no compiled-output
cache, on the same physical host/kernel; independent hardware was not tested.
The Ubuntu 24.04 packaging build also produced the same runtime bytes.

Stock runtime: **526,976 bytes**, SHA-256
`d5510b4077eb0e05083e576085326e76f3ffffb55ef81fd34e2a75279d80f912`.
Modified runtime: SHA-256
`7d31b6ac49f87ca1acf74c710f89cdbe0c25ae864ac7a8d1833fd9c8a3526199`.
No unexplained binary differences remained between the two stock builds.

The recipient procedure was exercised from `THIRD-PARTY-SOURCES.zip`:

1. Extract and verify all source/recipe checksums.
2. Rebuild the stock runtime using only the kit and pinned system build inputs.
3. Apply `libfuse-demo.patch` to libfuse's retained initialization function.
4. Rebuild and relink; observe its marker when the modified runtime executes.
5. Extract the stock AppImage's AppDir and package that same payload with the
   modified runtime using the existing pinned appimagetool 1.9.1.
6. Exercise extraction, extraction startup, actual FUSE-mounted startup and
   normal window close; verify the mount disappears after exit.

The [modified link audit](../third-party/evidence/appimage-runtime-relink.json)
binds the patch and inputs to that different output. The marker was observed
in both launch modes. The libfuse source and original notices remain LGPL;
the demonstration includes its own dated change notice. The distributed
[recipient instructions](../third-party/runtime/runtime-README.txt) explain
substituting an arbitrary recipient patch; release packaging deliberately
rejects that modified binary as an official stock runtime.

## Packaging and verification

The two-stage candidate/publication workflow, filenames, candidate provenance
and checksum verification remain in place. Only Linux gains the build-only
bubblewrap prerequisite and second clean runtime build. DEB/RPM assembly and
Windows/macOS packaging logic are unchanged. All formats continue to receive
the common conservative notice/source bundle.

`appimage-tools.json` pins both the build manifest and expected runtime output.
Packaging verifies the source kit, builds the runtime, rejects unexpected
output, and checks every byte of the final embedded launcher except its
documented 16-byte `.digest_md5` field at offset `0x7d900`. Independent final
ABI extraction now verifies runtime identity **before executing** the image's
offset command. Source/patch/script changes, missing kit entries and missing or
stale stock/modified link evidence fail verification. Compilation never edits
publication-blocker state.

The local non-publishing Linux candidate used the existing Linux workflow
commands inside Ubuntu 24.04 with Rust 1.99.0 and cargo-packager 0.11.8. It is
not a dispatched/aggregated GitHub Actions six-platform release candidate.
Final bytes, checksums and detailed test evidence are recorded in the retained
verification archive linked below. All final payload ELFs, including the outer
runtime, were inspected: CodeConvoy requires at most **GLIBC 2.39**; the runtime
has no versioned GLIBC requirement. Embedded notice/inventory/source files
match the generated bundle exactly. The existing host Git/CLI/environment and
process-cancellation probe passed.

| Check | Result / limit |
| --- | --- |
| Native Debian Forky, existing GNOME XWayland desktop | Stock and modified images: window and isolated store created, extraction/FUSE tested separately, normal close status 0, FUSE mount observed and cleaned |
| Ubuntu 24.04 userspace + Xvfb | Exact candidate extraction startup and normal close passed; not a native Ubuntu desktop certification |
| Debian 13 userspace + Xvfb | Exact candidate extraction startup and normal close as an ordinary user passed; not a native Debian desktop certification |
| Native Ubuntu 24.04 / Debian 13 desktops | Unavailable on this host; remain unverified |
| Native Wayland rendering/session | Not exercised; AppRun and application code are unchanged. XWayland and host-environment preservation were exercised |
| Rust fmt / strict Clippy / tests / release build | Passed with Rust 1.99.0 in Ubuntu 24.04; 621 tests passed, 10 ignored. Also passed with the installed Rust 1.96.1 after moving temporary fixtures outside the sandbox-protected `/tmp/.git` boundary |
| Packaging tests | 91 passed; full runtime rebuilds are excluded from ordinary Rust/Python unit tests |
| Release gate | Still refuses publication for `nsis-plugins` and `windows-crt`; their original blocker content and the reviewed dispatch resolution are unchanged |

## Disposition and maintenance

`appimage-versions` is resolved **for the replacement only**: actual compiled
sources, patches, startup objects and complete tool inputs are identified.
`appimage-relink` is resolved under the stated section 6(a) interpretation:
the source/relink materials are distributed and the recipient modification was
rebuilt and run with the same AppDir. These are manual, evidence-backed catalog
changes. They do not authorize publication, resolve unrelated Windows terms,
or turn outstanding native platform checks into passes. Maintainer acceptance
and any desired qualified legal review remain separate from this technical
handoff.

For future updates, review all changed source/patch/package/header identities
and their notices, regenerate the input manifest, build twice in clean roots,
repeat the extracted-kit modified-library demonstration, inspect final package
bytes and rerun platform checks. Update the expected runtime digest/layout only
after review. Keep a copy of the pinned system input cache: an unavailable or
changed upstream package must fail rather than silently selecting a replacement.
The final runtime, source kit, link evidence and instructions are the maintained
unit; updating a compiler or static library requires reviewing that unit.

## Retained final evidence and changed files

[Verification archive](../third-party/evidence/appimage-runtime-20261016.zip),
SHA-256 `a847fb3086cab997c4840542a81207254c68f2913861f88c7561e9943a7992f1`,
contains all three build logs/maps/audits, final native and virtual-desktop
results, the native test harness, payload comparison and validation summaries.
It contains no application/runtime binaries or compiler packages. Final package
hashes are recorded after assembly, outside the inventory embedded in those
same packages, avoiding a circular self-hash requirement.

| Final artifact/material | SHA-256 |
| --- | --- |
| `CodeConvoy-1.0.0-x86_64.AppImage` | `61cd335dc7a6ef2628166b852acb5435664f894983413e1d7943db0c0534572d` |
| Recipient-modified AppImage | `abeff2d780fd726da60e485731e705f6af2eeee4ec64668ff70f7c9a99bcfbb5` |
| Embedded `THIRD-PARTY-SOURCES.zip` | `599b07742fd8657889225104a7e0d05f12a7e17b05e536b93915f3003acca997` |
| Pinned runtime input manifest | `1eacc73d986ed729c9b91df107b4ebce29ef6d2cb53388af052ecf68d2d1a707` |

The stock and modified final payloads match in every regular file and symlink
(16 entries). The embedded source ZIP is byte-identical to the one extracted
for the recipient builds. Independent final-byte verification passed again
with the final checking code. `git diff --check` passed. Local package outputs
are retained under `target/ubuntu-dist/`; the modified image and comparison
are under `target/final-relink/`.

The changed implementation/evidence is limited to:

- `third-party/runtime/`: standalone builder, shell recipe, input manifest,
  original MIT terms, recipient instructions, two patches and exact GCC Alpine
  recipe archive; `third-party/sources/linux-uapi-6.6-r1.tar.gz` supplies the
  additional identified header source.
- `packaging/appimage_runtime.py`, `appimage-tools.json`, `linux.py`,
  `linux_abi.py`, `licenses.py`: source-built runtime integration and final-byte,
  source-kit and provenance gates. `test_appimage_runtime.py`,
  `test_licenses.py`, `test_linux_abi.py`: focused regression coverage.
- `.github/workflows/release.yml`: Linux-only build prerequisite and clean
  repeatability check. Existing publication/candidate scripts are unchanged.
- `.gitattributes`: preserve required context whitespace in the checksummed
  relinking patches.
- `third-party/catalog.json`, `texts.json`, `NOTICE-INTRO.txt`,
  `SOURCE-README.txt`, stock/modified link audits and verification ZIP;
  `docs/releasing.md`, `third-party-licenses.md`, `license-blocker-evidence.md`
  and this record.

Application code, Cargo manifests/lockfile, `packaging/AppRun`, Windows/macOS
packaging code and the project's root MIT license are unchanged. The two
unrelated blocker objects and dispatch evidence compare exactly with the base
revision. Work stops here for maintainer review; no publication is authorized.


## Candidate 38936028334: Ubuntu sandbox failure and correction

The Linux job in [candidate 38936028334](https://github.com/ChrisLauinger77/code-convoy/actions/runs/38936028334)
failed while invoking the offline APK installation at source commit
`56dfe5548a1a4594e9abe45b0ed83290b48507f4`. Its original `build.log` was not
retained, so its exact contents cannot be recovered. Before changing the recipe,
[diagnostic run 38940110607](https://github.com/ChrisLauinger77/code-convoy/actions/runs/38940110607)
checked out that exact commit, verified the same pinned inputs, and reproduced
exit 1 on the same Ubuntu 24.04 runner image, `20261012.334.1`, with Python
3.15.0 and bubblewrap 0.9.0. The uploaded `build.log` contains:

```text
bwrap: loopback: Failed RTM_NEWADDR: Operation not permitted
```

This is sandbox setup failure before APK executes. In the controlled
[comparison run 38940484471](https://github.com/ChrisLauinger77/code-convoy/actions/runs/38940484471),
the unchanged build failed again. The kernel audit showed `/usr/bin/bwrap`
transitioning from `unconfined` to `unprivileged_userns`, followed by AppArmor
`DENIED` entries for `setpcap` and `net_admin`. This establishes a reproducible
host-policy incompatibility, rather than a transient download or APK defect.
It is the supported explanation for the original failure; the original missing
log is not represented as recovered evidence.

The correction adds an AppArmor profile for `/usr/bin/bwrap` on the ephemeral
Linux candidate runner, allowing `userns` for that executable. The controlled
run confirmed that `kernel.apparmor_restrict_unprivileged_userns` remained `1`.
The build still runs as the ordinary runner user; the unchanged recipe retains
`--unshare-all`, including network isolation, and offline APK installation.
No host-wide AppArmor disablement, privileged build, source/toolchain update,
recipe change or runtime pin change is needed.
[Ubuntu's AppArmor documentation](https://documentation.ubuntu.com/security/security-features/privilege-restriction/apparmor/)
describes explicit per-application permission for unprivileged user namespaces;
[bubblewrap 0.9.0's loopback setup](https://github.com/containers/bubblewrap/blob/v0.9.0/network.c#L127-L170)
identifies the failing operation. This runner prerequisite does not change the
runtime's corresponding-source or recipient relinking materials.

After applying only the host profile, that same controlled job completed two
fresh builds. `runtime`, `runtime.full`, `runtime.map` and `audit.json` matched
byte for byte, including the independently rebuilt local disk-backed output.
The runtime SHA-256 remains
`d5510b4077eb0e05083e576085326e76f3ffffb55ef81fd34e2a75279d80f912`;
the audit matches the committed stock evidence. Both diagnostic runs retained
their logs; the first did so despite the build step and job failing.
The release workflow now uploads both primary and repeat-build `build.log`
paths with `if: always()`, in a separate `linux-runtime-diagnostics` artifact.
Missing logs before a build starts produce a warning, not a replacement failure.
This artifact is excluded from `packages-*` aggregation and release assets.

Local validation passed all 109 packaging tests, formatting, strict Clippy,
all non-ignored Rust tests and the release build (installed Rust 1.96.1).
The Ubuntu comparison used the release Python version and exact pinned runtime
toolchain. The GLIBC 2.39 gate, LGPL relinking evidence, source pins and
`windows-crt` publication guard are unchanged. No full replacement release
candidate was dispatched: after review, start a fresh non-publishing candidate
from the chosen final committed source, not a rerun of 38936028334. A successful
runtime diagnostic is not a six-asset candidate or publication approval.
