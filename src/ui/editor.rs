use super::*;

impl App {
    pub(super) fn editor_pane(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let rect = ui.available_rect_before_wrap();
        let execution_top = (rect.bottom() - self.execution_height).max(rect.top() + 60.0);
        let editor_rect = egui::Rect::from_min_max(
            rect.min,
            egui::pos2(rect.right(), execution_top - theme::GAP),
        );
        // Render in visual order for Tab traversal; reserve a fixed footer area.
        ui.scope_builder(egui::UiBuilder::new().max_rect(editor_rect), |ui| {
            egui::ScrollArea::vertical()
                .id_salt("editor_scroll")
                .max_height(editor_rect.height())
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.add_enabled_ui(self.prepared.is_none() && !self.closing, |ui| {
                        self.editor(ui, ctx)
                    });
                });
        });
        let execution_rect =
            egui::Rect::from_min_max(egui::pos2(rect.left(), execution_top), rect.max);
        let response = ui.scope_builder(egui::UiBuilder::new().max_rect(execution_rect), |ui| {
            ui.add_enabled_ui(self.prepared.is_none() && !self.closing, |ui| {
                self.execution_section(ui, ctx)
            });
        });
        let height = response.response.rect.height();
        if (self.execution_height - height).abs() > 0.5 {
            self.execution_height = height;
            ctx.request_repaint();
        }
    }

    pub(super) fn editor(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        if self.focus_draft {
            ui.scroll_to_cursor(Some(egui::Align::Min));
        }
        theme::eyebrow(ui, "NEW CONVOY");
        self.task_section(ui);
        self.agent_section(ui, ctx);
        self.repositories_section(ui, ctx);
    }

    fn task_section(&mut self, ui: &mut egui::Ui) {
        theme::section(ui, "Task");
        ui.weak("One instruction, applied to each repository.");
        if !self.draft_message.is_empty() {
            ui.small(&self.draft_message);
        }
        let response = ui.add(
            egui::TextEdit::multiline(&mut self.state.draft.prompt)
                .desired_rows(5)
                .desired_width(f32::INFINITY)
                .hint_text("Describe the change or review…"),
        );
        self.dirty |= response.changed();
        if self.focus_draft {
            response.request_focus();
            self.focus_draft = false;
        }
    }

    pub(super) fn execution_section(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.separator();
        ui.strong("Execution");
        ui.horizontal(|ui| {
            let label = ui.label("This convoy");
            self.dirty |= ui
                .add(
                    egui::DragValue::new(&mut self.state.draft.concurrency)
                        .range(1..=16)
                        .speed(0.1),
                )
                .labelled_by(label.id)
                .changed();
        });
        ui.horizontal(|ui| {
            let label = ui.label("Global job limit");
            if ui.add(egui::DragValue::new(&mut self.state.global_concurrency).range(1..=domain::MAX_CONCURRENCY).speed(0.1))
                .labelled_by(label.id).on_hover_text("Shared by all convoys. Lowering the limit lets existing jobs finish before admitting more.").changed() {
                if let Err(error) = self.manager.set_global_limit(self.state.global_concurrency) { self.notice = error.to_string(); }
                self.dirty = true;
            }
        });
        ui.weak(format!(
            "Run this task in {} selected repositories.",
            self.selected.len()
        ));
        let enabled = !self.busy
            && !self.current_cli_check()
            && !self.selected.is_empty()
            && !self.state.draft.prompt.trim().is_empty()
            && agents::backend(self.state.draft.agent).is_ok();
        if theme::primary(ui, enabled, "Run Convoy")
            .on_hover_text("Review repository state before starting jobs")
            .clicked()
        {
            self.preflight(ctx.clone());
        }
        let hint = if self.busy {
            "Checking repository state…"
        } else if self.current_cli_check() {
            "Checking agent CLI before convoy review…"
        } else if self.state.draft.prompt.trim().is_empty() {
            "Enter a task, then select repositories."
        } else if self.selected.is_empty() {
            "Select at least one repository."
        } else {
            "After launch, task text and selections clear; settings stay."
        };
        ui.small(hint);
    }

    pub(super) fn preflight_window(&mut self, ctx: &egui::Context) {
        let Some(prepared) = &self.prepared else {
            return;
        };
        let mut start = false;
        let mut cancel = false;
        let response = egui::Modal::new(egui::Id::new("review_convoy")).show(ctx, |ui| {
            ui.set_width(540.0_f32.min((ctx.content_rect().width() - 64.0).max(240.0)));
            ui.heading("Review convoy");
            egui::ScrollArea::vertical()
                .max_height((ctx.content_rect().height() - 230.0).max(160.0))
                .show(ui, |ui| {
                    ui.strong(format!(
                        "{} repositories · {} concurrent jobs",
                        prepared.repositories.len(),
                        prepared.task.concurrency
                    ));
                    ui.label(prepared.task.agent.label());
                    ui.label(&prepared.task.prompt);
                    ui.small(format!(
                        "Global job limit: {} · busy repositories remain queued",
                        self.state.global_concurrency
                    ));
                    if let Ok(backend) = agents::backend(prepared.task.agent) {
                        ui.small(backend.execution_summary(&prepared.task.options));
                    }
                    for entry in &prepared.repositories {
                        ui.separator();
                        ui.strong(&entry.repository.name);
                        ui.small(entry.repository.path.display().to_string());
                        ui.label(format!(
                            "{} · {} existing changes",
                            entry.state.summary.branch, entry.state.summary.changed
                        ));
                        for line in &entry.state.entries {
                            ui.label(egui::RichText::new(line).monospace());
                        }
                    }
                    if prepared.dirty() {
                        ui.add_space(theme::GAP);
                        ui.colored_label(
                            theme::Palette::of(ui).warning,
                            "Some repositories already have changes. Agent edits may overlap them.",
                        );
                        ui.checkbox(
                            &mut self.dirty_ack,
                            "I reviewed the existing changes and want to proceed.",
                        );
                    }
                });
            ui.separator();
            start =
                theme::primary(ui, !prepared.dirty() || self.dirty_ack, "Start convoy").clicked();
            cancel = ui.add(theme::quiet("Back to task")).clicked();
        });
        cancel |= response.should_close();
        if start {
            self.start();
        } else if cancel {
            self.prepared = None;
        }
    }
}
