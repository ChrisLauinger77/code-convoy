//! Completion policy and session-only delivery; independent of scheduler and UI.
mod native;
#[cfg(test)]
mod tests;

use crate::domain::{ExecutionMode, JobStatus, ResultResolution, Run};
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, future::Future, pin::Pin, sync::Arc};
use tokio::{runtime::Handle, task::JoinSet};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Preferences {
    pub enabled: bool,
    pub success: bool,
    pub failure: bool,
    pub cancellation: bool,
    pub review: bool,
    pub suppress_foreground: bool,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            enabled: true,
            success: true,
            failure: true,
            cancellation: false,
            review: true,
            suppress_foreground: true,
        }
    }
}
impl Preferences {
    pub fn allows(&self, outcome: Outcome, focused: bool) -> bool {
        self.enabled
            && match outcome {
                Outcome::Failure => self.failure,
                Outcome::Cancellation => self.cancellation,
                Outcome::Review => self.review,
                Outcome::Success => self.success,
            }
            && !(focused && self.suppress_foreground && outcome != Outcome::Failure)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Failure,
    Cancellation,
    Review,
    Success,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Completion {
    pub run: u64,
    pub outcome: Outcome,
    pub title: String,
    pub body: String,
}
impl Completion {
    /// Wait for terminal execution and, for success, the existing Git observations.
    /// Unknown results require attention, never imply an unchanged working tree.
    pub fn classify(run: &Run) -> Option<Self> {
        if run.active() || run.jobs.is_empty() {
            return None;
        }
        let mut success = 0;
        let mut failure = 0;
        let mut cancelled = 0;
        let mut changed = 0;
        let mut unknown = 0;
        let mut pending = false;
        for job in &run.jobs {
            match job.status {
                JobStatus::Succeeded => success += 1,
                JobStatus::Failed => failure += 1,
                JobStatus::Cancelled => cancelled += 1,
                _ => return None,
            }
            if matches!(
                job.resolution,
                ResultResolution::Applied | ResultResolution::Discarded
            ) {
                continue;
            }
            if job.execution_mode == ExecutionMode::IsolatedWorktree
                && let Some(result) = &job.worktree_result
                && result.observed_this_session
                && result.exists
                && let Some(dirty) = result.changed
            {
                changed += usize::from(dirty);
                continue;
            }
            match &job.review {
                Some(Ok(stats)) => {
                    changed += usize::from(stats.files > 0 || stats.index_alternatives > 0)
                }
                Some(Err(_)) => unknown += 1,
                None => pending = true,
            }
        }
        let outcome = if failure > 0 {
            Outcome::Failure
        } else if cancelled > 0 {
            Outcome::Cancellation
        } else if pending {
            return None;
        } else if changed > 0 || unknown > 0 {
            Outcome::Review
        } else {
            Outcome::Success
        };
        let verb = match outcome {
            Outcome::Failure => "finished with failures",
            Outcome::Cancellation => "cancelled",
            Outcome::Review | Outcome::Success => "completed",
        };
        let mut body = format!("{success} successful, {failure} failed, {cancelled} cancelled");
        if changed > 0 {
            body.push_str(&format!(" — changes need review in {changed} repositories"));
        }
        if unknown > 0 {
            body.push_str(&format!(
                " — {unknown} results could not be checked; review needed"
            ));
        }
        Some(Self {
            run: run.id,
            outcome,
            title: format!("Convoy #{} {verb}", run.id),
            body,
        })
    }
}

#[derive(Default)]
pub struct CompletionTracker {
    // Only launch attempts saved in this process enter here. Never seed from history.
    live: HashSet<u64>,
}
impl CompletionTracker {
    pub fn launched(&mut self, run: u64) {
        self.live.insert(run);
    }
    pub fn completed(&mut self, run: &Run) -> Option<Run> {
        if run.active() || !self.live.remove(&run.id) {
            return None;
        }
        // A small owned snapshot survives history trimming/removal while Git is
        // observed. Never copy prompts, attachments, output or ownership records.
        Some(Run {
            id: run.id,
            provenance: None,
            created_at: run.created_at,
            task: Default::default(),
            jobs: run
                .jobs
                .iter()
                .map(|job| {
                    let mut snapshot = crate::domain::Job::queued(job.repository.clone());
                    snapshot.status = job.status;
                    snapshot.execution_mode = job.execution_mode;
                    snapshot.resolution = job.resolution;
                    snapshot.worktree_result = job.worktree_result.clone();
                    snapshot.review = job.review.clone();
                    snapshot
                })
                .collect(),
        })
    }
    pub fn retain(&mut self, runs: &[Run]) {
        self.live.retain(|id| runs.iter().any(|run| run.id == *id));
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Ready(Completion),
    Activated(u64),
    Failed(u64, String),
}

#[derive(Clone)]
pub struct Events(Arc<dyn Fn(Event) + Send + Sync>);
impl Events {
    pub fn new(callback: impl Fn(Event) + Send + Sync + 'static) -> Self {
        Self(Arc::new(callback))
    }
    pub fn emit(&self, event: Event) {
        (self.0)(event);
    }
}

pub type Delivery = Pin<Box<dyn Future<Output = Result<(), String>> + Send>>;
pub trait Backend: Send + Sync {
    /// May observe interaction for as long as the native notification is alive.
    /// Must be cancellable and must not block the caller or a Tokio runtime worker.
    fn deliver(&self, completion: Completion, events: Events) -> Delivery;
}

pub struct Service {
    backend: Arc<dyn Backend>,
    events: Events,
    workers: JoinSet<()>,
}
impl Service {
    pub fn native(events: Events) -> Self {
        Self::new(Arc::new(native::Native), events)
    }
    pub fn new(backend: Arc<dyn Backend>, events: Events) -> Self {
        Self {
            backend,
            events,
            workers: JoinSet::new(),
        }
    }
    pub fn assess(&mut self, runtime: &Handle, mut run: Run) {
        self.reap();
        let events = self.events.clone();
        self.workers.spawn_on(
            async move {
                // Failures/cancellation and already observed isolated results are ready
                // immediately. Direct results need only a read-only Git status, not a
                // second full numstat/diff calculation. These are current observations.
                if Completion::classify(&run).is_none() {
                    for job in &mut run.jobs {
                        if job.review.is_none() {
                            job.review = Some(if job.execution_mode == ExecutionMode::Direct {
                                crate::git::status(&job.repository.path)
                                    .await
                                    .map(|state| crate::review::Statistics {
                                        files: state.summary.changed,
                                        ..Default::default()
                                    })
                                    .map_err(|e| e.to_string())
                            } else {
                                Err("Isolated result observation is unavailable.".into())
                            });
                        }
                    }
                }
                if let Some(completion) = Completion::classify(&run) {
                    events.emit(Event::Ready(completion));
                }
            },
            runtime,
        );
    }
    pub fn submit(&mut self, runtime: &Handle, completion: Completion) {
        self.reap();
        let backend = self.backend.clone();
        let events = self.events.clone();
        self.workers.spawn_on(
            async move {
                let run = completion.run;
                // A separate task boundary also contains native library panics.
                let result = tokio::spawn(backend.deliver(completion, events.clone()));
                let mut result = AbortOnDrop(result);
                match (&mut result.0).await {
                    Ok(Ok(())) => {}
                    Ok(Err(error)) => events.emit(Event::Failed(run, error)),
                    Err(error) => events.emit(Event::Failed(
                        run,
                        format!("Notification worker failed: {error}"),
                    )),
                }
            },
            runtime,
        );
    }
    pub fn reap(&mut self) {
        while self.workers.try_join_next().is_some() {}
    }
    pub fn stop(&mut self) {
        self.workers.abort_all();
    }
}
struct AbortOnDrop<T>(tokio::task::JoinHandle<T>);
impl<T> Drop for AbortOnDrop<T> {
    fn drop(&mut self) {
        self.0.abort();
    }
}
