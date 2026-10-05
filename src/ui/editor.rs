use super::*;
use crate::{
    agents::{OptionKind, value},
    domain::AgentId,
};

impl App {
    pub(super) fn editor(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.heading("Task");
        ui.label("Describe the change or review to perform in each repository.");
        self.dirty |= ui
            .add(
                egui::TextEdit::multiline(&mut self.state.draft.prompt)
                    .desired_rows(7)
                    .desired_width(f32::INFINITY)
                    .hint_text("Update these extensions for GNOME 52 compatibility…"),
            )
            .changed();
        ui.add_space(8.0);
        let old_agent = self.state.draft.agent;
        let mut chosen_agent = old_agent;
        egui::ComboBox::from_label("Agent")
            .selected_text(old_agent.label())
            .show_ui(ui, |ui| {
                for agent in AgentId::ALL {
                    ui.selectable_value(&mut chosen_agent, agent, agent.label());
                }
            });
        if chosen_agent != old_agent {
            self.state.select_agent(chosen_agent);
            self.dirty = true;
        }
        match agents::backend(self.state.draft.agent) {
            Ok(backend) => {
                for spec in backend.options() {
                    ui.push_id(spec.key, |ui| {
                        let mut current = value(&self.state.draft.options, spec).to_owned();
                        match spec.kind {
                            OptionKind::Text { hint } => {
                                let label = ui.label(spec.label).on_hover_text(spec.help);
                                if ui
                                    .add(
                                        egui::TextEdit::singleline(&mut current)
                                            .hint_text(hint)
                                            .desired_width(f32::INFINITY),
                                    )
                                    .labelled_by(label.id)
                                    .on_hover_text(spec.help)
                                    .changed()
                                {
                                    self.state.draft.options.insert(spec.key.into(), current);
                                    self.dirty = true;
                                }
                            }
                            OptionKind::Choice(choices) => {
                                let previous = current.clone();
                                let selected = choices
                                    .iter()
                                    .find(|(v, _)| *v == current)
                                    .map_or("Unknown value", |(_, label)| *label);
                                egui::ComboBox::from_label(spec.label)
                                    .selected_text(selected)
                                    .show_ui(ui, |ui| {
                                        for (value, label) in choices {
                                            ui.selectable_value(
                                                &mut current,
                                                (*value).into(),
                                                *label,
                                            )
                                            .on_hover_text(spec.help);
                                        }
                                    });
                                if previous != current {
                                    self.state.draft.options.insert(spec.key.into(), current);
                                    self.dirty = true;
                                }
                            }
                        }
                    });
                }
                ui.small(backend.execution_summary(&self.state.draft.options));
                if ui
                    .add_enabled(!self.busy, egui::Button::new("Check CLI"))
                    .clicked()
                {
                    self.busy = true;
                    let options = self.state.draft.options.clone();
                    let agent = self.state.draft.agent;
                    let directory = self.store.directory().to_owned();
                    self.dispatch(ctx.clone(), async move {
                        let result = agents::detect(backend.as_ref(), &options, &directory)
                            .await
                            .map_err(|e| format!("{e:#}"));
                        Message::Detected(agent, options, result)
                    });
                }
                let status = self.detection.as_ref().filter(|(agent, options, _)| {
                    *agent == self.state.draft.agent && *options == self.state.draft.options
                });
                ui.small(status.map_or(
                    "Check the selected CLI before running.",
                    |(_, _, message)| message.as_str(),
                ));
            }
            Err(_) => {
                ui.label("This backend is planned. Select an implemented backend to run a task.");
            }
        }
        ui.separator();
        ui.heading("Repositories");
        ui.label("Register the root of an existing Git working tree.");
        ui.add(
            egui::TextEdit::singleline(&mut self.repository_input)
                .hint_text("/path/to/repository")
                .desired_width(f32::INFINITY),
        );
        ui.horizontal(|ui| {
            if ui
                .add_enabled(
                    !self.busy && !self.repository_input.trim().is_empty(),
                    egui::Button::new("Register path"),
                )
                .clicked()
            {
                self.register(ctx.clone());
            }
            if ui
                .add_enabled(!self.busy, egui::Button::new("Refresh Git state"))
                .clicked()
            {
                self.refresh(ctx.clone());
            }
            if self.busy {
                ui.spinner();
            }
        });
        if self.state.repositories.is_empty() {
            ui.weak("No repositories registered yet.");
        }
        let mut remove = None;
        for (index, repository) in self.state.repositories.iter().enumerate() {
            ui.push_id(&repository.path, |ui| {
                ui.horizontal(|ui| {
                    let mut selected = self.selected.contains(&repository.path);
                    if ui
                        .checkbox(&mut selected, &repository.name)
                        .on_hover_text(repository.path.display().to_string())
                        .changed()
                    {
                        if selected {
                            self.selected.insert(repository.path.clone());
                        } else {
                            self.selected.remove(&repository.path);
                        }
                    }
                    if ui
                        .add_enabled(self.active.is_none(), egui::Button::new("Remove").small())
                        .on_hover_text("Unregister only; files on disk are untouched.")
                        .clicked()
                    {
                        remove = Some(index);
                    }
                });
                match self.repository_states.get(&repository.path) {
                    Some(Ok(state)) => {
                        let label = if state.summary.changed == 0 {
                            "clean".into()
                        } else {
                            format!("{} changed entries", state.summary.changed)
                        };
                        ui.colored_label(
                            if state.summary.changed == 0 {
                                Color32::LIGHT_GREEN
                            } else {
                                Color32::YELLOW
                            },
                            format!("{} · {label}", state.summary.branch),
                        );
                        if !state.entries.is_empty() {
                            ui.collapsing("Existing changes", |ui| {
                                for entry in state.entries.iter().take(100) {
                                    ui.monospace(entry);
                                }
                                if state.entries.len() > 100 {
                                    ui.label("More entries; use preflight or Git to inspect.");
                                }
                            });
                        }
                    }
                    Some(Err(error)) => {
                        ui.colored_label(Color32::LIGHT_RED, error);
                    }
                    None => {
                        ui.weak("State needs refresh; checked again before execution.");
                    }
                }
            });
        }
        if let Some(index) = remove {
            let repo = self.state.repositories.remove(index);
            self.selected.remove(&repo.path);
            self.repository_states.remove(&repo.path);
            self.dirty = true;
        }
        ui.separator();
        self.dirty |= ui
            .add(
                egui::Slider::new(&mut self.state.draft.concurrency, 1..=16)
                    .text("Concurrent jobs"),
            )
            .changed();
        ui.small("Runs use the selected folders directly. Agent permissions apply; review resulting changes before committing.");
        let enabled = self.active.is_none()
            && !self.busy
            && !self.selected.is_empty()
            && !self.state.draft.prompt.trim().is_empty()
            && agents::backend(self.state.draft.agent).is_ok();
        if ui
            .add_enabled(
                enabled,
                egui::Button::new(format!(
                    "Review & run in {} repositories",
                    self.selected.len()
                )),
            )
            .clicked()
        {
            self.preflight(ctx.clone());
        }
        ui.small("Prompts and run metadata are saved locally. Do not put credentials in the task.");
    }
    pub(super) fn preflight_window(&mut self, ctx: &egui::Context) {
        let Some(prepared) = &self.prepared else {
            return;
        };
        let mut start = false;
        let mut cancel = false;
        egui::Window::new("Review repository state")
            .collapsible(false)
            .resizable(true)
            .default_width(570.0)
            .show(ctx, |ui| {
                ui.label(format!(
                    "{} jobs · up to {} running at once",
                    prepared.repositories.len(),
                    prepared.task.concurrency
                ));
                ui.label(prepared.task.agent.label());
                if let Ok(backend) = agents::backend(prepared.task.agent) {
                    ui.label(backend.execution_summary(&prepared.task.options));
                }
                egui::ScrollArea::vertical()
                    .max_height(330.0)
                    .show(ui, |ui| {
                        for entry in &prepared.repositories {
                            ui.separator();
                            ui.strong(&entry.repository.name);
                            ui.small(entry.repository.path.display().to_string());
                            ui.label(format!(
                                "{} · {} existing changes",
                                entry.state.summary.branch, entry.state.summary.changed
                            ));
                            for line in &entry.state.entries {
                                ui.monospace(line);
                            }
                        }
                    });
                if prepared.dirty() {
                    ui.colored_label(
                        Color32::YELLOW,
                        "Some repositories already have changes. Agent edits may overlap them.",
                    );
                    ui.checkbox(
                        &mut self.dirty_ack,
                        "I reviewed the existing changes and want to proceed.",
                    );
                }
                ui.horizontal(|ui| {
                    start = ui
                        .add_enabled(
                            !prepared.dirty() || self.dirty_ack,
                            egui::Button::new("Run jobs"),
                        )
                        .clicked();
                    cancel = ui.button("Back to task").clicked();
                });
            });
        if start {
            self.start();
        } else if cancel {
            self.prepared = None;
        }
    }
}
