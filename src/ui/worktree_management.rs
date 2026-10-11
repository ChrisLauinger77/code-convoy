//! Native presentation and dispatch; filesystem/Git work stays on the runtime.
use super::*;
use crate::{
    persistence::worktree_cleanup::{self, Batch, Plan, Summary},
    storage::management::{self, Lifecycle},
};

#[derive(Default)]
pub(super) struct Management {
    pub selected: HashSet<PathBuf>,
    pub confirmation: Option<Plan>,
    pub batch: Option<Batch>,
    pub report: Option<Summary>,
    focus_cancel: bool,
}

pub(super) fn rows(
    ui: &mut egui::Ui,
    state: &AppState,
    snapshot: &crate::storage::Snapshot,
    management: &mut Management,
    ready: bool,
) -> bool {
    ui.strong("Manage Worktrees");
    ui.small("Unresolved results remain retained. Ignored files also require review.");
    let ready = ready && management.batch.is_none();
    egui::ScrollArea::vertical()
        .id_salt("managed_worktree_rows")
        .max_height(220.0)
        .show(ui, |ui| {
            for entry in &snapshot.entries {
                let identity = worktree_cleanup::identity(&entry.metadata);
                ui.push_id(&identity, |ui| {
                    let (kind, reason) =
                        management::classify(state, &entry.metadata, entry.inspection.as_ref());
                    let mut selected = management.selected.contains(&identity);
                    ui.horizontal_wrapped(|ui| {
                        if ui
                            .add_enabled(
                                ready && (kind == Lifecycle::Candidate || selected),
                                egui::Checkbox::new(&mut selected, &entry.metadata.repository.name),
                            )
                            .changed()
                        {
                            if selected {
                                management.selected.insert(identity.clone());
                            } else {
                                management.selected.remove(&identity);
                            }
                        }
                        ui.label(kind.label()).on_hover_text(&reason);
                        ui.small(entry.measurement.label());
                    });
                    ui.add(egui::Label::new(entry.path.display().to_string()).truncate())
                        .on_hover_text(entry.path.display().to_string());
                    ui.small(format!(
                        "Convoy #{} · Job {} · {}",
                        entry.metadata.run,
                        entry.metadata.job + 1,
                        if kind == Lifecycle::Candidate {
                            "Eligible after final checks"
                        } else {
                            "Protected"
                        }
                    ));
                    let changes = entry
                        .inspection
                        .as_ref()
                        .map(|i| {
                            if !i.changes.is_empty() {
                                "Modified / Needs Review"
                            } else if i.proof.is_some() {
                                "No local or ignored changes found"
                            } else {
                                "Change status unknown"
                            }
                        })
                        .unwrap_or("Not inspected");
                    ui.small(changes);
                    egui::CollapsingHeader::new("Inspect details").show(ui, |ui| {
                        ui.label(format!("Identity: {}", identity.display()));
                        ui.label(format!("Recorded path: {}", entry.path.display()));
                        ui.label(format!(
                            "Repository: {}",
                            entry.metadata.repository.path.display()
                        ));
                        ui.label(format!("Fixed base: {}", entry.metadata.base_commit));
                        ui.label(&reason);
                        if let Some(i) = &entry.inspection {
                            let yes_no = |value| match value {
                                Some(true) => "Yes",
                                Some(false) => "No",
                                None => "Unknown",
                            };
                            ui.label(format!(
                                "Exists: {} · Registered and owned: {} · Detached HEAD: {}",
                                yes_no(i.exists),
                                yes_no(i.registered),
                                yes_no(i.detached)
                            ));
                            ui.label(&i.detail);
                            for change in i.changes.iter().take(100) {
                                ui.label(change);
                            }
                            if i.changes.len() > 100 {
                                ui.small("More changed paths omitted; cleanup remains blocked.");
                            }
                        }
                    });
                    ui.separator();
                });
            }
        });
    ui.add_enabled(
        ready && !management.selected.is_empty(),
        egui::Button::new("Clean Up Selected…"),
    )
    .clicked()
}

pub(super) fn progress(ui: &mut egui::Ui, management: &Management) {
    if let Some(batch) = &management.batch {
        ui.label(format!(
            "Cleaning managed worktrees… {} / {} processed",
            batch.summary.entries.len(),
            batch.plan.targets.len()
        ));
        ui.small(batch.summary.label());
    }
    if let Some(report) = &management.report {
        ui.label(format!("Cleanup completed · {}", report.label()));
        egui::CollapsingHeader::new("Per-worktree results").show(ui, |ui| {
            egui::ScrollArea::vertical()
                .id_salt("cleanup_report")
                .max_height(130.0)
                .show(ui, |ui| {
                    for entry in &report.entries {
                        ui.label(format!("{:?}: {}", entry.outcome, entry.path.display()));
                        ui.small(&entry.detail);
                    }
                });
        });
    }
}
impl App {
    pub(super) fn offer_worktree_cleanup(&mut self) {
        if self.closing
            || self.quit_requested
            || self.worktree_management.batch.is_some()
            || self.result_operation.is_some()
            || self.bulk_discard.is_some()
            || self.history_write.is_some()
            || self.state_write.is_some()
            || self.storage.status == crate::storage::Status::Measuring
        {
            return;
        }
        let Some(snapshot) = &self.storage.snapshot else {
            return;
        };
        let plan = Plan::new(
            &self.state,
            &snapshot.entries,
            &self.worktree_management.selected,
        );
        if !plan.targets.is_empty() {
            self.worktree_management.confirmation = Some(plan);
            self.worktree_management.focus_cancel = true;
        }
    }
    pub(super) fn pump_worktree_cleanup(&mut self, ctx: &egui::Context) {
        if self.state_write.is_some() {
            return;
        }
        if self.result_operation.is_some() {
            return;
        }
        let Some(batch) = &mut self.worktree_management.batch else {
            return;
        };
        if self.closing {
            batch.stop();
        }
        if batch.done() {
            if let Some(batch) = self.worktree_management.batch.take() {
                self.worktree_management.report = Some(batch.summary);
            }
            self.worktree_management.selected.clear();
            self.storage.invalidate();
            if self.storage_open && !self.closing {
                self.refresh_storage();
            }
            return;
        }
        if self.quit_requested
            || self.review_pending.is_some()
            || self.history_write.is_some()
            || self.state_write.is_some()
        {
            return;
        }
        if let Some(op) = batch.begin_next(&self.store, &mut self.state) {
            self.dispatch_result_operation(ctx, op);
        }
        ctx.request_repaint();
    }
    pub(super) fn worktree_cleanup_window(&mut self, ctx: &egui::Context) {
        let Some(plan) = &self.worktree_management.confirmation else {
            return;
        };
        let mut cancel = false;
        let mut confirm = false;
        let response = egui::Modal::new(egui::Id::new("managed_cleanup_confirmation")).show(ctx, |ui| {
            ui.set_width(500.0_f32.min(ctx.content_rect().width() - 64.0));
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
            ui.heading("Clean Up Managed Worktrees");
            ui.label(format!("Selected: {} · Eligible: {} · Protected/skipped: {}", plan.targets.len(), plan.eligible(), plan.targets.len() - plan.eligible()));
            ui.label(format!("Previously measured logical size: {}", crate::storage::format_bytes(plan.measured_bytes())));
            ui.small("Known size subtotal; some measurements may be partial or unavailable. This is not guaranteed reclaimed disk space.");
            ui.label("This removes the exact eligible worktrees below from disk after final checks. Modified, active, retained and unverified worktrees are protected. History and validation records are preserved.");
            egui::ScrollArea::vertical().max_height((ctx.content_rect().height() - 360.0).max(80.0)).show(ui, |ui| {
                for target in &plan.targets {
                    ui.label(format!("{} · Convoy #{} / Job {} · {}", target.metadata.repository.name, target.metadata.run, target.metadata.job + 1, if target.eligible() { "Eligible" } else { "Skipped" }));
                    ui.label(target.metadata.path.display().to_string());
                    ui.small(&target.reason);
                    ui.separator();
                }
            });
            ui.horizontal_wrapped(|ui| {
                let button = ui.button("Cancel");
                if self.worktree_management.focus_cancel { button.request_focus(); self.worktree_management.focus_cancel = false; }
                cancel = button.clicked();
                confirm = ui.add_enabled(plan.eligible() > 0 && self.result_operation.is_none() && self.bulk_discard.is_none() && self.history_write.is_none(), egui::Button::new(format!("Clean Up {} Worktrees", plan.eligible()))).clicked();
            });
        });
        if cancel || response.should_close() {
            self.worktree_management.confirmation = None;
        } else if confirm && let Some(plan) = self.worktree_management.confirmation.take() {
            self.storage.cancel();
            self.worktree_management.report = None;
            self.worktree_management.batch = Some(Batch::new(plan));
            ctx.request_repaint();
        }
    }
}
