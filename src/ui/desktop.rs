//! UI routing shared by tray, native application actions and close events.
use super::*;
use crate::desktop::{Action, CloseBehavior, Status};

impl App {
    pub(super) fn desktop_settings(&mut self, ui: &mut egui::Ui) {
        ui.strong("Desktop workflow");
        self.dirty |= ui
            .checkbox(&mut self.state.desktop.enabled, "Enable system tray")
            .changed();
        match &self.desktop.status {
            Status::Disabled => {
                ui.small("System tray disabled.");
            }
            Status::Starting => {
                ui.small("Connecting to system tray…");
            }
            Status::Available => {
                ui.small("System tray available.");
            }
            Status::Unavailable(error) => {
                ui.colored_label(
                    theme::Palette::of(ui).warning,
                    "System tray unavailable. Window close uses Quit.",
                );
                ui.small(error);
                ui.small("Re-enable system tray to retry after fixing host support.");
            }
        }
        ui.add_enabled_ui(
            self.state.desktop.enabled && self.desktop.available(),
            |ui| {
                self.dirty |= ui
                    .checkbox(
                        &mut self.state.desktop.show_running_count,
                        "Show running convoy count in tray",
                    )
                    .changed();
            },
        );
        ui.label("Close window behavior");
        self.dirty |= ui
            .selectable_value(
                &mut self.state.desktop.close_behavior,
                CloseBehavior::Quit,
                "Quit application",
            )
            .changed();
        ui.add_enabled_ui(
            self.state.desktop.enabled && self.desktop.available(),
            |ui| {
                self.dirty |= ui
                    .selectable_value(
                        &mut self.state.desktop.close_behavior,
                        CloseBehavior::MinimizeToTray,
                        "Minimize to tray, when supported",
                    )
                    .changed();
            },
        );
        ui.small("Minimized convoys keep running. Dock/taskbar access stays available.");
        ui.separator();
        if ui.button("Quit CodeConvoy").clicked() {
            self.explicit_quit = true;
            ui.close();
        }
    }

    pub(super) fn desktop_action(&mut self, action: Action) {
        if self.closing {
            return;
        }
        match action {
            Action::Show => self.window.request_restore(),
            Action::ShowActive => {
                let selected_active = self
                    .state
                    .runs
                    .iter()
                    .any(|run| Some(run.id) == self.selected_run && run.active());
                if !selected_active {
                    let id = self
                        .state
                        .runs
                        .iter()
                        .find(|run| run.active())
                        .map(|run| run.id);
                    if id.is_some() {
                        self.select_run(id);
                    }
                }
                self.tab = Tab::Review;
                self.show_active_navigation = true;
                self.window.request_restore();
            }
            Action::Quit => self.explicit_quit = true,
        }
    }

    pub(super) fn sync_desktop(&mut self) {
        if self.desktop_native && !self.closing {
            let runtime = self.runtime().handle().clone();
            self.desktop.sync(
                self.state.desktop,
                self.state.runs.iter().filter(|run| run.active()).count(),
                &runtime,
            );
        }
        if self.window.tray_minimized && !self.desktop.available() && !self.closing {
            self.window.request_restore();
        }
    }
}
