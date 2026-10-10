//! One quit decision for window close and native application quit.
use super::*;

impl App {
    fn has_active_work(&self) -> bool {
        self.result_operation.is_some()
            || self.bulk_discard.is_some()
            || !self.manager.is_idle()
            || self.state.runs.iter().any(Run::active)
    }

    /// Returns true only when the close request may proceed without a dialog.
    pub(super) fn request_quit(&mut self) -> bool {
        if self.exit_ready {
            return true;
        }
        if self.closing || self.quit_requested {
            return false;
        }
        if !self.has_active_work() {
            self.confirm_quit();
            self.exit_ready = true;
            self.save();
            return true;
        }
        self.quit_requested = true;
        self.focus_quit_cancel = true;
        false
    }

    pub(super) fn cancel_quit(&mut self) {
        self.quit_requested = false;
        self.focus_quit_cancel = false;
    }

    pub(super) fn confirm_quit(&mut self) {
        if self.closing {
            return;
        }
        self.closing = true;
        self.quit_requested = false;
        self.focus_quit_cancel = false;
        self.cli_checks.stop();
        self.notification_service.stop();
        self.desktop.stop();
        // Shutdown closes admission synchronously, then uses Stop All's tokens.
        self.manager.shutdown();
    }

    pub(super) fn handle_close(&mut self, ctx: &egui::Context) {
        let close = ctx.input(|i| i.viewport().close_requested());
        let explicit = std::mem::take(&mut self.explicit_quit);
        if (close || explicit) && !self.exit_ready {
            if close
                && !explicit
                && !self.closing
                && !self.quit_requested
                && self.desktop.can_minimize(self.state.desktop)
            {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                self.window.minimize(ctx);
                self.notification_focused = false;
                self.save();
            } else if self.request_quit() {
                if explicit {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            } else {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                self.window.request_restore();
                ctx.request_repaint();
            }
        }
        if self.closing
            && !self.exit_ready
            && self.manager.join.is_finished()
            && self.result_operation.is_none()
            && self.bulk_discard.is_none()
        {
            // poll() is bounded. Drain the last lifecycle events before saving,
            // even when the manager finished between frames or its worker failed.
            while let Ok(event) = self.events_rx.try_recv() {
                self.apply_event(event);
            }
            self.state.recover_interrupted();
            self.save();
            self.exit_ready = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        self.window.restore(ctx);
    }

    pub(super) fn quit_window(&mut self, ctx: &egui::Context) {
        if !self.quit_requested {
            return;
        }
        let mut cancel = false;
        let mut confirm = false;
        let mut running = 0;
        let mut preparing = 0;
        let mut queued = 0;
        for job in self.state.runs.iter().flat_map(|run| &run.jobs) {
            match job.status {
                JobStatus::Running => running += 1,
                JobStatus::Preparing => preparing += 1,
                JobStatus::Queued => queued += 1,
                _ => {}
            }
        }
        let response = egui::Modal::new(egui::Id::new("confirm_quit")).show(ctx, |ui| {
            ui.set_max_width(420.0);
            ui.heading("Work is still in progress");
            ui.label(format!(
                "Preparing: {preparing} · Running: {running} · Queued: {queued}"
            ));
            ui.label("Closing CodeConvoy will stop all active jobs.");
            if self.bulk_discard.is_some() || self.result_operation.is_some() {
                ui.label("Closing waits for result-operation cleanup; remaining bulk discards will not be attempted.");
            }
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                let button = ui.button("Cancel");
                if self.focus_quit_cancel {
                    button.request_focus();
                    self.focus_quit_cancel = false;
                }
                cancel = button.clicked();
                confirm = ui.button("Stop jobs and quit").clicked();
            });
        });
        if cancel || response.should_close() {
            self.cancel_quit();
        } else if confirm {
            self.confirm_quit();
            ctx.request_repaint();
        }
    }
}
