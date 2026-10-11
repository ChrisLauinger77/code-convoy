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
        self.export_status(ui);
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
        let mut export_id = None;
        ui.horizontal_wrapped(|ui| {
            ui.small(format!(
                "{} / {} finished · {} elapsed · per-convoy limit {}",
                run.completed_jobs(),
                run.jobs.len(),
                duration(run.elapsed(domain::now())),
                run.task.concurrency
            ));
            copy_results::summary(ui, run);
            if !run.active()
                && ui
                    .add_enabled(self.export_available(), theme::quiet("Export…").small())
                    .clicked()
            {
                export_id = Some(run.id);
            }
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
        if let Some(id) = export_id {
            self.offer_export(Some(id));
        }
        let Some(run) = self
            .state
            .runs
            .iter()
            .find(|r| Some(r.id) == self.selected_run)
        else {
            return;
        };
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
            copy_results::repository(ui, run, self.selected_job);
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
                ui.horizontal_wrapped(|ui| {
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
                ui.horizontal_wrapped(|ui| {
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
                        ctx.copy_text(diff.text.clone());
                    }
                    if loading {
                        ui.spinner();
                    }
                });
                match &self.diff {
                    Some(Ok(diff)) if diff.changed == Some(false) => {
                        ui.weak(format::diff_empty(job.execution_mode));
                    }
                    Some(Ok(diff)) => self.diff_view.show(
                        ui,
                        ("diff", run.id, self.selected_job),
                        &diff.text,
                        true,
                        bottom - ui.cursor().top() - 26.0,
                    ),
                    Some(Err(error)) => {
                        ui.colored_label(p.error, diagnostics::summary(error));
                        diagnostics::details(ui, "diff_error", error);
                    }
                    None if !loading => {
                        ui.weak(if job.execution_mode == domain::ExecutionMode::Direct {
                            "Refresh to inspect this repository's current Git diff."
                        } else {
                            "Refresh to compare the retained worktree with its saved base commit."
                        });
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
                if job.execution_mode == domain::ExecutionMode::IsolatedWorktree {
                    let Some(metadata) = job.worktree.clone() else {
                        return Message::Diff(
                            path,
                            Err("The isolated worktree has not been prepared.".into()),
                        );
                    };
                    let report = lifecycle.inspect(run_id, job_index, &job, true).await;
                    return Message::Reconciled(run_id, job_index, metadata, report, true);
                }
                let result = git::inspect_diff(&job.repository.path)
                    .await
                    .map_err(|e| format!("{e:#}"));
                Message::Diff(path, result)
            });
        }
    }

    pub(super) fn run_navigation(&mut self, ui: &mut egui::Ui) {
        if ui
            .add_enabled(
                self.export_available() && self.state.runs.iter().any(|r| !r.active()),
                theme::quiet("Export History…").small(),
            )
            .clicked()
        {
            self.offer_export(None);
        }
        if self.state.runs.is_empty() {
            if self.show_active_navigation {
                self.show_active_navigation = false;
                self.notice = "No active convoys.".into();
            }
            if ui.button("Clean Up History…").clicked() {
                self.offer_history_cleanup(domain::now());
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
                // Membership is O(1), and only visible rows create widgets or
                // format summaries. The references die with this frame: no
                // cached presentation can outlive a validation/result mutation.
                let matching: HashSet<_> = matching.iter().copied().collect();
                let active_runs: Vec<_> = self.state.runs.iter().filter(|r| r.active()).collect();
                let history_runs: Vec<_> = self
                    .state
                    .runs
                    .iter()
                    .filter(|r| matching.contains(&r.id))
                    .collect();
                let all: Vec<_> = active_runs.iter().chain(&history_runs).copied().collect();
                let keyboard = navigate_history(ui, &all, &mut selected);
                for (is_active, title, rows) in [
                    (true, "ACTIVE", &active_runs),
                    (false, "HISTORY", &history_runs),
                ] {
                    theme::eyebrow(ui, &format!("{title} ({})", rows.len()));
                    if rows.is_empty() {
                        ui.weak(if is_active {
                            "No active convoys"
                        } else if history == 0 {
                            "No history"
                        } else {
                            "No history matches"
                        });
                        continue;
                    }
                    let spacing = ui.spacing().item_spacing.y;
                    let row_height = ui.spacing().interact_size.y
                        + ui.text_style_height(&egui::TextStyle::Small) * 2.0
                        + spacing * 2.0;
                    let mut scroll = egui::ScrollArea::vertical()
                        .id_salt(("history_rows", is_active))
                        .max_height(if is_active && !history_runs.is_empty() {
                            100.0
                        } else {
                            220.0
                        });
                    if keyboard
                        && let Some(index) = rows.iter().position(|r| Some(r.id) == selected)
                    {
                        scroll =
                            scroll.vertical_scroll_offset(index as f32 * (row_height + spacing));
                    }
                    scroll.show_rows(ui, row_height, rows.len(), |ui, visible| {
                        for index in visible {
                            let run = rows[index];
                            ui.push_id(("convoy", run.id), |ui| {
                                let response = ui.add(
                                    egui::Button::selectable(
                                        selected == Some(run.id),
                                        format!(
                                            "{} · {}",
                                            run_label(run),
                                            duration(run.elapsed(domain::now()))
                                        ),
                                    )
                                    .truncate(),
                                );
                                response.widget_info(|| {
                                    egui::WidgetInfo::selected(
                                        egui::WidgetType::Button,
                                        response.enabled(),
                                        selected == Some(run.id),
                                        format!(
                                            "{} · {}\n{}",
                                            run_label(run),
                                            duration(run.elapsed(domain::now())),
                                            run.task
                                                .prompt
                                                .lines()
                                                .next()
                                                .unwrap_or("Untitled task")
                                                .chars()
                                                .take(64)
                                                .collect::<String>()
                                        ),
                                    )
                                });
                                if response.clicked() {
                                    selected = Some(run.id);
                                }
                                if keyboard && selected == Some(run.id) {
                                    response.request_focus();
                                }
                                if response.has_focus() {
                                    response.scroll_to_me(None);
                                }
                                response.on_hover_ui(|ui| {
                                    ui.label(&run.task.prompt);
                                    ui.small(format!(
                                        "Created {}",
                                        format::timestamp(run.created_at)
                                    ));
                                });
                                ui.add(
                                    egui::Label::new(
                                        egui::RichText::new(
                                            run.task
                                                .prompt
                                                .lines()
                                                .next()
                                                .unwrap_or("Untitled task"),
                                        )
                                        .small(),
                                    )
                                    .truncate(),
                                );
                                if is_active {
                                    ui.add(
                                        egui::Label::new(
                                            egui::RichText::new("In progress").small(),
                                        )
                                        .truncate(),
                                    );
                                } else {
                                    let s = crate::visibility::summary(run);
                                    ui.add(egui::Label::new(egui::RichText::new(format!(
                                        "{} changed · {} unchanged · {} unknown · {} need review",
                                        s.changed, s.unchanged, s.unknown, s.review
                                    )).small()).truncate());
                                }
                            });
                        }
                    });
                }
            });
        if self.show_active_navigation {
            self.show_active_navigation = false;
            egui::Popup::open_id(ui.ctx(), navigation.response.id.with("popup"));
            navigation.response.scroll_to_me(Some(egui::Align::Min));
        }
        self.select_run(selected);
        let history_protected = self.history_protections();
        let mut reuse = None;
        let mut remove = None;
        let mut clear = false;
        let mut cleanup = false;
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
            {
                ui.menu_button("History cleanup", |ui| {
                    if ui.add_enabled(discard_enabled && self.state.runs.iter().any(|r| !r.active() && r.unresolved_results()),
                        egui::Button::new("Discard all unresolved results…"))
                        .on_hover_text("Discard unresolved isolated results in terminal convoys only, after confirmation. Active convoys are excluded.").clicked() {
                        discard = Some(crate::persistence::bulk_discard::Scope::All);
                        ui.close();
                    }
                    ui.separator();
                    if ui.add_enabled(self.history_available(), egui::Button::new("Clean Up History…")).clicked() {
                        cleanup = true; ui.close();
                    }
                    ui.label("History removal deletes local metadata and session output only.");
                    ui.weak("Retained copies stay protected until explicit cleanup.");
                    if let Some(run)=self.state.runs.iter().find(|r|Some(r.id)==self.selected_run)
                        && !run.active() && ui.add_enabled(!run.history_protected() && !self.manager.history_protected(run.id) && !self.validations.history_protected(run.id) && self.history_available(), egui::Button::new(format!("Remove convoy #{} from history",run.id))).on_hover_text("Retained isolated results must remain in history. Use Discard convoy… for unresolved results; Applied copies require Clean up retained copy in Review.").clicked() {
                        remove=Some(run.id); ui.close();
                    }
                    ui.separator();
                    if ui.add_enabled(self.history_available(), egui::Button::new(format!("Clear removable history ({})", self.state.runs.iter().filter(|r| crate::history::eligible(r, &history_protected)).count()))).clicked() {
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
            && (run.history_protected() || history_protected.contains(&run.id))
        {
            ui.small(if history_protected.contains(&run.id) {
                "History removal blocked: a worker or lifecycle operation still owns resources, or process cleanup is unconfirmed."
            } else if run.jobs.iter().any(|j| j.validation.as_ref().is_some_and(|v| v.status == crate::validation::Status::Running)) {
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
        if cleanup {
            self.offer_history_cleanup(domain::now());
        }
        if clear {
            self.remove_history(None);
        } else if let Some(id) = remove {
            self.remove_history(Some(id));
        }
    }
}

// Arrow/Home/End navigation reaches off-screen rows without depending on which
// widgets virtualization created. Selection always uses the immutable convoy ID.
fn navigate_history(ui: &mut egui::Ui, runs: &[&Run], selected: &mut Option<u64>) -> bool {
    if runs.is_empty() {
        return false;
    }
    let current = runs.iter().position(|r| Some(r.id) == *selected);
    let target = ui.input_mut(|input| {
        if input.consume_key(egui::Modifiers::NONE, egui::Key::Home) {
            Some(0)
        } else if input.consume_key(egui::Modifiers::NONE, egui::Key::End) {
            Some(runs.len() - 1)
        } else if input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown) {
            Some(current.map_or(0, |i| (i + 1).min(runs.len() - 1)))
        } else if input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp) {
            Some(current.map_or(0, |i| i.saturating_sub(1)))
        } else {
            None
        }
    });
    if let Some(index) = target {
        *selected = Some(runs[index].id);
    }
    target.is_some()
}
