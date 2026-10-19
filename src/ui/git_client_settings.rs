//! Global Git client preference. Lookups run on a blocking worker, never while
//! rendering, and only after the preference changes.
use super::*;
use crate::git_client::{Client, Platform, Preferences};

#[derive(Default)]
pub(super) struct GitClientEditor {
    /// Custom arguments, one per line, mirrored into the saved preference.
    pub arguments: String,
    pub browsing: bool,
    /// Preference snapshot of the latest dispatched check.
    pub checked: Option<Preferences>,
    pub generation: u64,
    pub status: Option<Result<String, String>>,
}
impl GitClientEditor {
    pub fn new(preferences: &Preferences) -> Self {
        Self {
            arguments: preferences.arguments.join("\n"),
            ..Default::default()
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
    pub(super) fn git_client_settings(&mut self, ui: &mut egui::Ui) {
        ui.set_max_width(420.0);
        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
        let label = ui.strong("Git client");
        let current = self.state.git_client.client;
        let mut chosen = current;
        ui.add_enabled_ui(!self.closing, |ui| {
            egui::ComboBox::from_id_salt("git_client")
                .width(320.0)
                .selected_text(current.map_or("Not configured", Client::label))
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut chosen, None, "Not configured");
                    for client in Client::ALL {
                        match client.availability(Platform::current()) {
                            Ok(()) => {
                                ui.selectable_value(&mut chosen, Some(client), client.label());
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
                })
                .response
                .labelled_by(label.id)
                .on_hover_text("Client used by Open in Git Client");
        });
        if chosen != current {
            self.select_git_client(chosen);
        }
        if let Some(client) = self.state.git_client.client {
            self.git_client_fields(ui, client);
        }
        self.git_client_status(ui);
        ui.small(
            "Opens the exact repository or worktree path as a separate process argument, without a shell. Opening runs no Git command and changes nothing.",
        );
    }

    fn git_client_fields(&mut self, ui: &mut egui::Ui, client: Client) {
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
        ui.add_enabled_ui(!self.git_client.browsing && !self.closing, |ui| {
            ui.horizontal(|ui| {
                let width = 320.0 - 68.0 - ui.spacing().item_spacing.x;
                self.dirty |= ui
                    .add(
                        egui::TextEdit::singleline(&mut self.state.git_client.executable)
                            .hint_text(hint)
                            .desired_width(width),
                    )
                    .labelled_by(label.id)
                    .changed();
                if ui
                    .add(theme::quiet("Browse…").min_size(egui::vec2(68.0, 0.0)))
                    .clicked()
                {
                    self.git_client.browsing = true;
                    let selection = self
                        .repository_dialog
                        .clone()
                        .set_title(format!("Choose {} {}", client.label(), name.to_lowercase()))
                        .pick_file();
                    self.dispatch(async move {
                        Message::GitClientExecutable(selection.await.map(|f| f.path().to_owned()))
                    });
                }
            });
        });
        if custom {
            let label = ui.label("Arguments, one per line");
            let response = ui.add_enabled(
                !self.closing,
                egui::TextEdit::multiline(&mut self.git_client.arguments)
                    .desired_rows(3)
                    .desired_width(320.0)
                    .code_editor(),
            );
            if response.labelled_by(label.id).changed() {
                self.state.git_client.arguments = parse_arguments(&self.git_client.arguments);
                self.dirty = true;
            }
            ui.small("{repo} is replaced by the repository or worktree path.");
        }
    }

    fn git_client_status(&mut self, ui: &mut egui::Ui) {
        if self.state.git_client.client.is_none() {
            ui.small("Choose a client to enable Open in Git Client.");
            return;
        }
        if self.git_client.checked.as_ref() != Some(&self.state.git_client) && !self.closing {
            self.check_git_client();
        }
        match &self.git_client.status {
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

    pub(super) fn select_git_client(&mut self, client: Option<Client>) {
        if self.closing || client == self.state.git_client.client {
            return;
        }
        self.state.git_client = Preferences::select(client);
        self.git_client.arguments = self.state.git_client.arguments.join("\n");
        self.dirty = true;
    }

    fn check_git_client(&mut self) {
        let preferences = self.state.git_client.clone();
        self.git_client.checked = Some(preferences.clone());
        self.git_client.generation += 1;
        self.git_client.status = None;
        let generation = self.git_client.generation;
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
        if generation == self.git_client.generation {
            self.git_client.status = Some(result);
        }
    }

    pub(super) fn git_client_executable_selected(&mut self, path: Option<PathBuf>) {
        self.git_client.browsing = false;
        let Some(path) = path else {
            return;
        };
        if self.closing || self.state.git_client.client.is_none() {
            return;
        }
        match path.to_str() {
            Some(text) => {
                self.state.git_client.executable = text.to_owned();
                self.dirty = true;
            }
            None => {
                self.git_client.status =
                    Some(Err("Choose an executable with a Unicode path.".into()));
            }
        }
    }
}
