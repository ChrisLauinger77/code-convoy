//! Independent retry requests and follow-up drafts; no scheduling dependencies.
use crate::{domain::*, git, runner};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Provenance {
    Retry { run: u64, job: usize, attempt: u32 },
    FollowUp { run: u64, jobs: Vec<usize> },
}
impl Provenance {
    pub fn source_run(&self) -> u64 {
        match self {
            Self::Retry { run, .. } | Self::FollowUp { run, .. } => *run,
        }
    }
    pub fn label(&self) -> String {
        match self {
            Self::Retry { run, job, attempt } => format!(
                "Retry · attempt {attempt} · from previous convoy #{run}, result {}",
                job.saturating_add(1)
            ),
            Self::FollowUp { run, .. } => format!("Created from previous convoy #{run}"),
        }
    }
    pub fn is_retry(&self) -> bool {
        matches!(self, Self::Retry { .. })
    }
}

pub struct Retry {
    pub task: TaskConfig,
    pub repository: Repository,
    pub provenance: Provenance,
}
impl Retry {
    pub fn from_state(state: &AppState, run: u64, index: usize) -> Result<Self> {
        let source = state
            .runs
            .iter()
            .find(|r| r.id == run)
            .context("Retry blocked: original convoy is unavailable.")?;
        let job = source
            .jobs
            .get(index)
            .context("Retry blocked: original result is unavailable.")?;
        ensure!(
            job.retryable(),
            "Retry is only available for failed, cancelled or interrupted jobs."
        );
        let repository = state
            .repositories
            .iter()
            .find(|r| r.path == job.repository.path)
            .context("Retry blocked: repository is no longer registered.")?
            .clone();
        let attempt = match source.provenance {
            Some(Provenance::Retry { attempt, .. }) => attempt
                .checked_add(1)
                .context("Retry attempt limit reached.")?,
            _ => 2,
        };
        Ok(Self {
            task: source.task.clone(),
            repository,
            provenance: Provenance::Retry {
                run,
                job: index,
                attempt,
            },
        })
    }
    pub async fn prepare(self) -> Result<(runner::PreparedRun, Provenance)> {
        // Check the current canonical registration before probing its CLI. Normal
        // preparation checks attachments, backend capabilities and Git again.
        git::status(&self.repository.path)
            .await
            .context("Retry blocked: repository is unavailable.")?;
        let prepared = runner::prepare(self.task, vec![self.repository])
            .await
            .context("Retry blocked; review prerequisites and try again.")?;
        Ok((prepared, self.provenance))
    }
}

pub struct FollowUp {
    pub task: TaskConfig,
    pub provenance: Provenance,
    pub repositories: Vec<Repository>,
    pub omitted: Vec<String>,
}
impl FollowUp {
    pub fn from_state(state: &AppState, run: u64, selected: &HashSet<usize>) -> Result<Self> {
        ensure!(
            !selected.is_empty(),
            "No repositories selected for follow-up."
        );
        let source = state
            .runs
            .iter()
            .find(|r| r.id == run)
            .context("Source convoy is no longer in history.")?;
        ensure!(
            selected.iter().all(|&i| i < source.jobs.len()),
            "A selected result is no longer available."
        );
        let mut jobs: Vec<_> = selected.iter().copied().collect();
        jobs.sort_unstable();
        let mut task = source.task.clone();
        task.prompt.clear();
        task.attachments.clear();
        let mut seen = HashSet::new();
        let mut repositories = Vec::new();
        let mut omitted = Vec::new();
        for &index in &jobs {
            let job = &source.jobs[index];
            if !seen.insert(&job.repository.path) {
                continue;
            }
            if let Some(repo) = state
                .repositories
                .iter()
                .find(|r| r.path == job.repository.path)
            {
                repositories.push(repo.clone());
            } else {
                omitted.push(format!("{}: no longer registered", job.repository.name));
            }
        }
        Ok(Self {
            task,
            provenance: Provenance::FollowUp { run, jobs },
            repositories,
            omitted,
        })
    }
    /// Availability is external state; never perform these checks in rendering.
    pub async fn check(mut self) -> Self {
        let candidates = std::mem::take(&mut self.repositories);
        for repo in candidates {
            if git::status(&repo.path).await.is_ok() {
                self.repositories.push(repo);
            } else {
                self.omitted.push(format!(
                    "{}: repository is unavailable; restore its location or register it again",
                    repo.name
                ));
            }
        }
        self
    }
    pub fn populate(mut self, state: &mut AppState) -> (HashSet<std::path::PathBuf>, String) {
        self.repositories.retain(|repo| {
            let registered = state.repositories.iter().any(|r| r.path == repo.path);
            if !registered {
                self.omitted
                    .push(format!("{}: no longer registered", repo.name));
            }
            registered
        });
        let selected: HashSet<_> = self.repositories.into_iter().map(|r| r.path).collect();
        state.reuse_task(self.task);
        state.draft_provenance = Some(self.provenance);
        let mut message = format!(
            "Follow-up draft: {} repositories selected. Enter a new task; nothing has started.",
            selected.len()
        );
        if !self.omitted.is_empty() {
            message.push_str(&format!(" Not selected: {}.", self.omitted.join("; ")));
        }
        (selected, message)
    }
}
