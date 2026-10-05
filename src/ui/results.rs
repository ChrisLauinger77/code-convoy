use super::*;

impl App {
    pub(super) fn results(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.heading("Runs & results");
        if self.state.runs.is_empty() {
            ui.add_space(45.0);
            ui.heading("Your first convoy starts with a task.");
            ui.label("Choose an agent, register repositories, then review their state and run.");
            ui.label("Each repository gets its own job and output. A failure in one leaves the others running.");
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
            .width(ui.available_width().min(650.0))
            .selected_text(label)
            .show_ui(ui, |ui| {
                for run in &self.state.runs {
                    ui.selectable_value(&mut self.selected_run, Some(run.id), run_label(run));
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
        ui.small(format!(
            "{} · {} repositories · created {} seconds ago",
            run.task.agent.label(),
            run.jobs.len(),
            domain::now().saturating_sub(run.created_at)
        ));
        egui::ScrollArea::vertical()
            .id_salt("job_list")
            .max_height(190.0)
            .show(ui, |ui| {
                for (index, job) in run.jobs.iter().enumerate() {
                    ui.horizontal(|ui| {
                        if ui
                            .selectable_label(self.selected_job == index, &job.repository.name)
                            .on_hover_text(job.repository.path.display().to_string())
                            .clicked()
                        {
                            self.selected_job = index;
                            self.diff = None;
                            self.diff_target = None;
                        }
                        let color = match job.status {
                            JobStatus::Queued => Color32::GRAY,
                            JobStatus::Running => Color32::LIGHT_BLUE,
                            JobStatus::Succeeded => Color32::LIGHT_GREEN,
                            JobStatus::Failed => Color32::LIGHT_RED,
                            JobStatus::Cancelled => Color32::YELLOW,
                        };
                        ui.colored_label(color, job.status.label());
                        if let Some(start) = job.started_at {
                            ui.small(format!(
                                "{}s",
                                job.finished_at
                                    .unwrap_or_else(domain::now)
                                    .saturating_sub(start)
                            ));
                        }
                        if !job.status.is_terminal()
                            && ui.small_button("Stop").clicked()
                            && let Some(handle) = &self.active
                        {
                            handle.cancel(index);
                        }
                    });
                }
            });
        ui.separator();
        let Some(job) = run.jobs.get(self.selected_job) else {
            return;
        };
        ui.strong(&job.repository.name);
        ui.small(job.repository.path.display().to_string());
        if job.interrupted {
            ui.colored_label(
                Color32::YELLOW,
                "Interrupted by application exit. Inspect repository state before retrying.",
            );
        }
        if !job.detail.is_empty() {
            ui.label(&job.detail);
        }
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.tab, Tab::Output, "Output");
            ui.selectable_value(&mut self.tab, Tab::Diff, "Current Git diff");
            ui.selectable_value(&mut self.tab, Tab::Task, "Task & settings");
        });
        let mut load_diff = None;
        let mut reuse_task = None;
        match self.tab {
            Tab::Output => {
                ui.small("Output is retained for this application session only.");
                if job.log.truncated {
                    ui.colored_label(
                        Color32::YELLOW,
                        "Earlier output omitted by the session's log memory limit.",
                    );
                }
                if job.log.text.is_empty() {
                    ui.weak("No output available. Previous-session logs are not persisted.");
                } else {
                    if ui.small_button("Copy output").clicked() {
                        ctx.copy_text(job.log.text.clone());
                    }
                    egui::ScrollArea::both()
                        .id_salt(("output", run.id, self.selected_job))
                        .auto_shrink([false, false])
                        .stick_to_bottom(true)
                        .show(ui, |ui| {
                            let mut text = job.log.text.as_str();
                            ui.add(
                                egui::TextEdit::multiline(&mut text)
                                    .code_editor()
                                    .desired_width(f32::INFINITY),
                            );
                        });
                }
            }
            Tab::Diff => {
                ui.small("Live staged / unstaged diff. Includes pre-existing edits; this is not a historical snapshot.");
                let loading = self.diff_target.is_some() && self.diff.is_none();
                if ui
                    .add_enabled(!loading, egui::Button::new("Load / refresh current diff"))
                    .clicked()
                {
                    load_diff = Some(job.repository.path.clone());
                }
                if loading {
                    ui.spinner();
                }
                match &self.diff {
                    Some(Ok(diff)) => {
                        if ui.small_button("Copy diff").clicked() {
                            ctx.copy_text(diff.clone());
                        }
                        egui::ScrollArea::both()
                            .id_salt("diff_view")
                            .auto_shrink([false, false])
                            .show(ui, |ui| {
                                let mut text = diff.as_str();
                                ui.add(
                                    egui::TextEdit::multiline(&mut text)
                                        .code_editor()
                                        .desired_width(f32::INFINITY),
                                );
                            });
                    }
                    Some(Err(error)) => {
                        ui.colored_label(Color32::LIGHT_RED, error);
                    }
                    None => {}
                }
            }
            Tab::Task => {
                egui::ScrollArea::vertical()
                    .id_salt("task_detail")
                    .show(ui, |ui| {
                        ui.label(&run.task.prompt);
                        ui.separator();
                        for (key, value) in &run.task.options {
                            ui.monospace(format!("{key}: {value}"));
                        }
                        ui.label(format!("Concurrency: {}", run.task.concurrency));
                        if let Some(before) = &job.before {
                            ui.label(format!(
                                "Before execution: {} · {} changes",
                                before.branch, before.changed
                            ));
                            ui.monospace(before.head.as_deref().unwrap_or("No initial commit"));
                        }
                        if let Some(code) = job.exit_code {
                            ui.label(format!("Exit code: {code}"));
                        }
                        if ui.button("Use this task again").clicked() {
                            reuse_task = Some(run.task.clone());
                        }
                    });
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
fn run_label(run: &Run) -> String {
    let title: String = run
        .task
        .prompt
        .lines()
        .next()
        .unwrap_or("Untitled task")
        .chars()
        .take(70)
        .collect();
    format!(
        "#{} · {} · {title}",
        run.id,
        if run.active() { "Active" } else { "Finished" }
    )
}
