//! Periodic saves share the history transaction's single-writer reservation.
use super::*;
use std::sync::Arc;

pub(super) struct StateWrite {
    // Keep the Store reservation until the UI acknowledges the worker outcome.
    _reservation: Arc<crate::persistence::history::HistoryWrite>,
    task: tokio::task::JoinHandle<Result<(), String>>,
}

impl App {
    pub(super) fn save_background(&mut self) {
        // Lifecycle intent/outcome transactions retain priority and cannot be
        // overtaken by an older background snapshot.
        if !self.dirty
            || self.state_write.is_some()
            || self.history_write.is_some()
            || self.result_operation.is_some()
            || self.bulk_discard.is_some()
            || self.worktree_management.batch.is_some()
            || self.closing
            || self.quit_requested
            || self.explicit_quit
        {
            return;
        }
        self.last_save = Instant::now();
        match self.store.begin_save(&self.state) {
            Ok((write, staged)) => {
                let write = Arc::new(write);
                let reservation = write.clone();
                // Later edits set dirty again; acknowledgement must not erase them.
                self.dirty = false;
                let tx = self.tx.clone();
                let task = self.runtime().spawn(async move {
                    let result = tokio::task::spawn_blocking(move || write.persist(&staged))
                        .await
                        .map_err(|e| e.to_string())
                        .and_then(|r| r.map_err(|e| format!("{e:#}")));
                    let _ = tx.send(Message::StateSaved(result.clone()));
                    result
                });
                self.state_write = Some(StateWrite {
                    _reservation: reservation,
                    task,
                });
            }
            Err(error) => self.notice = format!("Could not save state: {error:#}"),
        }
    }
    pub(super) fn state_saved(&mut self, result: Result<(), String>) {
        if self.state_write.take().is_none() {
            return;
        }
        if let Err(error) = result {
            self.dirty = true;
            self.notice = format!("Could not save state: {error}");
        }
    }

    pub(super) fn finish_state_write_on_exit(&mut self) {
        let Some(write) = self.state_write.as_mut() else {
            return;
        };
        // Native teardown can bypass normal close handling and UI message polls.
        // Join before releasing the reservation so an older snapshot cannot
        // overwrite the final interrupted state, even on a slow filesystem.
        let result = self
            .runtime
            .as_ref()
            .expect("Application runtime exists until Drop")
            .block_on(&mut write.task)
            .unwrap_or_else(|error| Err(error.to_string()));
        self.state_saved(result);
    }
}
