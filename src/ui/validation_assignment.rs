use super::*;
use crate::validation::{
    configuration::{Assignment, Existing, Report, Selection},
    presets::BUILT_INS,
};

#[derive(Default)]
pub(super) struct Dialog {
    pub selection: Selection,
    pub preset: usize,
    pub existing: Existing,
    pub review: Option<Assignment>,
    pub report: Option<Report>,
    focus: bool,
}
impl Dialog {
    pub fn new() -> Self {
        Self {
            focus: true,
            ..Default::default()
        }
    }
}

impl App {
    pub(super) fn validation_assignment_window(&mut self, ctx: &egui::Context) {
        let Some(dialog) = &mut self.validation_assignment else {
            return;
        };
        let mut close = false;
        let mut review = false;
        let mut back = false;
        let mut apply = false;
        let response = egui::Modal::new(egui::Id::new("validation_assignment")).show(ctx, |ui| {
            ui.set_width(450.0_f32.min(ctx.content_rect().width() - 60.0));
            ui.heading("Assign validation preset");
            ui.small("Configuration only. No validation or agent will run.");
            if let Some(report) = &dialog.report {
                ui.label(report.summary());
                egui::ScrollArea::vertical()
                    .max_height((ctx.content_rect().height() - 210.0).max(80.0))
                    .show(ui, |ui| {
                        for (label, repos) in
                            [("Updated", &report.updated), ("Preserved", &report.skipped)]
                        {
                            for repo in repos {
                                ui.add(egui::Label::new(format!("{label}: {}", repo.name)).wrap())
                                    .on_hover_text(repo.path.display().to_string());
                            }
                        }
                        for (repo, error) in &report.failed {
                            ui.label(format!("Failed: {}", repo.name));
                            ui.add(egui::Label::new(repo.path.display().to_string()).wrap());
                            ui.colored_label(theme::Palette::of(ui).error, error);
                        }
                    });
                let button = ui.button("Close");
                if dialog.focus {
                    button.request_focus();
                    dialog.focus = false;
                }
                close = button.clicked();
            } else if let Some(assignment) = &dialog.review {
                ui.strong(format!("Preset: {}", assignment.preset_name));
                ui.add(
                    egui::Label::new(egui::RichText::new(assignment.command.preview()).monospace())
                        .wrap(),
                );
                ui.label(format!(
                    "{} repositories · {} existing configurations",
                    assignment.targets.len(),
                    assignment.existing_count()
                ));
                if assignment.existing == Existing::Overwrite {
                    ui.colored_label(
                        theme::Palette::of(ui).warning,
                        format!(
                            "Confirm replacement of {} existing configurations.",
                            assignment.existing_count()
                        ),
                    );
                } else {
                    ui.label("Existing configurations will be preserved.");
                }
                if assignment.missing_count() > 0 {
                    ui.label(format!(
                        "{} unregistered targets will be reported as failed.",
                        assignment.missing_count()
                    ));
                }
                egui::ScrollArea::vertical()
                    .max_height((ctx.content_rect().height() - 300.0).max(80.0))
                    .show(ui, |ui| {
                        for target in &assignment.targets {
                            let action = if target.previous.is_some() {
                                if assignment.existing == Existing::Preserve {
                                    "Preserve"
                                } else {
                                    "Replace"
                                }
                            } else {
                                "Assign"
                            };
                            ui.add(
                                egui::Label::new(format!("{action}: {}", target.repository.name))
                                    .truncate(),
                            )
                            .on_hover_text(target.repository.path.display().to_string());
                        }
                    });
                ui.horizontal_wrapped(|ui| {
                    let cancel = ui.button("Cancel");
                    if dialog.focus {
                        cancel.request_focus();
                        dialog.focus = false;
                    }
                    close = cancel.clicked();
                    back = ui.button("Back").clicked();
                    apply = ui
                        .button(if assignment.existing == Existing::Overwrite {
                            "Confirm overwrite"
                        } else {
                            "Confirm assignment"
                        })
                        .clicked();
                });
            } else {
                egui::ComboBox::from_label("Preset")
                    .selected_text(BUILT_INS[dialog.preset].name)
                    .show_ui(ui, |ui| {
                        for (index, preset) in BUILT_INS.iter().enumerate() {
                            if ui
                                .selectable_value(&mut dialog.preset, index, preset.name)
                                .on_hover_text(preset.description)
                                .clicked()
                            {
                                ui.close();
                            }
                        }
                    });
                ui.monospace(BUILT_INS[dialog.preset].command().preview());
                let existing = dialog
                    .selection
                    .paths
                    .iter()
                    .filter(|p| {
                        self.state.repositories.iter().any(|r| r.path == **p)
                            && self.state.repository_validation.contains_key(*p)
                    })
                    .count();
                ui.label(format!(
                    "{} repositories · {existing} existing configurations",
                    dialog.selection.paths.len()
                ));
                ui.small("Groups select members here; convoy selection is unchanged.");
                egui::ScrollArea::vertical()
                    .id_salt("validation_targets")
                    .max_height((ctx.content_rect().height() - 340.0).clamp(80.0, 340.0))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            if ui.button("All repositories").clicked() {
                                dialog
                                    .selection
                                    .paths
                                    .extend(self.state.repositories.iter().map(|r| r.path.clone()));
                            }
                            if ui.button("Clear selection").clicked() {
                                dialog.selection.paths.clear();
                            }
                        });
                        if !self.state.groups.is_empty() {
                            ui.strong("Groups");
                        }
                        for (index, group) in self.state.groups.iter().enumerate() {
                            ui.push_id(("group", index), |ui| {
                                let mut selected = !group.repositories.is_empty()
                                    && group
                                        .repositories
                                        .iter()
                                        .all(|p| dialog.selection.paths.contains(p));
                                let partial = !selected
                                    && group
                                        .repositories
                                        .iter()
                                        .any(|p| dialog.selection.paths.contains(p));
                                let response = ui.add_enabled(
                                    !group.repositories.is_empty(),
                                    egui::Checkbox::new(&mut selected, &group.name)
                                        .indeterminate(partial),
                                );
                                if response.has_focus() {
                                    response.scroll_to_me(None);
                                }
                                if response.changed() {
                                    dialog.selection.select_group(group, selected);
                                }
                            });
                        }
                        ui.strong("Repositories");
                        for repo in &self.state.repositories {
                            ui.push_id(&repo.path, |ui| {
                                let mut selected = dialog.selection.paths.contains(&repo.path);
                                let response = ui
                                    .add(egui::Checkbox::new(&mut selected, &repo.name))
                                    .on_hover_text(repo.path.display().to_string());
                                if response.has_focus() {
                                    response.scroll_to_me(None);
                                }
                                if response.changed() {
                                    if selected {
                                        dialog.selection.paths.insert(repo.path.clone());
                                    } else {
                                        dialog.selection.paths.remove(&repo.path);
                                    }
                                }
                            });
                        }
                    });
                ui.radio_value(
                    &mut dialog.existing,
                    Existing::Preserve,
                    "Only repositories without validation (default)",
                );
                ui.radio_value(
                    &mut dialog.existing,
                    Existing::Overwrite,
                    format!("Overwrite {existing} existing configurations"),
                );
                ui.horizontal_wrapped(|ui| {
                    let cancel = ui.button("Cancel");
                    if dialog.focus {
                        cancel.request_focus();
                        dialog.focus = false;
                    }
                    close = cancel.clicked();
                    review = ui
                        .add_enabled(
                            !dialog.selection.paths.is_empty(),
                            egui::Button::new("Review assignment…"),
                        )
                        .clicked();
                });
            }
        });
        if close || response.should_close() {
            self.validation_assignment = None;
            return;
        }
        if review {
            dialog.review = Some(Assignment::review(
                &self.state,
                &dialog.selection,
                &BUILT_INS[dialog.preset],
                dialog.existing,
            ));
            dialog.focus = true;
        }
        if back {
            dialog.review = None;
            dialog.focus = true;
        }
        if apply && let Some(assignment) = dialog.review.take() {
            let report = self.store.assign_validation(&mut self.state, &assignment);
            self.notice = report.summary();
            dialog.report = Some(report);
            dialog.focus = true;
        }
    }
}
