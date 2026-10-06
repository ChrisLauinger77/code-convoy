//! One quit decision for window close and native application quit.
use super::*;

impl App {
    fn has_active_work(&self) -> bool {
        !self.manager.is_idle() || self.state.runs.iter().any(Run::active)
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
        // Shutdown closes admission synchronously, then uses Stop All's tokens.
        self.manager.shutdown();
    }

    pub(super) fn handle_close(&mut self, ctx: &egui::Context) {
        if ctx.input(|i| i.viewport().close_requested()) && !self.request_quit() {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
            ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
            ctx.request_repaint();
        }
        if self.closing && !self.exit_ready && self.manager.join.is_finished() {
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
    }

    pub(super) fn quit_window(&mut self, ctx: &egui::Context) {
        if !self.quit_requested {
            return;
        }
        let mut cancel = false;
        let mut confirm = false;
        let mut running = 0;
        let mut queued = 0;
        for job in self.state.runs.iter().flat_map(|run| &run.jobs) {
            match job.status {
                JobStatus::Running => running += 1,
                JobStatus::Queued => queued += 1,
                _ => {}
            }
        }
        let response = egui::Modal::new(egui::Id::new("confirm_quit")).show(ctx, |ui| {
            ui.set_max_width(420.0);
            ui.heading("Convoys are still running");
            ui.label(format!("Running jobs: {running} · Queued jobs: {queued}"));
            ui.label("Closing CodeConvoy will stop all active jobs.");
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
