use super::{egui, result_markdown, theme};
use crate::domain::Run;

pub(super) fn summary(ui: &mut egui::Ui, run: &Run) {
    if !run.active() && !run.jobs.is_empty() {
        button(
            ui,
            ("copy_summary", run.id, 0),
            "Copy Summary",
            "Summary copied",
            || result_markdown::convoy(run),
        );
    }
}

pub(super) fn repository(ui: &mut egui::Ui, run: &Run, index: usize) {
    if let Some(job) = run.jobs.get(index)
        && job.status.is_terminal()
    {
        button(
            ui,
            ("copy_repository", run.id, index),
            "Copy Repository Result",
            "Repository result copied",
            || result_markdown::repository(run, job),
        );
    }
}

fn button(
    ui: &mut egui::Ui,
    key: (&str, u64, usize),
    label: &str,
    confirmation: &str,
    text: impl FnOnce() -> String,
) {
    let id = egui::Id::new(key);
    let now = ui.input(|input| input.time);
    let response = ui
        .add(theme::quiet(label).small())
        .on_hover_text("Copy saved results as Markdown. Includes bounded task/name/command metadata; review before sharing. No logs, diffs or attachments.");
    if response.has_focus() {
        response.scroll_to_me(None);
    }
    if response.clicked() {
        ui.ctx().copy_text(text());
        ui.data_mut(|data| data.insert_temp(id, now + 3.0));
    }
    if let Some(until) = ui.data(|data| data.get_temp::<f64>(id))
        && until > now
    {
        ui.small(confirmation);
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_secs_f64(until - now));
    }
}
