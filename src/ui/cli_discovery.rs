use super::*;
use crate::domain::{AgentId, AgentOptions};

pub(super) struct CliSearch {
    pub request: u64,
    pub agent: AgentId,
    pub options: AgentOptions,
    pub result: Option<Result<Vec<PathBuf>, String>>,
    pub selected: Option<usize>,
}

impl App {
    pub(super) fn find_cli(&mut self, ctx: egui::Context) {
        if self.cli_search.is_some() {
            return;
        }
        let agent = self.state.draft.agent;
        let Ok(backend) = agents::backend(agent) else {
            return;
        };
        let Some(spec) = backend
            .options()
            .iter()
            .find(|spec| spec.key == "executable")
        else {
            return;
        };
        let configured = agents::value(&self.state.draft.options, spec).to_owned();
        let request = self.next_cli_search;
        self.next_cli_search += 1;
        self.cli_search = Some(CliSearch {
            request,
            agent,
            options: self.state.draft.options.clone(),
            result: None,
            selected: None,
        });
        self.dispatch(ctx, async move {
            let result =
                tokio::task::spawn_blocking(move || agents::discovery::find(agent, &configured))
                    .await
                    .map_err(|error| {
                        format!("Could not search for installed executables: {error}")
                    });
            Message::FoundCli(request, result)
        });
    }

    pub(super) fn cli_search_completed(
        &mut self,
        request: u64,
        result: Result<Vec<PathBuf>, String>,
    ) {
        let Some(search) = self
            .cli_search
            .as_mut()
            .filter(|search| search.request == request)
        else {
            return;
        };
        if search.agent != self.state.draft.agent || search.options != self.state.draft.options {
            self.cli_search = None;
            return;
        }
        search.selected = match &result {
            Ok(paths) if paths.len() == 1 => Some(0),
            _ => None,
        };
        search.result = Some(result);
    }

    pub(super) fn use_discovered_cli(&mut self, ctx: egui::Context) {
        let Some(search) = self.cli_search.take() else {
            return;
        };
        if search.agent != self.state.draft.agent || search.options != self.state.draft.options {
            return;
        }
        let Some(Ok(paths)) = search.result else {
            return;
        };
        let Some(path) = search
            .selected
            .and_then(|index| paths.get(index))
            .and_then(|path| path.to_str())
        else {
            return;
        };
        self.state
            .draft
            .options
            .insert("executable".into(), path.to_owned());
        self.dirty = true;
        self.check_cli(ctx);
    }

    pub(super) fn cli_search_window(&mut self, ctx: &egui::Context) {
        // Never offer stale results after edits, agent changes or task reuse.
        if self.cli_search.as_ref().is_some_and(|search| {
            search.agent != self.state.draft.agent || search.options != self.state.draft.options
        }) {
            self.cli_search = None;
        }
        let Some(search) = self.cli_search.as_mut() else {
            return;
        };
        let mut use_selected = false;
        let mut cancel = false;
        let response = egui::Modal::new(egui::Id::new("find_cli")).show(ctx, |ui| {
            ui.set_width(560.0_f32.min(ctx.content_rect().width() - 64.0));
            ui.heading(format!("Find {}", search.agent.label()));
            match &search.result {
                None => {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label("Searching installed locations…");
                    });
                }
                Some(Err(error)) => { ui.label(error); }
                Some(Ok(paths)) if paths.is_empty() => {
                    ui.label("No executable found in the search path or common installation folders.");
                    ui.label("Enter an absolute executable path manually, or install this CLI separately.");
                    #[cfg(windows)]
                    ui.small("Find CLI looks for native .exe files, not .cmd, .bat or PowerShell wrappers.");
                }
                Some(Ok(paths)) => {
                    ui.label("Choose an executable to use and check. Your current setting stays unchanged until you apply it.");
                    egui::ScrollArea::vertical().max_height(240.0).show(ui, |ui| {
                        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
                        for (index, path) in paths.iter().enumerate() {
                            ui.radio_value(&mut search.selected, Some(index), path.display().to_string());
                        }
                    });
                    ui.small("Checks CLI compatibility only. No agent task is started.");
                }
            }
            ui.add_space(theme::GAP);
            ui.horizontal(|ui| {
                use_selected = ui.add_enabled(search.selected.is_some(), egui::Button::new("Use and check")).clicked();
                cancel = ui.button("Cancel").clicked();
            });
        });
        if use_selected {
            self.use_discovered_cli(ctx.clone());
        } else if cancel || response.should_close() {
            self.cli_search = None;
        }
    }
}
