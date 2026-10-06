use super::*;
use domain::{RepositoryGroup, TaskTemplate};

pub(super) enum LibraryEditor {
    Group {
        index: Option<usize>,
        value: RepositoryGroup,
        error: String,
    },
    Template {
        index: Option<usize>,
        value: TaskTemplate,
        error: String,
    },
}

impl App {
    pub(super) fn load_template(&mut self, index: usize) {
        if self.prepared.is_some() || self.closing {
            return;
        }
        let Some(template) = self.state.templates.get(index) else {
            return;
        };
        self.state.draft.prompt = template.prompt.clone();
        self.draft_message = format!("Loaded {}. Task text remains editable.", template.name);
        self.focus_draft = true;
        self.dirty = true;
    }

    pub(super) fn template_menu(&mut self, ui: &mut egui::Ui) {
        let mut load = None;
        ui.menu_button("Templates", |ui| {
            ui.set_max_width(360.0);
            for (index, template) in self.state.templates.iter().enumerate() {
                ui.horizontal(|ui| {
                    ui.add_sized([200.0, 24.0], egui::Label::new(&template.name).truncate())
                        .on_hover_text(&template.name);
                    if ui.button("Load").clicked() {
                        load = Some(index);
                        ui.close();
                    }
                    if ui.button("Edit…").clicked() {
                        self.library_editor = Some(LibraryEditor::Template {
                            index: Some(index),
                            value: template.clone(),
                            error: String::new(),
                        });
                        ui.close();
                    }
                });
            }
            if self.state.templates.is_empty() {
                ui.weak("No saved templates");
            }
            ui.separator();
            if ui
                .add_enabled(
                    !self.state.draft.prompt.trim().is_empty(),
                    egui::Button::new("Save current task as template…"),
                )
                .clicked()
            {
                self.library_editor = Some(LibraryEditor::Template {
                    index: None,
                    value: TaskTemplate {
                        name: String::new(),
                        prompt: self.state.draft.prompt.clone(),
                    },
                    error: String::new(),
                });
                ui.close();
            }
        });
        if let Some(index) = load {
            self.load_template(index);
        }
    }

    /// No group execution state: membership actions change only explicit paths.
    pub(super) fn group_members(&self, group: &RepositoryGroup) -> (Vec<PathBuf>, Vec<PathBuf>) {
        group.repositories.iter().cloned().partition(|path| {
            self.state.repositories.iter().any(|r| &r.path == path)
                && !matches!(self.repository_states.get(path), Some(Err(_)))
        })
    }

    pub(super) fn select_group(&mut self, index: usize, selected: bool) {
        let Some(group) = self.state.groups.get(index) else {
            return;
        };
        let (members, missing) = self.group_members(group);
        if selected {
            self.selected.extend(members);
            if !missing.is_empty() {
                self.notice = format!(
                    "Group {}: {} unavailable or unregistered member(s) were not selected. Refresh repository state or repair the group. Membership is retained.",
                    group.name,
                    missing.len()
                );
            }
        } else {
            for path in &group.repositories {
                self.selected.remove(path);
            }
        }
    }

    pub(super) fn groups_section(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.small("Groups");
            if ui.add(theme::quiet("Manage groups…").small()).clicked() {
                self.library_editor = Some(LibraryEditor::Group {
                    index: None,
                    value: RepositoryGroup {
                        name: String::new(),
                        repositories: Vec::new(),
                    },
                    error: String::new(),
                });
            }
        });
        let mut action = None;
        for (index, group) in self.state.groups.iter().enumerate() {
            let (members, missing) = self.group_members(group);
            let count = members
                .iter()
                .filter(|path| self.selected.contains(*path))
                .count();
            let mut selected = !members.is_empty() && count == members.len();
            ui.horizontal_wrapped(|ui| {
                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
                let response = ui.add_enabled(!self.busy && !members.is_empty(),
                    egui::Checkbox::new(&mut selected, format!("{} · {} repos", group.name, group.repositories.len()))
                        .indeterminate(count > 0 && (count < members.len() || !missing.is_empty())))
                    .on_hover_text("Select adds available members. Deselect removes every current member, including individual and overlapping-group selections.");
                if response.changed() { action = Some((index, selected)); }
                if !missing.is_empty() {
                    ui.colored_label(theme::Palette::of(ui).warning, format!("{} unavailable", missing.len()))
                        .on_hover_text(missing.iter().map(|p| p.display().to_string()).collect::<Vec<_>>().join("\n"));
                }
            });
        }
        if let Some((index, selected)) = action {
            self.select_group(index, selected);
        }
    }

    pub(super) fn library_window(&mut self, ctx: &egui::Context) {
        let Some(mut editor) = self.library_editor.take() else {
            return;
        };
        let mut close = false;
        let response = egui::Modal::new(egui::Id::new("task_library")).show(ctx, |ui| {
            ui.set_width(480.0_f32.min((ctx.content_rect().width() - 64.0).max(240.0)));
            match &mut editor {
                LibraryEditor::Group { index, value, error } => {
                    ui.heading("Repository groups");
                    let previous = *index;
                    egui::ComboBox::from_id_salt("edit_group").width(ui.available_width())
                        .selected_text(index.and_then(|i| self.state.groups.get(i)).map_or("New group", |g| g.name.as_str()))
                        .show_ui(ui, |ui| {
                            ui.selectable_value(index, None, "New group");
                            for (i, group) in self.state.groups.iter().enumerate() { ui.selectable_value(index, Some(i), &group.name); }
                        });
                    if previous != *index {
                        *value = index.and_then(|i| self.state.groups.get(i)).cloned().unwrap_or(RepositoryGroup { name: String::new(), repositories: Vec::new() });
                        error.clear();
                    }
                    ui.label("Name");
                    ui.text_edit_singleline(&mut value.name);
                    ui.small("Membership changes only the group. Runs and current selections stay unchanged.");
                    egui::ScrollArea::vertical().max_height((ctx.content_rect().height() - 320.0).max(100.0)).show(ui, |ui| {
                        for repository in &self.state.repositories {
                            let mut member = value.repositories.contains(&repository.path);
                            let response = ui.checkbox(&mut member, &repository.name).on_hover_text(repository.path.display().to_string());
                            if response.changed() {
                                if member { value.repositories.push(repository.path.clone()); } else { value.repositories.retain(|p| p != &repository.path); }
                            }
                            if matches!(self.repository_states.get(&repository.path), Some(Err(_))) {
                                ui.colored_label(theme::Palette::of(ui).warning, "Unavailable · membership retained");
                            }
                        }
                        let orphaned: Vec<_> = value.repositories.iter().filter(|path| !self.state.repositories.iter().any(|r| &r.path == *path)).cloned().collect();
                        for path in orphaned {
                            ui.horizontal_wrapped(|ui| {
                                ui.colored_label(theme::Palette::of(ui).warning, format!("Unregistered: {}", path.display()));
                                if ui.button("Remove membership").clicked() { value.repositories.retain(|p| p != &path); }
                            });
                        }
                    });
                    if !error.is_empty() { ui.colored_label(theme::Palette::of(ui).error, error.as_str()); }
                    ui.horizontal_wrapped(|ui| {
                        if ui.button("Save group").clicked() {
                            match self.state.save_group(*index, value.clone()) {
                                Ok(()) => { self.dirty = true; close = true; }
                                Err(e) => *error = e.to_string(),
                            }
                        }
                        if let Some(i) = *index && ui.button("Delete group").on_hover_text("Remove only this group; registrations, selections and runs stay intact.").clicked() {
                            self.state.groups.remove(i); self.dirty = true; close = true;
                        }
                    });
                }
                LibraryEditor::Template { index, value, error } => {
                    ui.heading(if index.is_some() { "Edit template" } else { "Save task template" });
                    ui.label("Name");
                    ui.text_edit_singleline(&mut value.name);
                    ui.label("Task text");
                    egui::ScrollArea::vertical().max_height((ctx.content_rect().height() - 270.0).max(100.0)).show(ui, |ui| {
                        ui.add(egui::TextEdit::multiline(&mut value.prompt).desired_rows(8).desired_width(f32::INFINITY));
                    });
                    ui.small("Templates store only name and task text. Load from Templates to populate the draft.");
                    if !error.is_empty() { ui.colored_label(theme::Palette::of(ui).error, error.as_str()); }
                    ui.horizontal_wrapped(|ui| {
                        if ui.button("Save template").clicked() {
                            match self.state.save_template(*index, value.clone()) {
                                Ok(()) => { self.dirty = true; close = true; }
                                Err(e) => *error = e.to_string(),
                            }
                        }
                        if let Some(i) = *index && ui.button("Delete template").clicked() { self.state.templates.remove(i); self.dirty = true; close = true; }
                    });
                }
            }
            if ui.button("Close").clicked() { close = true; }
        });
        close |= response.should_close();
        if !close {
            self.library_editor = Some(editor);
        }
    }
}
