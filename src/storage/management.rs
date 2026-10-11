//! Lifecycle policy is independent from inventory, measurements and Git evidence.
use crate::{domain::*, worktrees::cleanup::Inspection};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lifecycle {
    Active,
    Retained,
    Candidate,
    Modified,
    Unavailable,
    Unknown,
}
impl Lifecycle {
    pub fn label(self) -> &'static str {
        match self {
            Self::Active => "Active",
            Self::Retained => "Retained",
            Self::Candidate => "Cleanup Candidate",
            Self::Modified => "Modified / Needs Review",
            Self::Unavailable => "Unavailable",
            Self::Unknown => "Unknown",
        }
    }
}

/// Re-evaluated from the live state both in presentation and in the Store intent
/// transaction. No completion status or saved clean observation releases retention.
pub fn protection(state: &AppState, m: &WorktreeMetadata) -> Option<(Lifecycle, String)> {
    let blocked = |kind, reason: &str| Some((kind, reason.to_owned()));
    let references: Vec<_> = state
        .runs
        .iter()
        .flat_map(|r| r.jobs.iter().enumerate().map(move |(n, j)| (r, n, j)))
        .filter(|(_, _, j)| j.worktree.as_ref().is_some_and(|w| w.path == m.path))
        .collect();
    if references.len() != 1 {
        return blocked(
            Lifecycle::Unknown,
            "Missing or ambiguous lifecycle record; inspection only.",
        );
    }
    let (run, index, job) = references[0];
    if job.worktree.as_ref() != Some(m)
        || run.id != m.run
        || index != m.job
        || job.repository != m.repository
        || job.execution_mode != m.execution_mode
    {
        return blocked(
            Lifecycle::Unknown,
            "Lifecycle identity does not match the ownership record.",
        );
    }
    if !job.status.is_terminal() || crate::validation::blocks_result_operation(state, job) {
        return blocked(
            Lifecycle::Active,
            "The affected job or validation is active.",
        );
    }
    if state.runs.iter().flat_map(|r| &r.jobs).any(|j| {
        !j.status.is_terminal()
            && (j.repository.path.starts_with(&m.repository.path)
                || m.repository.path.starts_with(&j.repository.path)
                || j.before.as_ref().and_then(|b| b.common_dir.as_ref()) == Some(&m.common_dir)
                || j.worktree
                    .as_ref()
                    .is_some_and(|w| w.common_dir == m.common_dir))
    }) {
        return blocked(
            Lifecycle::Active,
            "The repository has queued or running work.",
        );
    }
    if job.resolution == ResultResolution::Unresolved {
        return blocked(
            Lifecycle::Retained,
            "Unresolved result retained for review. Use the existing Apply or Discard workflow to resolve it.",
        );
    }
    if job.resolution != ResultResolution::Applied {
        return blocked(
            Lifecycle::Unknown,
            "Pending, uncertain or discarded result requires manual inspection.",
        );
    }
    if !job.result_checked || job.result_availability != ResultAvailability::Available {
        return blocked(
            Lifecycle::Unknown,
            "Refresh result Review first; saved availability is not cleanup authority.",
        );
    }
    if !state.repositories.contains(&m.repository) {
        return blocked(
            Lifecycle::Unavailable,
            "Original repository is no longer registered; inspection only.",
        );
    }
    if state
        .repositories
        .iter()
        .any(|r| r.path.starts_with(&m.path) || m.path.starts_with(&r.path))
    {
        return blocked(
            Lifecycle::Unknown,
            "Target overlaps a registered repository.",
        );
    }
    None
}
pub fn classify(
    state: &AppState,
    m: &WorktreeMetadata,
    inspection: Option<&Inspection>,
) -> (Lifecycle, String) {
    let protection = protection(state, m);
    if let Some(p @ (Lifecycle::Active, _)) = &protection {
        return p.clone();
    }
    let Some(i) = inspection else {
        return (
            Lifecycle::Unknown,
            "Awaiting current ownership and Git inspection.".into(),
        );
    };
    if i.exists == Some(false) {
        return (Lifecycle::Unavailable, i.detail.clone());
    }
    if !i.detail.is_empty() {
        return (Lifecycle::Unknown, i.detail.clone());
    }
    if let Some(protection) = protection {
        return protection;
    }
    if !i.changes.is_empty() {
        return (
            Lifecycle::Modified,
            "Local, ignored, staged or committed data needs review.".into(),
        );
    }
    if i.proof.is_some() {
        return (
            Lifecycle::Candidate,
            "Resolved, verified and currently clean. Final checks are required.".into(),
        );
    }
    (Lifecycle::Unknown, i.detail.clone())
}
