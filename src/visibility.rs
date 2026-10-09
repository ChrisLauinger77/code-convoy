//! Historical presentation uses completion observations, never today's Git state.
use crate::domain::{Job, JobStatus, ResultResolution, Run};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Changes {
    pub changed: Option<bool>,
    pub files: Option<usize>,
}
impl Changes {
    pub fn label(self) -> String {
        match (self.changed, self.files) {
            (Some(false), _) => "Unchanged".into(),
            (Some(true), Some(files)) => format!("Changed · {files} files"),
            (Some(true), None) => "Changed".into(),
            (None, _) => "Unknown".into(),
        }
    }
}

pub fn changes(job: &Job) -> Changes {
    // Older worktree_result values are mutable observations (recovery/refresh),
    // so even those must not be promoted to a historical completion snapshot.
    job.completion_changes.unwrap_or_default()
}

pub fn needs_review(job: &Job) -> bool {
    job.status.is_terminal()
        && !matches!(
            job.resolution,
            ResultResolution::Applied | ResultResolution::Discarded
        )
        && (job.unresolved_retained_result() || changes(job).changed != Some(false))
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Summary {
    pub total: usize,
    pub succeeded: usize,
    pub failed: usize,
    pub cancelled: usize,
    pub changed: usize,
    pub unchanged: usize,
    pub unknown: usize,
    pub review: usize,
}
pub fn summary(run: &Run) -> Summary {
    let mut s = Summary {
        total: run.jobs.len(),
        ..Summary::default()
    };
    for job in &run.jobs {
        match job.status {
            JobStatus::Succeeded => s.succeeded += 1,
            JobStatus::Failed => s.failed += 1,
            JobStatus::Cancelled => s.cancelled += 1,
            _ => {}
        }
        match changes(job).changed {
            Some(true) => s.changed += 1,
            Some(false) => s.unchanged += 1,
            None => s.unknown += 1,
        }
        s.review += usize::from(needs_review(job));
    }
    s
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum HistoryFilter {
    #[default]
    All,
    Completed,
    Failed,
    Cancelled,
    NeedsReview,
}
impl HistoryFilter {
    pub const ALL: [Self; 5] = [
        Self::All,
        Self::Completed,
        Self::Failed,
        Self::Cancelled,
        Self::NeedsReview,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::All => "All",
            Self::Completed => "Completed",
            Self::Failed => "Failed",
            Self::Cancelled => "Cancelled",
            Self::NeedsReview => "Needs Review",
        }
    }
    pub fn matches(self, run: &Run) -> bool {
        !run.active()
            && match self {
                Self::All => true,
                Self::Completed => run.status() == JobStatus::Succeeded,
                Self::Failed => run.status() == JobStatus::Failed,
                Self::Cancelled => run.status() == JobStatus::Cancelled,
                Self::NeedsReview => run.jobs.iter().any(needs_review),
            }
    }
}

/// Run IDs and their task/repository snapshots are immutable. Normalize their
/// potentially long prompt once; prune alongside history, without persisting an index.
#[derive(Default)]
pub struct HistorySearch {
    pub query: String,
    pub filter: HistoryFilter,
    terms: HashMap<u64, String>,
    cached_query: String,
    candidates: Vec<u64>,
    matches: Vec<u64>,
}
impl HistorySearch {
    pub fn clear(&mut self) {
        self.query.clear();
        self.filter = HistoryFilter::All;
    }
    pub fn matching(&mut self, runs: &[Run]) -> Vec<u64> {
        let candidates: Vec<_> = runs.iter().filter(|r| self.filter.matches(r)).collect();
        let ids: Vec<_> = candidates.iter().map(|r| r.id).collect();
        if self.cached_query == self.query && self.candidates == ids {
            return self.matches.clone();
        }
        let retained: std::collections::HashSet<_> = runs.iter().map(|r| r.id).collect();
        self.terms.retain(|id, _| retained.contains(id));
        let needle = self.query.trim().to_lowercase();
        self.matches = candidates
            .into_iter()
            .filter(|r| {
                let terms = self.terms.entry(r.id).or_insert_with(|| {
                    let mut terms =
                        format!("#{}\n{}\n{}", r.id, r.task.prompt, r.task.agent.label());
                    for job in &r.jobs {
                        terms.push_str(&format!(
                            "\n{}\n{}",
                            job.repository.name,
                            job.repository.path.display()
                        ));
                    }
                    terms.to_lowercase()
                });
                terms.contains(&needle)
            })
            .map(|r| r.id)
            .collect();
        self.cached_query.clone_from(&self.query);
        self.candidates = ids;
        self.matches.clone()
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum RepositoryFilter {
    #[default]
    All,
    Changed,
    Failed,
    NeedsReview,
}
impl RepositoryFilter {
    pub const ALL: [Self; 4] = [Self::All, Self::Changed, Self::Failed, Self::NeedsReview];
    pub fn label(self) -> &'static str {
        match self {
            Self::All => "All repositories",
            Self::Changed => "Changed",
            Self::Failed => "Failed",
            Self::NeedsReview => "Needs Review",
        }
    }
    pub fn matches(self, job: &Job) -> bool {
        match self {
            Self::All => true,
            Self::Changed => changes(job).changed == Some(true),
            Self::Failed => job.status == JobStatus::Failed,
            Self::NeedsReview => needs_review(job),
        }
    }
}
