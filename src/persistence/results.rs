//! Split Store transactions keep intent on the state owner and Git off the UI.
use super::Store;
use crate::{
    domain::*,
    runner::LifecycleClient,
    worktrees::{apply::Outcome, recovery::Report},
};
use anyhow::{Context, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Apply,
    Discard,
    CleanupApplied,
    InternalCleanup,
}
pub struct Operation {
    pub run: u64,
    pub index: usize,
    pub action: Action,
    pub job: Job,
}
pub enum Completion {
    Apply(Outcome),
    Cleanup(Result<Report, String>),
}
impl Operation {
    pub async fn execute(&self, client: &LifecycleClient) -> Completion {
        match self.action {
            Action::Apply => {
                Completion::Apply(client.apply_result(self.run, self.index, &self.job).await)
            }
            _ => Completion::Cleanup(match &self.job.worktree {
                Some(m) => client.cleanup(m).await.map_err(|e| format!("{e:#}")),
                None => Err("No isolated result was recorded.".into()),
            }),
        }
    }
}
fn job(state: &mut AppState, run: u64, index: usize) -> Result<&mut Job> {
    state
        .runs
        .iter_mut()
        .find(|r| r.id == run)
        .and_then(|r| r.jobs.get_mut(index))
        .context("Historical result was not found.")
}
impl Store {
    pub fn begin_result_operation(
        &self,
        state: &mut AppState,
        run: u64,
        index: usize,
        action: Action,
    ) -> Result<Operation> {
        let j = job(state, run, index)?.clone();
        let m = j
            .worktree
            .as_ref()
            .context("No isolated result was recorded.")?;
        anyhow::ensure!(j.status.is_terminal(), "Active work cannot be resolved.");
        anyhow::ensure!(
            j.resolution != ResultResolution::ApplyPending,
            "Apply outcome is pending or uncertain. Inspect the destination manually; the retained result and recovery evidence cannot be discarded or cleaned up."
        );
        anyhow::ensure!(
            m.run == run
                && m.job == index
                && m.repository == j.repository
                && j.execution_mode == ExecutionMode::IsolatedWorktree,
            "Result ownership does not match this job."
        );
        anyhow::ensure!(
            !state
                .repositories
                .iter()
                .any(|r| r.path.starts_with(&m.path) || m.path.starts_with(&r.path)),
            "A registered working tree cannot be a result operation target."
        );
        match action {
            Action::Apply => {
                anyhow::ensure!(
                    state.repositories.contains(&j.repository),
                    "Apply blocked: repository is no longer registered."
                );
                anyhow::ensure!(
                    j.resolution == ResultResolution::Unresolved
                        && j.result_checked
                        && j.result_availability == ResultAvailability::Available,
                    "Apply blocked: result is resolved, stale or unavailable."
                );
                job(state, run, index)?.resolution = ResultResolution::ApplyPending;
            }
            Action::Discard => {
                anyhow::ensure!(
                    !matches!(
                        j.resolution,
                        ResultResolution::Applied | ResultResolution::Discarded
                    ),
                    "Result is already resolved."
                );
                job(state, run, index)?.resolution = ResultResolution::DiscardPending;
                job(state, run, index)?.result_availability = ResultAvailability::CleanupPending;
            }
            Action::InternalCleanup => {
                job(state, run, index)?.result_availability = ResultAvailability::CleanupPending;
            }
            Action::CleanupApplied => {
                anyhow::ensure!(
                    j.resolution == ResultResolution::Applied,
                    "Only Applied results can use retained-copy cleanup."
                );
                job(state, run, index)?.result_availability = ResultAvailability::CleanupPending;
            }
        }
        if let Err(e) = self.save(state) {
            *job(state, run, index)? = j;
            return Err(e).context("Could not save result operation intent; no cleanup was started; no repository operation was started.");
        }
        job(state, run, index)?.log.append(match action {
            Action::Apply => "\n[CodeConvoy] Reviewing isolated result for Apply\n",
            Action::Discard => "\n[CodeConvoy] Discard requested\n",
            Action::CleanupApplied | Action::InternalCleanup => {
                "\n[CodeConvoy] Retained result cleanup requested\n"
            }
        });
        Ok(Operation {
            run,
            index,
            action,
            job: j,
        })
    }
    pub fn finish_result_operation(
        &self,
        state: &mut AppState,
        op: &Operation,
        completion: Completion,
    ) -> Result<()> {
        let j = job(state, op.run, op.index)?;
        anyhow::ensure!(
            j.worktree == op.job.worktree,
            "Result identity changed during operation; preserve ownership metadata."
        );
        match completion {
            Completion::Apply(Outcome::Applied(stats)) => {
                j.resolution = ResultResolution::Applied;
                j.resolved_at = Some(now());
                j.review = Some(Ok(stats));
                j.worktree_detail = "Applied to the registered working tree without staging or committing. Stable isolated copy retained.".into();
                j.log
                    .append("\n[CodeConvoy] Apply preflight passed\n[CodeConvoy] Result applied\n");
            }
            Completion::Apply(Outcome::Blocked(detail)) => {
                j.resolution = ResultResolution::Unresolved;
                j.worktree_detail = detail.clone();
                j.log.append(&format!("\n[CodeConvoy] {detail}\n"));
            }
            Completion::Apply(Outcome::Uncertain(detail)) => {
                j.resolution = ResultResolution::ApplyPending;
                j.worktree_detail = detail.clone();
                j.log.append(&format!("\n[CodeConvoy] {detail}\n"));
            }
            Completion::Cleanup(result) => match result {
                Ok(report) => {
                    report.apply(j);
                    if op.action == Action::Discard {
                        j.resolution = ResultResolution::Discarded;
                        j.resolved_at = Some(now());
                        j.log.append("\n[CodeConvoy] Result discarded\n");
                    }
                }
                Err(detail) => Report::unavailable(
                    ResultAvailability::CleanupFailed,
                    format!("Cleanup failed: {detail}"),
                )
                .apply(j),
            },
        }
        self.save(state).context("Result outcome could not be saved. Durable intent is retained; inspect the destination before retrying after restart.")
    }
}
