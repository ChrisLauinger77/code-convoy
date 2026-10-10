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
        ui.add_space(theme::SECTION_GAP);
        ui.horizontal_wrapped(|ui| {
            ui.label(egui::RichText::new("Repositories").strong().size(14.0));
            ui.small(format!(
                "{} / {} selected",
                self.selected.len(),
                self.state.repositories.len()
            ));
        });
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
        ui.small("Current Git state · refreshed on opening and on request");
        if !self.repository_pending.is_empty() {
            ui.small(format!(
                "Checking {} repositories…",
                self.repository_pending.len()
            ));
        }
        if self.state.repositories.is_empty() {
            ui.weak("Add an existing Git working tree to get started.");
        }
        if !self.state.repositories.is_empty() {
            ui.horizontal_wrapped(|ui| {
                if ui
                    .add_enabled(
                        self.selected.len() < self.state.repositories.len(),
                        theme::quiet("Select all").small(),
                    )
                    .on_hover_text(
                        "Select every registered repository, including those hidden by the filter",
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
                    .on_hover_text(
                        "Deselect every repository, including those hidden by the filter",
                    )
                    .clicked()
                {
                    self.select_repositories(false);
                }
            });
        }
        self.manage_groups_button(ui);
        if self.repository_filter_present() {
            ui.horizontal(|ui| {
                let search = ui.add_sized(
                    [
                        (ui.available_width() - 60.0).max(40.0),
                        ui.spacing().interact_size.y,
                    ],
                    egui::TextEdit::singleline(&mut self.repository_sections.query)
                        .id(shortcuts::repository_filter_id())
                        .hint_text("Filter repositories…")
                        .desired_width(f32::INFINITY),
                );
                search.widget_info(|| {
                    egui::WidgetInfo::labeled(
                        egui::WidgetType::TextEdit,
                        search.enabled(),
                        "Filter repositories",
                    )
                });
                if std::mem::take(&mut self.focus_repository_filter) {
                    search.request_focus();
                    search.scroll_to_me(None);
                }
                if search.has_focus() {
                    search.scroll_to_me(None);
                }
                if ui
                    .add_enabled(
                        !self.repository_sections.query.is_empty(),
                        theme::quiet("Clear").small(),
                    )
                    .on_hover_text("Clear repository filter")
                    .clicked()
                {
                    self.repository_sections.query.clear();
                    search.request_focus();
                }
            });
        }
        self.grouped_repositories(ui);
    }

    pub(super) fn grouped_repositories(&mut self, ui: &mut egui::Ui) {
        // Move out temporarily so rendering can mutate selection without cloning
        // the cached membership lists on every frame.
        let mut sections = std::mem::take(&mut self.repository_sections);
        sections.sync(&self.state.repositories, &self.state.groups);
        let filtering = sections.filtering();
        if filtering
            && !sections
                .sections
                .iter()
                .any(|section| sections.visible(section))
        {
            ui.weak("No repositories match. Clear the filter to see all.");
        }
        let mut remove = None;
        for section in sections
            .sections
            .iter()
            .filter(|section| sections.visible(section))
        {
            ui.push_id(section.id, |ui| {
                let counts = section.counts(self);
                let mut toggle = false;
                let mut selection = None;
                let header = if filtering {
                    // Search is a fully expanded presentation, never a write to
                    // the manual CollapsingState (including its animation).
                    ui.horizontal(|ui| {
                        let (_, arrow) = ui.allocate_exact_size(
                            egui::vec2(ui.spacing().indent, ui.spacing().icon_width),
                            egui::Sense::hover(),
                        );
                        egui::collapsing_header::paint_default_icon(ui, 1.0, &arrow);
                        (_, selection) =
                            group_header_controls(ui, section, &counts, self.busy, true);
                    });
                    None
                } else {
                    let mut header =
                        egui::collapsing_header::CollapsingState::load_with_default_open(
                            ui.ctx(),
                            section.id,
                            section.group.is_none(),
                        )
                        .show_header(ui, |ui| {
                            (toggle, selection) =
                                group_header_controls(ui, section, &counts, self.busy, false);
                        });
                    if toggle {
                        header.toggle();
                    }
                    Some(header)
                };
                if let Some(selected) = selection {
                    if let Some(index) = section.group {
                        self.select_group(index, selected);
                    } else {
                        for &index in &section.repositories {
                            let path = &self.state.repositories[index].path;
                            if selected {
                                if !matches!(self.repository_states.get(path), Some(Err(_))) {
                                    self.selected.insert(path.clone());
                                }
                            } else {
                                self.selected.remove(path);
                            }
                        }
                    }
                }
                let body = |ui: &mut egui::Ui| {
                    if section.repositories.is_empty() {
                        ui.weak("No registered repositories");
                    }
                    for &index in &section.repositories {
                        if sections.matches(index) && self.repository_row(ui, index) {
                            remove = Some(index);
                        }
                    }
                };
                if let Some(header) = header {
                    let (arrow, _, _) = header.body(body);
                    arrow.widget_info(|| {
                        egui::WidgetInfo::labeled(
                            egui::WidgetType::CollapsingHeader,
                            arrow.enabled(),
                            format!("Toggle {}", section.name),
                        )
                    });
                    if arrow.has_focus() {
                        arrow.scroll_to_me(None);
                    }
                } else {
                    ui.indent(section.id, body);
                }
                if counts.unavailable > 0 {
                    ui.colored_label(
                        theme::Palette::of(ui).warning,
                        format!("{} unavailable or unregistered", counts.unavailable),
                    )
                    .on_hover_ui(|ui| {
                        for path in &section.unregistered {
                            ui.label(path.display().to_string());
                        }
                        for &index in &section.repositories {
                            let path = &self.state.repositories[index].path;
                            if matches!(self.repository_states.get(path), Some(Err(_))) {
                                ui.label(path.display().to_string());
                            }
                        }
                    });
                }
            });
        }
        self.repository_sections = sections;
        if let Some(index) = remove {
            let repo = self.state.repositories.remove(index);
            self.selected.remove(&repo.path);
            self.state.repository_validation.remove(&repo.path);
            self.repository_states.remove(&repo.path);
            self.repository_pending.remove(&repo.path);
            self.dirty = true;
        }
    }

    fn repository_row(&mut self, ui: &mut egui::Ui, index: usize) -> bool {
        let p = theme::Palette::of(ui);
        let mut remove = false;
        let repository = &self.state.repositories[index];
        let in_use = self.repository_in_use(&repository.path);
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
                    remove = true;
                }
            });
            ui.add(
                egui::Label::new(
                    egui::RichText::new(repository.path.display().to_string())
                        .small()
                        .color(p.muted),
                )
                .truncate(),
            )
            .on_hover_text(repository.path.display().to_string());
            ui.horizontal_wrapped(|ui| {
                ui.small(
                    if self
                        .state
                        .repository_validation
                        .contains_key(&repository.path)
                    {
                        "Validation configured"
                    } else {
                        "Validation not configured"
                    },
                );
                if ui
                    .add(theme::quiet("Configure validation…").small())
                    .clicked()
                {
                    self.validation_editor = Some(validation::Editor::new(
                        repository,
                        self.state.repository_validation.get(&repository.path),
                    ));
                }
            });
            if in_use {
                ui.weak("Unknown · active work; refresh after completion");
                return;
            }
            if self.repository_pending.contains_key(&repository.path) {
                ui.weak("Unknown · checking Git…");
                return;
            }
            match self.repository_states.get(&repository.path) {
                Some(Ok(state)) => {
                    ui.horizontal_wrapped(|ui| {
                        ui.small("Available");
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
                                        egui::Label::new(egui::RichText::new(entry).monospace())
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
                    ui.colored_label(p.error, "Unavailable · Git state unknown")
                        .on_hover_text(diagnostics::summary(error));
                    diagnostics::details(ui, "repository_error", error);
                }
                None => {
                    ui.weak("Unknown · refresh state; checked before running");
                }
            }
        });
        remove
    }
}

fn group_header_controls(
    ui: &mut egui::Ui,
    section: &repository_sections::Section,
    counts: &repository_sections::Counts,
    busy: bool,
    filtering: bool,
) -> (bool, Option<bool>) {
    let selection_label = format!("{} selected", counts.selected);
    let selection_text = egui::RichText::new(&selection_label);
    let selection_width = egui::WidgetText::from(selection_text.clone())
        .into_galley(
            ui,
            Some(egui::TextWrapMode::Extend),
            f32::INFINITY,
            egui::TextStyle::Button,
        )
        .size()
        .x
        + ui.spacing().icon_width
        + ui.spacing().icon_spacing;
    let count_text = egui::RichText::new(format!("({})", counts.total));
    let count_width = egui::WidgetText::from(count_text.clone())
        .into_galley(
            ui,
            Some(egui::TextWrapMode::Extend),
            f32::INFINITY,
            egui::TextStyle::Button,
        )
        .size()
        .x;
    let width =
        (ui.available_width() - selection_width - count_width - 2.0 * ui.spacing().item_spacing.x)
            .max(20.0);
    let title = format!("{} ({})", section.name, counts.total);
    let response = ui
        .add_sized(
            [width, ui.spacing().interact_size.y],
            theme::quiet(&section.name)
                .right_text(())
                .truncate()
                .sense(if filtering {
                    egui::Sense::hover()
                } else {
                    egui::Sense::click()
                }),
        )
        .on_hover_text(&title);
    response.widget_info(|| {
        egui::WidgetInfo::labeled(
            egui::WidgetType::CollapsingHeader,
            response.enabled(),
            &title,
        )
    });
    if response.has_focus() {
        response.scroll_to_me(None);
    }
    // Keep the total visible even when a long group name is truncated.
    let count_response = ui.add(egui::Label::new(count_text).sense(if filtering {
        egui::Sense::hover()
    } else {
        egui::Sense::click()
    }));
    let toggle = response.clicked() || count_response.clicked();
    count_response.widget_info(|| {
        egui::WidgetInfo::labeled(
            egui::WidgetType::CollapsingHeader,
            count_response.enabled(),
            &title,
        )
    });
    if count_response.has_focus() {
        count_response.scroll_to_me(None);
    }
    let mut selected = counts.selected > 0 && counts.selected_available == counts.available;
    let response = ui.add_enabled(
        !busy && (counts.available > 0 || counts.selected > 0),
        egui::Checkbox::new(&mut selected, selection_text)
            .indeterminate(counts.selected > 0 && (counts.selected < counts.total || counts.unavailable > 0)),
    ).on_hover_text("Select adds all available members, including those hidden by the filter. Deselect removes every current member, including individual and overlapping-group selections.");
    response.widget_info(|| {
        egui::WidgetInfo::selected(
            egui::WidgetType::Checkbox,
            response.enabled(),
            selected,
            format!("Select group {}", section.name),
        )
    });
    if response.has_focus() {
        response.scroll_to_me(None);
    }
    (toggle, response.changed().then_some(selected))
}
