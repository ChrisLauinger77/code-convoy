use super::Measurement;
use crate::process::Cancellation;
use std::{fs, io, path::Path};

pub(super) fn link(metadata: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes() & 0x400 != 0 {
            return true;
        }
    }
    metadata.file_type().is_symlink()
}

/// Check the entire path before opening it: a replaced base/ancestor must not
/// make an otherwise ordinary final component lead outside managed storage.
pub(super) fn ordinary_path(path: &Path) -> io::Result<()> {
    for ancestor in path.ancestors() {
        if ancestor.as_os_str().is_empty() {
            continue;
        }
        if link(&fs::symlink_metadata(ancestor)?) {
            return Err(io::Error::other("Symlink or reparse point is excluded."));
        }
    }
    Ok(())
}

pub(super) fn measure(root: &Path, cancel: &Cancellation) -> Measurement {
    walk(root, cancel, |_| {})
}

// Streaming depth-first iteration bounds pending memory by directory depth, not
// node_modules entry count. No file contents, Git commands or external .git paths.
pub(super) fn walk(
    root: &Path,
    cancel: &Cancellation,
    mut before_directory: impl FnMut(&Path),
) -> Measurement {
    let mut result = Measurement::default();
    let checked = ordinary_path(root).and_then(|()| fs::symlink_metadata(root));
    let original = match checked {
        Ok(m) if m.is_dir() => m,
        Ok(_) => {
            result.issue("Worktree is not an ordinary directory.");
            return result;
        }
        Err(e) => {
            result.issue(format!("{}: {e}", root.display()));
            return result;
        }
    };
    let mut stack = Vec::new();
    let mut next = Some(root.to_owned());
    #[cfg(unix)]
    let mut files = std::collections::HashSet::new();
    loop {
        if cancel.is_cancelled() {
            return result;
        }
        if let Some(path) = next.take() {
            before_directory(&path);
            let directory = ordinary_path(&path).and_then(|()| fs::read_dir(&path));
            match directory {
                Ok(entries) => {
                    if result.bytes.is_none() {
                        result = Measurement::complete(0);
                    }
                    stack.push((path, entries));
                }
                Err(e) => result.issue(format!("{}: {e}", path.display())),
            }
        }
        let Some((parent, entries)) = stack.last_mut() else {
            break;
        };
        match entries.next() {
            None => {
                stack.pop();
            }
            Some(Err(e)) => result.issue(format!("{}: {e}", parent.display())),
            Some(Ok(entry)) => {
                let path = entry.path();
                // Ancestors are rechecked because a directory can be replaced
                // while its iterator is open. Snapshots are never atomic.
                let metadata = ordinary_path(parent).and_then(|()| fs::symlink_metadata(&path));
                match metadata {
                    Err(e) => result.issue(format!("{}: {e}", path.display())),
                    Ok(m) if link(&m) => {} // includes broken links and junctions
                    Ok(m) if m.is_dir() => next = Some(path),
                    Ok(m) if m.is_file() => {
                        #[cfg(unix)]
                        {
                            use std::os::unix::fs::MetadataExt;
                            if m.nlink() > 1 && !files.insert((m.dev(), m.ino())) {
                                continue;
                            }
                        }
                        if let Some(sum) = result.bytes.and_then(|n| n.checked_add(m.len())) {
                            result.bytes = Some(sum);
                        } else {
                            result.bytes = None;
                            result.issue("Size exceeds the supported byte counter.");
                            return result;
                        }
                    }
                    Ok(_) => {} // sockets, devices and FIFOs have no regular-file length
                }
            }
        }
    }
    let end = ordinary_path(root).and_then(|()| fs::symlink_metadata(root));
    if end.as_ref().map_or(true, |m| !same_directory(&original, m)) {
        result.issue("Worktree disappeared or was replaced during measurement.");
    }
    result
}
fn same_directory(a: &fs::Metadata, b: &fs::Metadata) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        b.is_dir() && a.dev() == b.dev() && a.ino() == b.ino()
    }
    #[cfg(not(unix))]
    {
        b.is_dir() && a.created().ok() == b.created().ok()
    }
}
