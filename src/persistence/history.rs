//! A history-only transaction using the existing atomic Store writer.
use super::Store;
use crate::domain::AppState;
use anyhow::{Context, Result};
use std::{collections::BTreeSet, sync::Arc};

/// Keep other writers out until the UI has merged a successful deletion.
/// Lifecycle events can still update live state; they are saved after the merge.
pub struct HistoryWrite {
    store: Arc<Store>,
    pub ids: BTreeSet<u64>,
}
impl Store {
    pub fn begin_history_write(
        self: &Arc<Self>,
        state: &AppState,
        ids: &BTreeSet<u64>,
        protected: &BTreeSet<u64>,
    ) -> Result<Option<(HistoryWrite, AppState)>> {
        let mut pending = self
            .history_write
            .lock()
            .map_err(|_| anyhow::anyhow!("History save lock failed."))?;
        anyhow::ensure!(!*pending, "History cleanup is already being saved.");
        if ids.is_empty() {
            return Ok(None);
        }
        let mut staged = state.clone();
        if crate::history::remove(&mut staged.runs, ids, protected) == 0 {
            return Ok(None);
        }
        let retained: BTreeSet<_> = staged.runs.iter().map(|r| r.id).collect();
        let ids = state
            .runs
            .iter()
            .map(|r| r.id)
            .filter(|id| !retained.contains(id))
            .collect();
        *pending = true;
        Ok(Some((
            HistoryWrite {
                store: self.clone(),
                ids,
            },
            staged,
        )))
    }
}
impl HistoryWrite {
    /// Run on the application's blocking worker pool, never in rendering code.
    pub fn persist(&self, staged: &AppState) -> Result<()> {
        self.store.save_atomic(staged).context("Could not confirm saving history cleanup. History remains in memory; the disk outcome may be uncertain after a sync failure.")
    }
    pub fn publish(&self, state: &mut AppState) {
        state.runs.retain(|r| !self.ids.contains(&r.id));
    }
}
impl Drop for HistoryWrite {
    fn drop(&mut self) {
        if let Ok(mut pending) = self.store.history_write.lock() {
            *pending = false;
        }
    }
}
