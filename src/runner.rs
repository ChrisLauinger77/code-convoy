use crate::{
    agents::AgentBackend,
    domain::{
        self, ExecutionMode, GitSummary, Job, JobStatus, QueueReason, Repository, Run, TaskConfig,
        WorktreeMetadata, WorktreeResult,
    },
    git::{self, WorkingTree},
    process::{self, Cancellation},
};
use anyhow::{Context, Result};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use tokio::sync::mpsc;
mod lifecycle;
mod manager;
pub use lifecycle::LifecycleClient;
mod raw_output;
mod schedule;
pub use manager::RunManager;

#[derive(Clone)]
pub struct PreparedRepository {
    pub repository: Repository,
    pub state: WorkingTree,
}
#[derive(Clone)]
pub struct PreparedRun {
    pub task: TaskConfig,
    pub repositories: Vec<PreparedRepository>,
}
impl PreparedRun {
    pub fn snapshot(&self, id: u64) -> Run {
        Run {
            id,
            provenance: None,
            created_at: domain::now(),
            task: self.task.clone(),
            jobs: self
                .repositories
                .iter()
                .map(|r| {
                    let mut job = Job::queued(r.repository.clone());
                    job.execution_mode = self.task.execution_mode;
                    job.before = Some(r.state.summary.clone());
                    job
                })
                .collect(),
        }
    }

    pub fn dirty(&self) -> bool {
        self.repositories
            .iter()
            .any(|r| r.state.summary.changed > 0)
    }
}
pub async fn prepare(mut task: TaskConfig, repositories: Vec<Repository>) -> Result<PreparedRun> {
    task.validate(&repositories)?;
    let backend = crate::agents::backend(task.agent)?;
    backend.validate_attachments(&task)?;
    let attachments = task.attachments.clone();
    tokio::task::spawn_blocking(move || crate::attachments::revalidate(&attachments))
        .await
        .context("Attachment validation worker failed.")??;
    for option in backend.options() {
        task.options
            .entry(option.key.to_owned())
            .or_insert_with(|| option.default.to_owned());
    }
    crate::agents::detect(backend.as_ref(), &task.options, &repositories[0].path).await?;
    crate::agents::check_attachment_interface(backend.as_ref(), &task, &repositories[0].path)
        .await?;
    let mut prepared = Vec::new();
    for repository in repositories {
        let state = git::status(&repository.path).await?;
        prepared.push(PreparedRepository { repository, state });
    }
    Ok(PreparedRun {
        task,
        repositories: prepared,
    })
}
#[derive(Debug)]
pub enum Event {
    Changes {
        run: u64,
        job: usize,
        changes: crate::visibility::Changes,
    },
    Queued {
        run: u64,
        job: usize,
        reason: QueueReason,
    },
    Preparing {
        run: u64,
        job: usize,
        worktree: Option<WorktreeMetadata>,
    },
    Result {
        run: u64,
        job: usize,
        result: Option<WorktreeResult>,
        detail: String,
    },
    Started {
        run: u64,
        job: usize,
        before: GitSummary,
    },
    Output {
        run: u64,
        job: usize,
        text: String,
        raw: String,
    },
    Finished {
        run: u64,
        job: usize,
        status: JobStatus,
        exit_code: Option<i32>,
        detail: String,
    },
}
pub(super) struct Administration {
    mutex: tokio::sync::Mutex<()>,
    healthy: AtomicBool,
}
impl Default for Administration {
    fn default() -> Self {
        Self {
            mutex: tokio::sync::Mutex::new(()),
            healthy: AtomicBool::new(true),
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn job_work(
    run: u64,
    job: usize,
    prepared: &PreparedRepository,
    task: &TaskConfig,
    backend: &Arc<dyn AgentBackend>,
    cancellation: &Cancellation,
    stopping: &AtomicBool,
    repository_safe: &AtomicBool,
    tx: &mpsc::Sender<Event>,
    storage: Option<std::path::PathBuf>,
    worktree_base: Option<std::path::PathBuf>,
    administration: Arc<Administration>,
) -> Result<(JobStatus, Option<i32>, String)> {
    let mut launched = false;
    if task.execution_mode == ExecutionMode::Direct {
        let outcome = run_agent(
            run,
            job,
            prepared,
            task,
            backend,
            cancellation,
            stopping,
            repository_safe,
            tx,
            &mut launched,
        )
        .await;
        if launched && repository_safe.load(Ordering::Acquire) {
            // Keep the execution lease until bounded inspection/cleanup finishes.
            // Stop has already cancelled the agent token, so use a fresh token.
            let token = Cancellation::default();
            let inspection = git::Inspection {
                cancellation: &token,
                safe: repository_safe,
            };
            let observation = inspection.completion_changes(
                &prepared.repository.path,
                prepared.state.summary.head.as_deref(),
            );
            tokio::pin!(observation);
            let changes = tokio::select! {
                result = &mut observation => result.ok(),
                _ = tokio::time::sleep(std::time::Duration::from_secs(5)) => {
                    token.cancel();
                    let _ = observation.await;
                    None
                }
            }
            .unwrap_or_default();
            let _ = tx.send(Event::Changes { run, job, changes }).await;
        }
        return outcome;
    }
    let mut retained = None;
    let mut ready = false;
    let outcome = async {
        tx.send(Event::Preparing { run, job, worktree: None }).await.context("Application closed.")?;
        lifecycle(tx, run, job, "Preparing isolated worktree").await;
        let guard = tokio::select! {
            biased;
            _ = cancellation.cancelled() => anyhow::bail!("Cancelled during worktree preparation."),
            guard = administration.mutex.lock() => guard,
        };
        anyhow::ensure!(administration.healthy.load(Ordering::Acquire), "Git administration is blocked after unconfirmed process cleanup. Inspect processes before restarting CodeConvoy.");
        anyhow::ensure!(!cancellation.is_cancelled(), "Cancelled during worktree preparation.");
        let root = storage.context("Cannot locate worktree storage. Check the application data directory.")?;
        let repo = prepared.repository.clone();
        let common = prepared.state.summary.common_dir.clone().context("Missing Git repository identity; run preflight again.")?;
        let base = prepared.state.summary.head.clone().context("Isolated execution requires a committed HEAD. Create an initial commit yourself, then start a fresh convoy.")?;
        // Await this small filesystem worker even on cancellation, so its ownership
        // record can always be delivered before the job becomes terminal.
        let crate::worktrees::Reservation { metadata, binding } = tokio::task::spawn_blocking(move || crate::worktrees::reserve_at(&root, worktree_base.as_deref(), run, job, repo, common, base)).await??;
        retained = Some(metadata.clone());
        tx.send(Event::Preparing { run, job, worktree: Some(metadata.clone()) }).await.context("Application closed.")?;
        // A failed Store binding must still pin the reserved attempt in history.
        // It grants no creation or cleanup authority, even if the checkout is absent.
        binding?;
        administration.healthy.store(false, Ordering::Release);
        let creation = crate::worktrees::create(&metadata, cancellation, repository_safe).await;
        administration.healthy.store(repository_safe.load(Ordering::Acquire), Ordering::Release);
        let state = creation?;
        drop(guard);
        anyhow::ensure!(!cancellation.is_cancelled(), "Cancelled during worktree preparation.");
        ready = true;
        lifecycle(tx, run, job, "Worktree ready").await;
        let execution = PreparedRepository {
            repository: Repository { path: metadata.path.clone(), name: prepared.repository.name.clone() }, state,
        };
        run_agent(run, job, &execution, task, backend, cancellation, stopping, repository_safe, tx, &mut launched).await
    }.await;
    let cancelled = cancellation.is_cancelled() && repository_safe.load(Ordering::Acquire);
    let message = if cancelled {
        if ready {
            "Agent cancelled"
        } else {
            "Preparation cancelled"
        }
    } else if !ready {
        "Preparation failed"
    } else if !launched {
        "Agent could not start"
    } else {
        match &outcome {
            Ok((JobStatus::Succeeded, ..)) => "Agent completed",
            Ok((JobStatus::Cancelled, ..)) => "Agent cancelled",
            _ => "Agent failed",
        }
    };
    lifecycle(tx, run, job, message).await;
    if let Some(metadata) = retained {
        // Inspection needs its own token after Stop, but must not hold shutdown
        // indefinitely. Cancel and await the inspection instead of dropping it.
        let inspection_cancel = Cancellation::default();
        let inspection =
            crate::worktrees::inspect_owned(&metadata, &inspection_cancel, repository_safe);
        tokio::pin!(inspection);
        let observation = tokio::select! {
            result = &mut inspection => result,
            _ = tokio::time::sleep(std::time::Duration::from_secs(5)) => {
                inspection_cancel.cancel();
                let _ = inspection.await;
                Err(anyhow::anyhow!("Result inspection timed out. Worktree and ownership metadata are retained; inspect the result before using it."))
            }
        };
        let (result, detail) = match observation {
            Ok(result) => (Some(result), String::new()),
            Err(e) => (
                None,
                format!("Retained worktree inspection unavailable: {e:#}"),
            ),
        };
        let message = match result.as_ref() {
            Some(WorktreeResult {
                exists: true,
                changed: Some(true),
                ..
            }) => "Isolated changes retained",
            Some(WorktreeResult {
                exists: true,
                changed: Some(false),
                ..
            }) => "No repository changes; isolated worktree retained",
            Some(WorktreeResult { exists: false, .. }) => {
                "No isolated checkout found; ownership metadata retained"
            }
            _ => "Isolated result changes unknown; ownership metadata retained",
        };
        lifecycle(tx, run, job, message).await;
        let _ = tx
            .send(Event::Result {
                run,
                job,
                result,
                detail,
            })
            .await;
    }
    if cancelled && repository_safe.load(Ordering::Acquire) {
        Ok((JobStatus::Cancelled, None, if ready {
            "Cancelled; any isolated changes are retained. Registered working tree untouched."
        } else {
            "Git worktree preparation was cancelled. Any partial state is retained; agent was not started."
        }.into()))
    } else {
        outcome
    }
}

async fn lifecycle(tx: &mpsc::Sender<Event>, run: u64, job: usize, message: &str) {
    let _ = tx
        .send(Event::Output {
            run,
            job,
            text: format!("\n[CodeConvoy] {message}\n"),
            raw: String::new(),
        })
        .await;
}

#[allow(clippy::too_many_arguments)]
async fn run_agent(
    run: u64,
    job: usize,
    prepared: &PreparedRepository,
    task: &TaskConfig,
    backend: &Arc<dyn AgentBackend>,
    cancellation: &Cancellation,
    stopping: &AtomicBool,
    repository_safe: &AtomicBool,
    tx: &mpsc::Sender<Event>,
    launched: &mut bool,
) -> Result<(JobStatus, Option<i32>, String)> {
    let cancelled = || {
        (
            JobStatus::Cancelled,
            None,
            "Cancelled; any existing edits are retained.".into(),
        )
    };
    let builder = backend.clone();
    let task_snapshot = task.clone();
    let repository = prepared.repository.clone();
    let spec = tokio::select! {
        biased;
        _ = cancellation.cancelled() => return Ok(cancelled()),
        result = tokio::task::spawn_blocking(move || {
            builder.validate_attachments(&task_snapshot)?;
            crate::attachments::revalidate(&task_snapshot.attachments)?;
            builder.build(&task_snapshot, &repository)
        }) => result.context("Command preparation worker failed.")??,
    };
    let state = match (git::Inspection {
        cancellation,
        safe: repository_safe,
    })
    .status(&prepared.repository.path)
    .await
    {
        Ok(state) => state,
        Err(_) if cancellation.is_cancelled() && repository_safe.load(Ordering::Acquire) => {
            return Ok(cancelled());
        }
        Err(error) => return Err(error),
    };
    anyhow::ensure!(
        state.summary == prepared.state.summary && state.entries == prepared.state.entries,
        "Repository state changed after preflight. Inspect the working tree and start a fresh run."
    );
    if stopping.load(Ordering::Acquire) || cancellation.is_cancelled() {
        return Ok(cancelled());
    }
    let child = backend.spawn(&spec)?;
    *launched = true;
    repository_safe.store(false, Ordering::Release);
    tokio::select! {
        biased;
        _ = cancellation.cancelled() => {},
        result = tx.send(Event::Started {
        run,
        job,
        before: state.summary,
        }) => { result.context("Application closed.")?; }
    }
    if task.execution_mode == ExecutionMode::IsolatedWorktree {
        lifecycle(tx, run, job, "Running agent in isolated worktree").await;
    }
    let decoder = Mutex::new((backend.output(), 0usize, raw_output::RawOutput::default()));
    let result = process::execute(
        child,
        spec.input,
        cancellation,
        |child| backend.cancel(child),
        |stream, bytes| {
            if let Ok(mut data) = decoder.lock() {
                let text = data.0.push(stream, bytes);
                let text = data.0.take_activity().unwrap_or(text);
                let raw = data.2.push(stream, bytes);
                if !text.is_empty() || !raw.is_empty() {
                    let size = text.len() + raw.len();
                    if tx
                        .try_send(Event::Output {
                            run,
                            job,
                            text,
                            raw,
                        })
                        .is_err()
                    {
                        data.1 += size;
                    }
                }
            }
        },
    )
    .await?;
    repository_safe.store(true, Ordering::Release);
    let (tail, raw_tail, dropped, outcome) = {
        let mut data = decoder
            .lock()
            .map_err(|_| anyhow::anyhow!("Output decoder failed."))?;
        let tail = data.0.finish();
        let tail = data.0.take_activity().unwrap_or(tail);
        let raw_tail = data.2.finish();
        let outcome = result.status.map(|status| data.0.interpret(status));
        (tail, raw_tail, data.1, outcome)
    };
    if !tail.is_empty() || !raw_tail.is_empty() {
        let _ = tx
            .send(Event::Output {
                run,
                job,
                text: tail,
                raw: raw_tail,
            })
            .await;
    }
    if dropped > 0 {
        let _ = tx
            .send(Event::Output {
                run,
                job,
                text: format!("\n[Output exceeded UI throughput: {dropped} bytes omitted.]\n"),
                raw: format!("\n[Output exceeded UI throughput: {dropped} bytes omitted.]\n"),
            })
            .await;
    }
    if result.cancelled {
        return Ok(cancelled());
    }
    let (status, detail) = outcome.context("Agent returned no exit status.")?;
    Ok((status, result.status.and_then(|s| s.code()), detail))
}
