//! Native save selection and background report generation, independent of Store saves.
use super::*;
use crate::export::{Format, Scope, Snapshot, file::Destination};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Choice {
    Selected,
    Filtered,
    All,
}
pub(super) struct Dialog {
    pub selected: Option<u64>,
    pub choice: Choice,
    pub format: Format,
    focus_cancel: bool,
}
#[derive(Default)]
pub(super) struct Export {
    pub dialog: Option<Dialog>,
    pub pending: Option<Pending>,
    pub overwrite: Option<Destination>,
    focus_replace_cancel: bool,
    pub feedback: String,
}
pub(super) struct Pending {
    pub snapshot: Option<Snapshot>,
    pub format: Format,
    pub cancel: crate::process::Cancellation,
}
impl App {
    pub(super) fn export_available(&self) -> bool {
        self.export.pending.is_none() && !self.closing && !self.quit_requested
    }
    pub(super) fn offer_export(&mut self, selected: Option<u64>) {
        if !self.export_available() {
            return;
        }
        self.export.feedback.clear();
        self.export.dialog = Some(Dialog {
            selected: selected.or(self.selected_run),
            choice: if selected.is_some() {
                Choice::Selected
            } else {
                Choice::Filtered
            },
            format: Format::Markdown,
            focus_cancel: true,
        });
    }
    /// Called once at Export confirmation. Selection is the dialog's stable ID,
    /// while filtered/all scope resolves against current history at this point.
    pub(super) fn capture_export(&mut self) -> anyhow::Result<Snapshot> {
        let dialog = self
            .export
            .dialog
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("No export is selected."))?;
        let scope = match dialog.choice {
            Choice::Selected => Scope::Selected(dialog.selected.into_iter().collect()),
            Choice::Filtered => Scope::Filtered(self.history_search.matching(&self.state.runs)),
            Choice::All => Scope::All,
        };
        Snapshot::capture(&self.state.runs, &scope)
    }
    fn choose_export_file(&mut self, ctx: &egui::Context) {
        if !self.export_available() {
            return;
        }
        let snapshot = match self.capture_export() {
            Ok(snapshot) => snapshot,
            Err(error) => {
                self.export.feedback = format!("Export failed: {error}");
                return;
            }
        };
        let Some(dialog) = self.export.dialog.take() else {
            return;
        };
        let selection = self
            .repository_dialog
            .clone()
            .set_title("Export history")
            .set_can_create_directories(true)
            .add_filter(dialog.format.label(), &[dialog.format.extension()])
            .set_file_name(snapshot.suggested_name(dialog.format, domain::now()))
            .save_file();
        self.export.pending = Some(Pending {
            snapshot: Some(snapshot),
            format: dialog.format,
            cancel: Default::default(),
        });
        let storage = self.store.directory().to_path_buf();
        self.dispatch(ctx.clone(), async move {
            let Some(file) = selection.await else {
                return Message::ExportDestination(Ok(None));
            };
            let path = file.path().to_owned();
            let result = tokio::task::spawn_blocking(move || {
                let destination = Destination::inspect(&path)?;
                // An export must never replace CodeConvoy's own state/ownership data.
                anyhow::ensure!(
                    !destination.path.starts_with(storage.canonicalize()?),
                    "Choose a destination outside CodeConvoy's application data directory."
                );
                Ok::<_, anyhow::Error>(destination)
            })
            .await
            .map_err(|e| e.to_string())
            .and_then(|r| r.map_err(|e| format!("{e:#}")));
            Message::ExportDestination(result.map(Some))
        });
    }
    pub(super) fn export_destination(&mut self, destination: Result<Option<Destination>, String>) {
        let Some(pending) = &self.export.pending else {
            return;
        };
        if pending.cancel.is_cancelled() || self.closing {
            self.export.pending = None;
            self.export.feedback = "Export cancelled.".into();
            return;
        }
        match destination {
            Ok(Some(destination)) if destination.exists() => {
                self.export.overwrite = Some(destination);
                self.export.focus_replace_cancel = true;
            }
            Ok(Some(destination)) => self.write_export(destination, false),
            Ok(None) => {
                self.export.pending = None;
                self.export.feedback = "Export cancelled.".into();
            }
            Err(error) => {
                self.export.pending = None;
                self.export.feedback = format!("Export failed: {error}");
            }
        }
    }
    pub(super) fn write_export(&mut self, destination: Destination, replace: bool) {
        let Some(pending) = &mut self.export.pending else {
            return;
        };
        let Some(snapshot) = pending.snapshot.take() else {
            return;
        };
        let format = pending.format;
        let cancel = pending.cancel.clone();
        let count = snapshot.convoys.len();
        let filename = destination
            .path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        let tx = self.tx.clone();
        self.runtime().spawn(async move {
            let result = tokio::task::spawn_blocking(move || {
                crate::export::file::save(&snapshot, format, &destination, replace, &cancel)
            })
            .await
            .map_err(|e| e.to_string())
            .and_then(|r| r.map_err(|e| format!("{e:#}")));
            let _ = tx.send(Message::ExportSaved(
                result.map(|()| format!("Exported {count} convoys to {filename}")),
            ));
        });
    }
    pub(super) fn export_saved(&mut self, result: Result<String, String>) {
        self.export.pending = None;
        self.export.feedback = match result {
            Ok(message) => message,
            Err(error) => format!("Export failed: {error}"),
        };
    }
    pub(super) fn cancel_export(&mut self) {
        if let Some(pending) = &self.export.pending {
            pending.cancel.cancel();
        }
        if self.export.overwrite.take().is_some() {
            self.export.pending = None;
            self.export.feedback = "Export cancelled.".into();
        }
        self.export.dialog = None;
    }
    pub(super) fn export_status(&mut self, ui: &mut egui::Ui) {
        if self.export.pending.is_some() {
            ui.horizontal_wrapped(|ui| {
                ui.spinner();
                ui.small("Export in progress…");
                if ui.button("Cancel export").clicked() {
                    self.cancel_export();
                }
            });
        }
        if !self.export.feedback.is_empty() {
            ui.small(&self.export.feedback);
        }
    }
    pub(super) fn export_window(&mut self, ctx: &egui::Context) {
        if self.quit_requested || self.closing {
            return;
        }
        if let Some(destination) = &self.export.overwrite {
            let mut cancel = false;
            let mut replace = false;
            let response = dialog::show(ctx, "export_replace", |ui, _opening| {
                ui.set_width(400.0_f32.min(ctx.content_rect().width() - 60.0));
                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
                ui.heading("Replace existing export file?");
                ui.label(destination.path.display().to_string());
                ui.label("This replaces the existing file with the captured history report.");
                ui.horizontal(|ui| {
                    let button = ui.button("Cancel");
                    if self.export.focus_replace_cancel {
                        button.request_focus();
                        self.export.focus_replace_cancel = false;
                    }
                    cancel = button.clicked();
                    replace = ui.button("Replace file").clicked();
                });
            });
            if cancel || response.should_close() {
                self.cancel_export();
            } else if replace && let Some(destination) = self.export.overwrite.take() {
                self.write_export(destination, true);
            }
            return;
        }
        let matching = if self.export.dialog.is_some() {
            self.history_search.matching(&self.state.runs)
        } else {
            return;
        };
        let Some(dialog) = &mut self.export.dialog else {
            return;
        };
        let selected_exists = dialog
            .selected
            .is_some_and(|id| self.state.runs.iter().any(|r| r.id == id && !r.active()));
        let mut cancel = false;
        let mut confirm = false;
        let response = dialog::show(ctx, "history_export", |ui, _opening| {
            ui.set_width(400.0_f32.min(ctx.content_rect().width() - 60.0));
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
            ui.heading("Export History");
            ui.label("Scope");
            ui.add_enabled_ui(dialog.selected.is_some(), |ui| {
                ui.radio_value(
                    &mut dialog.choice,
                    Choice::Selected,
                    dialog.selected.map_or_else(
                        || "Selected convoy".into(),
                        |id| format!("Selected convoy #{id}"),
                    ),
                );
            });
            ui.radio_value(&mut dialog.choice, Choice::Filtered, "Filtered history");
            ui.radio_value(&mut dialog.choice, Choice::All, "All retained history");
            let count = match dialog.choice {
                Choice::Selected => usize::from(selected_exists),
                Choice::Filtered => matching.len(),
                Choice::All => self.state.runs.iter().filter(|r| !r.active()).count(),
            };
            ui.horizontal(|ui| {
                ui.label("Format");
                ui.radio_value(&mut dialog.format, Format::Markdown, "Markdown");
                ui.radio_value(&mut dialog.format, Format::Json, "JSON");
            });
            ui.label(format!("Convoys: {count}"));
            ui.small("Destination: choose a file with Export…");
            ui.small("Saved results, bounded task/name metadata, and validation metadata. No logs, diffs or attachments. Review before sharing.");
            if dialog.choice == Choice::Selected && !selected_exists {
                ui.label("The selected convoy is missing or still active.");
            }
            if !self.export.feedback.is_empty() {
                ui.label(&self.export.feedback);
            }
            ui.horizontal(|ui| {
                let button = ui.button("Cancel");
                if dialog.focus_cancel {
                    button.request_focus();
                    dialog.focus_cancel = false;
                }
                cancel = button.clicked();
                confirm = ui
                    .add_enabled(count > 0, egui::Button::new("Export…"))
                    .clicked();
            });
        });
        if cancel || response.should_close() {
            self.export.dialog = None;
        } else if confirm {
            self.choose_export_file(ctx);
        }
    }
}
