use super::*;
use crate::persistence::bulk_discard::{Batch, Plan, Scope};

impl App {
    pub(super) fn offer_bulk_discard(&mut self, scope: Scope) {
        if self.closing
            || self.quit_requested
            || self.result_operation.is_some()
            || self.bulk_discard.is_some()
        {
            return;
        }
        let plan = Plan::new(&self.state, scope);
        if !plan.is_empty() {
            self.bulk_confirmation = Some(plan);
            self.focus_bulk_cancel = true;
        }
    }

    pub(super) fn pump_bulk_discard(&mut self, ctx: &egui::Context) {
        if self.result_operation.is_some() {
            return;
        }
        let Some(batch) = &mut self.bulk_discard else {
            return;
        };
        if self.closing {
            batch.stop();
        }
        if batch.done() {
            let batch = self.bulk_discard.take().expect("Batch was checked above");
            let label = batch.summary.label(batch.plan.scope);
            batch.activity(&mut self.state, &label);
            self.bulk_report = Some((batch.plan.scope, batch.summary));
            self.dirty = true;
            return;
        }
        // Let a previous review finish before starting the destructive transaction.
        // New reviews are suspended until this batch completes.
        if self.quit_requested || self.review_pending.is_some() {
            return;
        }
        if let Some(op) = batch.begin_next(&self.store, &mut self.state) {
            self.dispatch_result_operation(ctx, op);
        }
        ctx.request_repaint();
    }

    pub(super) fn bulk_discard_status(&mut self, ui: &mut egui::Ui) {
        if let Some(batch) = &self.bulk_discard {
            ui.small(format!(
                "Discarding retained results… {} discarded, {} failed / {} selected",
                batch.summary.discarded,
                batch.summary.failures.len(),
                batch.plan.len()
            ));
        }
        if let Some((scope, report)) = &self.bulk_report {
            let mut dismiss = false;
            ui.horizontal_wrapped(|ui| {
                ui.small(report.label(*scope));
                dismiss = ui.small_button("Dismiss").clicked();
            });
            if !report.failures.is_empty() {
                ui.small("Failed results remain protected in history. Inspect the diagnostics and retry when resolved.");
                diagnostics::details(ui, "bulk_discard_failures", &report.failures.join("\n\n"));
            }
            if dismiss {
                self.bulk_report = None;
            }
        }
    }

    pub(super) fn bulk_discard_window(&mut self, ctx: &egui::Context) {
        if self.quit_requested || self.closing {
            return;
        }
        let Some(plan) = &self.bulk_confirmation else {
            return;
        };
        let convoy = matches!(plan.scope, Scope::Convoy(_));
        let mut cancel = false;
        let mut confirm = false;
        let response = egui::Modal::new(egui::Id::new("bulk_discard")).show(ctx, |ui| {
            ui.set_max_width(460.0);
            ui.heading(if convoy { "Discard convoy results?" } else { "Discard all unresolved results?" });
            let location = match plan.scope {
                Scope::Convoy(id) => format!("in convoy #{id}"),
                Scope::All => format!("across {} {}", plan.convoys(), if plan.convoys() == 1 { "convoy" } else { "convoys" }),
            };
            ui.label(format!("This will attempt to discard {} retained isolated {} owned by CodeConvoy {location}.", plan.len(), if plan.len() == 1 { "result" } else { "results" }));
            ui.label("Registered repositories and direct working tree changes will not be modified. Active convoys and already resolved results are excluded.");
            ui.label("Each result must pass ownership and safety checks. Blocked results stay in history; successful discards cannot be undone. History entries are removed separately.");
            ui.horizontal(|ui| {
                let button = ui.button("Cancel");
                if self.focus_bulk_cancel { button.request_focus(); self.focus_bulk_cancel = false; }
                cancel = button.clicked();
                confirm = ui.button(if convoy { "Discard convoy" } else { "Discard all" }).clicked();
            });
        });
        if cancel || response.should_close() {
            self.bulk_confirmation = None;
        } else if confirm && let Some(plan) = self.bulk_confirmation.take() {
            self.bulk_report = None;
            self.bulk_discard = Some(Batch::new(plan, &mut self.state));
            ctx.request_repaint();
        }
    }
}
