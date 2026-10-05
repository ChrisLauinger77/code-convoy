use crate::{
    agents::AgentBackend,
    domain::{GitSummary, JobStatus, Repository, TaskConfig},
    git::{self, WorkingTree},
    process::{self, Cancellation},
};
use anyhow::{Context, Result};
use std::sync::{Arc, Mutex};
use tokio::{
    sync::{Semaphore, mpsc},
    task::JoinHandle,
};

pub struct PreparedRepository {
    pub repository: Repository,
    pub state: WorkingTree,
}
pub struct PreparedRun {
    pub task: TaskConfig,
    pub repositories: Vec<PreparedRepository>,
}
impl PreparedRun {
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
pub struct RunHandle {
    cancellations: Vec<Cancellation>,
    pub join: JoinHandle<()>,
}
impl RunHandle {
    pub fn cancel(&self, job: usize) {
        if let Some(c) = self.cancellations.get(job) {
            c.cancel();
        }
    }
    pub fn cancel_all(&self) {
        for c in &self.cancellations {
            c.cancel();
        }
    }
}
impl Drop for RunHandle {
    fn drop(&mut self) {
        self.cancel_all();
    }
}

pub fn start(
    run: u64,
    prepared: PreparedRun,
    backend: Arc<dyn AgentBackend>,
    tx: mpsc::Sender<Event>,
) -> RunHandle {
    let semaphore = Arc::new(Semaphore::new(prepared.task.concurrency));
    let task = Arc::new(prepared.task);
    let mut cancellations = Vec::new();
    let mut workers = Vec::new();
    for (job, repository) in prepared.repositories.into_iter().enumerate() {
        let cancellation = Cancellation::default();
        cancellations.push(cancellation.clone());
        let (semaphore, task, backend, tx) =
            (semaphore.clone(), task.clone(), backend.clone(), tx.clone());
        workers.push((
            job,
            tokio::spawn(async move {
                let result = job_work(
                    run,
                    job,
                    &repository,
                    task.as_ref(),
                    backend.as_ref(),
                    &cancellation,
                    semaphore,
                    &tx,
                )
                .await;
                let (status, exit_code, detail) = match result {
                    Ok(result) => result,
                    Err(error) => (JobStatus::Failed, None, format!("{error:#}")),
                };
                let _ = tx
                    .send(Event::Finished {
                        run,
                        job,
                        status,
                        exit_code,
                        detail,
                    })
                    .await;
            }),
        ));
    }
    let join = tokio::spawn(async move {
        for (job, worker) in workers {
            if let Err(error) = worker.await {
                let _ = tx
                    .send(Event::Finished {
                        run,
                        job,
                        status: JobStatus::Failed,
                        exit_code: None,
                        detail: format!("Job worker stopped unexpectedly: {error}"),
                    })
                    .await;
            }
        }
    });
    RunHandle {
        cancellations,
        join,
    }
}
#[allow(clippy::too_many_arguments)]
async fn job_work(
    run: u64,
    job: usize,
    prepared: &PreparedRepository,
    task: &TaskConfig,
    backend: &dyn AgentBackend,
    cancellation: &Cancellation,
    semaphore: Arc<Semaphore>,
    tx: &mpsc::Sender<Event>,
) -> Result<(JobStatus, Option<i32>, String)> {
    let cancelled = || {
        (
            JobStatus::Cancelled,
            None,
            "Cancelled; any existing edits are retained.".into(),
        )
    };
    let _permit = tokio::select! {
        biased;
        _ = cancellation.cancelled() => return Ok(cancelled()),
        permit = semaphore.acquire_owned() => permit.context("Execution queue closed.")?,
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
    if cancellation.is_cancelled() {
        return Ok(cancelled());
    }
    let spec = backend.build(task, &prepared.repository)?;
    let child = backend.spawn(&spec)?;
    tx.send(Event::Started {
        run,
        job,
        before: state.summary,
    })
    .await
    .context("Application closed.")?;
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
