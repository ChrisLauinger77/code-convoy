use super::*;
use crate::validation::{
    Command, Record, Status,
    presets::{BUILT_INS, Preset},
};

pub(super) struct Editor {
    path: PathBuf,
    name: String,
    executable: String,
    pub(super) arguments: String,
    pub(super) preset: Option<&'static str>,
    error: String,
    focus: bool,
}
impl Editor {
    pub fn new(repository: &Repository, command: Option<&Command>) -> Self {
        Self {
            path: repository.path.clone(),
            name: repository.name.clone(),
            executable: command.map(|c| c.executable.clone()).unwrap_or_default(),
            arguments: command
                .map(|c| serde_json::to_string(&c.arguments).unwrap_or_default())
                .unwrap_or_else(|| "[]".into()),
            preset: None,
            error: String::new(),
            focus: true,
        }
    }
    fn select_preset(&mut self, preset: Option<&Preset>) {
        self.preset = preset.map(|p| p.id);
        if let Some(preset) = preset {
            let command = preset.command();
            self.executable = command.executable;
            self.arguments = serde_json::to_string(&command.arguments).unwrap_or_default();
            self.error.clear();
        }
    }
    pub(super) fn command(&self) -> anyhow::Result<Command> {
        let arguments = serde_json::from_str::<Vec<String>>(&self.arguments).map_err(|_| {
            anyhow::anyhow!("Arguments must be a JSON array of strings, for example [\"test\"].")
        })?;
        let command = Command {
            executable: self.executable.clone(),
            arguments,
        };
        command.validate()?;
        Ok(command)
    }
    fn fields(&mut self, ui: &mut egui::Ui) {
        ui.add(egui::Label::new(&self.name).truncate())
            .on_hover_text(&self.name);
        let mut preset = self.preset;
        egui::ComboBox::from_label("Preset")
            .selected_text(
                BUILT_INS
                    .iter()
                    .find(|p| Some(p.id) == preset)
                    .map_or("Custom", |p| p.name),
            )
            .show_ui(ui, |ui| {
                if ui.selectable_value(&mut preset, None, "Custom").clicked() {
                    ui.close();
                }
                for p in BUILT_INS {
                    if ui
                        .selectable_value(&mut preset, Some(p.id), p.name)
                        .on_hover_text(p.description)
                        .clicked()
                    {
                        ui.close();
                    }
                }
            });
        if preset != self.preset {
            self.select_preset(BUILT_INS.iter().find(|p| Some(p.id) == preset));
        }
        ui.small("Choosing a preset replaces these draft fields. Edits become Custom; only Save changes configuration.");
        let executable_label = ui.label("Executable name on PATH or absolute path");
        let input = ui
            .add(
                egui::TextEdit::singleline(&mut self.executable)
                    .desired_width(f32::INFINITY)
                    .hint_text("cargo"),
            )
            .labelled_by(executable_label.id);
        if self.focus {
            input.request_focus();
            self.focus = false;
        }
        if input.changed() {
            self.preset = None;
        }
        let arguments_label =
            ui.label("Arguments (JSON array; each string is one literal argument)");
        if ui
            .add(
                egui::TextEdit::multiline(&mut self.arguments)
                    .desired_rows(2)
                    .desired_width(f32::INFINITY)
                    .hint_text("[\"test\"]"),
            )
            .labelled_by(arguments_label.id)
            .changed()
        {
            self.preset = None;
        }
        if let Ok(command) = self.command() {
            ui.add(egui::Label::new(egui::RichText::new(command.preview()).monospace()).wrap());
        }
        ui.small(
            "Preview only: quoted values use JSON notation to show literal argument boundaries.",
        );

        ui.label("Runs only when you choose Run Validation. Commands can modify files. The command and execution metadata are saved locally; avoid secrets in arguments. Output and diagnostics stay in memory for this session.");
        if !self.error.is_empty() {
            ui.colored_label(theme::Palette::of(ui).error, &self.error);
        }
    }
}
impl App {
    pub(super) fn validation_settings_window(&mut self, ctx: &egui::Context) {
        let Some(editor) = &mut self.validation_editor else {
            return;
        };
        let mut save = false;
        let mut remove = false;
        let mut cancel = false;
        let response =
            egui::Modal::new(egui::Id::new("validation_configuration")).show(ctx, |ui| {
                ui.set_width(460.0_f32.min(ctx.content_rect().width() - 60.0));
                ui.heading("Repository validation");
                egui::ScrollArea::vertical()
                    .max_height((ctx.content_rect().height() - 140.0).max(80.0))
                    .show(ui, |ui| {
                        editor.fields(ui);
                    });
                ui.horizontal_wrapped(|ui| {
                    cancel = ui.button("Cancel").clicked();
                    remove = ui.button("Remove command").clicked();
                    save = ui.button("Save command").clicked();
                });
            });
        if cancel || response.should_close() {
            self.validation_editor = None;
            return;
        }
        if save || remove {
            let command = if remove {
                Ok(None)
            } else {
                editor.command().map(Some)
            };
            match command.and_then(|c| {
                self.store
                    .save_validation_command(&mut self.state, editor.path.clone(), c)
            }) {
                Ok(()) => {
                    self.validation_editor = None;
                    self.notice = if remove {
                        "Validation command removed."
                    } else {
                        "Validation command saved. Nothing was executed."
                    }
                    .into();
                }
                Err(error) => editor.error = format!("Could not save configuration: {error:#}"),
            }
        }
    }
    pub(super) fn validation_controls(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let Some(run) = self
            .state
            .runs
            .iter()
            .find(|r| Some(r.id) == self.selected_run)
        else {
            return;
        };
        let Some(job) = run.jobs.get(self.selected_job) else {
            return;
        };
        let key = (run.id, self.selected_job);
        let command = self.state.repository_validation.get(&job.repository.path);
        let eligibility = crate::validation::ensure_eligible(job);
        let running = job
            .validation
            .as_ref()
            .is_some_and(|v| v.status == Status::Running);
        let mut start = false;
        ui.separator();
        ui.horizontal_wrapped(|ui| {
            ui.strong(format!(
                "Validation: {}",
                crate::validation::label(job, command.is_some())
            ));
            if running {
                if ui.button("Cancel Validation").clicked() {
                    self.validations.cancel(key);
                }
            } else if command.is_some() {
                start = ui
                    .add_enabled(
                        eligibility.is_ok()
                            && !self.closing
                            && !self.quit_requested
                            && self.review_pending.is_none()
                            && self.result_operation.is_none()
                            && self.bulk_discard.is_none()
                            && !(self.diff_target.is_some() && self.diff.is_none()),
                        egui::Button::new("Run Validation"),
                    )
                    .clicked();
            }
        });
        if let Err(error) = &eligibility {
            ui.small(error.to_string());
        }
        if let Some(command) = command {
            ui.add(egui::Label::new(egui::RichText::new(command.label()).monospace()).wrap());
        } else {
            ui.small("Configure a command in Repositories to run validation.");
        }
        if let Ok(path) = crate::validation::directory(job) {
            ui.add(egui::Label::new(format!("Directory: {}", path.display())).truncate())
                .on_hover_text(path.display().to_string());
        } else {
            ui.small("Original worktree was not recorded; validation is unavailable.");
        }
        if let Some(record) = &job.validation {
            if record.status == Status::Running {
                ui.small(format!(
                    "Elapsed: {}",
                    format::duration(domain::now().saturating_sub(record.started_at))
                ));
            } else {
                ui.small(format!(
                    "Executed {} · {:.3}s · exit {}",
                    format::timestamp(record.started_at),
                    record.duration_ms as f64 / 1000.0,
                    record
                        .exit_code
                        .map(|c| c.to_string())
                        .unwrap_or_else(|| "unavailable".into())
                ));
                ui.small(if eligibility.is_ok() {
                    "Historical execution only. Files may have changed since then; run again to validate current files."
                } else {
                    "Historical execution only. This record does not certify current files."
                });
            }
            if !record.detail.is_empty() {
                ui.label(&record.detail);
            }
        }
        if start {
            self.start_validation(key);
            ctx.request_repaint();
        }
    }
    pub(super) fn validation_view(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, bottom: f32) {
        self.validation_controls(ui, ctx);
        let Some(record) = self
            .state
            .runs
            .iter()
            .find(|r| Some(r.id) == self.selected_run)
            .and_then(|r| r.jobs.get(self.selected_job))
            .and_then(|j| j.validation.as_ref())
        else {
            return;
        };
        ui.label(format!("Executed command: {}", record.command.label()));
        ui.small("Validation output and diagnostics · this session only, not saved");
        if record.truncated {
            ui.small("Earlier output omitted; retaining the latest 64 KiB.");
        }
        if ui
            .add_enabled(
                !record.output.is_empty(),
                theme::quiet("Copy validation output"),
            )
            .clicked()
        {
            ctx.copy_text(record.output.clone());
        }
        if record.output.is_empty() {
            ui.weak("No captured output in this session. Output is not restored after restart.");
        } else {
            self.output_view.show(
                ui,
                ("validation", self.selected_run, self.selected_job),
                &record.output,
                false,
                bottom - ui.cursor().top() - 26.0,
            );
        }
    }
    fn start_validation(&mut self, key: (u64, usize)) {
        if self.closing
            || self.quit_requested
            || self.result_operation.is_some()
            || self.bulk_discard.is_some()
            || self.review_pending.is_some()
            || (self.diff_target.is_some() && self.diff.is_none())
        {
            return;
        }
        let Some(job) = self
            .state
            .runs
            .iter()
            .find(|r| r.id == key.0)
            .and_then(|r| r.jobs.get(key.1))
            .cloned()
        else {
            return;
        };
        if let Err(error) = crate::validation::ensure_eligible(&job) {
            self.notice = error.to_string();
            return;
        }
        if job
            .validation
            .as_ref()
            .is_some_and(|v| v.status == Status::Running)
        {
            return;
        }
        let Some(command) = self
            .state
            .repository_validation
            .get(&job.repository.path)
            .cloned()
        else {
            return;
        };
        if let Err(error) = command.validate() {
            self.notice = error.to_string();
            return;
        }
        let pending = Record::pending(command.clone(), &job);
        self.set_validation(key, Some(pending));
        if let Err(error) = self.store.save(&self.state) {
            self.set_validation(key, job.validation.clone());
            self.notice =
                format!("Validation was not started: could not save its intent. {error:#}");
            return;
        }
        let handle = self.runtime().handle().clone();
        match self
            .validations
            .start(&handle, self.manager.lifecycle(), key, job.clone(), command)
        {
            Ok(record) => {
                self.set_validation(key, Some(record));
                self.invalidate_validation_views(&job.repository.path);
            }
            Err(error) => {
                self.set_validation(key, job.validation);
                self.notice = error.to_string();
            }
        }
        self.dirty = true;
        self.save();
    }
    fn set_validation(&mut self, key: (u64, usize), record: Option<Record>) {
        if let Some(job) = self
            .state
            .runs
            .iter_mut()
            .find(|r| r.id == key.0)
            .and_then(|r| r.jobs.get_mut(key.1))
        {
            job.validation = record;
        }
    }
    fn invalidate_validation_views(&mut self, path: &std::path::Path) {
        self.diff = None;
        self.diff_target = None;
        self.invalidate_repository_health(path);
        for job in self.state.runs.iter_mut().flat_map(|r| &mut r.jobs) {
            if job.repository.path.starts_with(path) || path.starts_with(&job.repository.path) {
                job.review = None;
            }
        }
    }
    pub(super) fn poll_validations(&mut self) {
        let mut finished = false;
        for (key, record) in self.validations.poll() {
            let terminal = record.status != Status::Running;
            let path = self
                .state
                .runs
                .iter()
                .find(|r| r.id == key.0)
                .and_then(|r| r.jobs.get(key.1))
                .map(|j| j.repository.path.clone());
            self.set_validation(key, Some(record));
            if terminal && let Some(path) = path {
                self.invalidate_validation_views(&path);
                finished = true;
            }
            self.dirty = true;
        }
        if finished {
            self.save();
        }
    }
}
