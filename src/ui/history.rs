use super::*;
use crate::history::{CleanupPreview, RetentionPolicy};
use std::{collections::BTreeSet, sync::Arc};

pub(super) struct Dialog {
    pub(super) preview: CleanupPreview,
    pub(super) refreshed: bool,
    focus_cancel: bool,
}
pub(super) struct Settings {
    policy: RetentionPolicy,
    custom: bool,
    count: String,
    error: String,
}
impl Settings {
    pub fn new(policy: &RetentionPolicy) -> Self {
        Self {
            policy: policy.clone(),
            custom: policy
                .max_entries
                .is_some_and(|n| ![100, 250, 500, 1000].contains(&n.get())),
            count: policy
                .max_entries
                .map(|n| n.to_string())
                .unwrap_or_default(),
            error: String::new(),
        }
    }
}
impl App {
    pub(super) fn apply_startup_retention(&mut self, now: u64) {
        // Called only after Store::load has completely validated/recovered state.
        // Consume once even for no-op policies/errors; never retry automatically.
        if self.state_write.is_none()
            && std::mem::take(&mut self.automatic_retention_pending)
            && self.state.history_retention.automatic
            && !self.closing
        {
            let preview = self.history_preview(now);
            self.delete_history(&preview.remove);
        }
    }

    pub(super) fn history_protections(&self) -> BTreeSet<u64> {
        self.state
            .runs
            .iter()
            .filter(|run| {
                self.manager.history_protected(run.id)
                    || self.validations.history_protected(run.id)
                    || self.result_operation.is_some_and(|key| key.0 == run.id)
            })
            .map(|run| run.id)
            .collect()
    }
    pub(super) fn history_preview(&self, now: u64) -> CleanupPreview {
        crate::history::evaluate(
            &self.state.runs,
            &self.state.history_retention,
            &self.history_protections(),
            now,
        )
    }
    pub(super) fn offer_history_cleanup(&mut self, now: u64) {
        if !self.history_available() {
            return;
        }
        self.history_dialog = Some(Dialog {
            preview: self.history_preview(now),
            refreshed: false,
            focus_cancel: true,
        });
    }
    pub(super) fn history_available(&self) -> bool {
        !self.closing
            && !self.quit_requested
            && self.history_write.is_none()
            && self.state_write.is_none()
            && self.result_operation.is_none()
            && self.bulk_discard.is_none()
            && self.worktree_management.batch.is_none()
    }
    pub(super) fn confirm_history_cleanup(&mut self, now: u64) {
        if !self.history_available() {
            return;
        }
        let current = self.history_preview(now);
        let Some(dialog) = &mut self.history_dialog else {
            return;
        };
        // Includes policy/count changes as well as removal identities. A changed
        // preview always needs a second explicit activation, even if IDs overlap.
        if dialog.preview != current {
            dialog.preview = current;
            dialog.refreshed = true;
            dialog.focus_cancel = true;
            return;
        }
        self.delete_history(&current.remove);
        if self.history_write.is_some() || current.remove.is_empty() {
            self.history_dialog = None;
        }
    }
    pub(super) fn remove_history(&mut self, id: Option<u64>) {
        if !self.history_available() {
            return;
        }
        let protected = self.history_protections();
        let ids = self
            .state
            .runs
            .iter()
            .filter(|r| id.is_none_or(|id| r.id == id) && crate::history::eligible(r, &protected))
            .map(|r| r.id)
            .collect();
        self.delete_history(&ids);
    }
    pub(super) fn delete_history(&mut self, ids: &BTreeSet<u64>) {
        if !self.history_available() {
            return;
        }
        let protected = self.history_protections();
        match self.store.begin_history_write(&self.state, ids, &protected) {
            Ok(Some((write, staged))) => {
                if write.ids != *ids {
                    drop(write);
                    if self.history_dialog.is_some() {
                        self.history_dialog = Some(Dialog {
                            preview: self.history_preview(domain::now()),
                            refreshed: true,
                            focus_cancel: true,
                        });
                    }
                    self.notice = "History eligibility changed. Review cleanup again.".into();
                    return;
                }
                let write = Arc::new(write);
                self.history_write = Some(write.clone());
                self.dirty = false; // This snapshot includes all changes so far.
                let tx = self.tx.clone();
                self.runtime().spawn(async move {
                    let result = tokio::task::spawn_blocking(move || write.persist(&staged))
                        .await
                        .map_err(|e| e.to_string())
                        .and_then(|result| result.map_err(|e| format!("{e:#}")));
                    let _ = tx.send(Message::HistorySaved(result));
                });
            }
            Ok(None) => {
                self.notice = "No history removed. Active work, validations, unresolved isolated results and retained worktree ownership stay protected.".into();
            }
            Err(error) => self.notice = format!("Could not start history cleanup: {error:#}"),
        }
    }
    pub(super) fn history_saved(&mut self, result: Result<(), String>) {
        let Some(write) = self.history_write.take() else {
            return;
        };
        match result {
            Ok(()) => {
                let count = write.ids.len();
                write.publish(&mut self.state);
                // Release the writer before any subsequent intent or save. Merge
                // only IDs: events/settings received during I/O remain live.
                drop(write);
                self.reconcile_run_selection();
                self.session_runs
                    .retain(|id| self.state.runs.iter().any(|r| r.id == *id));
                self.history_search.matching(&self.state.runs);
                self.notice =
                    format!("Removed {count} history entries. Worktrees were not affected.");
            }
            Err(error) => {
                drop(write);
                self.dirty = true;
                self.notice = format!(
                    "History cleanup failed. History remains available in this session. {error}"
                );
            }
        }
    }
    pub(super) fn history_window(&mut self, ctx: &egui::Context) {
        if self.history_write.is_some() {
            egui::Modal::new(egui::Id::new("history_saving")).show(ctx, |ui| {
                ui.heading("Saving history cleanup…");
                ui.label(
                    "Running jobs and validations continue. Please wait for the save to finish.",
                );
            });
            return;
        }
        if self.quit_requested || self.closing {
            return;
        }
        let Some(dialog) = &mut self.history_dialog else {
            return;
        };
        let mut cancel = false;
        let mut confirm = false;
        let response = egui::Modal::new(egui::Id::new("history_cleanup")).show(ctx, |ui| {
            ui.set_width(430.0_f32.min(ctx.content_rect().width() - 60.0));
            ui.heading("History Cleanup");
            let p = &dialog.preview;
            ui.label(format!("Stored convoys: {}", p.stored));
            ui.label(format!("Keep newest: {}", count_label(&p.policy)));
            ui.label(format!("Maximum age: {}", age_label(&p.policy)));
            ui.separator();
            ui.label(format!("Entries to remove: {} · Entries to retain: {}", p.remove.len(), p.retained()));
            ui.small(format!("Protected: {} · Unreliable completion dates: {}", p.protected, p.undated));
            ui.label("Worktrees affected: None");
            if !p.remove.is_empty() {
                egui::ScrollArea::vertical().max_height(65.0).show(ui, |ui| {
                    ui.label(format!("Convoys: {}", p.remove.iter().map(|id| format!("#{id}")).collect::<Vec<_>>().join(", ")));
                });
            } else {
                ui.label("No entries match this policy.");
            }
            ui.small("History removal is permanent and may not be recoverable. Retained worktree ownership stays protected.");
            if dialog.refreshed { ui.colored_label(theme::Palette::of(ui).warning, "History changed. Review this updated preview and confirm again."); }
            if self.state_write.is_some() {
                ui.small("Saving application state. Please wait before cleaning history.");
            }
            ui.horizontal(|ui| {
                let button = ui.button("Cancel");
                if dialog.focus_cancel { button.request_focus(); dialog.focus_cancel = false; }
                cancel = button.clicked();
                confirm = ui.add_enabled(!p.remove.is_empty() && self.state_write.is_none(), theme::destructive(ui, "Clean Up History")).clicked();
            });
        });
        if cancel || response.should_close() {
            self.history_dialog = None;
        } else if confirm {
            self.confirm_history_cleanup(domain::now());
        }
    }
    pub(super) fn history_settings_window(&mut self, ctx: &egui::Context) {
        let Some(settings) = &mut self.history_settings else {
            return;
        };
        let mut cancel = false;
        let mut save = false;
        let response = egui::Modal::new(egui::Id::new("history_settings")).show(ctx, |ui| {
            ui.set_width(430.0_f32.min(ctx.content_rect().width() - 60.0));
            ui.heading("History Retention");
            egui::ComboBox::from_label("Maximum entries")
                .selected_text(if settings.custom { "Custom".into() } else { count_label(&settings.policy) })
                .show_ui(ui, |ui| {
                    if ui.selectable_label(!settings.custom && settings.policy.max_entries.is_none(), "Unlimited").clicked() {
                        settings.policy.max_entries = None; settings.custom = false;
                    }
                    for n in [100, 250, 500, 1000] {
                        if ui.selectable_label(!settings.custom && settings.policy.max_entries.map(|n| n.get()) == Some(n), n.to_string()).clicked() {
                            settings.policy.max_entries = std::num::NonZeroUsize::new(n); settings.custom = false;
                        }
                    }
                    ui.selectable_value(&mut settings.custom, true, "Custom positive integer");
                });
            if settings.custom {
                ui.horizontal(|ui| {
                    let label = ui.label("Custom maximum");
                    ui.add(egui::TextEdit::singleline(&mut settings.count).desired_width(120.0)).labelled_by(label.id);
                });
            }
            egui::ComboBox::from_label("Maximum age").selected_text(age_label(&settings.policy)).show_ui(ui, |ui| {
                ui.selectable_value(&mut settings.policy.max_age_days, None, "Unlimited");
                for days in [30, 90, 180, 365] {
                    ui.selectable_value(&mut settings.policy.max_age_days, std::num::NonZeroU32::new(days), format!("{days} days"));
                }
            });
            ui.checkbox(&mut settings.policy.automatic, "Automatically apply retention policy");
            ui.small("When enabled, retention runs once at the next application startup and can permanently remove eligible history. Deleted entries may not be recoverable.");
            ui.small("Both limits apply. Newest means completion time, then convoy ID. Active work, validations, retained worktree ownership and unreliable dates are preserved outside the limits.");
            ui.small("Use History cleanup → Clean Up History… to preview and apply the policy now. Unlimited is the default. Worktrees are never removed here.");
            if !settings.error.is_empty() { ui.colored_label(theme::Palette::of(ui).error, &settings.error); }
            ui.horizontal(|ui| { cancel = ui.button("Cancel").clicked(); save = ui.button("Save settings").clicked(); });
        });
        if cancel || response.should_close() {
            self.history_settings = None;
            return;
        }
        if save {
            if settings.custom {
                match RetentionPolicy::parse_count(&settings.count) {
                    Ok(count) => settings.policy.max_entries = Some(count),
                    Err(error) => {
                        settings.error = error.into();
                        return;
                    }
                }
            }
            let mut staged = self.state.persistence_snapshot();
            staged.history_retention = settings.policy.clone();
            match self.store.save(&staged) {
                Ok(()) => {
                    self.state.history_retention = staged.history_retention;
                    self.history_settings = None;
                    self.notice = "History retention settings saved. Manual cleanup is available in History cleanup; automatic retention runs on next startup.".into();
                }
                Err(error) => settings.error = format!("Settings were not saved: {error:#}"),
            }
        }
    }
}
fn count_label(policy: &RetentionPolicy) -> String {
    policy
        .max_entries
        .map(|n| n.to_string())
        .unwrap_or_else(|| "Unlimited".into())
}
fn age_label(policy: &RetentionPolicy) -> String {
    policy
        .max_age_days
        .map(|n| format!("{n} days"))
        .unwrap_or_else(|| "Unlimited".into())
}
