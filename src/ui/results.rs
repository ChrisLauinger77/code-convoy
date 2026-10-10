use super::format::{duration, run_label};
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
        self.run_navigation(ui);
        self.bulk_discard_status(ui);
        self.continuation_controls(ui, ctx);
        if self.state.runs.is_empty() {
            ui.add_space(theme::SECTION_GAP);
            ui.strong("No convoys yet");
            ui.add_space(theme::GAP);
            ui.label("Describe your task, choose an agent and add repositories in New Convoy.");
            ui.weak("Review their Git state, then start your convoy. Each repository gets its own output and diff.");
            return;
        }
        let Some(run) = self
            .state
            .runs
            .iter()
            .find(|r| Some(r.id) == self.selected_run)
        else {
            ui.weak("Select a convoy to inspect its jobs.");
            return;
        };
        ui.add(
            egui::Label::new(
                egui::RichText::new(
                    run.task
                        .prompt
                        .lines()
                        .find(|l| !l.trim().is_empty())
                        .unwrap_or("Untitled task"),
                )
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
        ui.horizontal_wrapped(|ui| {
            ui.selectable_value(&mut self.tab, Tab::Review, "Review");
            ui.selectable_value(&mut self.tab, Tab::Activity, "Activity");
            ui.selectable_value(&mut self.tab, Tab::Diff, "Diff");
            ui.selectable_value(&mut self.tab, Tab::Raw, "Raw output");
            ui.selectable_value(&mut self.tab, Tab::Validation, "Validation");
            ui.selectable_value(&mut self.tab, Tab::Task, "Task & settings");
        });
        if self.tab == Tab::Review {
            self.review_view(ui, ctx, bottom);
            return;
        }
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
                                    let controls_width = if job.status.is_terminal() {
                                        176.0
                                    } else {
                                        260.0
                                    };
                                    let name_width =
                                        (ui.available_width() - controls_width).max(40.0);
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
                                    if response.has_focus() {
                                        response.scroll_to_me(None);
                                    }
                                    if response
                                        .on_hover_text(job.repository.path.display().to_string())
                                        .clicked()
                                    {
                                        self.selected_job = index;
                                        self.diff = None;
                                        self.diff_target = None;
                                    }
                                    ui.allocate_ui_with_layout(
                                        egui::vec2(110.0, theme::ROW_HEIGHT),
                                        egui::Layout::left_to_right(egui::Align::Center),
                                        |ui| {
                                            ui.set_min_width(110.0);
                                            theme::job_status(ui, job)
                                        },
                                    )
                                    .inner
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
                                        && ui.add(theme::quiet("Stop job").small()).clicked()
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
            ui.weak("Select a repository job to inspect output, diff and settings.");
            return;
        };
        ui.horizontal_wrapped(|ui| {
            ui.strong(&job.repository.name);
            theme::job_status(ui, job);
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
        if let Some(label) = format::isolated_result(job) {
            ui.small(label);
        }
        if !job.worktree_detail.is_empty() {
            ui.small(diagnostics::summary(&job.worktree_detail));
            diagnostics::details(
                ui,
                ("result_diagnostics", run.id, self.selected_job),
                &job.worktree_detail,
            );
        }
        if !job.detail.is_empty() {
            if job.status == JobStatus::Failed {
                ui.colored_label(p.error, diagnostics::summary(&job.detail));
                diagnostics::details(
                    ui,
                    ("job_diagnostics", run.id, self.selected_job),
                    &job.detail,
                );
            } else if job.status == JobStatus::Cancelled {
                ui.small(&job.detail);
            } else {
                egui::CollapsingHeader::new("Completion details")
                    .id_salt((run.id, self.selected_job))
                    .show(ui, |ui| {
                        ui.label(&job.detail);
                    });
            }
        }
        ui.add_space(theme::GAP);
        if self.tab == Tab::Validation {
            self.validation_view(ui, ctx, bottom);
            return;
        }
        let mut load_diff = None;
        match self.tab {
            Tab::Review | Tab::Validation => unreachable!("Rendered above"),
            Tab::Activity | Tab::Raw => {
                let raw = self.tab == Tab::Raw;
                let log = if raw { &job.raw_log } else { &job.log };
                if !raw {
                    if job.execution_mode == domain::ExecutionMode::IsolatedWorktree {
                        ui.small("[CodeConvoy] entries describe the isolated job lifecycle.");
                    }
                    ui.small(if run.task.agent == domain::AgentId::Copilot {
                        "Copilot supplies plain text; Activity shows its actual CLI messages."
                    } else {
                        "Backend events summarized; full details remain in Raw output."
                    });
                }
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(
                            !log.text.is_empty(),
                            theme::quiet(if raw {
                                "Copy raw output"
                            } else {
                                "Copy activity"
                            }),
                        )
                        .clicked()
                    {
                        ctx.copy_text(log.text.clone());
                    }
                    ui.small("Session output").on_hover_text(
                        "Output stays in memory and is not restored after restarting.",
                    );
                });
                if log.truncated {
                    ui.colored_label(
                        p.warning,
                        "Earlier output omitted by the session's log memory limit.",
                    );
                }
                if log.text.is_empty() {
                    ui.weak(format::output_empty(
                        job,
                        self.session_runs.contains(&run.id),
                    ));
                } else {
                    self.output_view.show(
                        ui,
                        (
                            if raw { "raw" } else { "activity" },
                            run.id,
                            self.selected_job,
                        ),
                        &log.text,
                        false,
                        bottom - ui.cursor().top() - 26.0,
                    );
                }
            }
            Tab::Diff => {
                if job.resolution == domain::ResultResolution::Discarded
                    || job.result_availability == domain::ResultAvailability::Cleaned
                {
                    ui.label(if job.resolution == domain::ResultResolution::Discarded {
                        "Result discarded · retained Diff is no longer available."
                    } else {
                        "Retained copy cleaned up · Diff is no longer available."
                    });
                    return;
                }
                ui.small(if job.execution_mode == domain::ExecutionMode::Direct {
                    "Current working tree · staged and unstaged changes, including pre-existing edits."
                } else { "Retained isolated worktree · changes relative to its snapshotted base commit." });
                let loading = self.diff_target.is_some() && self.diff.is_none();
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(
                            !loading
                                && !self.repository_in_use(&job.repository.path)
                                && self.review_pending.is_none()
                                && self.result_operation.is_none()
                                && self.bulk_discard.is_none(),
                            theme::quiet("Refresh diff"),
                        )
                        .clicked()
                    {
                        load_diff = Some(job.clone());
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
                        ui.colored_label(p.error, diagnostics::summary(error));
                        diagnostics::details(ui, "diff_error", error);
                    }
                    None if !loading => {
                        ui.weak("Refresh to inspect this repository's current Git diff.");
                    }
                    None => {}
                }
            }
            Tab::Task => {
                egui::ScrollArea::vertical()
                    .id_salt(("task_snapshot", run.id, self.selected_job))
                    .max_height((bottom - ui.cursor().top() - 26.0).max(60.0))
                    .auto_shrink([false, false])
                    .show(ui, |ui| snapshot::show(ui, run, job));
            }
        }
        let run_id = run.id;
        let job_index = self.selected_job;
        if let Some(job) = load_diff {
            let path = job
                .worktree
                .as_ref()
                .map_or_else(|| job.repository.path.clone(), |w| w.path.clone());
            if job.worktree.is_some() {
                self.review_pending = Some((run_id, job_index));
            }
            self.diff_target = Some(path.clone());
            self.diff = None;
            let lifecycle = self.manager.lifecycle();
            self.dispatch(ctx.clone(), async move {
                if let Some(metadata) = job.worktree.clone() {
                    let report = lifecycle.inspect(run_id, job_index, &job, true).await;
                    return Message::Reconciled(run_id, job_index, metadata, report, true);
                }
                let result = git::diff_job(&job).await.map_err(|e| format!("{e:#}"));
                Message::Diff(path, result)
            });
        }
    }

    fn run_navigation(&mut self, ui: &mut egui::Ui) {
        if self.state.runs.is_empty() {
            if self.show_active_navigation {
                self.show_active_navigation = false;
                self.notice = "No active convoys.".into();
            }
            return;
        }
        let active = self.state.runs.iter().filter(|r| r.active()).count();
        let history = self.state.runs.len() - active;
        ui.add(
            egui::TextEdit::singleline(&mut self.history_search.query)
                .hint_text("Search history: ID, task, repository, backend")
                .desired_width(f32::INFINITY),
        );
        ui.horizontal_wrapped(|ui| {
            egui::ComboBox::from_id_salt("history_filter")
                .selected_text(self.history_search.filter.label())
                .show_ui(ui, |ui| {
                    for filter in crate::visibility::HistoryFilter::ALL {
                        ui.selectable_value(
                            &mut self.history_search.filter,
                            filter,
                            filter.label(),
                        );
                    }
                });
            if ui
                .add_enabled(
                    !self.history_search.query.is_empty()
                        || self.history_search.filter != crate::visibility::HistoryFilter::All,
                    theme::quiet("Clear filters").small(),
                )
                .clicked()
            {
                self.history_search.clear();
            }
        });
        let matching = self.history_search.matching(&self.state.runs);
        ui.small(format!(
            "Active {active} · History {} / {history}",
            matching.len()
        ));
        if history > 0 && matching.is_empty() {
            ui.weak("No history matches. Clear filters to see all convoys.");
        }
        if self.selected_run.is_some_and(|id| {
            self.state.runs.iter().any(|r| r.id == id && !r.active()) && !matching.contains(&id)
        }) {
            ui.small("Selected convoy is outside the history filters.");
        }
        let label = self
            .state
            .runs
            .iter()
            .find(|r| Some(r.id) == self.selected_run)
            .map_or_else(
                || "Select a convoy".into(),
                |run| {
                    format!(
                        "{} · {}",
                        if run.active() { "Active" } else { "History" },
                        run_label(run)
                    )
                },
            );
        let mut selected = self.selected_run;
        let navigation = egui::ComboBox::from_id_salt("run_history")
            .width(ui.available_width())
            .height(280.0)
            .truncate()
            .selected_text(label)
            .show_ui(ui, |ui| {
                for (is_active, title, count) in
                    [(true, "ACTIVE", active), (false, "HISTORY", matching.len())]
                {
                    theme::eyebrow(ui, &format!("{title} ({count})"));
                    if count == 0 {
                        ui.weak(if is_active {
                            "No active convoys"
                        } else if history == 0 {
                            "No history"
                        } else {
                            "No history matches"
                        });
                    }
                    for run in self.state.runs.iter().filter(|r| {
                        r.active() == is_active && (is_active || matching.contains(&r.id))
                    }) {
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
                            &mut selected,
                            Some(run.id),
                            format!(
                                "{} · {}\n{summary}",
                                run_label(run),
                                duration(run.elapsed(domain::now()))
                            ),
                        )
                        .on_hover_text(format!(
                            "{}\nCreated {}",
                            run.task.prompt,
                            format::timestamp(run.created_at)
                        ));
                        if !is_active {
                            let s = crate::visibility::summary(run);
                            ui.small(format!(
                                "{} changed · {} unchanged · {} unknown · {} need review",
                                s.changed, s.unchanged, s.unknown, s.review
                            ));
                        }
                    }
                    if is_active {
                        ui.separator();
                    }
                }
            });
        if self.show_active_navigation {
            self.show_active_navigation = false;
            egui::Popup::open_id(ui.ctx(), navigation.response.id.with("popup"));
            navigation.response.scroll_to_me(Some(egui::Align::Min));
        }
        self.select_run(selected);
        let mut reuse = None;
        let mut remove = None;
        let mut clear = false;
        let mut discard = None;
        let discard_enabled = !self.closing
            && !self.quit_requested
            && self.result_operation.is_none()
            && self.bulk_discard.is_none();
        ui.horizontal_wrapped(|ui| {
            if let Some(run) = self.state.runs.iter().find(|r| Some(r.id) == self.selected_run)
                && ui.add_enabled(!self.busy && self.prepared.is_none() && !self.closing,
                    theme::quiet("Reuse convoy").small())
                    .on_hover_text("Replace the draft with this convoy's task, agent settings, concurrency and still-registered repositories. Review before launching; this does not start jobs.")
                    .clicked() { reuse = Some(run.id); }
            if let Some(run) = self.state.runs.iter().find(|r| Some(r.id) == self.selected_run)
                && run.unresolved_results()
                && ui.add_enabled(discard_enabled && !run.active(), theme::quiet("Discard convoy…").small())
                    .on_hover_text(if run.active() { "Available after every job in this convoy finishes. This action does not stop active work." }
                        else { "Discard unresolved retained isolated results after confirmation. Direct and already resolved results stay untouched." }).clicked() {
                discard = Some(crate::persistence::bulk_discard::Scope::Convoy(run.id));
            }
            if history > 0 {
                ui.menu_button("History cleanup", |ui| {
                    if ui.add_enabled(discard_enabled && self.state.runs.iter().any(|r| !r.active() && r.unresolved_results()),
                        egui::Button::new("Discard all unresolved results…"))
                        .on_hover_text("Discard unresolved isolated results in terminal convoys only, after confirmation. Active convoys are excluded.").clicked() {
                        discard = Some(crate::persistence::bulk_discard::Scope::All);
                        ui.close();
                    }
                    ui.separator();
                    ui.label("History removal deletes local metadata and session output only.");
                    ui.weak("Retained copies stay protected until explicit cleanup.");
                    if let Some(run)=self.state.runs.iter().find(|r|Some(r.id)==self.selected_run)
                        && !run.active() && ui.add_enabled(!run.history_protected() && self.bulk_discard.is_none(), egui::Button::new(format!("Remove convoy #{} from history",run.id))).on_hover_text("Retained isolated results must remain in history. Use Discard convoy… for unresolved results; Applied copies require Clean up retained copy in Review.").clicked() {
                        remove=Some(run.id); ui.close();
                    }
                    ui.separator();
                    if ui.add_enabled(self.bulk_discard.is_none(), egui::Button::new(format!("Clear removable history ({})", self.state.runs.iter().filter(|r| !r.history_protected()).count()))).clicked() {
                        clear = true;
                        ui.close();
                    }
                });
            }
        });
        if let Some(run) = self
            .state
            .runs
            .iter()
            .find(|r| Some(r.id) == self.selected_run)
            && !run.active()
            && run.history_protected()
        {
            ui.small(if run.jobs.iter().any(|j| j.validation.as_ref().is_some_and(|v| v.status == crate::validation::Status::Running)) {
                "History removal blocked: validation is running."
            } else if run.unresolved_results() {
                "History removal blocked: retained isolated results still exist. Use Discard convoy… or resolve results in Review."
            } else {
                "History removal blocked: retained copies need explicit cleanup in Review, or cleanup verification is pending."
            });
        }
        if let Some(scope) = discard {
            self.offer_bulk_discard(scope);
        }
        if let Some(id) = reuse {
            self.reuse_convoy(id);
        }
        if clear {
            self.remove_history(None);
        } else if let Some(id) = remove {
            self.remove_history(Some(id));
        }
    }
}
