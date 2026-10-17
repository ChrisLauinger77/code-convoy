# Official AppImage runtime migration

## Phase 1 — accepted 2026-10-17

Baseline: CodeConvoy commit `ba96394b97842671b78cc9b7b50fb800bd4dd36b`.
This record precedes removal of the custom builder. The maintainer explicitly
accepted the identical, pre-existing extraction cleanup defect as a separate
upstream issue. No LGPL relinking requirement or other acceptance gate is waived.
The [machine-readable record](../third-party/evidence/appimage-upstream-phase1.json)
records hashes, same-payload comparisons and observed launch results.
The [older custom-runtime record](appimage-runtime-validation.md) remains historical.

### Official identity and unchanged application

Use [upstream release 20251108](https://github.com/AppImage/type2-runtime/releases/tag/20251108),
revision `dd6cebedcbddde9c82f89b011e8e1d40b6e43868`, x86_64 runtime SHA-256
`2fca8b443c92510f1483a883f60061ad09b46b978b2631c807cd873a47ec260d`.
The upstream detached signature verifies against its supplied key
`570C77ACEA40C0F1B758902CBF96CCA56490F695`; this is not independent identity trust.
The source archive digest is
`f5fec23be76e50e2445ed2d018bac49b367490fc483c62c4637e99ec705d27ba`.

The published 1.0.0 AppImage SHA-256 is
`298d6a179ad8ef251b1bfdfc1715f2cffdf7c4c86e1d34ca382b3ab4928357a9`.
Its application ELF SHA-256 is
`c80e5565df8beb8f910342a38b25b8954038ae1c6b88af6aa0ec774de9bcce51`.
Its normalized custom runtime hash is
`d5510b4077eb0e05083e576085326e76f3ffffb55ef81fd34e2a75279d80f912`.

The official-runtime comparison image SHA-256 is
`13adbd83c29ce944886585be392f1c38de1e6defdb4a9091f9d5828748d00089`.
All extracted regular files, symlinks, modes and directories match the published
AppDir. This includes AppRun and the application executable. appimagetool 1.9.1
was invoked with explicit `--runtime-file` and VERSION unset. Only the runtime's
16-byte `.digest_md5` field at offset 932096 changes during assembly. Clearing
that field in the first 944632 bytes recovers the official checksum. The runtime
is x86_64 static PIE, Type 2 magic, no interpreter, DT_NEEDED or versioned GLIBC
requirements. The unchanged application requires at most GLIBC 2.39.

### LGPL recipient demonstration

The proposed minimal source ZIP was extracted into a new directory. Its supplied
runtime/libfuse/squashfuse archives, upstream scripts and Makefile were used for
a fresh build following the [recipient instructions](../third-party/runtime/runtime-README.txt).
Only library download locations, source annotations, the demonstration patch
and the build-time version label were changed. No launcher source was patched.
The libfuse initialization modification prints an observable marker.

An ordinary Alpine 3.21 userspace supplied the tools listed by upstream's
`scripts/docker/Dockerfile`. Existing unprivileged user namespaces provided the
local disposable environment; Docker was unavailable. Full subordinate UID/GID
mapping and an exposed `/dev/fuse` were needed for ordinary package/libfuse
installation. These are test-environment details, not a release build system.
No historical 59-APK closure, custom compiler or double-build requirement was
needed. Current Alpine zlib was 1.3.2; the official binary's 1.3.1 source is still
included. This proves replacement/relinking, not byte-identical reproduction.

The extracted-kit replacement runtime SHA-256 is
`27d1b921d53e0708af6b6f088cbb5538c4bd1c4e2e97eb817555a207ab9688a5`.
Reassembly and extraction verified the entire same AppDir. Both native XWayland
FUSE and extraction launches printed the modified-libfuse marker, initialized
the window/store and closed normally with status 0. FUSE unmounted and removed
its mountpoint. Extraction reproduced the accepted limitation.

The LGPL-2.1 section 6(a) approach supplies the library and independent launcher
source, applicable patches, notices and usable build/relink instructions.
The separate application is unchanged. Original notices/licenses remain in the
source archives; the recipient annotations document modification dates. The
GNU CRT source, GPLv3/Runtime Library Exception 3.1 and UAPI notices remain
conservatively supplied. Upstream's visible build transforms C using clang,
ordinary open-source libraries and system tools, with no proprietary GCC
intermediate-code processing. The exception's eligible-compilation definition
also permits compilation without a GCC-derived compiler; it does not require
recovery of exact historical CRT object hashes. This is a documented license
interpretation supported by source/build evidence, not authoritative legal approval.
See [upstream build source](https://github.com/AppImage/type2-runtime/tree/dd6cebedcbddde9c82f89b011e8e1d40b6e43868/scripts)
and the supplied `gcc-14.2.0-COPYING.RUNTIME`. No new material obligation was found.

### Compatibility results and limits

| Environment | Results |
| --- | --- |
| Ubuntu 24.04.3 userspace, Xvfb/Openbox | Official unchanged payload: extraction and FUSE window/store startup, normal close 0; FUSE mount appears and disappears, mountpoint removed |
| Debian 13.7 userspace, Xvfb/Openbox | Same results |
| Both userspaces, existing Rust package probe | FUSE and extraction: host Git init/status/commit/worktree creation/removal, four CLI fixture lookups, unchanged PATH/LD_LIBRARY_PATH, descendant process cancellation pass |
| Native Debian Forky/sid GNOME XWayland | Published custom, official and recipient-modified runtimes: FUSE and extraction startup/normal close 0; FUSE unmount/mountpoint removal pass |
| Native Debian Forky/sid GNOME Wayland, DISPLAY unset | Published and official images both create Wayland surfaces and isolated stores in both modes; test terminates their own process groups and waits for FUSE teardown |

The target-distribution userspaces share the host kernel. FUSE checks ran as
namespace-local root with mount capability because single-user namespace mappings
hide the helper's root ownership. Native desktop checks ran as the ordinary host
user. Neither result is a native Ubuntu/Debian stable desktop certification.
Native Wayland normal window-close was not automated; XWayland exercises normal
close separately. A first Wayland harness cleanup ran before asynchronous FUSE
teardown finished; waiting for mount disappearance resolved the harness race.
There is no new mandatory host dependency. CLI fixtures verify executable lookup
and CodeConvoy process behavior; no authenticated agent task was launched.

### Accepted upstream extraction defect

`--appimage-extract-and-run` and presence of `APPIMAGE_EXTRACT_AND_RUN` (including
empty or `0`) retain `TMPDIR/appimage_extracted_*` after ordinary exit 0 or 17.
The same defect occurs in the published custom runtime. `NO_CLEANUP=1` and plain
`--appimage-extract` deliberately retain output and are separate controls.
Ordinary FUSE execution is unaffected in the verified cases.

Three native launches per runtime retained **one** directory after launches
1/2/3, containing 13 regular files / 42,194,493 bytes (40.24 MiB) for the exact
published payload. Identical renamed images reuse it; changed image bytes add
a directory (fixture counts 1/1/2). Different versions or TMPDIRs can accumulate
copies until user/system cleanup. Disk-backed TMPDIR consumes disk, tmpfs uses
memory/swap. A later package containing source materials can retain more bytes.
Do not delete shared extraction paths while another process may be using them.

Cause: `runtime.c` calls `nftw` with descriptor limit 0; musl 1.2.5 returns success
without invoking the deletion callback. At the 2026-10-17 search, no newer dated
release or matching report/fix was found among all 157 issue/PR bodies, 59
runtime.c history entries and additional keyword/comment searches. This is a
bounded search result, not proof that no report exists. Continuous revision
`8f39b89e2ac31e1640b3d3f7e9a5108e6ce805fa` (SHA-256
`156f4bdbde9c52d01814600013e0a273f0118dc2de98975f3c8c63427ec79074`) still reproduces
it and is not selected for packaging. Related
[#152](https://github.com/AppImage/type2-runtime/pull/152) and
[#157](https://github.com/AppImage/type2-runtime/pull/157) concern directory
permissions; [AppImageKit #1215](https://github.com/AppImage/AppImageKit/issues/1215)
concerns concurrent reuse, and [#1053](https://github.com/AppImage/AppImageKit/issues/1053)
concerns signals. No upstream message was posted.

The smallest resolution is the official pinned runtime, documented limitation
and an exact-pin regression observation. Do not maintain a runtime patch,
AppRun deletion or a cleanup supervisor. A private TMPDIR is suitable for tests,
which measure retention before cleaning up their own files. Adopt a later
reviewed official fix only after retesting. All other failures remain failures.

## Phase 2

Implementation and final artifact/CI results will be recorded here after the
Phase 1 evidence commit. Published 1.0.0 files remain unchanged.
