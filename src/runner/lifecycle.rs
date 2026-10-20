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
pub(crate) struct Permit {
    pub id: u64,
    pub(super) commands: mpsc::UnboundedSender<Command>,
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
    pub(super) async fn bind_worktree(
        &self,
        metadata: &WorktreeMetadata,
        cancellation: &Cancellation,
        stopping: &AtomicBool,
    ) -> Result<()> {
        anyhow::ensure!(
            !stopping.load(Ordering::Acquire) && !cancellation.is_cancelled(),
            "Cancelled during worktree preparation."
        );
        let (reply, response) = oneshot::channel();
        self.commands
            .send(Command::BindWorktree {
                key: (metadata.run, metadata.job),
                path: metadata.path.clone(),
                reply,
            })
            .context("The run manager stopped before worktree creation.")?;
        // The manager may be blocked delivering Activity. Shutdown sets stopping
        // before cancelling this token; neither path may depend on its reply.
        tokio::select! {
            biased;
            _ = cancellation.cancelled() => anyhow::bail!("Cancelled during worktree preparation."),
            result = response => result.context("Worktree lease registration was interrupted.")??,
        }
        // An acknowledgement racing Stop/quit is not permission to create Git
        // state. Dropping the receiver also invalidates a still-queued request.
        anyhow::ensure!(
            !stopping.load(Ordering::Acquire) && !cancellation.is_cancelled(),
            "Cancelled during worktree preparation."
        );
        Ok(())
    }
    async fn permit(&self, metadata: &WorktreeMetadata, cleanup: bool) -> Result<Permit> {
        self.acquire(
            (metadata.run, metadata.job),
            super::schedule::Lease {
                path: metadata.repository.path.clone(),
                worktree: Some(metadata.path.clone()),
                common_dir: Some(metadata.common_dir.clone()),
                mode: crate::domain::ExecutionMode::Direct,
            },
            cleanup,
            None,
        )
        .await
    }
    async fn acquire(
        &self,
        key: (u64, usize),
        lease: super::schedule::Lease,
        cleanup: bool,
        cancellation: Option<Cancellation>,
    ) -> Result<Permit> {
        let (reply, response) = oneshot::channel();
        self.commands
            .send(Command::Acquire {
                key,
                lease,
                cleanup,
                cancellation,
                reply,
                commands: self.commands.clone(),
            })
            .context("The run manager stopped; result was preserved.")?;
        response
            .await
            .context("Result operation was interrupted; result was preserved.")?
    }
    pub(crate) async fn validation_permit(
        &self,
        run: u64,
        index: usize,
        job: &Job,
        cancellation: &Cancellation,
    ) -> Result<Permit> {
        use crate::domain::ExecutionMode;
        crate::validation::ensure_eligible(job)?;
        let path = crate::validation::directory(job)?;
        let common = match job.execution_mode {
            ExecutionMode::IsolatedWorktree => {
                let m = job
                    .worktree
                    .as_ref()
                    .context("Original worktree is unavailable.")?;
                anyhow::ensure!(
                    m.run == run
                        && m.job == index
                        && m.repository == job.repository
                        && m.execution_mode == job.execution_mode,
                    "Original worktree identity does not match this result."
                );
                m.common_dir.clone()
            }
            ExecutionMode::Direct => job
                .before
                .as_ref()
                .and_then(|b| b.common_dir.clone())
                .context(
                    "Original repository identity was not recorded; validation is unavailable.",
                )?,
        };
        let permit = self
            .acquire(
                (run, index),
                super::schedule::Lease {
                    path: job.repository.path.clone(),
                    worktree: Some(path.to_owned()),
                    common_dir: Some(common.clone()),
                    mode: ExecutionMode::Direct,
                },
                true,
                Some(cancellation.clone()),
            )
            .await?;
        {
            let _guard = permit.admin.mutex.lock().await;
            let _health = AdministrationHealth::protect(&permit.admin, &permit.safe)?;
            let i = crate::git::Inspection {
                cancellation: &permit.cancellation,
                safe: &permit.safe,
            };
            if let Some(m) = &job.worktree {
                recovery::verify_available(&permit.root, m, &i).await
                    .context("Original retained worktree is unavailable; no alternative directory will be used.")?;
            }
            let identity = i.identity(path).await?;
            anyhow::ensure!(
                identity.repository.path == path && identity.common_dir()? == common,
                "Original working directory identity changed; validation is unavailable."
            );
        }
        Ok(permit)
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

    pub async fn inspect_cleanup(
        &self,
        m: &WorktreeMetadata,
        cancellation: &Cancellation,
    ) -> crate::worktrees::cleanup::Inspection {
        let result = async {
            let permit = self
                .acquire(
                    (m.run, m.job),
                    super::schedule::Lease {
                        path: m.repository.path.clone(),
                        worktree: Some(m.path.clone()),
                        common_dir: Some(m.common_dir.clone()),
                        mode: crate::domain::ExecutionMode::Direct,
                    },
                    true,
                    Some(cancellation.clone()),
                )
                .await?;
            let _guard = permit.admin.mutex.lock().await;
            let _health = AdministrationHealth::protect(&permit.admin, &permit.safe)?;
            let i = crate::git::Inspection {
                cancellation: &permit.cancellation,
                safe: &permit.safe,
            };
            Ok::<_, anyhow::Error>(crate::worktrees::cleanup::inspect(&permit.root, m, &i).await)
        }
        .await;
        result.unwrap_or_else(|e| crate::worktrees::cleanup::Inspection::unknown(format!("{e:#}")))
    }

    pub(crate) async fn clean_managed(
        &self,
        m: &WorktreeMetadata,
        proof: &crate::worktrees::cleanup::Proof,
    ) -> Result<Report, crate::worktrees::cleanup::Failure> {
        use crate::worktrees::cleanup::Failure;
        let permit = self.permit(m, true).await.map_err(Failure::Blocked)?;
        let _guard = permit.admin.mutex.lock().await;
        let _health = AdministrationHealth::protect(&permit.admin, &permit.safe)
            .map_err(Failure::Uncertain)?;
        crate::worktrees::cleanup::remove(
            &permit.root,
            m,
            proof,
            &permit.cancellation,
            &permit.safe,
        )
        .await
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;
    use crate::domain::{ExecutionMode, JobStatus, Repository, ResultResolution};

    fn metadata() -> WorktreeMetadata {
        WorktreeMetadata {
            owner: "fixture".into(),
            run: 1,
            job: 0,
            repository: Repository {
                name: "source".into(),
                path: "source".into(),
            },
            common_dir: "source/.git".into(),
            path: "reserved/checkout".into(),
            base_commit: "fixed-base".into(),
            execution_mode: ExecutionMode::IsolatedWorktree,
        }
    }

    #[tokio::test]
    async fn worktree_binding_cancellation_releases_administration_without_activity_drain() {
        for shutdown in [false, true] {
            let (commands, mut requests) = mpsc::unbounded_channel();
            let client = LifecycleClient { commands };
            let cancellation = Cancellation::default();
            let stopping = AtomicBool::new(false);
            let admin = super::super::Administration::default();
            let m = metadata();
            let (events, mut activity) = mpsc::channel(1);
            let queued = || super::super::Event::Queued {
                run: 2,
                job: 0,
                reason: crate::domain::QueueReason::GlobalLimit,
            };
            events.send(queued()).await.unwrap();
            // The manager cannot consume requests while this send is pending.
            let notification = events.send(queued());
            tokio::pin!(notification);
            tokio::select! {
                biased;
                _ = &mut notification => panic!("Activity must be full"),
                _ = std::future::ready(()) => {}
            }
            let work = async {
                let _guard = admin.mutex.lock().await;
                client.bind_worktree(&m, &cancellation, &stopping).await
            };
            tokio::pin!(work);
            tokio::select! {
                biased;
                _ = &mut work => panic!("Binding must wait for the manager"),
                _ = std::future::ready(()) => {}
            }
            assert_eq!(requests.len(), 1);
            assert!(admin.mutex.try_lock().is_err());
            // Match RunManager::shutdown's ordering: flag, then job token.
            stopping.store(shutdown, Ordering::Release);
            cancellation.cancel();
            let result = tokio::time::timeout(std::time::Duration::from_secs(1), &mut work)
                .await
                .expect("cancellation must not need Activity to drain");
            assert!(
                result.is_err(),
                "cancelled binding must never authorize Git creation"
            );
            assert!(admin.mutex.try_lock().is_ok());
            assert_eq!(events.capacity(), 0);
            // A late queued acknowledgement cannot revive the abandoned wait.
            activity.recv().await.unwrap();
            notification.await.unwrap();
            let Some(Command::BindWorktree { reply, .. }) = requests.recv().await else {
                panic!("expected queued binding");
            };
            assert!(reply.is_closed());
            assert!(reply.send(Ok(())).is_err());
        }
    }

    #[tokio::test]
    async fn worktree_binding_rechecks_cancellation_and_stopping_at_acknowledgement() {
        for shutdown in [false, true] {
            for already_stopped in [false, true] {
                let (commands, mut requests) = mpsc::unbounded_channel();
                let client = LifecycleClient { commands };
                let cancellation = Cancellation::default();
                let stopping = AtomicBool::new(false);
                let stop = || {
                    if shutdown {
                        // Exercise the flag check before shutdown signals tokens.
                        stopping.store(true, Ordering::Release);
                    } else {
                        cancellation.cancel();
                    }
                };
                let m = metadata();
                let binding = client.bind_worktree(&m, &cancellation, &stopping);
                tokio::pin!(binding);
                if !already_stopped {
                    tokio::select! {
                        biased;
                        _ = &mut binding => panic!("binding needs a reply"),
                        _ = std::future::ready(()) => {}
                    }
                    let Some(Command::BindWorktree { reply, .. }) = requests.recv().await else {
                        panic!("expected queued binding");
                    };
                    reply.send(Ok(())).unwrap();
                }
                stop();
                assert!(binding.await.unwrap_err().to_string().contains("Cancelled"));
                assert!(requests.is_empty());
            }
        }
    }

    #[tokio::test]
    async fn validation_rejects_resolved_and_pending_results_before_acquiring_a_lease() {
        let (commands, mut requests) = mpsc::unbounded_channel();
        let client = LifecycleClient { commands };
        let mut job = Job::queued(Repository {
            name: "fixture".into(),
            path: PathBuf::from("unused-repository"),
        });
        job.status = JobStatus::Succeeded;
        job.execution_mode = ExecutionMode::IsolatedWorktree;
        for resolution in [
            ResultResolution::Applied,
            ResultResolution::ApplyPending,
            ResultResolution::DiscardPending,
            ResultResolution::Discarded,
        ] {
            job.resolution = resolution;
            let result = client
                .validation_permit(1, 0, &job, &Cancellation::default())
                .await;
            let Err(error) = result else {
                panic!("Validation must reject {resolution:?}");
            };
            let message = error.to_string();
            assert!(message.contains("unavailable"), "{resolution:?}: {message}");
            if resolution == ResultResolution::Applied {
                assert!(message.contains("Applied"));
            }
            assert!(matches!(
                requests.try_recv(),
                Err(mpsc::error::TryRecvError::Empty)
            ));
        }
    }
}
