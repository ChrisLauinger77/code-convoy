//! Filesystem-only ownership inventory. This grants no lifecycle authority.
use super::{location, ordinary, recovery};
use crate::{domain::WorktreeMetadata, process::Cancellation};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug)]
pub struct Entry {
    pub metadata: WorktreeMetadata,
    pub readable: Result<(), String>,
}
#[derive(Debug, Default)]
pub struct Inventory {
    pub entries: Vec<Entry>,
    pub issues: Vec<String>,
    pub unknown: usize,
}
impl Inventory {
    fn unknown(&mut self, path: &Path, error: impl std::fmt::Display) {
        self.unknown += 1;
        if self.issues.len() < 100 {
            self.issues.push(format!("{}: {error}", path.display()));
        }
    }
}

/// Only inspect immediate children of the Store root and exact registered custom
/// attempts. Never enumerate today's custom base or any source repository.
pub fn discover(
    root: &Path,
    references: Vec<WorktreeMetadata>,
    cancel: &Cancellation,
) -> Inventory {
    let mut inventory = Inventory::default();
    let mut candidates = references;
    match fs::symlink_metadata(root) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => inventory.unknown(root, e),
        Ok(_) => {
            let checked = ordinary(root, true).and_then(|()| {
                anyhow::ensure!(
                    root.canonicalize()? == root,
                    "Worktree storage now resolves elsewhere."
                );
                Ok(())
            });
            if let Err(e) = checked {
                inventory.unknown(root, e);
            } else {
                read_directory(
                    root,
                    &mut inventory,
                    cancel,
                    |path| {
                        if path.file_name().is_some_and(|n| n == location::RECORDS) {
                            return Ok(None);
                        }
                        ordinary(path, true)?;
                        let m: WorktreeMetadata =
                            serde_json::from_slice(&recovery::bounded(&path.join("owner.json"))?)?;
                        anyhow::ensure!(m.path == path.join("tree"), "Manifest location mismatch.");
                        Ok(Some(m))
                    },
                    &mut candidates,
                );
                let records = root.join(location::RECORDS);
                match fs::symlink_metadata(&records) {
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                    Err(e) => inventory.unknown(&records, e),
                    Ok(_) => match location::trusted_records(root) {
                        Ok(records) => read_directory(
                            &records,
                            &mut inventory,
                            cancel,
                            |path| location::read_record(root, path).map(Some),
                            &mut candidates,
                        ),
                        Err(e) => inventory.unknown(&records, e),
                    },
                }
            }
        }
    }
    // The random reservation name is the existing attempt identity, not run/job
    // alone (a retry must remain distinct). Conflicting identity records are unknown.
    let mut unique: BTreeMap<PathBuf, Option<WorktreeMetadata>> = BTreeMap::new();
    for m in candidates {
        if cancel.is_cancelled() {
            return inventory;
        }
        let attempt = match recovery::layout(root, &m) {
            Ok(attempt) => attempt,
            Err(e) => {
                inventory.unknown(&m.path, e);
                continue;
            }
        };
        let Some(identity) = attempt.file_name() else {
            continue;
        };
        match unique.entry(PathBuf::from(identity)) {
            std::collections::btree_map::Entry::Vacant(e) => {
                e.insert(Some(m));
            }
            std::collections::btree_map::Entry::Occupied(mut e) => {
                if e.get().as_ref().is_some_and(|old| old != &m) {
                    inventory.unknown(
                        &m.path,
                        "Conflicting ownership records; excluded from measurement.",
                    );
                    e.insert(None);
                }
            }
        }
    }
    for m in unique.into_values().flatten() {
        if cancel.is_cancelled() {
            break;
        }
        let readable = recovery::manifest(root, &m).and_then(|()| {
            if recovery::physically_cleaned(&m)? {
                Ok(false)
            } else {
                Ok(true)
            }
        });
        if matches!(readable, Ok(false)) {
            continue;
        }
        inventory.entries.push(Entry {
            metadata: m,
            readable: readable.map(|_| ()).map_err(|e| format!("{e:#}")),
        });
    }
    inventory
}

fn read_directory(
    directory: &Path,
    inventory: &mut Inventory,
    cancel: &Cancellation,
    read: impl Fn(&Path) -> anyhow::Result<Option<WorktreeMetadata>>,
    candidates: &mut Vec<WorktreeMetadata>,
) {
    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(e) => {
            inventory.unknown(directory, e);
            return;
        }
    };
    for entry in entries {
        if cancel.is_cancelled() {
            break;
        }
        match entry {
            Err(e) => inventory.unknown(directory, e),
            Ok(entry) => match read(&entry.path()) {
                Ok(Some(m)) => candidates.push(m),
                Ok(None) => {}
                Err(e) => inventory.unknown(&entry.path(), e),
            },
        }
    }
}
