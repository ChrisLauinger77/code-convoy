use super::*;
use crate::attachments::{self, Attachment};

#[derive(Default)]
pub(super) struct AttachmentWork {
    pub pending: bool,
    request: u64,
    focus_add: bool,
}
impl AttachmentWork {
    pub fn invalidate(&mut self) {
        self.request = self.request.wrapping_add(1);
        self.pending = false;
        self.focus_add = false;
    }
    fn begin(&mut self) -> u64 {
        self.invalidate();
        self.pending = true;
        self.request
    }
    pub(super) fn begin_picker(&mut self) -> u64 {
        let request = self.begin();
        self.focus_add = true;
        request
    }
}

pub(super) fn size_label(size: u64) -> String {
    if size >= 1024 * 1024 {
        format!("{:.1} MiB", size as f64 / (1024.0 * 1024.0))
    } else if size >= 1024 {
        format!("{:.1} KiB", size as f64 / 1024.0)
    } else {
        format!("{size} B")
    }
}

fn inspect_files(paths: Vec<PathBuf>) -> (Vec<Attachment>, Vec<String>) {
    let mut files = Vec::new();
    let mut errors = Vec::new();
    for path in paths {
        match Attachment::inspect(&path) {
            Ok(file) => files.push(file),
            Err(error) => errors.push(format!("{error:#}")),
        }
    }
    (files, errors)
}

impl App {
    pub(super) fn attachment_error(&self) -> Option<String> {
        let backend = agents::backend(self.state.draft.agent).ok()?;
        backend
            .validate_attachments(&self.state.draft)
            .err()
            .map(|error| error.to_string())
    }

    fn add_attachment_paths(&mut self, paths: Vec<PathBuf>, ctx: egui::Context) {
        if self.attachment_work.pending || paths.is_empty() {
            return;
        }
        let request = self.attachment_work.begin();
        self.dispatch(ctx, async move {
            match tokio::task::spawn_blocking(move || inspect_files(paths)).await {
                Ok((files, errors)) => Message::Attachments(request, files, errors),
                Err(error) => Message::Attachments(
                    request,
                    Vec::new(),
                    vec![format!("Attachment worker failed: {error}")],
                ),
            }
        });
    }

    fn browse_attachments(&mut self, ctx: egui::Context) {
        if self.attachment_work.pending {
            return;
        }
        let request = self.attachment_work.begin_picker();
        // Construction stays on the UI thread, matching the existing macOS sheet.
        let selection = self
            .repository_dialog
            .clone()
            .set_title("Add task attachments")
            .add_filter("Task context and images", attachments::EXTENSIONS)
            .pick_files();
        self.dispatch(ctx, async move {
            let paths = selection
                .await
                .unwrap_or_default()
                .into_iter()
                .map(|file| file.path().to_owned())
                .collect();
            match tokio::task::spawn_blocking(move || inspect_files(paths)).await {
                Ok((files, errors)) => Message::Attachments(request, files, errors),
                Err(error) => Message::Attachments(
                    request,
                    Vec::new(),
                    vec![format!("Attachment worker failed: {error}")],
                ),
            }
        });
    }

    pub(super) fn attachments_added(
        &mut self,
        request: u64,
        files: Vec<Attachment>,
        mut errors: Vec<String>,
    ) {
        if request != self.attachment_work.request {
            return;
        }
        self.attachment_work.pending = false;
        for file in files {
            if self
                .state
                .draft
                .attachments
                .iter()
                .any(|existing| existing.path == file.path)
            {
                continue;
            }
            let mut candidate = self.state.draft.attachments.clone();
            candidate.push(file.clone());
            if let Err(error) = attachments::validate_limits(&candidate) {
                errors.push(format!("{}: {error}", file.filename));
            } else {
                self.state.draft.attachments.push(file);
                self.dirty = true;
            }
        }
        if !errors.is_empty() {
            self.notice = format!("Some attachments were not added:\n{}", errors.join("\n"));
        }
    }

    pub(super) fn validate_reused_attachments(&mut self) {
        self.attachment_work.invalidate();
        if self.state.draft.attachments.is_empty() {
            return;
        }
        let request = self.attachment_work.begin();
        let attachments = self.state.draft.attachments.clone();
        // Reuse is triggered outside rendering as well, so request a repaint via
        // the existing channel polling instead of relying on a picker context.
        let tx = self.tx.clone();
        self.runtime().spawn(async move {
            let (files, errors) =
                match tokio::task::spawn_blocking(move || attachments::for_reuse(&attachments))
                    .await
                {
                    Ok(result) => result,
                    Err(error) => (
                        Vec::new(),
                        vec![format!("Cannot validate reused attachments: {error}")],
                    ),
                };
            let _ = tx.send(Message::ReusedAttachments(request, files, errors));
        });
    }

    pub(super) fn attachments_reused(
        &mut self,
        request: u64,
        files: Vec<Attachment>,
        errors: Vec<String>,
    ) {
        if request != self.attachment_work.request {
            return;
        }
        self.attachment_work.pending = false;
        self.state.draft.attachments = files;
        self.dirty = true;
        if !errors.is_empty() {
            self.draft_message.push_str(&format!(
                " {} attachment(s) could not be restored; see the notice.",
                errors.len()
            ));
            self.notice = format!(
                "Reused convoy has missing, unreadable or changed attachments. Restore or re-add them before running:\n{}",
                errors.join("\n")
            );
        }
    }

    pub(super) fn attachments_section(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let section = ui.scope(|ui| {
            let add_button = ui
                .horizontal(|ui| {
                    ui.strong("Attachments");
                    let response = ui.add_enabled(
                        !self.attachment_work.pending,
                        theme::quiet("Add files…").small(),
                    );
                    if self.attachment_work.focus_add && response.enabled() {
                        response.request_focus();
                        self.attachment_work.focus_add = false;
                    }
                    if response.clicked() {
                        self.browse_attachments(ctx.clone());
                    }
                    if self.attachment_work.pending {
                        ui.spinner();
                    }
                    response.id
                })
                .inner;
            if self.state.draft.attachments.is_empty() {
                ui.weak("Add files or drop them here.");
            }
            let mut remove = None;
            for (index, attachment) in self.state.draft.attachments.iter().enumerate() {
                ui.horizontal(|ui| {
                    let width = (ui.available_width() - 190.0).max(40.0);
                    ui.allocate_ui_with_layout(
                        egui::vec2(width, 24.0),
                        egui::Layout::left_to_right(egui::Align::Center),
                        |ui| {
                            ui.set_min_width(width);
                            ui.add(egui::Label::new(&attachment.filename).truncate())
                                .on_hover_text(attachment.path.display().to_string());
                        },
                    );
                    ui.small(format!(
                        "{} · {}",
                        attachment.kind.label(),
                        size_label(attachment.size)
                    ));
                    let response = ui.add_enabled(
                        !self.attachment_work.pending,
                        theme::quiet("Remove").small(),
                    );
                    response.widget_info(|| {
                        egui::WidgetInfo::labeled(
                            egui::WidgetType::Button,
                            response.enabled(),
                            format!("Remove attachment {}", attachment.filename),
                        )
                    });
                    if response.clicked() {
                        remove = Some(index);
                    }
                });
            }
            if let Some(index) = remove {
                self.state.draft.attachments.remove(index);
                self.dirty = true;
                ui.memory_mut(|memory| memory.request_focus(add_button));
            }
            if let Some(error) = self.attachment_error() {
                ui.colored_label(theme::Palette::of(ui).error, error);
            }
            if let Ok(backend) = agents::backend(self.state.draft.agent) {
                ui.small("Supplied to every selected repository job.")
                    .on_hover_text(backend.attachment_summary());
            }
        });
        let hovered = ctx.input(|input| {
            input
                .pointer
                .hover_pos()
                .is_some_and(|pos| section.response.rect.expand(10.0).contains(pos))
        });
        if hovered && !self.attachment_work.pending && self.prepared.is_none() && !self.closing {
            let dropped = ctx.input(|input| input.raw.dropped_files.clone());
            let paths = dropped
                .iter()
                .map(|file| file.path().to_owned())
                .collect::<Vec<_>>();
            self.add_attachment_paths(paths, ctx.clone());
        }
    }
}
