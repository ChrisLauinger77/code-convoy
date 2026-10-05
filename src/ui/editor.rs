use super::*;
use crate::{
    agents::{OptionKind, value},
    domain::AgentId,
};

impl App {
    pub(super) fn editor(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        theme::eyebrow(ui, "NEW CONVOY");
        self.task_section(ui);
        self.agent_section(ui, ctx);
        self.repositories_section(ui, ctx);
    }

    fn task_section(&mut self, ui: &mut egui::Ui) {
        theme::section(ui, "Task");
        ui.weak("One instruction, applied to each repository.");
        self.dirty |= ui
            .add(
                egui::TextEdit::multiline(&mut self.state.draft.prompt)
                    .desired_rows(5)
                    .desired_width(f32::INFINITY)
                    .hint_text("Describe the change or review…"),
            )
            .changed();
    }

    fn agent_section(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        theme::section(ui, "Agent");
        let old_agent = self.state.draft.agent;
        let mut chosen_agent = old_agent;
        egui::ComboBox::from_id_salt("agent")
            .width(ui.available_width())
            .selected_text(old_agent.label())
            .show_ui(ui, |ui| {
                for agent in AgentId::ALL {
                    ui.selectable_value(&mut chosen_agent, agent, agent.label());
                }
            })
            .response
            .on_hover_text("Agent used for every repository in this convoy");
        if chosen_agent != old_agent {
            self.state.select_agent(chosen_agent);
            self.dirty = true;
        }
        match agents::backend(self.state.draft.agent) {
            Ok(backend) => {
                for spec in backend.options() {
                    ui.push_id(spec.key, |ui| {
                        ui.horizontal(|ui| {
                            let label = match spec.key {
                                "executable" => "Executable",
                                "model_reasoning_effort" | "reasoning_effort" => "Reasoning",
                                "temp_access" => "Temp directory",
                                _ => spec.label,
                            };
                            let label = ui
                                .allocate_ui_with_layout(
                                    egui::vec2(100.0, 26.0),
                                    egui::Layout::left_to_right(egui::Align::Center),
                                    |ui| {
                                        ui.set_min_width(100.0);
                                        ui.label(label).on_hover_text(spec.help)
                                    },
                                )
                                .inner;
                            let mut current = value(&self.state.draft.options, spec).to_owned();
                            let previous = current.clone();
                            let field_width = ui.available_width();
                            match spec.kind {
                                OptionKind::Text { hint } => {
                                    ui.add(
                                        egui::TextEdit::singleline(&mut current)
                                            .hint_text(hint)
                                            .desired_width(field_width),
                                    )
                                    .labelled_by(label.id)
                                    .on_hover_text(spec.help);
                                }
                                OptionKind::Choice(choices) => {
                                    let selected = choices
                                        .iter()
                                        .find(|(v, _)| *v == current)
                                        .map_or("Unknown value", |(_, label)| *label);
                                    egui::ComboBox::from_id_salt(spec.key)
                                        .width(field_width)
                                        .truncate()
                                        .selected_text(selected)
                                        .show_ui(ui, |ui| {
                                            for (value, label) in choices {
                                                ui.selectable_value(
                                                    &mut current,
                                                    (*value).into(),
                                                    *label,
                                                );
                                            }
                                        })
                                        .response
                                        .labelled_by(label.id)
                                        .on_hover_text(spec.help);
                                }
                            }
                            if previous != current {
                                self.state.draft.options.insert(spec.key.into(), current);
                                self.dirty = true;
                            }
                        });
                    });
                }
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(!self.busy, theme::quiet("Check CLI"))
                        .clicked()
                    {
                        self.busy = true;
                        let options = self.state.draft.options.clone();
                        let agent = self.state.draft.agent;
                        let directory = self.store.directory().to_owned();
                        let backend = backend.clone();
                        self.dispatch(ctx.clone(), async move {
                            let result = agents::detect(backend.as_ref(), &options, &directory)
                                .await
                                .map_err(|e| format!("{e:#}"));
                            Message::Detected(agent, options, result)
                        });
                    }
                    if self.busy {
                        ui.spinner();
                    }
                    if self.current_detection().is_none() {
                        ui.weak("Not checked");
                    }
                });
                if let Some((_, _, result)) = self.current_detection() {
                    match result {
                        Ok(message) => {
                            ui.colored_label(theme::Palette::of(ui).success, "CLI available")
                                .on_hover_text(message);
                        }
                        Err(message) => {
                            ui.colored_label(theme::Palette::of(ui).error, "CLI check failed");
                            ui.small(message);
                        }
                    }
                }
                egui::CollapsingHeader::new("Execution permissions").show(ui, |ui| {
                    ui.small(backend.execution_summary(&self.state.draft.options));
                });
            }
            Err(_) => {
                ui.weak("Planned backend. Choose Codex or Copilot to run a task.");
            }
        }
    }

    fn repositories_section(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        theme::section(ui, "Repositories");
        ui.add(
            egui::TextEdit::singleline(&mut self.repository_input)
                .hint_text("/path/to/git/repository")
                .desired_width(f32::INFINITY),
        );
        ui.horizontal(|ui| {
            if ui
                .add_enabled(
                    !self.busy && !self.repository_input.trim().is_empty(),
                    egui::Button::new("Add repository"),
                )
                .clicked()
            {
                self.register(ctx.clone());
            }
            if ui
                .add_enabled(!self.busy, theme::quiet("Refresh state"))
                .clicked()
            {
                self.refresh(ctx.clone());
            }
        });
        if self.state.repositories.is_empty() {
            ui.weak("Add an existing Git working tree to get started.");
        }
        let mut remove = None;
        let p = theme::Palette::of(ui);
        for (index, repository) in self.state.repositories.iter().enumerate() {
            ui.push_id(&repository.path, |ui| {
                ui.separator();
                ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .add_enabled(self.active.is_none(), theme::quiet("Remove").small())
                            .on_hover_text("Unregister only; files on disk are untouched.")
                            .clicked()
                        {
                            remove = Some(index);
                        }
                        let mut selected = self.selected.contains(&repository.path);
                        let width = ui.available_width();
                        if ui
                            .allocate_ui_with_layout(
                                egui::vec2(width, 26.0),
                                egui::Layout::left_to_right(egui::Align::Center),
                                |ui| {
                                    ui.set_min_width(width);
                                    ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
                                    ui.checkbox(
                                        &mut selected,
                                        egui::RichText::new(&repository.name).strong(),
                                    )
                                },
                            )
                            .inner
                            .on_hover_text(repository.path.display().to_string())
                            .changed()
                        {
                            if selected {
                                self.selected.insert(repository.path.clone());
                            } else {
                                self.selected.remove(&repository.path);
                            }
                        }
                    });
                });
                match self.repository_states.get(&repository.path) {
                    Some(Ok(state)) => {
                        ui.horizontal_wrapped(|ui| {
                            ui.label(
                                egui::RichText::new(&state.summary.branch)
                                    .small()
                                    .color(p.muted),
                            );
                            if state.summary.changed == 0 {
                                ui.label(egui::RichText::new("Clean").small().color(p.success));
                            } else {
                                ui.label(
                                    egui::RichText::new(format!(
                                        "Dirty · {} changed entries",
                                        state.summary.changed
                                    ))
                                    .small()
                                    .color(p.warning),
                                );
                            }
                        });
                        if !state.entries.is_empty() {
                            ui.collapsing("Existing changes", |ui| {
                                egui::ScrollArea::both().max_height(150.0).show(ui, |ui| {
                                    for entry in state.entries.iter().take(100) {
                                        ui.add(
                                            egui::Label::new(
                                                egui::RichText::new(entry).monospace(),
                                            )
                                            .extend(),
                                        );
                                    }
                                    if state.entries.len() > 100 {
                                        ui.label("More entries; inspect all in the run review.");
                                    }
                                });
                            });
                        }
                    }
                    Some(Err(error)) => {
                        ui.colored_label(p.error, "Git state unavailable")
                            .on_hover_text(error);
                    }
                    None => {
                        ui.weak("State needs refresh · checked before running");
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
    }

    pub(super) fn execution_section(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.separator();
        ui.strong("Execution");
        ui.horizontal(|ui| {
            let label = ui.label("Concurrent jobs");
            self.dirty |= ui
                .add(
                    egui::DragValue::new(&mut self.state.draft.concurrency)
                        .range(1..=16)
                        .speed(0.1),
                )
                .labelled_by(label.id)
                .changed();
        });
        ui.weak(format!(
            "Run this task in {} selected repositories.",
            self.selected.len()
        ));
        let enabled = self.active.is_none()
            && !self.busy
            && !self.selected.is_empty()
            && !self.state.draft.prompt.trim().is_empty()
            && agents::backend(self.state.draft.agent).is_ok();
        if theme::primary(ui, enabled, "Run Convoy")
            .on_hover_text("Review repository state before starting jobs")
            .clicked()
        {
            self.preflight(ctx.clone());
        }
        let hint = if self.active.is_some() {
            "Wait for the current convoy to finish."
        } else if self.busy {
            "Checking CLI or repository state…"
        } else if self.state.draft.prompt.trim().is_empty() {
            "Enter a task, then select repositories."
        } else if self.selected.is_empty() {
            "Select at least one repository."
        } else {
            "Review Git state before execution. No automatic commits."
        };
        ui.small(hint);
    }

    pub(super) fn preflight_window(&mut self, ctx: &egui::Context) {
        let Some(prepared) = &self.prepared else {
            return;
        };
        let mut start = false;
        let mut cancel = false;
        egui::Window::new("Review convoy")
            .collapsible(false).resizable(true).default_width(540.0)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .max_width((ctx.content_rect().width() - 48.0).max(300.0))
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().max_height((ctx.content_rect().height() - 230.0).max(160.0)).show(ui, |ui| {
                    ui.strong(format!("{} repositories · {} concurrent jobs", prepared.repositories.len(), prepared.task.concurrency));
                    ui.label(prepared.task.agent.label());
                    if let Ok(backend) = agents::backend(prepared.task.agent) {
                        ui.small(backend.execution_summary(&prepared.task.options));
                    }
                    for entry in &prepared.repositories {
                        ui.separator();
                        ui.strong(&entry.repository.name);
                        ui.small(entry.repository.path.display().to_string());
                        ui.label(format!("{} · {} existing changes", entry.state.summary.branch, entry.state.summary.changed));
                        for line in &entry.state.entries { ui.label(egui::RichText::new(line).monospace()); }
                    }
                    if prepared.dirty() {
                        ui.add_space(theme::GAP);
                        ui.colored_label(theme::Palette::of(ui).warning, "Some repositories already have changes. Agent edits may overlap them.");
                        ui.checkbox(&mut self.dirty_ack, "I reviewed the existing changes and want to proceed.");
                    }
                });
                ui.separator();
                start = theme::primary(ui, !prepared.dirty() || self.dirty_ack, "Start convoy").clicked();
                cancel = ui.add(theme::quiet("Back to task")).clicked();
            });
        if start {
            self.start();
        } else if cancel {
            self.prepared = None;
        }
    }
}
