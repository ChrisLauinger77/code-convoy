mod about;
#[cfg(target_os = "macos")]
mod about_macos;
mod agent_config;
mod cli_discovery;
mod diagnostics;
mod editor;
mod format;
mod quit;
#[cfg(target_os = "macos")]
mod quit_macos;
mod repositories;
#[cfg(target_os = "macos")]
pub use quit_macos::init_native_application;
mod results;
mod snapshot;
#[cfg(test)]
mod tests;
mod text_view;
mod theme;

use crate::{
    agents,
    domain::{self, AppState, JobStatus, MAX_REPOSITORIES, Repository, Run},
    git::{self, WorkingTree},
    persistence::Store,
    runner::{self, Event, PreparedRun, RunManager},
};
use eframe::egui;
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::mpsc,
    time::{Duration, Instant},
};
use tokio::{runtime::Runtime, sync::mpsc as async_mpsc};

enum Message {
    FoundCli(u64, Result<Vec<PathBuf>, String>),
    RepositoryFolder(Option<PathBuf>),
    Registered(Result<(Repository, WorkingTree), String>),
    Refreshed(Vec<(PathBuf, Result<WorkingTree, String>)>),
    Prepared(Result<PreparedRun, String>),
    Detected(
        domain::AgentId,
        domain::AgentOptions,
        Result<String, diagnostics::CliError>,
    ),
    Diff(PathBuf, Result<String, String>),
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Output,
    Diff,
    Task,
}

pub struct App {
    store: Store,
    state: AppState,
    runtime: Runtime,
    selected: HashSet<PathBuf>,
    draft_message: String,
    focus_draft: bool,
    repository_input: String,
    picked_repository_path: Option<PathBuf>,
    repository_dialog: rfd::AsyncFileDialog,
    browsing_repository: bool,
    focus_repository_input: bool,
    repository_states: HashMap<PathBuf, Result<WorkingTree, String>>,
    busy: bool,
    checking_cli: Option<(domain::AgentId, domain::AgentOptions)>,
    cli_search: Option<cli_discovery::CliSearch>,
    next_cli_search: u64,
    about_open: bool,
    #[cfg(target_os = "macos")]
    native_about: Option<about_macos::NativeAbout>,
    execution_height: f32,
    session_runs: HashSet<u64>,
    notice: String,
    detection: Option<(
        domain::AgentId,
        domain::AgentOptions,
        Result<String, diagnostics::CliError>,
    )>,
    output_view: text_view::TextView,
    diff_view: text_view::TextView,
    tx: mpsc::Sender<Message>,
    rx: mpsc::Receiver<Message>,
    events_rx: async_mpsc::Receiver<Event>,
    manager: RunManager,
    prepared: Option<PreparedRun>,
    dirty_ack: bool,
    selected_run: Option<u64>,
    selected_job: usize,
    tab: Tab,
    diff_target: Option<PathBuf>,
    diff: Option<Result<String, String>>,
    dirty: bool,
    last_save: Instant,
    closing: bool,
    quit_requested: bool,
    focus_quit_cancel: bool,
    exit_ready: bool,
}
impl App {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        store: Store,
        state: AppState,
        runtime: Runtime,
    ) -> Self {
        let mut app = Self::with_context(&cc.egui_ctx, store, state, runtime);
        // eframe owns this root window for the lifetime of the app.
        app.repository_dialog = app.repository_dialog.set_parent(cc);
        #[cfg(target_os = "macos")]
        {
            app.native_about = about_macos::NativeAbout::install();
            quit_macos::connect(&cc.egui_ctx);
        }
        app
    }
    fn with_context(ctx: &egui::Context, store: Store, state: AppState, runtime: Runtime) -> Self {
        theme::install(ctx);
        let (tx, rx) = mpsc::channel();
        let (events_tx, events_rx) = async_mpsc::channel(256);
        let selected_run = state.runs.first().map(|r| r.id);
        let manager = {
            let _entered = runtime.enter();
            RunManager::new(state.global_concurrency, events_tx)
        };
        let mut app = Self {
            store,
            state,
            runtime,
            selected: HashSet::new(),
            draft_message: String::new(),
            focus_draft: false,
            repository_input: String::new(),
            picked_repository_path: None,
            repository_dialog: rfd::AsyncFileDialog::new()
                .set_title("Choose a Git repository root")
                .set_can_create_directories(false),
            browsing_repository: false,
            focus_repository_input: false,
            repository_states: HashMap::new(),
            busy: false,
            checking_cli: None,
            cli_search: None,
            next_cli_search: 0,
            about_open: false,
            #[cfg(target_os = "macos")]
            native_about: None,
            execution_height: 184.0,
            session_runs: HashSet::new(),
            notice: String::new(),
            detection: None,
            output_view: text_view::TextView::default(),
            diff_view: text_view::TextView::default(),
            tx,
            rx,
            events_rx,
            manager,
            prepared: None,
            dirty_ack: false,
            selected_run,
            selected_job: 0,
            tab: Tab::Output,
            diff_target: None,
            diff: None,
            dirty: true,
            last_save: Instant::now(),
            closing: false,
            quit_requested: false,
            focus_quit_cancel: false,
            exit_ready: false,
        };
        if !app.state.repositories.is_empty() {
            app.refresh(ctx.clone());
        }
        app
    }
    fn current_detection(
        &self,
    ) -> Option<&(
        domain::AgentId,
        domain::AgentOptions,
        Result<String, diagnostics::CliError>,
    )> {
        self.detection.as_ref().filter(|(agent, options, _)| {
            *agent == self.state.draft.agent && *options == self.state.draft.options
        })
    }

    fn current_cli_check(&self) -> bool {
        self.checking_cli.as_ref().is_some_and(|(agent, options)| {
            *agent == self.state.draft.agent && *options == self.state.draft.options
        })
    }

    fn header(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        egui::Panel::top("header")
            .frame(
                egui::Frame::side_top_panel(ui.style())
                    .inner_margin(egui::Margin::symmetric(theme::PANEL_MARGIN, 10)),
            )
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.heading("CodeConvoy");
                    if ui.available_width() > 850.0 {
                        ui.weak("One task. Multiple repositories. Your agent.");
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.menu_button("Appearance", |ui| {
                            let mut preference = ctx.options(|o| o.theme_preference);
                            for (value, name) in [
                                (egui::ThemePreference::System, "System"),
                                (egui::ThemePreference::Dark, "Dark"),
                                (egui::ThemePreference::Light, "Light"),
                            ] {
                                if ui.selectable_value(&mut preference, value, name).changed() {
                                    ctx.set_theme(preference);
                                }
                            }
                        });
                        if !self.manager.is_idle() {
                            ui.menu_button("All convoys", |ui| {
                                ui.label(format!("{} active convoys", self.manager.active_count()));
                                if ui.button("Stop All Convoys").on_hover_text("Emergency stop: cancels every running and queued job in all convoys.").clicked() {
                                    self.manager.cancel_all();
                                    ui.close();
                                }
                            });
                        }
                        let p = theme::Palette::of(ui);
                        if self.closing {
                            ui.colored_label(p.warning, "Stopping processes…");
                        } else {
                            let (suffix, color) = match self.current_detection().map(|(_, _, r)| r)
                            {
                                Some(Ok(_)) => ("Available", p.success),
                                Some(Err(error)) => (error.label(), p.warning),
                                None if self.current_cli_check() => ("Checking…", p.muted),
                                None => ("Unchecked", p.muted),
                            };
                            ui.label(
                                egui::RichText::new(format!(
                                    "{} · {suffix}",
                                    format::agent_name(self.state.draft.agent)
                                ))
                                .small()
                                .color(color),
                            );
                        }
                    });
                });
            });
    }

    fn footer(&mut self, ui: &mut egui::Ui) {
        egui::Panel::bottom("footer")
            .frame(egui::Frame::side_top_panel(ui.style()).inner_margin(egui::Margin::symmetric(theme::PANEL_MARGIN, 5)))
            .show(ui, |ui| {
                if !self.notice.is_empty() {
                    ui.horizontal_wrapped(|ui| {
                        ui.colored_label(theme::Palette::of(ui).error, diagnostics::summary(&self.notice));
                        if ui.add(theme::quiet("Dismiss").small()).clicked() { self.notice.clear(); }
                    });
                    if !self.notice.is_empty() { diagnostics::details(ui, "notice_diagnostics", &self.notice); }
                }
                ui.horizontal(|ui| {
                    ui.small("Tab / Shift+Tab to navigate · Enter / Space to activate");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.add(theme::quiet("About").small()).clicked() { self.about_open = true; }
                        ui.small("Local session").on_hover_text(format!("Local state: {}\nPrompts and run metadata are saved locally. Output stays in memory. Do not put credentials in the task.", self.store.directory().display()));
                    });
                });
            });
    }

    fn dispatch(&self, ctx: egui::Context, future: impl Future<Output = Message> + Send + 'static) {
        let tx = self.tx.clone();
        self.runtime.spawn(async move {
            let _ = tx.send(future.await);
            ctx.request_repaint();
        });
    }
    fn refresh(&mut self, ctx: egui::Context) {
        self.busy = true;
        let repositories = self.state.repositories.clone();
        self.dispatch(ctx, async move {
            let mut states = Vec::new();
            for repository in repositories {
                let result = git::status(&repository.path)
                    .await
                    .map_err(|e| format!("{e:#}"));
                states.push((repository.path, result));
            }
            Message::Refreshed(states)
        });
    }
    fn register(&mut self, ctx: egui::Context) {
        if self.state.repositories.len() >= MAX_REPOSITORIES {
            self.notice = format!("At most {MAX_REPOSITORIES} repositories can be registered.");
            return;
        }
        // Preserve a picked directory exactly, including trailing whitespace.
        // An edited/manual field retains the existing whitespace-trimming behavior.
        let path = self
            .picked_repository_path
            .as_ref()
            .filter(|path| path.to_str() == Some(self.repository_input.as_str()))
            .cloned()
            .unwrap_or_else(|| PathBuf::from(self.repository_input.trim()));
        self.busy = true;
        self.dispatch(ctx, async move {
            let result = async {
                let repository = git::register(&path).await?;
                let state = git::status(&repository.path).await?;
                Ok::<_, anyhow::Error>((repository, state))
            }
            .await
            .map_err(|e| format!("{e:#}"));
            Message::Registered(result)
        });
    }
    fn preflight(&mut self, ctx: egui::Context) {
        if self.closing || self.quit_requested {
            return;
        }
        let task = self.state.draft.clone();
        let repositories = self
            .state
            .repositories
            .iter()
            .filter(|r| self.selected.contains(&r.path))
            .cloned()
            .collect();
        self.busy = true;
        self.notice.clear();
        self.dispatch(ctx, async move {
            Message::Prepared(
                runner::prepare(task, repositories)
                    .await
                    .map_err(|e| format!("{e:#}")),
            )
        });
    }
    fn start(&mut self) {
        if self.closing || self.quit_requested {
            return;
        }
        let Some(prepared) = self.prepared.take() else {
            return;
        };
        let backend = match agents::backend(prepared.task.agent) {
            Ok(backend) => backend,
            Err(e) => {
                self.notice = e.to_string();
                return;
            }
        };
        let id = self.state.next_run;
        self.state.next_run = id.saturating_add(1);
        self.state.runs.insert(0, prepared.snapshot(id));
        // Persist intent before an agent can touch a repository.
        if let Err(error) = self.store.save(&self.state) {
            self.state.runs.remove(0);
            self.notice = format!("Cannot save run; nothing was started: {error:#}");
            return;
        }
        if let Err(error) = self.manager.start(id, prepared, backend) {
            for job in &mut self.state.runs[0].jobs {
                job.finish(
                    JobStatus::Failed,
                    None,
                    format!("Could not queue convoy: {error:#}"),
                );
            }
        } else {
            self.state.draft.prompt.clear();
            self.selected.clear();
            self.draft_message = format!("Convoy #{id} launched. Ready for your next task.");
            self.focus_draft = true;
        }
        self.session_runs.insert(id);
        self.state.trim_history();
        self.dirty = true;
        self.select_run(Some(id));
        self.tab = Tab::Output;
        self.dirty_ack = false;
    }
    fn poll(&mut self) {
        while let Ok(message) = self.rx.try_recv() {
            match message {
                Message::RepositoryFolder(path) => self.repository_folder_selected(path),
                Message::Registered(result) => {
                    self.busy = false;
                    match result {
                        Ok((repository, state)) => {
                            if self
                                .state
                                .repositories
                                .iter()
                                .any(|r| r.path == repository.path)
                            {
                                self.notice = "This repository is already registered.".into();
                            } else {
                                self.selected.insert(repository.path.clone());
                                self.repository_states
                                    .insert(repository.path.clone(), Ok(state));
                                self.state.repositories.push(repository);
                                self.repository_input.clear();
                                self.picked_repository_path = None;
                                self.notice.clear();
                                self.dirty = true;
                            }
                        }
                        Err(e) => self.notice = e,
                    }
                }
                Message::Refreshed(states) => {
                    self.busy = false;
                    self.repository_states.extend(states);
                }
                Message::Prepared(result) => {
                    self.busy = false;
                    match result {
                        Ok(prepared) => {
                            self.prepared = Some(prepared);
                            self.dirty_ack = false;
                        }
                        Err(e) => self.notice = e,
                    }
                }
                Message::FoundCli(request, result) => self.cli_search_completed(request, result),
                Message::Detected(agent, options, result) => {
                    self.checking_cli = None;
                    self.detection = Some((agent, options, result));
                }
                Message::Diff(path, result) => {
                    if self.diff_target.as_ref() == Some(&path) {
                        self.diff = Some(result);
                    }
                }
            }
        }
        for _ in 0..256 {
            let Ok(event) = self.events_rx.try_recv() else {
                break;
            };
            self.apply_event(event);
        }
        self.state.trim_logs();
        self.manager.reap();
        self.state.trim_history();
        self.session_runs
            .retain(|id| self.state.runs.iter().any(|run| run.id == *id));
        self.reconcile_run_selection();
    }
    fn select_run(&mut self, id: Option<u64>) {
        if self.selected_run != id {
            self.selected_run = id;
            self.selected_job = 0;
            self.diff = None;
            self.diff_target = None;
            self.output_view = text_view::TextView::default();
            self.diff_view = text_view::TextView::default();
        }
    }
    fn reconcile_run_selection(&mut self) {
        if !self
            .state
            .runs
            .iter()
            .any(|r| Some(r.id) == self.selected_run)
        {
            let next = self
                .state
                .runs
                .iter()
                .find(|r| r.active())
                .or_else(|| self.state.runs.first())
                .map(|r| r.id);
            self.select_run(next);
        }
    }
    fn remove_history(&mut self, id: Option<u64>) {
        let changed = match id {
            Some(id) => self.state.remove_from_history(id),
            None => self.state.clear_history() > 0,
        };
        if changed {
            self.reconcile_run_selection();
            self.dirty = true;
            self.save();
        }
    }
    fn reuse_convoy(&mut self, id: u64) {
        if self.busy || self.prepared.is_some() || self.closing {
            return;
        }
        let Some(run) = self.state.runs.iter().find(|r| r.id == id) else {
            return;
        };
        self.selected = run
            .jobs
            .iter()
            .filter(|job| {
                self.state
                    .repositories
                    .iter()
                    .any(|r| r.path == job.repository.path)
            })
            .map(|job| job.repository.path.clone())
            .collect();
        let missing = run.jobs.len().saturating_sub(self.selected.len());
        let task = run.task.clone();
        self.state.reuse_task(task);
        self.draft_message =
            format!("Copied convoy #{id} into the draft. Review it before launching.");
        if missing > 0 {
            let noun = if missing == 1 {
                "repository"
            } else {
                "repositories"
            };
            self.draft_message.push_str(&format!(
                " Not selected: {missing} unregistered {noun}. Re-register to include them."
            ));
        }
        self.focus_draft = true;
        self.dirty = true;
    }
    fn apply_event(&mut self, event: Event) {
        let (id, job) = match &event {
            Event::Queued { run, job, .. }
            | Event::Started { run, job, .. }
            | Event::Output { run, job, .. }
            | Event::Finished { run, job, .. } => (*run, *job),
        };
        let Some(job) = self
            .state
            .runs
            .iter_mut()
            .find(|r| r.id == id)
            .and_then(|r| r.jobs.get_mut(job))
        else {
            return;
        };
        match event {
            Event::Queued { reason, .. } => {
                if job.status == JobStatus::Queued {
                    job.queue_reason = Some(reason);
                }
            }
            Event::Started { before, .. } => {
                job.status = JobStatus::Running;
                job.queue_reason = None;
                self.repository_states.remove(&job.repository.path);
                job.started_at = Some(domain::now());
                job.before = Some(before);
                self.dirty = true;
            }
            Event::Output { text, .. } => job.log.append(&text),
            Event::Finished {
                status,
                exit_code,
                detail,
                ..
            } => {
                job.finish(status, exit_code, detail);
                self.repository_states.remove(&job.repository.path);
                self.dirty = true;
            }
        }
    }
    fn save(&mut self) {
        match self.store.save(&self.state) {
            Ok(()) => self.dirty = false,
            Err(error) => self.notice = format!("Could not save state: {error:#}"),
        }
        self.last_save = Instant::now();
    }
}
impl eframe::App for App {
    fn logic(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        // eframe also calls logic for hidden/minimized windows. A native quit
        // must never bypass confirmation just because no UI pass is rendered.
        self.poll();
        self.handle_close(ctx);
        ctx.request_repaint_after(Duration::from_millis(
            if !self.manager.is_idle()
                || self.closing
                || self.state.runs.iter().any(Run::active)
                || self.busy
                || self.checking_cli.is_some()
                || self
                    .cli_search
                    .as_ref()
                    .is_some_and(|search| search.result.is_none())
            {
                100
            } else {
                1000
            },
        ));
    }
    fn ui(&mut self, ui: &mut egui::Ui, _: &mut eframe::Frame) {
        let ctx = &ui.ctx().clone();
        self.header(ui, ctx);
        self.footer(ui);
        egui::Panel::left("task_editor")
            .resizable(true)
            .default_size(360.0)
            .size_range(310.0..=(ui.available_width() * 0.55).max(310.0))
            .frame(egui::Frame::side_top_panel(ui.style()).inner_margin(theme::PANEL_MARGIN))
            .show(ui, |ui| {
                self.editor_pane(ui, ctx);
            });
        egui::CentralPanel::default()
            .frame(egui::Frame::central_panel(ui.style()).inner_margin(theme::PANEL_MARGIN))
            .show(ui, |ui| self.results(ui, ctx));
        if !self.quit_requested && !self.closing {
            self.preflight_window(ctx);
            self.about_window(ctx);
            self.cli_search_window(ctx);
        }
        self.quit_window(ctx);
        if self.dirty && self.last_save.elapsed() > Duration::from_secs(2) {
            self.save();
        }
    }
    fn on_exit(&mut self, _: Option<&eframe::glow::Context>) {
        self.manager.shutdown();
        // Continue draining lifecycle/output events while process trees stop.
        let final_events = self.runtime.block_on(async {
            let mut final_events = Vec::new();
            let deadline = tokio::time::sleep(Duration::from_secs(8));
            tokio::pin!(deadline);
            loop {
                tokio::select! {
                    _ = &mut deadline => break,
                    _ = tokio::time::sleep(Duration::from_millis(20)) => {
                        while let Ok(event) = self.events_rx.try_recv() {
                            if !matches!(event, Event::Output { .. }) { final_events.push(event); }
                        }
                        if self.manager.join.is_finished() { break; }
                    }
                }
            }
            final_events
        });
        for event in final_events {
            self.apply_event(event);
        }
        self.state.recover_interrupted();
        self.save();
    }
}
