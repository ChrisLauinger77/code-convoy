use crate::{
    agents::AgentBackend,
    domain::{self, GitSummary, Job, JobStatus, QueueReason, Repository, Run, TaskConfig},
    git::{self, WorkingTree},
    process::{self, Cancellation},
};
use anyhow::{Context, Result};
use std::sync::{
    Mutex,
    atomic::{AtomicBool, Ordering},
};
use tokio::sync::mpsc;
mod manager;
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
            created_at: domain::now(),
            task: self.task.clone(),
            jobs: self
                .repositories
                .iter()
                .map(|r| {
                    let mut job = Job::queued(r.repository.clone());
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
    for option in backend.options() {
        task.options
            .entry(option.key.to_owned())
            .or_insert_with(|| option.default.to_owned());
    }
    crate::agents::detect(backend.as_ref(), &task.options, &repositories[0].path).await?;
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
    Queued {
        run: u64,
        job: usize,
        reason: QueueReason,
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
    },
    Finished {
        run: u64,
        job: usize,
        status: JobStatus,
        exit_code: Option<i32>,
        detail: String,
    },
}
#[allow(clippy::too_many_arguments)]
async fn job_work(
    run: u64,
    job: usize,
    prepared: &PreparedRepository,
    task: &TaskConfig,
    backend: &dyn AgentBackend,
    cancellation: &Cancellation,
    stopping: &AtomicBool,
    repository_safe: &AtomicBool,
    tx: &mpsc::Sender<Event>,
) -> Result<(JobStatus, Option<i32>, String)> {
    let cancelled = || {
        (
            JobStatus::Cancelled,
            None,
            "Cancelled; any existing edits are retained.".into(),
        )
    };
    let state = tokio::select! {
        biased;
        _ = cancellation.cancelled() => return Ok(cancelled()),
        result = git::status(&prepared.repository.path) => result?,
    };
    anyhow::ensure!(
        state.summary == prepared.state.summary && state.entries == prepared.state.entries,
        "Repository state changed after preflight. Inspect the working tree and start a fresh run."
    );
    let spec = backend.build(task, &prepared.repository)?;
    if stopping.load(Ordering::Acquire) || cancellation.is_cancelled() {
        return Ok(cancelled());
    }
    let child = backend.spawn(&spec)?;
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
    let decoder = Mutex::new((backend.output(), 0usize));
    let result = process::execute(
        child,
        spec.input,
        cancellation,
        |child| backend.cancel(child),
        |stream, bytes| {
            if let Ok(mut data) = decoder.lock() {
                let text = data.0.push(stream, bytes);
                if !text.is_empty() {
                    let size = text.len();
                    if tx.try_send(Event::Output { run, job, text }).is_err() {
                        data.1 += size;
                    }
                }
            }
        },
    )
    .await?;
    repository_safe.store(true, Ordering::Release);
    let (tail, dropped, outcome) = {
        let mut data = decoder
            .lock()
            .map_err(|_| anyhow::anyhow!("Output decoder failed."))?;
        let tail = data.0.finish();
        let outcome = result.status.map(|status| data.0.interpret(status));
        (tail, data.1, outcome)
    };
    if !tail.is_empty() {
        let _ = tx
            .send(Event::Output {
                run,
                job,
                text: tail,
            })
            .await;
    }
    if dropped > 0 {
        let _ = tx
            .send(Event::Output {
                run,
                job,
                text: format!("\n[Output exceeded UI throughput: {dropped} bytes omitted.]\n"),
            })
            .await;
    }
    if result.cancelled {
        return Ok(cancelled());
    }
    let (status, detail) = outcome.context("Agent returned no exit status.")?;
    Ok((status, result.status.and_then(|s| s.code()), detail))
}
