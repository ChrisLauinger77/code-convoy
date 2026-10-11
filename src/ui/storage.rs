//! Presentation and background dispatch only; opening this view never saves state.
use super::*;
use crate::storage::{Measurement, Request, Status, service::Work};

impl App {
    pub(super) fn open_storage(&mut self, ctx: egui::Context) {
        if self.closing {
            return;
        }
        self.storage_open = true;
        self.refresh_storage();
        ctx.request_repaint();
    }
    pub(super) fn refresh_storage(&mut self) {
        if self.closing || self.worktree_management.batch.is_some() {
            return;
        }
        let request = Request {
            history_path: self.store.history_path(),
            worktree_root: self.store.directory().join("worktrees"),
            references: self
                .state
                .runs
                .iter()
                .flat_map(|r| &r.jobs)
                .filter_map(|j| j.worktree.clone())
                .collect(),
        };
        if let Some(work) = self.storage.refresh(request) {
            self.dispatch_storage(work);
        }
    }
    pub(super) fn dispatch_storage(&self, work: Work) {
        let tx = self.tx.clone();
        let lifecycle = self.manager.lifecycle();
        self.runtime().spawn(async move {
            let generation = work.generation;
            let cancel = work.cancel.clone();
            let mut result = tokio::task::spawn_blocking(move || {
                crate::storage::measure(work.request, &work.cancel)
            })
            .await
            .map_err(|e| format!("Storage worker failed: {e}"));
            if let Ok(Some(snapshot)) = &mut result {
                for entry in &mut snapshot.entries {
                    if cancel.is_cancelled() {
                        break;
                    }
                    entry.inspection =
                        Some(lifecycle.inspect_cleanup(&entry.metadata, &cancel).await);
                }
            }
            let _ = tx.send(Message::StorageMeasured(generation, result));
        });
    }
    pub(super) fn storage_window(&mut self, ctx: &egui::Context) {
        if !self.storage_open || self.worktree_management.confirmation.is_some() {
            return;
        }
        let mut refresh = false;
        let mut history = false;
        let mut close = false;
        let mut cleanup = false;
        let response = egui::Modal::new(egui::Id::new("storage_overview")).show(ctx, |ui| {
            ui.set_width(500.0_f32.min(ctx.content_rect().width() - 64.0));
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
            ui.heading("Storage Overview");
            ui.horizontal(|ui| {
                if self.storage.status == Status::Measuring { ui.spinner(); }
                ui.label(self.storage.status.label());
            });
            egui::ScrollArea::vertical().max_height((ctx.content_rect().height() - 205.0).max(100.0)).show(ui, |ui| {
                if let Some(error) = &self.storage.error {
                    ui.colored_label(theme::Palette::of(ui).error, "Storage measurement failed. Check file access in Diagnostics, then refresh.");
                    diagnostics::details(ui, "storage_diagnostics", error);
                }
                if self.storage.snapshot.is_some() && matches!(self.storage.status, Status::Measuring | Status::Error) {
                    ui.small("Previous snapshot shown below.");
                }
                if let Some(snapshot) = &self.storage.snapshot {
                    ui.strong("History");
                    ui.label(format!("Stored historical convoys: {}", snapshot.historical_convoys.map_or_else(|| "Unknown".into(), |n| n.to_string())));
                    ui.label(snapshot.history.label());
                    ui.small("Shared state file includes history, active runs and settings.");
                    ui.label(format!("Location: {}", snapshot.history_path.display()));
                    success(ui, self.storage.history_success);
                    issues(ui, "history_storage_errors", &snapshot.history);
                    ui.separator();
                    ui.strong("Managed Worktrees");
                    ui.label(format!("Known worktrees: {} · Unavailable: {} · Partial: {}", snapshot.entries.len(), snapshot.unavailable, snapshot.partial));
                    ui.label(snapshot.worktrees.label());
                    ui.label(format!("Storage locations used: {}", snapshot.locations.len()));
                    if snapshot.unknown_ownership > 0 {
                        ui.label(format!("Unknown ownership / inventory errors: {}. Known subset only.", snapshot.unknown_ownership));
                    }
                    success(ui, self.storage.worktrees_success);
                    issues(ui, "worktree_storage_errors", &snapshot.worktrees);
                    egui::CollapsingHeader::new("Locations and worktrees").show(ui, |ui| {
                        for path in &snapshot.locations { ui.label(path.display().to_string()); }
                        // Rows are virtualized; very large inventories do not render every frame.
                        egui::ScrollArea::vertical().id_salt("storage_entries").max_height(150.0)
                            .show_rows(ui, 52.0, snapshot.entries.len(), |ui, range| {
                                for entry in &snapshot.entries[range] {
                                    ui.add(egui::Label::new(egui::RichText::new(entry.path.display().to_string()).small()).truncate()).on_hover_text(entry.path.display().to_string());
                                    ui.small(entry.measurement.label()).on_hover_text(entry.measurement.issues.join("\n"));
                                }
                            });
                    });
                    ui.separator();
                    if self.state_write.is_some() {
                        ui.small("Saving application state. Please wait before choosing cleanup.");
                    }
                    cleanup = worktree_management::rows(ui, &self.state, snapshot, &mut self.worktree_management,
                        self.storage.status != Status::Measuring && self.result_operation.is_none()
                        && self.bulk_discard.is_none() && self.history_write.is_none() && self.state_write.is_none());
                    ui.separator();
                    ui.strong(format!("{}: {}", if snapshot.total.status == Status::Complete { "Total measured storage" } else { "Partial measured total" }, snapshot.total.label()));
                    ui.small(format!("Last refreshed: {}", super::format::timestamp(snapshot.finished_at)));
                } else {
                    ui.label("History: Not measured");
                    ui.label("Managed worktrees: Not measured");
                    ui.label("Total: Not measured");
                }
                let base = self.state.worktree_base.clone().unwrap_or_else(|| self.store.directory().join("worktrees"));
                ui.label(format!("Current worktree base: {}", base.display()));
                ui.small("Logical regular-file sizes, including build output. External links, Git administration and exports are excluded. Snapshots can change while jobs run; this is not allocated disk space.");
                worktree_management::progress(ui, &self.worktree_management);
            });
            ui.separator();
            ui.horizontal_wrapped(|ui| {
                refresh = ui.add_enabled(self.worktree_management.batch.is_none(), egui::Button::new("Refresh")).clicked();
                history = ui.button("Manage History…").clicked();
                close = ui.button("Close").clicked();
            });
        });
        if close || history || response.should_close() {
            self.storage_open = false;
            self.storage.cancel();
        } else if refresh {
            self.refresh_storage();
        }
        if cleanup {
            self.offer_worktree_cleanup();
        }
        if history {
            self.history_settings = Some(history::Settings::new(&self.state.history_retention));
        }
    }
}
fn success(ui: &mut egui::Ui, time: Option<u64>) {
    ui.small(format!(
        "Last successful measurement: {}",
        time.map_or_else(|| "Not measured".into(), super::format::timestamp)
    ));
}
fn issues(ui: &mut egui::Ui, id: &str, measurement: &Measurement) {
    if measurement.issue_count == 0 {
        return;
    }
    egui::CollapsingHeader::new(format!("Measurement details ({})", measurement.issue_count))
        .id_salt(id)
        .show(ui, |ui| {
            for error in &measurement.issues {
                ui.label(error);
            }
            if measurement.issue_count > measurement.issues.len() {
                ui.small("Additional errors omitted.");
            }
        });
}
