use super::*;
use crate::notifications;

impl App {
    pub(super) fn update_notification_focus(&mut self, ctx: &egui::Context) {
        // A close-to-tray frame can still carry focus from before minimization.
        if ctx.input(|i| i.viewport().minimized == Some(false)) {
            self.window.tray_minimized = false;
        }
        self.notification_focused = !self.window.tray_minimized
            && ctx.input(|i| {
                i.viewport().focused.unwrap_or(i.focused) && i.viewport().minimized != Some(true)
            });
    }
    pub(super) fn notification_settings(&mut self, ui: &mut egui::Ui) {
        ui.strong("Desktop notifications");
        let preferences = &mut self.state.notifications;
        self.dirty |= ui
            .checkbox(&mut preferences.enabled, "Enable desktop notifications")
            .changed();
        ui.add_enabled_ui(preferences.enabled, |ui| {
            for (value, label) in [
                (&mut preferences.success, "Successful completion"),
                (&mut preferences.failure, "Failure"),
                (&mut preferences.cancellation, "Cancellation"),
                (&mut preferences.review, "Review needed"),
                (
                    &mut preferences.suppress_foreground,
                    "Suppress while foreground",
                ),
            ] {
                self.dirty |= ui.checkbox(value, label).changed();
            }
        });
        ui.small("One per convoy: failure, cancellation, review, then success.");
        ui.small("Failures can notify while focused. System permissions still apply.");
        if !self.notification_error.is_empty() {
            ui.colored_label(theme::Palette::of(ui).warning, &self.notification_error);
            if ui.button("Dismiss notification diagnostic").clicked() {
                self.notification_error.clear();
            }
        }
    }

    pub(super) fn pump_notifications(&mut self) {
        if self.closing {
            return;
        }
        let runtime = self.runtime().handle().clone();
        for run in &self.state.runs {
            if let Some(snapshot) = self.notification_tracker.completed(run)
                && self.state.notifications.enabled
            {
                self.notification_service.assess(&runtime, snapshot);
            }
        }
        self.notification_service.reap();
        self.notification_tracker.retain(&self.state.runs);
    }

    pub(super) fn notification_event(&mut self, event: notifications::Event) {
        match event {
            notifications::Event::Ready(completion) => {
                if !self.closing
                    && self
                        .state
                        .notifications
                        .allows(completion.outcome, self.notification_focused)
                {
                    let runtime = self.runtime().handle().clone();
                    self.notification_service.submit(&runtime, completion);
                }
            }
            notifications::Event::Activated(id) => {
                if self.closing || !self.activated_notifications.insert(id) {
                    return;
                }
                if self.activated_notifications.len() > 1024 {
                    self.activated_notifications.retain(|run_id| {
                        self.state.runs.iter().any(|run| run.id == *run_id) || *run_id == id
                    });
                }
                self.window.request_restore();
                if self.state.runs.iter().any(|run| run.id == id) {
                    self.select_run(Some(id));
                    self.tab = Tab::Review;
                } else {
                    self.notice = format!("Convoy #{id} is no longer in history.");
                }
            }
            notifications::Event::Failed(id, error) => {
                self.notification_error =
                    format!("Convoy #{id}: desktop notification unavailable. {error}");
            }
        }
    }
}
