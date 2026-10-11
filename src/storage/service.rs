//! One worker and one coalesced refresh. A cancelled worker must return before
//! another starts; generations reject stale results, including late failures.
use super::{Request, Snapshot, Status};
use crate::process::Cancellation;

pub struct Work {
    pub generation: u64,
    pub request: Request,
    pub cancel: Cancellation,
}
#[derive(Default)]
pub struct Service {
    generation: u64,
    active: Option<(u64, Cancellation)>,
    pending: Option<Request>,
    pub snapshot: Option<Snapshot>,
    pub status: Status,
    pub error: Option<String>,
    pub history_success: Option<u64>,
    pub worktrees_success: Option<u64>,
}
impl Service {
    pub fn refresh(&mut self, request: Request) -> Option<Work> {
        self.generation += 1;
        self.pending = Some(request);
        self.status = Status::Measuring;
        self.error = None;
        if let Some((_, cancel)) = &self.active {
            cancel.cancel();
        }
        self.next()
    }
    fn next(&mut self) -> Option<Work> {
        if self.active.is_some() {
            return None;
        }
        let request = self.pending.take()?;
        let cancel = Cancellation::default();
        self.active = Some((self.generation, cancel.clone()));
        Some(Work {
            generation: self.generation,
            request,
            cancel,
        })
    }
    pub fn finish(
        &mut self,
        generation: u64,
        result: Result<Option<Snapshot>, String>,
    ) -> Option<Work> {
        let Some((active, cancel)) = &self.active else {
            return None;
        };
        if *active != generation {
            return None;
        }
        if generation == self.generation && !cancel.is_cancelled() {
            match result {
                Ok(Some(snapshot)) => {
                    if snapshot.history.status == Status::Complete {
                        self.history_success = Some(snapshot.finished_at);
                    }
                    if snapshot.worktrees.status == Status::Complete {
                        self.worktrees_success = Some(snapshot.finished_at);
                    }
                    self.status = snapshot.total.status;
                    self.snapshot = Some(snapshot);
                }
                other => {
                    self.status = Status::Error;
                    self.error = Some(
                        other
                            .err()
                            .unwrap_or_else(|| "Measurement stopped before completion.".into()),
                    );
                }
            }
        }
        self.active = None;
        self.next()
    }
    pub fn cancel(&mut self) {
        self.generation += 1;
        self.pending = None;
        if let Some((_, cancel)) = &self.active {
            cancel.cancel();
        }
        self.status = self
            .snapshot
            .as_ref()
            .map_or(Status::NotMeasured, |s| s.total.status);
    }
    pub fn invalidate(&mut self) {
        self.cancel();
        self.snapshot = None;
        self.history_success = None;
        self.worktrees_success = None;
        self.status = Status::NotMeasured;
    }
    pub fn running(&self) -> bool {
        self.active.is_some()
    }
}
impl Drop for Service {
    fn drop(&mut self) {
        self.cancel();
    }
}
