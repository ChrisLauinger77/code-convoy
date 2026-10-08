//! Scoped lifecycle leases use the scheduler's repository boundary, without
//! consuming agent slots or changing round-robin admission.
use super::{Administration, manager::Command};
use crate::{
    domain::{Job, ResultAvailability, WorktreeMetadata},
    process::Cancellation,
    worktrees::recovery::{self, Report},
};
use anyhow::{Context, Result};
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
use tokio::sync::{mpsc, oneshot};

#[derive(Clone)]
pub struct LifecycleClient {
    pub(super) commands: mpsc::UnboundedSender<Command>,
}
pub(super) struct Permit {
    pub id: u64,
    pub commands: mpsc::UnboundedSender<Command>,
    pub admin: Arc<Administration>,
    pub cancellation: Cancellation,
    pub safe: Arc<AtomicBool>,
    pub root: PathBuf,
}
impl Drop for Permit {
    fn drop(&mut self) {
        let safe = self.safe.load(Ordering::Acquire);
        if !safe {
            self.admin.healthy.store(false, Ordering::Release);
        }
        let _ = self.commands.send(Command::Release(self.id, safe));
    }
}
// Declared after the mutex guard so Drop restores/poisons health before the
// mutex can wake another worker, including when a future is aborted or panics.
struct AdministrationHealth<'a> {
    admin: &'a Administration,
    safe: &'a AtomicBool,
}
impl<'a> AdministrationHealth<'a> {
    fn protect(admin: &'a Administration, safe: &'a AtomicBool) -> Result<Self> {
        anyhow::ensure!(
            admin.healthy.load(Ordering::Acquire),
            "Git process cleanup is unconfirmed; inspect processes before restarting."
        );
        admin.healthy.store(false, Ordering::Release);
        Ok(Self { admin, safe })
    }
}
impl Drop for AdministrationHealth<'_> {
    fn drop(&mut self) {
        self.admin
            .healthy
            .store(self.safe.load(Ordering::Acquire), Ordering::Release);
    }
}

impl LifecycleClient {
    async fn permit(&self, metadata: &WorktreeMetadata, cleanup: bool) -> Result<Permit> {
        let (reply, response) = oneshot::channel();
        self.commands
            .send(Command::Acquire {
                metadata: metadata.clone(),
                cleanup,
                reply,
                commands: self.commands.clone(),
            })
            .context("The run manager stopped; result was preserved.")?;
        response
            .await
            .context("Result operation was interrupted; result was preserved.")?
    }
    /// Validates the immutable job identity before trusting any persisted resource.
    pub async fn inspect(&self, run: u64, index: usize, job: &Job, with_diff: bool) -> Report {
        let Some(m) = &job.worktree else {
            return Report::unavailable(
                ResultAvailability::Missing,
                "No isolated worktree was recorded.",
            );
        };
        if m.run != run
            || m.job != index
            || m.repository != job.repository
            || job.execution_mode != m.execution_mode
        {
            return Report::unavailable(
                ResultAvailability::Invalid,
                "Worktree ownership does not match this historical job.",
            );
        }
        let result = async {
            let permit = self.permit(m, false).await?;
            let _guard = permit.admin.mutex.lock().await;
            let _health = AdministrationHealth::protect(&permit.admin, &permit.safe)?;
            Ok::<_, anyhow::Error>(
                recovery::reconcile(
                    &permit.root,
                    m,
                    job.result_availability,
                    with_diff,
                    &permit.cancellation,
                    &permit.safe,
                )
                .await,
            )
        }
        .await;
        result.unwrap_or_else(|e| {
            Report::unavailable(
                if matches!(
                    job.result_availability,
                    ResultAvailability::CleanupFailed | ResultAvailability::CleanupPending
                ) {
                    ResultAvailability::CleanupFailed
                } else {
                    ResultAvailability::Stale
                },
                format!("Could not inspect isolated result. {e:#}"),
            )
        })
    }
    pub(crate) async fn apply_result(
        &self,
        run: u64,
        index: usize,
        job: &Job,
    ) -> crate::worktrees::apply::Outcome {
        use crate::worktrees::apply::Outcome;
        let attempt = async {
            let m = job
                .worktree
                .as_ref()
                .context("No isolated result was recorded.")?;
            anyhow::ensure!(
                job.status.is_terminal()
                    && m.run == run
                    && m.job == index
                    && m.repository == job.repository
                    && job.execution_mode == m.execution_mode,
                "Result is active or its ownership does not match this job."
            );
            anyhow::ensure!(
                job.resolution == crate::domain::ResultResolution::Unresolved,
                "Result is already resolved or an earlier Apply needs manual inspection."
            );
            let expected = job
                .review
                .as_ref()
                .and_then(|r| r.as_ref().ok())
                .and_then(|s| s.tree.as_deref())
                .context("Refresh Review before applying; result statistics are unavailable.")?;
            let permit = self.permit(m, true).await?;
            let _guard = permit.admin.mutex.lock().await;
            let _health = AdministrationHealth::protect(&permit.admin, &permit.safe)?;
            let i = crate::git::Inspection {
                cancellation: &permit.cancellation,
                safe: &permit.safe,
            };
            Ok::<_, anyhow::Error>(
                crate::worktrees::apply::apply(&permit.root, m, expected, &i).await,
            )
        }
        .await;
        attempt.unwrap_or_else(|e| Outcome::Blocked(format!("Apply blocked: {e:#}")))
    }
    pub(crate) async fn cleanup(&self, m: &WorktreeMetadata) -> Result<Report> {
        let permit = self.permit(m, true).await?;
        let _guard = permit.admin.mutex.lock().await;
        let _health = AdministrationHealth::protect(&permit.admin, &permit.safe)?;
        recovery::cleanup(&permit.root, m, &permit.cancellation, &permit.safe).await
    }
}
