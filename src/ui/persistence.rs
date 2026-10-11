//! Periodic saves share the history transaction's single-writer reservation.
use super::*;
use std::sync::Arc;

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
                self.state_write = Some(write.clone());
                // Later edits set dirty again; acknowledgement must not erase them.
                self.dirty = false;
                let tx = self.tx.clone();
                self.runtime().spawn(async move {
                    let result = tokio::task::spawn_blocking(move || write.persist(&staged))
                        .await
                        .map_err(|e| e.to_string())
                        .and_then(|r| r.map_err(|e| format!("{e:#}")));
                    let _ = tx.send(Message::StateSaved(result));
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
}
