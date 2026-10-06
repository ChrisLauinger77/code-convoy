use super::*;
use crate::{
    agents::{OptionKind, value},
    domain::AgentId,
};

impl App {
    pub(super) fn check_cli(&mut self, ctx: egui::Context) {
        if self.checking_cli.is_some() {
            return;
        }
        let agent = self.state.draft.agent;
        let Ok(backend) = agents::backend(agent) else {
            return;
        };
        let options = self.state.draft.options.clone();
        self.checking_cli = Some((agent, options.clone()));
        self.detection = None;
        let directory = self.store.directory().to_owned();
        self.dispatch(ctx, async move {
            let result = agents::detect(backend.as_ref(), &options, &directory)
                .await
                .map_err(diagnostics::CliError::from_error);
            Message::Detected(agent, options, result)
        });
    }

    pub(super) fn agent_section(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
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
                let mut find_cli = false;
                for spec in backend.options() {
                    ui.push_id(spec.key, |ui| {
                        ui.horizontal(|ui| {
                            let label = format::option_label(spec);
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
                                    let executable = spec.key == "executable";
                                    let field_width = if executable {
                                        field_width - 68.0 - ui.spacing().item_spacing.x
                                    } else {
                                        field_width
                                    };
                                    ui.add(
                                        egui::TextEdit::singleline(&mut current)
                                            .hint_text(hint)
                                            .desired_width(field_width),
                                    )
                                    .labelled_by(label.id)
                                    .on_hover_text(spec.help);
                                    if executable {
                                        find_cli = ui.add_enabled(
                                            self.checking_cli.is_none() && self.cli_search.is_none(),
                                            theme::quiet("Find CLI").min_size(egui::vec2(68.0, 0.0)),
                                        ).on_hover_text("Find installed executables, then choose one to check")
                                            .clicked();
                                    }
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
                if find_cli {
                    self.find_cli(ctx.clone());
                }
                ui.horizontal_wrapped(|ui| {
                    if ui
                        .add_enabled(
                            self.checking_cli.is_none() && self.cli_search.is_none(),
                            theme::quiet("Check CLI").small(),
                        )
                        .clicked()
                    {
                        self.check_cli(ctx.clone());
                    }
                    let p = theme::Palette::of(ui);
                    if self.current_cli_check() {
                        ui.spinner();
                        ui.weak("Checking…");
                    } else {
                        match self.current_detection().map(|(_, _, r)| r) {
                            Some(Ok(message)) => {
                                theme::success_label(ui, "Available").on_hover_text(message);
                            }
                            Some(Err(error)) => {
                                ui.colored_label(p.warning, error.label());
                            }
                            None => {
                                ui.weak("Unchecked");
                            }
                        }
                    }
                });
                if let Some((_, _, Err(error))) = self.current_detection() {
                    ui.small(error.hint());
                    diagnostics::details(ui, "cli_diagnostics", &error.detail);
                }
                egui::CollapsingHeader::new("Execution permissions").show(ui, |ui| {
                    ui.small(backend.execution_summary(&self.state.draft.options));
                });
            }
            Err(error) => {
                ui.weak(format!("Backend unavailable: {error}"));
            }
        }
    }
}
