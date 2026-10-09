use super::*;
use crate::{
    domain::{ResultAvailability as A, ResultResolution as R},
    persistence::results::Action,
};

impl App {
    /// One bounded background request at a time; completed/cache/error values stop
    /// automatic refresh until job completion or the user explicitly requests it.
    pub(super) fn pump_review(&mut self, ctx: &egui::Context) {
        if self.closing
            || self.review_pending.is_some()
            || self.result_operation.is_some()
            || self.bulk_discard.is_some()
        {
            return;
        }
        let next = self
            .state
            .runs
            .iter()
            .flat_map(|r| {
                r.jobs
                    .iter()
                    .enumerate()
                    .map(move |(index, j)| (r.id, index, j))
            })
            .find(|(_, _, j)| j.status.is_terminal() && j.review.is_none());
        let Some((run, index, job)) = next else {
            return;
        };
        let job = job.clone();
        self.review_pending = Some((run, index));
        let lifecycle = self.manager.lifecycle();
        self.dispatch(ctx.clone(), async move {
            if let Some(m) = job.worktree.clone() {
                let report = lifecycle.inspect(run, index, &job, false).await;
                Message::Reconciled(run, index, m, report, false)
            } else {
                let result = if job.execution_mode == domain::ExecutionMode::Direct {
                    crate::review::direct(&job.repository.path)
                        .await
                        .map_err(|e| format!("{e:#}"))
                } else {
                    Err("No retained result was created.".into())
                };
                Message::ReviewStats(run, index, result)
            }
        });
    }
    pub(super) fn review_view(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, bottom: f32) {
        let Some(run) = self
            .state
            .runs
            .iter()
            .find(|r| Some(r.id) == self.selected_run)
        else {
            return;
        };
        let id = run.id;
        let mut followup = false;
        ui.horizontal_wrapped(|ui| {
            ui.small(format!("{} selected for follow-up", self.review_selected.len()));
            followup = ui.add_enabled(!self.review_selected.is_empty() && !self.busy && self.prepared.is_none() && !self.closing && !self.quit_requested,
                theme::quiet("New convoy from selected")).on_hover_text("Restore repository selection and settings into a draft with a new task and no attachments. Nothing starts automatically.").clicked();
        });
        let total = crate::visibility::summary(run);
        ui.small(format!(
            "{} repositories · {} succeeded · {} failed · {} cancelled",
            total.total, total.succeeded, total.failed, total.cancelled
        ));
        ui.small(format!(
            "{} changed · {} unchanged · {} unknown · {} need review",
            total.changed, total.unchanged, total.unknown, total.review
        ));
        ui.small("Completion observations · Unknown means no reliable saved measurement.")
            .on_hover_text("Changes are independent of agent success. Direct observations compare with the reviewed HEAD and include pre-existing edits. Isolated observations compare with the fixed base. Review and Diff below inspect current files.");
        let mut refresh = false;
        ui.horizontal_wrapped(|ui| {
            egui::ComboBox::from_id_salt("comparison_filter")
                .selected_text(self.comparison_filter.label())
                .show_ui(ui, |ui| {
                    for filter in crate::visibility::RepositoryFilter::ALL {
                        ui.selectable_value(&mut self.comparison_filter, filter, filter.label());
                    }
                });
            refresh = ui
                .add_enabled(
                    self.review_pending.is_none()
                        && self.result_operation.is_none()
                        && self.bulk_discard.is_none(),
                    theme::quiet("Refresh review"),
                )
                .on_hover_text(
                    "Refresh live Review statistics; completion observations stay unchanged.",
                )
                .clicked();
        });
        if self.review_pending.is_some() {
            ui.small("Inspecting live Git results…");
        }
        let mut inspect = None;
        let mut visible = 0;
        egui::ScrollArea::vertical()
            .id_salt(("convoy_review", id))
            .max_height((bottom - ui.cursor().top() - 210.0).max(130.0))
            .show(ui, |ui| {
                for (index, job) in run
                    .jobs
                    .iter()
                    .enumerate()
                    .filter(|(_, j)| self.comparison_filter.matches(j))
                {
                    visible += 1;
                    ui.push_id(index, |ui| {
                        ui.separator();
                        ui.horizontal(|ui| {
                            let mut included = self.review_selected.contains(&index);
                            let checkbox = ui
                                .checkbox(&mut included, "")
                                .on_hover_text("Select for follow-up");
                            checkbox.widget_info(|| {
                                egui::WidgetInfo::selected(
                                    egui::WidgetType::Checkbox,
                                    checkbox.enabled(),
                                    included,
                                    format!("Select {} for follow-up", job.repository.name),
                                )
                            });
                            if checkbox.has_focus() {
                                checkbox.scroll_to_me(None);
                            }
                            if checkbox.changed() {
                                if included {
                                    self.review_selected.insert(index);
                                } else {
                                    self.review_selected.remove(&index);
                                }
                            }
                            let name = ui
                                .add(
                                    egui::Button::selectable(
                                        self.selected_job == index,
                                        egui::RichText::new(&job.repository.name).strong(),
                                    )
                                    .truncate(),
                                )
                                .on_hover_text(job.repository.path.display().to_string());
                            if name.has_focus() {
                                name.scroll_to_me(None);
                            }
                            if name.clicked() {
                                self.selected_job = index;
                                self.diff = None;
                                self.diff_target = None;
                            }
                        });
                        ui.horizontal_wrapped(|ui| {
                            theme::job_status(ui, job);
                            theme::change_status(ui, crate::visibility::changes(job));
                            theme::review_status(ui, job);
                            if let Some(start) = job.started_at {
                                ui.small(format::duration(
                                    job.finished_at
                                        .unwrap_or_else(domain::now)
                                        .saturating_sub(start),
                                ));
                            }
                        });
                    });
                }
                if visible == 0 {
                    ui.weak("No repositories match this filter.");
                }
            });
        ui.separator();
        let mut action = None;
        if let Some(job) = run.jobs.get(self.selected_job) {
            ui.strong(&job.repository.name);
            ui.label(result_label(job));
            if !self.comparison_filter.matches(job) {
                ui.small("Selected repository is outside the comparison filter.");
            }
            if let Some(Ok(statistics)) = &job.review {
                ui.small(format!("Live Review: {}", statistics.label()))
                    .on_hover_text("Current Git statistics, separate from the saved completion observation. Renames count as deletion plus addition.");
            }
            if job.execution_mode == domain::ExecutionMode::Direct {
                ui.small("Live working tree, including pre-existing edits. Direct Diff is not a historical patch.");
            }
            if !job.worktree_detail.is_empty() {
                ui.label(diagnostics::summary(&job.worktree_detail));
                diagnostics::details(
                    ui,
                    ("review_detail", id, self.selected_job),
                    &job.worktree_detail,
                );
            }
            if job.result_availability != A::Cleaned
                && let Some(Err(e)) = &job.review
            {
                diagnostics::details(ui, ("review_stats_error", id, self.selected_job), e);
            }
            ui.horizontal_wrapped(|ui| {
                for (tab, label) in [
                    (Tab::Activity, "Inspect activity"),
                    (Tab::Diff, "Inspect diff"),
                    (Tab::Raw, "Raw output"),
                    (Tab::Task, "Task & settings"),
                ] {
                    if ui.button(label).clicked() {
                        inspect = Some(tab);
                    }
                }
            });
            if job.worktree.is_some() && job.status.is_terminal() {
                let enabled = !self.closing
                    && !self.quit_requested
                    && self.review_pending != Some((id, self.selected_job))
                    && self.result_operation.is_none()
                    && self.bulk_discard.is_none();
                ui.horizontal_wrapped(|ui| {
                    if job.resolution == R::Unresolved {
                        let changed = job
                            .review
                            .as_ref()
                            .and_then(|r| r.as_ref().ok())
                            .is_some_and(|s| s.files > 0);
                        if ui
                            .add_enabled(
                                enabled
                                    && changed
                                    && job.review.as_ref().is_some_and(|r| r.is_ok())
                                    && job.result_checked
                                    && job.result_availability == A::Available,
                                egui::Button::new("Apply result"),
                            )
                            .clicked()
                        {
                            action = Some(Action::Apply);
                        }
                    }
                    if job.resolution == R::Applied && job.result_availability != A::Cleaned {
                        if ui
                            .add_enabled(enabled, egui::Button::new("Clean up retained copy"))
                            .clicked()
                        {
                            action = Some(Action::CleanupApplied);
                        }
                    } else if job.resolution != R::Discarded
                        && job.resolution != R::Applied
                        && job.resolution != R::ApplyPending
                        && job.result_availability != A::Cleaned
                        && ui
                            .add_enabled(enabled, egui::Button::new("Discard result"))
                            .clicked()
                    {
                        action = Some(Action::Discard);
                    }
                });
                if self.result_operation == Some((id, self.selected_job)) {
                    ui.label("Result operation in progress…");
                }
            }
        }
        if refresh && let Some(run) = self.state.runs.iter_mut().find(|r| r.id == id) {
            for j in &mut run.jobs {
                if j.status.is_terminal() {
                    j.review = None;
                }
            }
        }
        if let Some(tab) = inspect {
            self.tab = tab;
            self.diff = None;
            self.diff_target = None;
        }
        if let Some(action) = action {
            if action == Action::Apply {
                self.start_result_operation(ctx, id, self.selected_job, action);
            } else {
                self.discard_confirmation = Some((id, self.selected_job, action));
                self.focus_discard_cancel = true;
            }
        }
        if followup {
            self.followup(ctx, id);
        }
    }
    fn start_result_operation(
        &mut self,
        ctx: &egui::Context,
        run: u64,
        index: usize,
        action: Action,
    ) {
        if self.result_operation.is_some()
            || self.bulk_discard.is_some()
            || self.closing
            || self.quit_requested
            || self.review_pending == Some((run, index))
        {
            return;
        }
        let op = match self
            .store
            .begin_result_operation(&mut self.state, run, index, action)
        {
            Ok(op) => op,
            Err(e) => {
                self.notice = format!("{e:#}");
                return;
            }
        };
        self.dispatch_result_operation(ctx, op);
    }
    pub(super) fn dispatch_result_operation(
        &mut self,
        ctx: &egui::Context,
        op: crate::persistence::results::Operation,
    ) {
        self.result_operation = Some((op.run, op.index));
        let lifecycle = self.manager.lifecycle();
        self.dispatch(ctx.clone(), async move {
            let completion = op.execute(&lifecycle).await;
            Message::Resolved(Box::new(op), completion)
        });
    }
    pub(super) fn discard_window(&mut self, ctx: &egui::Context) {
        let Some((run, index, action)) = self.discard_confirmation else {
            return;
        };
        if self.quit_requested {
            return;
        }
        let mut cancel = false;
        let mut confirm = false;
        let response = egui::Modal::new(egui::Id::new("discard_result")).show(ctx, |ui| {
            ui.set_max_width(430.0);
            ui.heading(if action == Action::Discard { "Discard isolated changes?" } else { "Remove retained isolated copy?" });
            ui.label("This removes the retained CodeConvoy worktree and its historical Diff. The registered repository will not be changed.");
            ui.horizontal(|ui| {
                let button = ui.button("Cancel");
                if self.focus_discard_cancel { button.request_focus(); self.focus_discard_cancel = false; }
                cancel = button.clicked();
                confirm = ui.button(if action == Action::Discard { "Discard result" } else { "Clean up retained copy" }).clicked();
            });
        });
        if cancel || response.should_close() {
            self.discard_confirmation = None;
        } else if confirm {
            self.discard_confirmation = None;
            self.start_result_operation(ctx, run, index, action);
        }
    }
}
fn result_label(job: &domain::Job) -> String {
    if job.execution_mode == domain::ExecutionMode::Direct {
        return "Direct · current working tree".into();
    }
    let resolution = match job.resolution {
        R::Applied => "Applied",
        R::Discarded => "Discarded",
        R::ApplyPending => "Apply pending / uncertain",
        R::DiscardPending => "Discard pending",
        R::Unresolved => "Isolated",
    };
    let availability = if !job.result_checked {
        "not validated"
    } else {
        match job.result_availability {
            A::Available => "available",
            A::Missing => "missing",
            A::Stale => "stale",
            A::Invalid => "ownership mismatch",
            A::CleanupFailed => "cleanup failed",
            A::CleanupPending => "cleanup pending",
            A::Cleaned => "copy removed",
            A::Unchecked => "not validated",
        }
    };
    format!("{resolution} · {availability}")
}
