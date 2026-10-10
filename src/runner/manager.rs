use super::{
    Administration, Event, PreparedRepository, PreparedRun, job_work,
    lifecycle::{LifecycleClient, Permit},
    schedule::{Key, Lease, Schedule},
};
use crate::{
    agents::AgentBackend,
    domain::{JobStatus, MAX_CONCURRENCY, QueueReason, TaskConfig},
    process::Cancellation,
};
use anyhow::{Context, Result};
use std::{
    collections::{BTreeMap, HashMap},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
use tokio::{
    sync::mpsc,
    task::{Id, JoinHandle, JoinSet},
};

type Outcome = (JobStatus, Option<i32>, String);
struct Control {
    cancellations: Vec<Cancellation>,
    done: Arc<AtomicBool>,
}
pub(super) struct Convoy {
    worktree_base: Option<std::path::PathBuf>,
    task: Arc<TaskConfig>,
    backend: Arc<dyn AgentBackend>,
    repositories: Vec<Option<PreparedRepository>>,
    cancellations: Vec<Cancellation>,
    done: Arc<AtomicBool>,
    remaining: usize,
}
pub(super) enum Command {
    Acquire {
        metadata: crate::domain::WorktreeMetadata,
        cleanup: bool,
        reply: tokio::sync::oneshot::Sender<Result<Permit>>,
        commands: mpsc::UnboundedSender<Command>,
    },
    Release(u64, bool),
    Start(u64, Convoy),
    Limit(usize),
    Wake,
    Shutdown,
}
/// Application-owned execution. Methods enqueue work or signal cancellation;
/// Git, processes, admission, and cleanup run entirely on the Tokio runtime.
pub struct RunManager {
    worktree_base: Option<std::path::PathBuf>,
    commands: mpsc::UnboundedSender<Command>,
    controls: BTreeMap<u64, Control>,
    pub join: JoinHandle<()>,
    closing: bool,
    stopping: Arc<AtomicBool>,
}
impl RunManager {
    pub fn new(global_limit: usize, events: mpsc::Sender<Event>) -> Self {
        Self::with_storage(
            global_limit,
            events,
            crate::persistence::data_directory()
                .ok()
                .map(|p| p.join("worktrees")),
        )
    }
    pub fn with_worktree_directory(
        global_limit: usize,
        events: mpsc::Sender<Event>,
        directory: std::path::PathBuf,
    ) -> Self {
        Self::with_storage(global_limit, events, Some(directory))
    }
    fn with_storage(
        global_limit: usize,
        events: mpsc::Sender<Event>,
        storage: Option<std::path::PathBuf>,
    ) -> Self {
        let storage = storage.map(|path| {
            // Store parents already exist; canonicalize platform aliases once,
            // before persisted result paths are inspected. Never create on recovery.
            path.parent()
                .and_then(|p| p.canonicalize().ok())
                .and_then(|p| path.file_name().map(|n| p.join(n)))
                .unwrap_or(path)
        });
        let (commands, rx) = mpsc::unbounded_channel();
        let stopping = Arc::new(AtomicBool::new(false));
        let join = tokio::spawn(manage(
            rx,
            events,
            global_limit.clamp(1, MAX_CONCURRENCY),
            stopping.clone(),
            storage,
        ));
        Self {
            worktree_base: None,
            commands,
            controls: BTreeMap::new(),
            join,
            closing: false,
            stopping,
        }
    }
    /// Changes only the base captured by subsequent convoy launches.
    pub fn set_worktree_base(&mut self, base: Option<std::path::PathBuf>) {
        self.worktree_base = base;
    }
    pub fn lifecycle(&self) -> LifecycleClient {
        LifecycleClient {
            commands: self.commands.clone(),
        }
    }
    pub fn start(
        &mut self,
        id: u64,
        prepared: PreparedRun,
        backend: Arc<dyn AgentBackend>,
    ) -> Result<()> {
        anyhow::ensure!(!self.closing, "The run manager is closing.");
        anyhow::ensure!(
            !self.controls.contains_key(&id),
            "Convoy #{id} already exists."
        );
        prepared.task.validate(
            &prepared
                .repositories
                .iter()
                .map(|r| r.repository.clone())
                .collect::<Vec<_>>(),
        )?;
        let cancellations: Vec<_> = prepared
            .repositories
            .iter()
            .map(|_| Cancellation::default())
            .collect();
        let done = Arc::new(AtomicBool::new(false));
        let convoy = Convoy {
            worktree_base: self.worktree_base.clone(),
            task: Arc::new(prepared.task),
            backend,
            remaining: prepared.repositories.len(),
            repositories: prepared.repositories.into_iter().map(Some).collect(),
            cancellations: cancellations.clone(),
            done: done.clone(),
        };
        self.commands
            .send(Command::Start(id, convoy))
            .context("The run manager stopped.")?;
        self.controls.insert(
            id,
            Control {
                cancellations,
                done,
            },
        );
        Ok(())
    }
    pub fn set_global_limit(&self, limit: usize) -> Result<()> {
        anyhow::ensure!(
            (1..=MAX_CONCURRENCY).contains(&limit),
            "Global concurrency must be between 1 and {MAX_CONCURRENCY}."
        );
        self.commands
            .send(Command::Limit(limit))
            .context("The run manager stopped.")
    }
    pub fn cancel(&self, run: u64, job: usize) {
        if let Some(token) = self
            .controls
            .get(&run)
            .and_then(|r| r.cancellations.get(job))
        {
            token.cancel();
        }
        let _ = self.commands.send(Command::Wake);
    }
    pub fn cancel_run(&self, run: u64) {
        if let Some(control) = self.controls.get(&run) {
            for token in &control.cancellations {
                token.cancel();
            }
        }
        let _ = self.commands.send(Command::Wake);
    }
    pub fn cancel_all(&self) {
        for id in self.controls.keys() {
            self.cancel_run(*id);
        }
    }
    pub fn is_active(&self, run: u64) -> bool {
        self.controls
            .get(&run)
            .is_some_and(|r| !r.done.load(Ordering::Acquire))
    }
    pub fn active_count(&self) -> usize {
        self.controls
            .values()
            .filter(|r| !r.done.load(Ordering::Acquire))
            .count()
    }
    pub fn is_idle(&self) -> bool {
        self.active_count() == 0
    }
    pub fn reap(&mut self) {
        self.controls.retain(|_, r| !r.done.load(Ordering::Acquire));
    }
    pub fn shutdown(&mut self) {
        if self.closing {
            return;
        }
        self.closing = true;
        // Also visible while the manager is awaiting delivery of a lifecycle
        // event; admission must not depend on when it reads Command::Shutdown.
        self.stopping.store(true, Ordering::Release);
        self.cancel_all();
        let _ = self.commands.send(Command::Shutdown);
    }
}
impl Drop for RunManager {
    fn drop(&mut self) {
        self.shutdown();
    }
}

#[derive(Default)]
struct Operations {
    next: u64,
    active: BTreeMap<u64, Cancellation>,
    administration: HashMap<std::path::PathBuf, Arc<Administration>>,
    storage: Option<std::path::PathBuf>,
}
fn command(
    command: Command,
    runs: &mut BTreeMap<u64, Convoy>,
    schedule: &mut Schedule,
    closing: &mut bool,
    operations: &mut Operations,
) {
    match command {
        Command::Acquire {
            metadata,
            cleanup,
            reply,
            commands,
        } => {
            let result = (|| -> Result<Permit> {
                anyhow::ensure!(
                    !*closing,
                    "The application is closing; result was preserved."
                );
                let root = operations
                    .storage
                    .clone()
                    .context("Worktree storage is unavailable.")?;
                let id = operations.next;
                operations.next = id
                    .checked_add(1)
                    .context("Result operation IDs exhausted.")?;
                let lease = Lease {
                    path: metadata.repository.path.clone(),
                    common_dir: Some(metadata.common_dir.clone()),
                    mode: crate::domain::ExecutionMode::Direct,
                };
                anyhow::ensure!(
                    schedule.begin_maintenance(id, (metadata.run, metadata.job), lease, cleanup),
                    "Repository is in use by preparation, execution or another result operation. Wait for it to finish; result was preserved."
                );
                let cancellation = Cancellation::default();
                operations.active.insert(id, cancellation.clone());
                let admin = operations
                    .administration
                    .entry(metadata.common_dir)
                    .or_default()
                    .clone();
                Ok(Permit {
                    id,
                    commands,
                    admin,
                    cancellation,
                    safe: Arc::new(AtomicBool::new(true)),
                    root,
                })
            })();
            // If the caller disappeared, dropping its permit releases the lease.
            let _ = reply.send(result);
        }
        Command::Release(id, safe) => {
            operations.active.remove(&id);
            schedule.end_maintenance(id, safe);
        }
        Command::Start(id, convoy) => {
            schedule.insert(
                id,
                convoy.task.concurrency,
                convoy.repositories.iter().filter_map(|r| {
                    r.as_ref().map(|r| Lease {
                        path: r.repository.path.clone(),
                        common_dir: r.state.summary.common_dir.clone(),
                        mode: convoy.task.execution_mode,
                    })
                }),
            );
            runs.insert(id, convoy);
        }
        Command::Limit(limit) => schedule.limit = limit,
        Command::Wake => {}
        Command::Shutdown => {
            *closing = true;
            for token in operations.active.values() {
                token.cancel();
            }
            for convoy in runs.values() {
                for token in &convoy.cancellations {
                    token.cancel();
                }
            }
        }
    }
}
async fn finish(
    events: &mpsc::Sender<Event>,
    runs: &mut BTreeMap<u64, Convoy>,
    key: Key,
    outcome: Outcome,
) {
    let (status, exit_code, detail) = outcome;
    let _ = events
        .send(Event::Finished {
            run: key.0,
            job: key.1,
            status,
            exit_code,
            detail,
        })
        .await;
    if let Some(convoy) = runs.get_mut(&key.0) {
        convoy.remaining -= 1;
        if convoy.remaining == 0 {
            convoy.done.store(true, Ordering::Release);
            runs.remove(&key.0);
        }
    }
}
async fn manage(
    mut commands: mpsc::UnboundedReceiver<Command>,
    events: mpsc::Sender<Event>,
    limit: usize,
    stopping: Arc<AtomicBool>,
    storage: Option<std::path::PathBuf>,
) {
    let mut schedule = Schedule::new(limit);
    let mut runs: BTreeMap<u64, Convoy> = BTreeMap::new();
    let mut workers = JoinSet::new();
    let mut keys: HashMap<Id, (Key, Arc<AtomicBool>)> = HashMap::new();
    let mut reasons = HashMap::new();
    let mut operations = Operations {
        storage: storage.clone(),
        ..Operations::default()
    };
    let mut closing = false;
    loop {
        // Batch arrivals before admission, preserving each convoy's round-robin turn.
        while let Ok(c) = commands.try_recv() {
            command(c, &mut runs, &mut schedule, &mut closing, &mut operations);
        }
        let cancelled: Vec<_> = schedule
            .waiting()
            .into_iter()
            .map(|(key, _)| key)
            .filter(|(run, job)| {
                stopping.load(Ordering::Acquire) || runs[run].cancellations[*job].is_cancelled()
            })
            .collect();
        for key in cancelled {
            schedule.cancel_pending(key);
            reasons.remove(&key);
            finish(
                &events,
                &mut runs,
                key,
                (
                    JobStatus::Cancelled,
                    None,
                    "Cancelled before execution; repository untouched.".into(),
                ),
            )
            .await;
        }
        while !closing && !stopping.load(Ordering::Acquire) {
            let Some(key) = schedule.next() else { break };
            let convoy = runs.get_mut(&key.0).expect("scheduled convoy exists");
            let repository = convoy.repositories[key.1]
                .take()
                .expect("job admitted once");
            let task = convoy.task.clone();
            let backend = convoy.backend.clone();
            let cancellation = convoy.cancellations[key.1].clone();
            let safe = Arc::new(AtomicBool::new(true));
            let worker_safe = safe.clone();
            reasons.remove(&key);
            let _ = events
                .send(Event::Queued {
                    run: key.0,
                    job: key.1,
                    reason: QueueReason::CheckingRepository,
                })
                .await;
            if stopping.load(Ordering::Acquire) || cancellation.is_cancelled() {
                finish(
                    &events,
                    &mut runs,
                    key,
                    (
                        JobStatus::Cancelled,
                        None,
                        "Cancelled before execution; repository untouched.".into(),
                    ),
                )
                .await;
                schedule.finish(key, true);
                continue;
            }
            let tx = events.clone();
            let worker_stopping = stopping.clone();
            let storage = storage.clone();
            let worktree_base = convoy.worktree_base.clone();
            let admin = operations
                .administration
                .entry(
                    repository
                        .state
                        .summary
                        .common_dir
                        .clone()
                        .unwrap_or_else(|| repository.repository.path.clone()),
                )
                .or_default()
                .clone();
            let handle = workers.spawn(async move {
                job_work(
                    key.0,
                    key.1,
                    &repository,
                    &task,
                    &backend,
                    &cancellation,
                    &worker_stopping,
                    &worker_safe,
                    &tx,
                    storage,
                    worktree_base,
                    admin,
                )
                .await
            });
            keys.insert(handle.id(), (key, safe));
        }
        for (key, reason) in schedule.waiting() {
            if reasons.insert(key, reason) != Some(reason) {
                let _ = events
                    .send(Event::Queued {
                        run: key.0,
                        job: key.1,
                        reason,
                    })
                    .await;
            }
        }
        if closing && runs.is_empty() && operations.active.is_empty() {
            break;
        }
        tokio::select! {
            // Ready control messages take precedence over admitting replacement jobs.
            biased;
            c = commands.recv(), if !commands.is_closed() => {
                command(c.unwrap_or(Command::Shutdown), &mut runs, &mut schedule, &mut closing, &mut operations);
            }
            joined = workers.join_next_with_id(), if !workers.is_empty() => {
                let (id, result) = match joined.expect("workers are nonempty") {
                    Ok((id, result)) => (id, result),
                    Err(error) => (error.id(), Err(anyhow::anyhow!("Job worker stopped unexpectedly: {error}"))),
                };
                if let Some((key, safe)) = keys.remove(&id) {
                    let repository_safe = safe.load(Ordering::Acquire);
                    let mut outcome = result.unwrap_or_else(|e| (JobStatus::Failed, None, format!("{e:#}")));
                    if !repository_safe {
                        outcome.2.push_str(" Repository access and job capacity remain reserved because process cleanup could not be confirmed; inspect remaining processes before restarting CodeConvoy.");
                    }
                    // Notify completion before allowing a replacement start, so
                    // UI/tests observe causal lifecycle order across all convoys.
                    finish(&events, &mut runs, key, outcome).await;
                    schedule.finish(key, repository_safe);
                }
            }
        }
    }
}
