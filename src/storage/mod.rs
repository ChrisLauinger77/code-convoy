//! Read-only storage snapshots. No Git/process execution, retention or writes.
mod filesystem;
mod history;
pub mod service;
#[cfg(test)]
mod tests;
use crate::{domain::WorktreeMetadata, process::Cancellation, worktrees::inventory};
use std::{collections::BTreeSet, path::PathBuf};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Status {
    #[default]
    NotMeasured,
    Measuring,
    Complete,
    Partial,
    Unavailable,
    Error,
}
impl Status {
    pub fn label(self) -> &'static str {
        match self {
            Self::NotMeasured => "Not measured",
            Self::Measuring => "Measuring…",
            Self::Complete => "Complete",
            Self::Partial => "Partial",
            Self::Unavailable => "Unavailable",
            Self::Error => "Measurement failed",
        }
    }
}
#[derive(Debug, Default)]
pub struct Measurement {
    pub bytes: Option<u64>,
    pub status: Status,
    pub issues: Vec<String>,
    pub issue_count: usize,
}
impl Measurement {
    fn complete(bytes: u64) -> Self {
        Self {
            bytes: Some(bytes),
            status: Status::Complete,
            ..Default::default()
        }
    }
    fn issue(&mut self, message: impl Into<String>) {
        self.issue_count += 1;
        if self.issues.len() < 100 {
            self.issues.push(message.into());
        }
        self.status = if self.bytes.is_some() {
            Status::Partial
        } else {
            Status::Unavailable
        };
    }
    pub fn label(&self) -> String {
        match self.bytes {
            Some(bytes) if self.status == Status::Complete => {
                format!("{} measured", format_bytes(bytes))
            }
            Some(bytes) => format!("{} measured (partial)", format_bytes(bytes)),
            None => self.status.label().into(),
        }
    }
}
#[derive(Clone)]
pub struct Request {
    pub history_path: PathBuf,
    pub worktree_root: PathBuf,
    // Minimal live references cover preparation not yet in the last atomic save.
    pub references: Vec<WorktreeMetadata>,
}
#[derive(Debug)]
pub struct Worktree {
    pub path: PathBuf,
    pub measurement: Measurement,
}
#[derive(Debug)]
pub struct Snapshot {
    pub finished_at: u64,
    pub history_path: PathBuf,
    pub history: Measurement,
    pub historical_convoys: Option<usize>,
    pub worktrees: Measurement,
    pub entries: Vec<Worktree>,
    pub unavailable: usize,
    pub partial: usize,
    pub unknown_ownership: usize,
    pub locations: BTreeSet<PathBuf>,
    pub total: Measurement,
}

pub fn measure(mut request: Request, cancel: &Cancellation) -> Option<Snapshot> {
    if cancel.is_cancelled() {
        return None;
    }
    let history = history::measure(&request.history_path, cancel);
    if cancel.is_cancelled() {
        return None;
    }
    request.references.extend(history.references);
    let inventory = inventory::discover(&request.worktree_root, request.references, cancel);
    if cancel.is_cancelled() {
        return None;
    }
    let locations = inventory
        .entries
        .iter()
        .filter_map(|e| {
            e.metadata
                .path
                .parent()
                .and_then(std::path::Path::parent)
                .map(ToOwned::to_owned)
        })
        .collect();
    let mut entries = Vec::new();
    for entry in inventory.entries {
        if cancel.is_cancelled() {
            return None;
        }
        let measurement = match entry.readable {
            Ok(()) => filesystem::measure(&entry.metadata.path, cancel),
            Err(e) => {
                let mut m = Measurement::default();
                m.issue(format!("{}: {e}", entry.metadata.path.display()));
                m
            }
        };
        entries.push(Worktree {
            path: entry.metadata.path,
            measurement,
        });
    }
    if cancel.is_cancelled() {
        return None;
    }
    let unavailable = entries
        .iter()
        .filter(|e| e.measurement.bytes.is_none())
        .count();
    let partial = entries
        .iter()
        .filter(|e| e.measurement.status == Status::Partial)
        .count();
    let mut worktrees = aggregate(entries.iter().map(|e| &e.measurement));
    let omitted = inventory.unknown.saturating_sub(inventory.issues.len());
    for issue in inventory.issues {
        worktrees.issue(issue);
    }
    worktrees.issue_count += omitted;
    if history.convoys.is_none() {
        worktrees.issue("Saved history could not be read; worktree inventory may omit references.");
    }
    // No measured entries and an incomplete inventory is unknown, not an empty total.
    if entries.is_empty() && worktrees.status != Status::Complete {
        worktrees.bytes = None;
        worktrees.status = Status::Unavailable;
    }
    let total = aggregate([&history.measurement, &worktrees]);
    Some(Snapshot {
        finished_at: crate::domain::now(),
        history_path: request.history_path,
        history: history.measurement,
        historical_convoys: history.convoys,
        worktrees,
        entries,
        unavailable,
        partial,
        unknown_ownership: inventory.unknown,
        locations,
        total,
    })
}

fn aggregate<'a>(parts: impl IntoIterator<Item = &'a Measurement>) -> Measurement {
    let mut total = Measurement::complete(0);
    let mut any = false;
    let mut empty = true;
    for part in parts {
        empty = false;
        if let Some(bytes) = part.bytes {
            any = true;
            match total.bytes.and_then(|n| n.checked_add(bytes)) {
                Some(sum) => total.bytes = Some(sum),
                None => {
                    total.bytes = None;
                    total.issue("Size exceeds the supported byte counter.");
                    return total;
                }
            }
        }
        if part.status != Status::Complete {
            if part.issues.is_empty() {
                total.issue("Some storage could not be measured completely.");
            } else {
                for issue in &part.issues {
                    total.issue(issue.clone());
                }
                total.issue_count += part.issue_count.saturating_sub(part.issues.len());
            }
        }
    }
    if !empty && !any {
        total.bytes = None;
        total.status = Status::Unavailable;
    }
    total
}

pub fn format_bytes(bytes: u64) -> String {
    if bytes < 1024 {
        return format!("{bytes} B");
    }
    let mut size = bytes as f64;
    let mut unit = "B";
    for next in ["KiB", "MiB", "GiB", "TiB", "PiB", "EiB"] {
        size /= 1024.0;
        unit = next;
        if size < 1024.0 {
            break;
        }
    }
    format!("{size:.2} {unit}")
}
