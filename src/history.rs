//! Pure retention policy evaluation. Deletion and persistence are separate operations.
use crate::domain::Run;
use serde::{Deserialize, Deserializer, Serialize};
use std::{collections::BTreeSet, num::NonZeroUsize};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct RetentionPolicy {
    pub max_entries: Option<NonZeroUsize>,
    pub max_age_days: Option<std::num::NonZeroU32>,
    pub automatic: bool,
}

impl RetentionPolicy {
    pub fn is_default(&self) -> bool {
        self == &Self::default()
    }
    pub fn parse_count(input: &str) -> Result<NonZeroUsize, &'static str> {
        input
            .trim()
            .parse()
            .map_err(|_| "Enter a positive whole number.")
    }
}

/// An invalid retention object fails closed without losing unrelated settings.
/// In particular, a partially malformed policy must never authorize automatic deletion.
pub fn deserialize_policy<'de, D: Deserializer<'de>>(d: D) -> Result<RetentionPolicy, D::Error> {
    let value = serde_json::Value::deserialize(d)?;
    Ok(serde_json::from_value(value).unwrap_or_default())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CleanupPreview {
    pub policy: RetentionPolicy,
    pub stored: usize,
    pub protected: usize,
    pub undated: usize,
    pub remove: BTreeSet<u64>,
}
impl CleanupPreview {
    pub fn retained(&self) -> usize {
        self.stored - self.remove.len()
    }
}

/// No inferred completion date: every job must have a coherent timestamp.
/// Future dates (clock changes) and interrupted recovery with no observed end
/// remain outside retention until they can be ordered reliably.
fn completion(run: &Run, now: u64) -> Option<u64> {
    if run.jobs.is_empty() || run.created_at > now {
        return None;
    }
    let mut end = 0;
    for job in &run.jobs {
        let finished = job.finished_at?;
        if !job.status.is_terminal()
            || job.interrupted
            || finished < run.created_at
            || finished > now
            || job
                .started_at
                .is_some_and(|start| start < run.created_at || start > finished)
        {
            return None;
        }
        end = end.max(finished);
    }
    Some(end)
}

pub fn eligible(run: &Run, runtime_protected: &BTreeSet<u64>) -> bool {
    !run.history_protected() && !runtime_protected.contains(&run.id)
}

pub fn evaluate(
    runs: &[Run],
    policy: &RetentionPolicy,
    runtime_protected: &BTreeSet<u64>,
    now: u64,
) -> CleanupPreview {
    let mut preview = CleanupPreview {
        policy: policy.clone(),
        stored: runs.len(),
        protected: 0,
        undated: 0,
        remove: BTreeSet::new(),
    };
    let mut candidates = Vec::new();
    let mut seen = BTreeSet::new();
    let mut duplicates = BTreeSet::new();
    for run in runs {
        if !seen.insert(run.id) {
            duplicates.insert(run.id);
        }
    }
    for run in runs {
        if !eligible(run, runtime_protected) || duplicates.contains(&run.id) {
            preview.protected += 1;
        } else if let Some(end) = completion(run, now) {
            candidates.push((end, run.id));
        } else {
            preview.undated += 1;
        }
    }
    // A stable ID breaks timestamp ties; presentation order has no role.
    candidates.sort_unstable_by(|a, b| b.cmp(a));
    for (rank, (end, id)) in candidates.into_iter().enumerate() {
        let over_count = policy.max_entries.is_some_and(|limit| rank >= limit.get());
        let over_age = policy
            .max_age_days
            .is_some_and(|days| now - end > u64::from(days.get()) * 86_400);
        if over_count || over_age {
            preview.remove.insert(id);
        }
    }
    preview
}

/// Shared metadata-only deletion primitive, also used by Remove and Clear.
/// Callers supply live runtime protections and persist a staged state before
/// publishing this change. Duplicate identifiers are conservatively retained.
pub fn remove(
    runs: &mut Vec<Run>,
    ids: &BTreeSet<u64>,
    runtime_protected: &BTreeSet<u64>,
) -> usize {
    let mut seen = BTreeSet::new();
    let mut protected = runtime_protected.clone();
    for run in runs.iter() {
        if !seen.insert(run.id) || run.history_protected() {
            protected.insert(run.id);
        }
    }
    let before = runs.len();
    runs.retain(|run| !ids.contains(&run.id) || protected.contains(&run.id));
    before - runs.len()
}
