//! Frozen explicit selections; each target uses the existing Store transaction
//! and manager lifecycle lease independently. No inventory discovery during removal.
use super::{
    Store,
    results::{Action, Completion, Operation},
};
use crate::{
    domain::*,
    storage::{
        Worktree,
        management::{self, Lifecycle},
    },
    worktrees::cleanup::Proof,
};
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
};

pub fn identity(m: &WorktreeMetadata) -> PathBuf {
    m.path
        .parent()
        .and_then(std::path::Path::file_name)
        .map(PathBuf::from)
        .unwrap_or_default()
}
#[derive(Clone)]
pub struct Target {
    pub metadata: WorktreeMetadata,
    pub bytes: Option<u64>,
    pub reason: String,
    proof: Option<Proof>,
}
impl Target {
    pub fn eligible(&self) -> bool {
        self.proof.is_some()
    }
}
pub struct Plan {
    pub targets: Vec<Target>,
}
impl Plan {
    pub fn new(state: &AppState, entries: &[Worktree], selected: &HashSet<PathBuf>) -> Self {
        let mut records = HashMap::new();
        let mut ambiguous = HashSet::new();
        for e in entries {
            let id = identity(&e.metadata);
            if records
                .insert(id.clone(), &e.metadata)
                .is_some_and(|old| old != &e.metadata)
            {
                ambiguous.insert(id);
            }
        }
        let mut seen = HashSet::new();
        let targets = entries
            .iter()
            .filter(|e| {
                selected.contains(&identity(&e.metadata)) && seen.insert(identity(&e.metadata))
            })
            .map(|e| {
                let (kind, mut reason) =
                    management::classify(state, &e.metadata, e.inspection.as_ref());
                let mut proof = (kind == Lifecycle::Candidate)
                    .then(|| e.inspection.as_ref().and_then(|i| i.proof.clone()))
                    .flatten();
                if ambiguous.contains(&identity(&e.metadata)) {
                    proof = None;
                    reason = "Ambiguous duplicate identity; preserved.".into();
                }
                Target {
                    metadata: e.metadata.clone(),
                    bytes: e.measurement.bytes,
                    reason,
                    proof,
                }
            })
            .collect();
        Self { targets }
    }
    pub fn eligible(&self) -> usize {
        self.targets.iter().filter(|t| t.eligible()).count()
    }
    pub fn measured_bytes(&self) -> u64 {
        self.targets
            .iter()
            .filter(|t| t.eligible())
            .filter_map(|t| t.bytes)
            .fold(0, u64::saturating_add)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Removed,
    Skipped,
    Failed,
}
pub struct Entry {
    pub path: PathBuf,
    pub outcome: Outcome,
    pub detail: String,
}
#[derive(Default)]
pub struct Summary {
    pub entries: Vec<Entry>,
}
impl Summary {
    pub fn count(&self, outcome: Outcome) -> usize {
        self.entries.iter().filter(|e| e.outcome == outcome).count()
    }
    pub fn label(&self) -> String {
        format!(
            "Removed: {} · Skipped: {} · Failed: {}",
            self.count(Outcome::Removed),
            self.count(Outcome::Skipped),
            self.count(Outcome::Failed)
        )
    }
}
pub struct Batch {
    pub plan: Plan,
    pub summary: Summary,
    next: usize,
}
impl Batch {
    pub fn new(plan: Plan) -> Self {
        Self {
            plan,
            summary: Summary::default(),
            next: 0,
        }
    }
    pub fn done(&self) -> bool {
        self.next == self.plan.targets.len()
    }
    pub fn stop(&mut self) {
        for t in &self.plan.targets[self.next..] {
            self.summary.entries.push(Entry {
                path: t.metadata.path.clone(),
                outcome: Outcome::Skipped,
                detail: "Application is closing; not attempted.".into(),
            });
        }
        self.next = self.plan.targets.len();
    }
    pub fn begin_next(&mut self, store: &Store, state: &mut AppState) -> Option<Operation> {
        let target = self.plan.targets.get(self.next)?.clone();
        self.next += 1;
        let Some(proof) = target.proof else {
            self.record(target.metadata.path, Outcome::Skipped, target.reason);
            return None;
        };
        let m = &target.metadata;
        if let Some((_, reason)) = management::protection(state, m) {
            self.record(m.path.clone(), Outcome::Skipped, reason);
            return None;
        }
        match store.begin_result_operation(state, m.run, m.job, Action::ManagedCleanup) {
            Ok(mut operation) => {
                operation.cleanup_proof = Some(proof);
                Some(operation)
            }
            Err(e) => {
                self.record(m.path.clone(), Outcome::Failed, format!("{e:#}"));
                None
            }
        }
    }
    pub fn finish(
        &mut self,
        store: &Store,
        state: &mut AppState,
        operation: &Operation,
        completion: Completion,
    ) {
        let error = match &completion {
            Completion::Cleanup(Ok(r)) if r.availability == ResultAvailability::Cleaned => None,
            Completion::Cleanup(Err(e)) => Some(e.clone()),
            _ => Some("Cleanup did not confirm physical removal.".into()),
        };
        let saved = store.finish_result_operation(state, operation, completion);
        let error = saved.err().map(|e| format!("{e:#}")).or(error);
        let path = operation
            .job
            .worktree
            .as_ref()
            .map(|m| m.path.clone())
            .unwrap_or_default();
        match error {
            Some(e) => self.record(path, Outcome::Failed, e),
            None => self.record(
                path,
                Outcome::Removed,
                "Verified removal; history preserved.".into(),
            ),
        }
    }
    fn record(&mut self, path: PathBuf, outcome: Outcome, detail: String) {
        self.summary.entries.push(Entry {
            path,
            outcome,
            detail,
        });
    }
}
