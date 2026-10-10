//! Folder selection and validation stay outside egui rendering work.
use super::*;

#[derive(Default)]
pub(super) struct LocationEditor {
    pub input: String,
    pub pending: bool,
    pub detail: String,
}

impl App {
    pub(super) fn worktree_settings(&mut self, ui: &mut egui::Ui) {
        ui.set_max_width(420.0);
        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
        ui.strong("Worktree location");
        ui.small(match &self.state.worktree_base {
            Some(path) => format!("Current: {}", path.display()),
            None => format!(
                "Default: {}",
                self.store.directory().join("worktrees").display()
            ),
        });
        ui.add_enabled_ui(!self.worktree_location.pending && !self.closing, |ui| {
            ui.add(
                egui::TextEdit::singleline(&mut self.worktree_location.input)
                    .hint_text("Absolute base directory")
                    .desired_width(320.0),
            );
            ui.horizontal(|ui| {
                if ui.button("Browse…").clicked() {
                    self.worktree_location.pending = true;
                    let mut dialog = self
                        .repository_dialog
                        .clone()
                        .set_title("Choose worktree base directory")
                        .set_can_create_directories(true);
                    if !self.worktree_location.input.is_empty() {
                        dialog = dialog.set_directory(&self.worktree_location.input);
                    }
                    let selection = dialog.pick_folder();
                    self.dispatch(ui.ctx().clone(), async move {
                        Message::WorktreeFolder(selection.await.map(|f| f.path().to_owned()))
                    });
                }
                if ui
                    .add_enabled(
                        !self.worktree_location.input.is_empty(),
                        egui::Button::new("Apply location"),
                    )
                    .clicked()
                {
                    self.validate_worktree_location(
                        ui.ctx().clone(),
                        PathBuf::from(&self.worktree_location.input),
                    );
                }
                if ui
                    .add_enabled(
                        self.state.worktree_base.is_some(),
                        egui::Button::new("Use default"),
                    )
                    .clicked()
                {
                    self.state.worktree_base = None;
                    self.worktree_location.input.clear();
                    self.worktree_location.detail =
                        "Default restored for new convoys. Existing worktrees stay in place."
                            .into();
                    self.dirty = true;
                }
            });
        });
        ui.small("New convoys only. Existing worktrees stay in their original locations.");
        if self.worktree_location.pending {
            ui.small("Choosing or checking location…");
        }
        if !self.worktree_location.detail.is_empty() {
            ui.small(&self.worktree_location.detail);
        }
    }

    pub(super) fn worktree_folder_selected(&mut self, path: Option<PathBuf>) {
        self.worktree_location.pending = false;
        if let Some(path) = path {
            if let Some(text) = path.to_str() {
                self.worktree_location.input = text.to_owned();
                self.worktree_location.detail = "Use Apply location to save this directory.".into();
            } else {
                self.worktree_location.detail = "Choose a directory with a Unicode path.".into();
            }
        }
    }

    fn validate_worktree_location(&mut self, ctx: egui::Context, path: PathBuf) {
        self.worktree_location.pending = true;
        let storage = self.store.directory().join("worktrees");
        self.dispatch(ctx, async move {
            let result = tokio::task::spawn_blocking(move || {
                crate::worktrees::location::resolve_base(&storage, Some(&path))
            })
            .await;
            Message::WorktreeLocation(match result {
                Ok(result) => result.map_err(|e| format!("{e:#}")),
                Err(e) => Err(e.to_string()),
            })
        });
    }

    pub(super) fn worktree_location_checked(&mut self, result: Result<PathBuf, String>) {
        self.worktree_location.pending = false;
        match result {
            Ok(path) if !self.closing => {
                self.worktree_location.input = path.display().to_string();
                self.state.worktree_base = Some(path);
                self.worktree_location.detail = "Saved for new convoys. Missing directories are created when needed; access is checked again before creation.".into();
                self.dirty = true;
            }
            Ok(_) => {}
            Err(error) => self.worktree_location.detail = format!("Location not changed. {error}"),
        }
    }
}
