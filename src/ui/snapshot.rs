use super::*;

pub(super) fn show(ui: &mut egui::Ui, run: &Run, job: &domain::Job) {
    ui.strong("Task");
    ui.add(egui::Label::new(&run.task.prompt).wrap().selectable(true));
    if ui.add(theme::quiet("Copy task").small()).clicked() {
        ui.ctx().copy_text(run.task.prompt.clone());
    }
    ui.separator();
    if !run.task.attachments.is_empty() {
        ui.strong(format!("Attachments ({})", run.task.attachments.len()));
        ui.small("Original references and metadata; file contents are not saved. Files may have changed or disappeared.");
        for attachment in &run.task.attachments {
            ui.label(format!(
                "{} · {} · {}",
                attachment.filename,
                attachment.kind.label(),
                attachments_ui::size_label(attachment.size)
            ));
            ui.add(
                egui::Label::new(attachment.path.display().to_string())
                    .wrap()
                    .selectable(true),
            );
        }
        ui.separator();
    }
    ui.label(run.task.execution_mode.label());
    if let Some(worktree) = &job.worktree {
        ui.label(format!(
            "Base: {}",
            worktree.base_commit.chars().take(12).collect::<String>()
        ));
        if let Some(label) = format::isolated_result(job) {
            ui.label(label);
        }
        ui.small("Results are retained working directories, checked after restart. External edits remain visible. Apply/Discard is planned for Part 3.2.");
        egui::CollapsingHeader::new("Worktree location").show(ui, |ui| {
            ui.add(
                egui::Label::new(worktree.path.display().to_string())
                    .wrap()
                    .selectable(true),
            );
        });
    }

    ui.strong(run.task.agent.label());
    if let Ok(backend) = agents::backend(run.task.agent) {
        for spec in backend.options() {
            ui.horizontal_wrapped(|ui| {
                ui.label(format!("{}:", format::option_label(spec)))
                    .on_hover_text(spec.help);
                ui.add(
                    egui::Label::new(format::option_value(spec, run.task.options.get(spec.key)))
                        .wrap()
                        .selectable(true),
                );
            });
        }
        // Render only declared non-secret settings. Old unsupported keys do not
        // become an arbitrary serialized dump in the historical view.
        if run
            .task
            .options
            .keys()
            .any(|key| !backend.options().iter().any(|s| s.key == key))
        {
            ui.weak("Some saved settings are no longer recognized by this version.");
        }
    }
    ui.label(format!(
        "This convoy: up to {} concurrent jobs",
        run.task.concurrency
    ));
    ui.small("The global limit is a live preference and is not recorded in this snapshot.");
    ui.label(format!("Created: {}", format::timestamp(run.created_at)));
    ui.label(format!(
        "Convoy duration: {}",
        format::duration(run.elapsed(domain::now()))
    ));
    if let Some(start) = job.started_at {
        ui.label(format!(
            "Selected job started: {}",
            format::timestamp(start)
        ));
        ui.label(format!(
            "Selected job duration: {}",
            format::duration(
                job.finished_at
                    .unwrap_or_else(domain::now)
                    .saturating_sub(start)
            )
        ));
    }
    if let Some(end) = job.finished_at {
        ui.label(format!("Selected job finished: {}", format::timestamp(end)));
    }
    ui.separator();
    ui.strong(format!("Repositories ({})", run.jobs.len()));
    for entry in &run.jobs {
        ui.label(&entry.repository.name);
        ui.add(
            egui::Label::new(
                egui::RichText::new(entry.repository.path.display().to_string()).small(),
            )
            .wrap()
            .selectable(true),
        );
    }
    if let Some(before) = &job.before {
        ui.separator();
        ui.strong("Selected repository before execution");
        ui.label(format!(
            "Branch: {} · {} existing changes",
            before.branch, before.changed
        ));
        ui.add(
            egui::Label::new(
                egui::RichText::new(
                    before
                        .head
                        .as_deref()
                        .map(|head| {
                            if job.execution_mode == domain::ExecutionMode::IsolatedWorktree {
                                head.chars().take(12).collect::<String>()
                            } else {
                                head.to_owned()
                            }
                        })
                        .unwrap_or_else(|| "No initial commit".into()),
                )
                .monospace(),
            )
            .wrap()
            .selectable(true),
        );
    }
    if let Some(code) = job.exit_code {
        ui.label(format!("Exit code: {code}"));
    }
}
