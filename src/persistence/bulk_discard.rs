//! Session-only bulk coordination. Every target uses the ordinary Store transaction.
use super::{
    Store,
    results::{Action, Completion, Operation},
};
use crate::domain::*;
use anyhow::{Context, Result, ensure};
use std::collections::HashSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scope {
    Convoy(u64),
    All,
}

#[derive(Clone)]
struct Target {
    run: u64,
    index: usize,
    metadata: WorktreeMetadata,
}

/// Freeze the exact identities shown in confirmation; never expand at execution.
pub struct Plan {
    pub scope: Scope,
    targets: Vec<Target>,
}
impl Plan {
    pub fn new(state: &AppState, scope: Scope) -> Self {
        let targets = state
            .runs
            .iter()
            .filter(|r| !r.active() && (scope == Scope::All || scope == Scope::Convoy(r.id)))
            .flat_map(|r| {
                r.jobs.iter().enumerate().filter_map(move |(index, j)| {
                    (j.execution_mode == ExecutionMode::IsolatedWorktree
                        && j.unresolved_retained_result())
                    .then(|| j.worktree.clone())
                    .flatten()
                    .map(|metadata| Target {
                        run: r.id,
                        index,
                        metadata,
                    })
                })
            })
            .collect();
        Self { scope, targets }
    }
    pub fn len(&self) -> usize {
        self.targets.len()
    }
    pub fn is_empty(&self) -> bool {
        self.targets.is_empty()
    }
    pub fn convoys(&self) -> usize {
        self.targets
            .iter()
            .map(|t| t.run)
            .collect::<HashSet<_>>()
            .len()
    }
}

#[derive(Default)]
pub struct Summary {
    pub discarded: usize,
    pub failures: Vec<String>,
    pub not_attempted: usize,
}
impl Summary {
    pub fn label(&self, scope: Scope) -> String {
        let name = match scope {
            Scope::Convoy(_) => "Convoy discard",
            Scope::All => "Global discard",
        };
        let outcome = if self.not_attempted > 0 {
            "stopped"
        } else if self.failures.is_empty() {
            "completed"
        } else {
            "partially failed"
        };
        let mut label = format!(
            "{name} {outcome}: {} discarded, {} failed",
            self.discarded,
            self.failures.len()
        );
        if self.not_attempted > 0 {
            label.push_str(&format!(", {} not attempted", self.not_attempted));
        }
        label
    }
}

pub struct Batch {
    pub plan: Plan,
    pub summary: Summary,
    next: usize,
}
impl Batch {
    pub fn new(plan: Plan, state: &mut AppState) -> Self {
        let batch = Self {
            plan,
            summary: Summary::default(),
            next: 0,
        };
        batch.activity(
            state,
            match batch.plan.scope {
                Scope::Convoy(_) => "Discarding convoy results",
                Scope::All => "Discarding all unresolved results",
            },
        );
        batch
    }
    pub fn done(&self) -> bool {
        self.next == self.plan.len()
    }
    pub fn stop(&mut self) {
        self.summary.not_attempted = self.plan.len() - self.next;
        self.next = self.plan.len();
    }
    /// Called only with no operation in flight. Recheck the convoy and identity
    /// before the existing transaction saves intent or performs any Git work.
    pub fn begin_next(&mut self, store: &Store, state: &mut AppState) -> Option<Operation> {
        let target = self.plan.targets.get(self.next)?.clone();
        self.next += 1;
        let result = (|| -> Result<Operation> {
            let run = state
                .runs
                .iter()
                .find(|r| r.id == target.run)
                .context("Convoy no longer exists.")?;
            ensure!(!run.active(), "Active convoys cannot be discarded.");
            let job = run
                .jobs
                .get(target.index)
                .context("Result no longer exists.")?;
            ensure!(
                job.worktree.as_ref() == Some(&target.metadata),
                "Result ownership changed since confirmation."
            );
            ensure!(
                job.unresolved_retained_result(),
                "Result is already resolved; left untouched."
            );
            store.begin_result_operation(state, target.run, target.index, Action::Discard)
        })();
        match result {
            Ok(op) => Some(op),
            Err(e) => {
                self.failed(state, target.run, target.index, format!("{e:#}"));
                None
            }
        }
    }
    pub fn finish(
        &mut self,
        store: &Store,
        state: &mut AppState,
        op: &Operation,
        completion: Completion,
    ) {
        // Capture the cleanup diagnostic before merging; save failures also count
        // as failures, never as confirmed successful discards.
        let cleanup_error = match &completion {
            Completion::Cleanup(Err(e)) => Some(e.clone()),
            _ => None,
        };
        let result = store.finish_result_operation(state, op, completion);
        if let Err(e) = result {
            self.failed(state, op.run, op.index, format!("{e:#}"));
        } else if let Some(error) = cleanup_error {
            self.failed(state, op.run, op.index, error);
        } else {
            self.summary.discarded += 1;
        }
    }
    fn failed(&mut self, state: &mut AppState, run: u64, index: usize, detail: String) {
        let diagnostic = format!("Convoy #{run}, result {}: {detail}", index + 1);
        if let Some(job) = state
            .runs
            .iter_mut()
            .find(|r| r.id == run)
            .and_then(|r| r.jobs.get_mut(index))
        {
            job.log
                .append(&format!("\n[CodeConvoy] Discard failed: {detail}\n"));
        }
        self.summary.failures.push(diagnostic);
    }
    pub fn activity(&self, state: &mut AppState, message: &str) {
        // One summary per affected convoy, attached to its first targeted job.
        let mut seen = HashSet::new();
        for target in &self.plan.targets {
            if seen.insert(target.run)
                && let Some(job) = state
                    .runs
                    .iter_mut()
                    .find(|r| r.id == target.run)
                    .and_then(|r| r.jobs.get_mut(target.index))
            {
                job.log.append(&format!("\n[CodeConvoy] {message}\n"));
            }
        }
    }
}
