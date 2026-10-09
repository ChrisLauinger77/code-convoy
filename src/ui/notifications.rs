use super::*;
use crate::notifications;

impl App {
    pub(super) fn update_notification_focus(&mut self, ctx: &egui::Context) {
        self.notification_focused = ctx.input(|i| {
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
                if self.closing {
                    return;
                }
                self.notification_activation = true;
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

    pub(super) fn activate_notification_window(&mut self, ctx: &egui::Context) {
        if self.notification_activation {
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
            ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
            // Focus cannot take effect while minimized. Defer it to a subsequent
            // event-loop pass after the restore commands have been processed.
            self.notification_activation = false;
            self.notification_focus_pending = true;
            ctx.request_repaint();
        } else if self.notification_focus_pending {
            ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
            self.notification_focus_pending = false;
        }
    }
}
