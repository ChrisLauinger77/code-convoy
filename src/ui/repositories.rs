use super::*;

impl App {
    fn browse_repository(&mut self, ctx: egui::Context) {
        if self.browsing_repository || self.busy {
            return;
        }
        self.browsing_repository = true;
        // Construct on the UI thread for macOS's NSOpenPanel sheet, then await
        // on Tokio. rfd handles Linux portal / Windows COM work off-thread.
        let selection = self
            .repository_dialog
            .clone()
            .set_directory(self.repository_input.trim())
            .pick_folder();
        self.dispatch(ctx, async move {
            Message::RepositoryFolder(selection.await.map(|folder| folder.path().to_owned()))
        });
    }

    pub(super) fn repository_folder_selected(&mut self, path: Option<PathBuf>) {
        self.browsing_repository = false;
        self.focus_repository_input = true;
        if let Some(path) = path {
            match path.to_str() {
                Some(text) => {
                    self.repository_input = text.to_owned();
                    self.picked_repository_path = Some(path);
                }
                None => {
                    self.notice = "The selected path cannot be represented as text. Choose a directory with a Unicode path.".into();
                }
            }
        }
        // Cancellation preserves the field. Selection never registers a repo:
        // only the explicit Add action calls the existing Git validation path.
    }

    pub(super) fn select_repositories(&mut self, selected: bool) {
        self.selected.clear();
        if selected {
            self.selected
                .extend(self.state.repositories.iter().map(|r| r.path.clone()));
        }
    }

    pub(super) fn repositories_section(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        theme::section(ui, "Repositories");
        let path = ui.add(
            egui::TextEdit::singleline(&mut self.repository_input)
                .hint_text("/path/to/git/repository")
                .desired_width(f32::INFINITY),
        );
        if path.changed() {
            self.picked_repository_path = None;
        }
        if self.focus_repository_input {
            path.request_focus();
            path.scroll_to_me(None);
            self.focus_repository_input = false;
        }
        ui.horizontal_wrapped(|ui| {
            if ui
                .add_enabled(
                    !self.busy && !self.browsing_repository,
                    theme::quiet("Browse…"),
                )
                .on_hover_text("Choose a folder, then use Add repository to register it")
                .clicked()
            {
                self.browse_repository(ctx.clone());
            }
            if ui
                .add_enabled(
                    !self.busy
                        && !self.browsing_repository
                        && !self.repository_input.trim().is_empty(),
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
        if self.browsing_repository {
            ui.weak("Choosing a folder…");
        }
        if self.state.repositories.is_empty() {
            ui.weak("Add an existing Git working tree to get started.");
        }
        if !self.state.repositories.is_empty() {
            ui.horizontal_wrapped(|ui| {
                ui.small(format!(
                    "{} of {} selected",
                    self.selected.len(),
                    self.state.repositories.len()
                ));
                if ui
                    .add_enabled(
                        self.selected.len() < self.state.repositories.len(),
                        theme::quiet("Select all").small(),
                    )
                    .clicked()
                {
                    self.select_repositories(true);
                }
                if ui
                    .add_enabled(
                        !self.selected.is_empty(),
                        theme::quiet("Select none").small(),
                    )
                    .clicked()
                {
                    self.select_repositories(false);
                }
            });
        }
        self.groups_section(ui);
        if !self.state.groups.is_empty() {
            ui.small("Individual repositories");
        }
        let mut remove = None;
        let p = theme::Palette::of(ui);
        for (index, repository) in self.state.repositories.iter().enumerate() {
            ui.push_id(&repository.path, |ui| {
                ui.separator();
                ui.horizontal(|ui| {
                    ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
                    let mut selected = self.selected.contains(&repository.path);
                    let width = (ui.available_width() - 80.0).max(40.0);
                    let response = ui
                        .allocate_ui_with_layout(
                            egui::vec2(width, 26.0),
                            egui::Layout::left_to_right(egui::Align::Center),
                            |ui| {
                                ui.set_min_width(width);
                                ui.add(egui::Checkbox::new(
                                    &mut selected,
                                    egui::RichText::new(&repository.name).strong(),
                                ))
                            },
                        )
                        .inner
                        .on_hover_text(repository.path.display().to_string());
                    if response.has_focus() {
                        response.scroll_to_me(None);
                    }
                    if response.changed() {
                        if selected {
                            self.selected.insert(repository.path.clone());
                        } else {
                            self.selected.remove(&repository.path);
                        }
                    }
                    if ui
                        .add(theme::quiet("Remove").small())
                        .on_hover_text(
                            "Unregister this repository only; files and history are untouched.",
                        )
                        .clicked()
                    {
                        remove = Some(index);
                    }
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
                                        "Dirty · {} {}",
                                        state.summary.changed,
                                        if state.summary.changed == 1 {
                                            "change"
                                        } else {
                                            "changes"
                                        }
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
                        ui.colored_label(p.error, "Repository unavailable");
                        diagnostics::details(ui, "repository_error", error);
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
}
