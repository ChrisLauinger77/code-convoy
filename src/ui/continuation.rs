use super::*;

impl App {
    pub(super) fn continuation_controls(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let Some(run) = self
            .state
            .runs
            .iter()
            .find(|r| Some(r.id) == self.selected_run)
        else {
            return;
        };
        let id = run.id;
        let mut retry = false;
        let mut source = None;
        if let Some(origin) = &run.provenance {
            ui.horizontal_wrapped(|ui| {
                ui.small(origin.label());
                if self.state.runs.iter().any(|r| r.id == origin.source_run())
                    && ui.add(theme::quiet("Show source convoy").small()).clicked()
                {
                    source = Some(origin.source_run());
                }
            });
        }
        if run
            .jobs
            .get(self.selected_job)
            .is_some_and(domain::Job::retryable)
        {
            retry = ui.add_enabled(!self.busy && self.prepared.is_none() && !self.closing && !self.quit_requested,
                theme::quiet("Retry").small()).on_hover_text("Review a new one-repository attempt with the original task and settings. The original result and current draft stay intact.").clicked();
        }
        if retry {
            self.retry(ctx, id, self.selected_job);
        }
        if let Some(source) = source {
            self.select_run(Some(source));
        }
    }
    pub(super) fn retry(&mut self, ctx: &egui::Context, run: u64, index: usize) {
        if self.busy || self.prepared.is_some() || self.closing || self.quit_requested {
            return;
        }
        let request = match crate::continuation::Retry::from_state(&self.state, run, index) {
            Ok(request) => request,
            Err(e) => {
                self.notice = e.to_string();
                return;
            }
        };
        self.busy = true;
        self.notice.clear();
        self.dispatch(ctx.clone(), async move {
            Message::RetryPrepared(request.prepare().await.map_err(|e| format!("{e:#}")))
        });
    }
    pub(super) fn followup(&mut self, ctx: &egui::Context, run: u64) {
        if self.busy || self.prepared.is_some() || self.closing || self.quit_requested {
            return;
        }
        let request = match crate::continuation::FollowUp::from_state(
            &self.state,
            run,
            &self.review_selected,
        ) {
            Ok(request) => request,
            Err(e) => {
                self.notice = e.to_string();
                return;
            }
        };
        self.busy = true;
        self.followup_pending = true;
        self.dispatch(ctx.clone(), async move {
            Message::FollowUp(Box::new(request.check().await))
        });
    }
    pub(super) fn apply_followup(&mut self, draft: crate::continuation::FollowUp) {
        let (selected, message) = draft.populate(&mut self.state);
        self.selected = selected;
        self.draft_message = message;
        self.attachment_work.invalidate();
        self.prepared_provenance = None;
        self.focus_draft = true;
        self.dirty = true;
        self.sync_cli_checks();
    }
}
