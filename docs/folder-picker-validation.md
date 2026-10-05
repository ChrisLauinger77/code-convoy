# Native repository folder selection

Recorded 2026-10-05 on macOS (Apple Silicon).

## Dependency choice

The existing direct dependencies and lockfile contained no file-dialog implementation. eframe already supplies the parent window handles and Tokio already runs asynchronous UI work. Added [rfd 0.17.2](https://docs.rs/rfd/0.17.2/rfd/), a maintained Rust native-dialog library, using `default-features = false` with explicit `xdg-portal` and `wayland` features. It covers all three desktop platforms without hand-written platform FFI, an extra UI toolkit, or another async runtime. Only `rfd` and `pollster` are newly added lockfile packages; platform bindings were already present.

The [upstream changelog](https://github.com/PolyMeilex/rfd/blob/0.17.2/CHANGELOG.md) documents current maintenance, the removal of Tokio/async-std feature switches in 0.17, and the 1.88 minimum Rust version in 0.17.2, below CodeConvoy's 1.95 requirement. This release uses libdbus for portals, rather than the ashpd/zbus implementation in older rfd releases.

## Platform behavior

| Platform | Implementation and prerequisites |
| --- | --- |
| Linux | XDG Desktop Portal FileChooser with directory selection and the application parent identifier, including Wayland support. Uses the desktop's GTK/GNOME/KDE portal implementation. Runtime libdbus and a FileChooser-capable portal backend are needed; wlroots alone does not provide FileChooser. rfd falls back to Zenity if the portal is unavailable; downstream packaging should include Zenity. No GTK development dependency is added. |
| macOS | Native NSOpenPanel sheet attached to CodeConvoy's window. The picker future is constructed on the UI/main thread and awaited on Tokio; the app's event loop keeps running. Directory creation is disabled on this platform because registration expects an existing working tree. |
| Windows | Native COM IFileOpenDialog with FOS_PICKFOLDERS and the application parent window. rfd runs the dialog on its own COM-initialized thread and returns a future, without blocking egui or Tokio's executor. |

The [rfd documentation](https://docs.rs/rfd/0.17.2/rfd/) and released [macOS](https://github.com/PolyMeilex/rfd/blob/0.17.2/src/backend/macos/modal_future.rs), [Windows](https://github.com/PolyMeilex/rfd/blob/0.17.2/src/backend/win_cid/file_dialog.rs), and [portal](https://github.com/PolyMeilex/rfd/blob/0.17.2/src/backend/xdg_desktop_portal.rs) implementations were inspected. rfd returns `Option`, not a distinct dialog error: cancellation and inability to open a picker both leave the field intact. Manual entry remains usable if platform services are missing.

## Workflow and safety

Browse only fills the draft field. Add repository remains explicit, uses the existing Git validation/status functions and canonical duplicate check, and retains the existing repository limit. No backend, scheduling, Git inspection, locking, dirty-tree review, execution or persistence semantics change. The exact chosen path is retained until edited, so a directory ending in whitespace cannot be silently trimmed to a different repository. Non-Unicode paths leave the old field intact and show a diagnostic instead of selecting a lossy replacement path.

Browse and Add are disabled while the picker is pending; repository refresh and ongoing execution retain their own state. A picker result never clears unrelated Git work. The path stays editable, cancellation preserves edits, and completion restores focus to the path. Standard Tab/Shift+Tab and Enter/Space remain available. The action row wraps in narrow panes.

## Validation

Automated tests cover selection without registration or persisted-state changes, cancellation with existing/manual input, independence from repository busy state, non-Unicode paths, exact trailing-whitespace paths, and manual edits after a selection. Real temporary Git repositories exercise the UI's existing explicit registration path for valid, non-Git, missing, nested and duplicate paths, including Unicode, spaces, canonical aliases and pre-existing changes. Existing compact layout and all backend/safety tests remain in the suite.

Native macOS checks used disposable repositories and an isolated Store with the real App/eframe integration:

- Browse opened an attached native folder sheet; a temporary UI-frame counter advanced from 600 to 620 while the sheet remained open, confirming continued egui execution.
- Choosing a Git repository containing spaces and Unicode filled the path and left the repository list empty until Add was activated.
- Tab reached Browse; Space opened it; Escape cancelled; the original path and focus were restored. Tab reached Add and Enter registered the repository. Shift+Tab returned to manual entry; Browse's focus outline was visible.
- Choosing a non-Git directory only filled the field. Add rejected it and retained the existing registration.
- A manually entered missing path produced the existing missing-path diagnostic and retained the input.
- Registering the same repository through a canonical alias produced “This repository is already registered.” and kept one entry.
- Manual entry successfully registered the valid repository after removing only its test registration metadata. Existing untracked content stayed intact and the dirty status remained visible.
- A final native check selected `space repo ` while a separate `space repo` directory also existed. Add registered the exact trailing-space path, confirmed in the isolated saved state.

Required checks passed:

```text
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo build --release
```

Result: 91 tests passed; 3 existing optional installed-CLI probes ignored. Release build succeeded on aarch64 macOS. No authenticated agent task was used.

Native Linux and Windows runtime checks were not performed on this Mac; their dialog behavior is verified against the dependency's released source. The temporary native test launcher is not shipped.
