use super::format::{duration, run_label, status_label};
use super::*;

impl App {
    pub(super) fn results(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let bottom = ui.max_rect().bottom();
        egui::ScrollArea::vertical()
            .id_salt("results_scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| self.results_content(ui, ctx, bottom));
    }

    fn results_content(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, bottom: f32) {
        theme::eyebrow(ui, "RUNS / RESULTS");
        if self.state.runs.is_empty() {
            ui.add_space(theme::SECTION_GAP * 2.0);
            ui.heading("One task. Independent jobs.");
            ui.add_space(theme::GAP);
            ui.label("Describe your task, choose an agent and add repositories in New Convoy.");
            ui.weak("Review their Git state, then start your convoy. Each repository gets its own output and diff.");
            return;
        }
        let old = self.selected_run;
        let label = self
            .state
            .runs
            .iter()
            .find(|r| Some(r.id) == self.selected_run)
            .map_or_else(|| "Select a run".into(), run_label);
        egui::ComboBox::from_id_salt("run_history")
            .width(ui.available_width())
            .truncate()
            .selected_text(label)
            .show_ui(ui, |ui| {
                for run in &self.state.runs {
                    let summary: String = run
                        .task
                        .prompt
                        .lines()
                        .next()
                        .unwrap_or("Untitled task")
                        .chars()
                        .take(64)
                        .collect();
                    ui.selectable_value(
                        &mut self.selected_run,
                        Some(run.id),
                        format!("{}\n{summary}", run_label(run)),
                    )
                    .on_hover_text(&run.task.prompt);
                }
            });
        if self.selected_run != old {
            self.selected_job = 0;
            self.diff = None;
            self.diff_target = None;
        }
        let Some(run) = self
            .state
            .runs
            .iter()
            .find(|r| Some(r.id) == self.selected_run)
        else {
            return;
        };
        ui.add(
            egui::Label::new(
                egui::RichText::new(run.task.prompt.lines().next().unwrap_or("Untitled task"))
                    .color(theme::Palette::of(ui).muted),
            )
            .truncate(),
        )
        .on_hover_text(&run.task.prompt);
        ui.horizontal_wrapped(|ui| {
            ui.small(format!(
                "{} / {} finished · {} elapsed · per-convoy limit {}",
                run.completed_jobs(),
                run.jobs.len(),
                duration(run.elapsed(domain::now())),
                run.task.concurrency
            ));
            if run.active()
                && ui
                    .button("Stop Convoy")
                    .on_hover_text(
                        "Cancel this convoy's running and queued jobs. Other convoys continue.",
                    )
                    .clicked()
            {
                self.manager.cancel_run(run.id);
            }
        });
        ui.add_space(theme::GAP);
        let p = theme::Palette::of(ui);
        let job_height = ((bottom - ui.cursor().top()) * 0.3).clamp(72.0, 180.0);
        egui::ScrollArea::vertical()
            .id_salt("job_list")
            .max_height(job_height)
            .show(ui, |ui| {
                for (index, job) in run.jobs.iter().enumerate() {
                    ui.push_id(index, |ui| {
                        let selected = self.selected_job == index;
                        egui::Frame::new()
                            .fill(if selected {
                                p.selection
                            } else {
                                egui::Color32::TRANSPARENT
                            })
                            .inner_margin(egui::Margin::symmetric(6, 2))
                            .corner_radius(theme::CORNER)
                            .show(ui, |ui| {
                                ui.set_min_width(ui.available_width());
                                ui.horizontal(|ui| {
                                    let name_width = (ui.available_width() - 210.0).max(55.0);
                                    let response = ui
                                        .allocate_ui_with_layout(
                                            egui::vec2(name_width, theme::ROW_HEIGHT),
                                            egui::Layout::left_to_right(egui::Align::Center),
                                            |ui| {
                                                ui.set_min_width(name_width);
                                                ui.add(
                                                    egui::Button::selectable(
                                                        selected,
                                                        egui::RichText::new(&job.repository.name)
                                                            .strong(),
                                                    )
                                                    .truncate(),
                                                )
                                            },
                                        )
                                        .inner;
                                    if response
                                        .on_hover_text(job.repository.path.display().to_string())
                                        .clicked()
                                    {
                                        self.selected_job = index;
                                        self.diff = None;
                                        self.diff_target = None;
                                    }
                                    ui.add_sized(
                                        [92.0, theme::ROW_HEIGHT],
                                        egui::Label::new(
                                            egui::RichText::new(status_label(job.status))
                                                .color(p.status(job.status)),
                                        ),
                                    )
                                    .on_hover_text(
                                        job.queue_reason.map_or_else(
                                            || job.status.label().into(),
                                            |r| r.label(),
                                        ),
                                    );
                                    let elapsed = job
                                        .started_at
                                        .map(|start| {
                                            duration(
                                                job.finished_at
                                                    .unwrap_or_else(domain::now)
                                                    .saturating_sub(start),
                                            )
                                        })
                                        .unwrap_or_else(|| "—".into());
                                    ui.add_sized(
                                        [50.0, theme::ROW_HEIGHT],
                                        egui::Label::new(
                                            egui::RichText::new(elapsed).monospace().color(p.muted),
                                        ),
                                    );
                                    if !job.status.is_terminal()
                                        && ui.add(theme::quiet("Stop").small()).clicked()
                                    {
                                        self.manager.cancel(run.id, index);
                                    }
                                });
                            });
                    });
                }
            });
        ui.add_space(theme::GAP);
        ui.separator();
        let Some(job) = run.jobs.get(self.selected_job) else {
            return;
        };
        ui.horizontal_wrapped(|ui| {
            ui.strong(&job.repository.name);
            ui.colored_label(p.status(job.status), status_label(job.status));
        });
        ui.add(
            egui::Label::new(
                egui::RichText::new(job.repository.path.display().to_string())
                    .small()
                    .color(p.muted),
            )
            .truncate(),
        )
        .on_hover_text(job.repository.path.display().to_string());
        if let Some(reason) = job.queue_reason {
            ui.weak(reason.label());
        }
        if job.interrupted {
            ui.colored_label(
                p.warning,
                "Interrupted by application exit. Inspect repository state before retrying.",
            );
        }
        if !job.detail.is_empty() {
            ui.label(&job.detail);
        }
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.tab, Tab::Output, "Output");
            ui.selectable_value(&mut self.tab, Tab::Diff, "Diff");
            ui.selectable_value(&mut self.tab, Tab::Task, "Task & settings");
        });
        ui.add_space(theme::GAP);
        let mut load_diff = None;
        let mut reuse_task = None;
        match self.tab {
            Tab::Output => {
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(!job.log.text.is_empty(), theme::quiet("Copy output"))
                        .clicked()
                    {
                        ctx.copy_text(job.log.text.clone());
                    }
                    ui.small("Session output").on_hover_text(
                        "Output stays in memory and is not restored after restarting.",
                    );
                });
                if job.log.truncated {
                    ui.colored_label(
                        p.warning,
                        "Earlier output omitted by the session's log memory limit.",
                    );
                }
                if job.log.text.is_empty() {
                    ui.weak(if job.status == JobStatus::Queued {
                        "Waiting for an available job slot…"
                    } else if job.status == JobStatus::Running {
                        "Waiting for agent output…"
                    } else {
                        "No output in this session. Previous-session logs are not saved."
                    });
                } else {
                    self.output_view.show(
                        ui,
                        ("output", run.id, self.selected_job),
                        &job.log.text,
                        false,
                        bottom - ui.cursor().top() - 26.0,
                    );
                }
            }
            Tab::Diff => {
                ui.small("Current working tree · staged and unstaged changes, including pre-existing edits.");
                let loading = self.diff_target.is_some() && self.diff.is_none();
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(!loading, theme::quiet("Refresh diff"))
                        .clicked()
                    {
                        load_diff = Some(job.repository.path.clone());
                    }
                    if let Some(Ok(diff)) = &self.diff
                        && ui.add(theme::quiet("Copy diff")).clicked()
                    {
                        ctx.copy_text(diff.clone());
                    }
                    if loading {
                        ui.spinner();
                    }
                });
                match &self.diff {
                    Some(Ok(diff)) => self.diff_view.show(
                        ui,
                        ("diff", run.id, self.selected_job),
                        diff,
                        true,
                        bottom - ui.cursor().top() - 26.0,
                    ),
                    Some(Err(error)) => {
                        ui.colored_label(p.error, error);
                    }
                    None if !loading => {
                        ui.weak("Refresh to inspect this repository's current Git diff.");
                    }
                    None => {}
                }
            }
            Tab::Task => {
                ui.label(&run.task.prompt);
                ui.separator();
                ui.label(run.task.agent.label());
                for (key, value) in &run.task.options {
                    ui.label(egui::RichText::new(format!("{key}: {value}")).monospace());
                }
                ui.label(format!("Concurrency: {}", run.task.concurrency));
                if let Some(before) = &job.before {
                    ui.label(format!(
                        "Before execution: {} · {} changes",
                        before.branch, before.changed
                    ));
                    ui.label(
                        egui::RichText::new(before.head.as_deref().unwrap_or("No initial commit"))
                            .monospace(),
                    );
                }
                if let Some(code) = job.exit_code {
                    ui.label(format!("Exit code: {code}"));
                }
                if ui.button("Use this task again").clicked() {
                    reuse_task = Some(run.task.clone());
                }
            }
        }
        if let Some(task) = reuse_task {
            self.state.reuse_task(task);
            self.dirty = true;
        }
        if let Some(path) = load_diff {
            self.diff_target = Some(path.clone());
            self.diff = None;
            self.dispatch(ctx.clone(), async move {
                let result = git::diff(&path).await.map_err(|e| format!("{e:#}"));
                Message::Diff(path, result)
            });
        }
    }
}
