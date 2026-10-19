//! Global Git client preference, edited in a dialog because egui menus close on
//! any click. Lookups run on a blocking worker, never while rendering, and only
//! after the draft changes.
use super::*;
use crate::git_client::{Client, Platform, Preferences};

pub(super) struct GitClientDialog {
    pub draft: Preferences,
    /// Custom arguments, one per line, mirrored into the draft.
    pub arguments: String,
    pub browsing: bool,
    /// Draft snapshot of the latest dispatched check.
    pub checked: Option<Preferences>,
    pub status: Option<Result<String, String>>,
    pub error: String,
}
impl GitClientDialog {
    pub fn new(preferences: &Preferences) -> Self {
        Self {
            draft: preferences.clone(),
            arguments: preferences.arguments.join("\n"),
            browsing: false,
            checked: None,
            status: None,
            error: String::new(),
        }
    }
}

/// One argument per line. Blank lines are ignored and Windows line endings are
/// removed; other whitespace is kept because paths can contain it.
pub(super) fn parse_arguments(text: &str) -> Vec<String> {
    text.lines()
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
        .collect()
}

impl App {
    pub(super) fn open_git_client_settings(&mut self) {
        if !self.closing {
            self.git_client_dialog = Some(GitClientDialog::new(&self.state.git_client));
        }
    }

    pub(super) fn git_client_settings_window(&mut self, ctx: &egui::Context) {
        if self.git_client_dialog.is_none() {
            return;
        }
        let mut chosen = None;
        let mut browse = false;
        let mut cancel = false;
        let mut save = false;
        let response = dialog::show(ctx, "git_client_settings", |ui, opening| {
            let Some(dialog) = self.git_client_dialog.as_mut() else {
                return;
            };
            ui.set_width(460.0_f32.min(ctx.content_rect().width() - 60.0));
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
            ui.heading("Git client");
            let current = dialog.draft.client;
            let mut selected = current;
            egui::ComboBox::from_label("Client")
                .selected_text(current.map_or("Not configured", Client::label))
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut selected, None, "Not configured");
                    for client in Client::ALL {
                        match client.availability(Platform::current()) {
                            Ok(()) => {
                                ui.selectable_value(&mut selected, Some(client), client.label());
                            }
                            Err(reason) => {
                                ui.add_enabled(
                                    false,
                                    egui::Button::selectable(false, client.label()),
                                )
                                .on_disabled_hover_text(reason);
                            }
                        }
                    }
                });
            if selected != current {
                chosen = Some(selected);
            }
            if let Some(client) = dialog.draft.client {
                browse = git_client_fields(ui, dialog, client);
            }
            git_client_status(ui, dialog);
            ui.small(
                "Opens the exact repository or worktree path as a separate process argument, without a shell. Opening runs no Git command and changes nothing.",
            );
            if !dialog.error.is_empty() {
                ui.colored_label(theme::Palette::of(ui).error, &dialog.error);
            }
            ui.horizontal(|ui| {
                let button = ui.button("Cancel");
                if opening {
                    button.request_focus();
                }
                cancel = button.clicked();
                save = ui
                    .add_enabled(!dialog.browsing, egui::Button::new("Save"))
                    .clicked();
            });
        });
        if cancel || response.should_close() {
            self.git_client_dialog = None;
            return;
        }
        if let Some(client) = chosen {
            self.select_git_client(client);
        }
        if browse {
            self.browse_git_client();
        }
        if save {
            self.save_git_client();
        } else {
            self.check_git_client();
        }
    }

    pub(super) fn select_git_client(&mut self, client: Option<Client>) {
        if let Some(dialog) = self
            .git_client_dialog
            .as_mut()
            .filter(|dialog| dialog.draft.client != client)
        {
            dialog.draft = Preferences::select(client);
            dialog.arguments = dialog.draft.arguments.join("\n");
        }
    }

    fn browse_git_client(&mut self) {
        let Some(dialog) = self.git_client_dialog.as_mut() else {
            return;
        };
        let Some(client) = dialog.draft.client else {
            return;
        };
        dialog.browsing = true;
        let name = if client.opens_application(Platform::current()) {
            "application"
        } else {
            "executable"
        };
        let selection = self
            .repository_dialog
            .clone()
            .set_title(format!("Choose {} {name}", client.label()))
            .pick_file();
        self.dispatch(async move {
            Message::GitClientExecutable(selection.await.map(|f| f.path().to_owned()))
        });
    }

    /// One filesystem-only check per draft change; stale results are ignored.
    fn check_git_client(&mut self) {
        let Some(dialog) = self.git_client_dialog.as_mut() else {
            return;
        };
        if dialog.draft.client.is_none() || dialog.checked.as_ref() == Some(&dialog.draft) {
            return;
        }
        let preferences = dialog.draft.clone();
        dialog.checked = Some(preferences.clone());
        dialog.status = None;
        self.git_client_generation += 1;
        let generation = self.git_client_generation;
        self.dispatch(async move {
            let result = tokio::task::spawn_blocking(move || {
                crate::git_client::resolve(&preferences)
                    .map(|command| command.describe())
                    .map_err(|error| error.to_string())
            })
            .await
            .unwrap_or_else(|error| Err(error.to_string()));
            Message::GitClientChecked(generation, result)
        });
    }

    pub(super) fn git_client_checked(&mut self, generation: u64, result: Result<String, String>) {
        if generation == self.git_client_generation
            && let Some(dialog) = &mut self.git_client_dialog
        {
            dialog.status = Some(result);
        }
    }

    pub(super) fn git_client_executable_selected(&mut self, path: Option<PathBuf>) {
        let Some(dialog) = self.git_client_dialog.as_mut() else {
            return;
        };
        dialog.browsing = false;
        let Some(path) = path else {
            return;
        };
        if dialog.draft.client.is_none() {
            return;
        }
        match path.to_str() {
            Some(text) => dialog.draft.executable = text.to_owned(),
            None => dialog.error = "Choose an executable with a Unicode path.".into(),
        }
    }

    /// Explicit configuration saves are synchronous, like History Retention.
    pub(super) fn save_git_client(&mut self) {
        let Some(dialog) = self.git_client_dialog.as_mut() else {
            return;
        };
        let mut staged = self.state.persistence_snapshot();
        staged.git_client = dialog.draft.clone();
        match self.store.save(&staged) {
            Ok(()) => {
                self.state.git_client = staged.git_client;
                self.git_client_dialog = None;
                self.notice = match self.state.git_client.client {
                    Some(client) => format!("Git client set to {}.", client.label()),
                    None => "Git client cleared.".into(),
                };
            }
            Err(error) => dialog.error = format!("Settings were not saved: {error:#}"),
        }
    }
}

/// Returns whether Browse was clicked.
fn git_client_fields(ui: &mut egui::Ui, dialog: &mut GitClientDialog, client: Client) -> bool {
    let custom = client == Client::Custom;
    let application = client.opens_application(Platform::current());
    let name = if application {
        "Application"
    } else {
        "Executable"
    };
    let hint = match (custom, application) {
        (true, _) => "Absolute path or command name",
        (false, true) => "Detect in Applications",
        (false, false) => "Detect automatically",
    };
    let label = ui.label(name);
    let browse = ui
        .add_enabled_ui(!dialog.browsing, |ui| {
            ui.horizontal(|ui| {
                let width = ui.available_width() - 76.0;
                ui.add(
                    egui::TextEdit::singleline(&mut dialog.draft.executable)
                        .hint_text(hint)
                        .desired_width(width),
                )
                .labelled_by(label.id);
                ui.add(theme::quiet("Browse…").min_size(egui::vec2(68.0, 0.0)))
                    .clicked()
            })
            .inner
        })
        .inner;
    if custom {
        let label = ui.label("Arguments, one per line");
        if ui
            .add(
                egui::TextEdit::multiline(&mut dialog.arguments)
                    .desired_rows(3)
                    .desired_width(f32::INFINITY)
                    // Not code_editor(): it captures Tab and traps keyboard focus.
                    .font(egui::TextStyle::Monospace),
            )
            .labelled_by(label.id)
            .changed()
        {
            dialog.draft.arguments = parse_arguments(&dialog.arguments);
        }
        ui.small("{repo} is replaced by the repository or worktree path.");
    }
    browse
}

fn git_client_status(ui: &mut egui::Ui, dialog: &GitClientDialog) {
    if dialog.draft.client.is_none() {
        ui.small("Choose a client to enable Open in Git Client.");
        return;
    }
    match &dialog.status {
        None => {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.weak("Checking…");
            });
        }
        Some(Ok(command)) => {
            theme::success_label(ui, "Ready");
            ui.small(command);
        }
        Some(Err(error)) => {
            ui.colored_label(theme::Palette::of(ui).warning, "Git client unavailable");
            ui.small(error);
        }
    }
}
