mod about;
#[cfg(target_os = "macos")]
mod about_macos;
mod agent_config;
mod attachments_ui;
mod bulk_discard;
mod cli_discovery;
mod continuation;
mod diagnostics;
mod editor;
mod format;
mod library;
mod quit;
#[cfg(target_os = "macos")]
mod quit_macos;
mod repositories;
#[cfg(target_os = "macos")]
pub use quit_macos::init_native_application;
mod results;
mod review;
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
    Attachments(u64, Vec<crate::attachments::Attachment>, Vec<String>),
    ReusedAttachments(u64, Vec<crate::attachments::Attachment>, Vec<String>),
    Registered(Result<(Repository, WorkingTree), String>),
    Refreshed(Vec<(PathBuf, Result<WorkingTree, String>)>),
    Prepared(Result<PreparedRun, String>),
    RetryPrepared(Result<(PreparedRun, crate::continuation::Provenance), String>),
    FollowUp(Box<crate::continuation::FollowUp>),
    Diff(PathBuf, Result<String, String>),
    Reconciled(
        u64,
        usize,
        domain::WorktreeMetadata,
        crate::worktrees::recovery::Report,
        bool,
    ),
    ReviewStats(u64, usize, Result<crate::review::Statistics, String>),
    Resolved(
        Box<crate::persistence::results::Operation>,
        crate::persistence::results::Completion,
    ),
    Orphans(Result<Vec<PathBuf>, String>),
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Review,
    Activity,
    Raw,
    Diff,
    Task,
}

pub struct App {
    store: Store,
    state: AppState,
    // Taken only by Drop, which shuts down without waiting for filesystem workers.
    runtime: Option<Runtime>,
    selected: HashSet<PathBuf>,
    draft_message: String,
    focus_draft: bool,
    library_editor: Option<library::LibraryEditor>,
    attachment_work: attachments_ui::AttachmentWork,
    repository_input: String,
    picked_repository_path: Option<PathBuf>,
    repository_dialog: rfd::AsyncFileDialog,
    browsing_repository: bool,
    focus_repository_input: bool,
    repository_states: HashMap<PathBuf, Result<WorkingTree, String>>,
    busy: bool,
    cli_checks: agents::availability::CliChecks,
    cli_search: Option<cli_discovery::CliSearch>,
    next_cli_search: u64,
    about_open: bool,
    #[cfg(target_os = "macos")]
    native_about: Option<about_macos::NativeAbout>,
    execution_height: f32,
    session_runs: HashSet<u64>,
    notice: String,
    orphan_notice: String,
    output_view: text_view::TextView,
    diff_view: text_view::TextView,
    tx: mpsc::Sender<Message>,
    rx: mpsc::Receiver<Message>,
    events_rx: async_mpsc::Receiver<Event>,
    manager: RunManager,
    prepared: Option<PreparedRun>,
    prepared_provenance: Option<crate::continuation::Provenance>,
    followup_pending: bool,
    review_selected: HashSet<usize>,
    dirty_ack: bool,
    selected_run: Option<u64>,
    selected_job: usize,
    tab: Tab,
    review_pending: Option<(u64, usize)>,
    result_operation: Option<(u64, usize)>,
    discard_confirmation: Option<(u64, usize, crate::persistence::results::Action)>,
    focus_discard_cancel: bool,
    bulk_confirmation: Option<crate::persistence::bulk_discard::Plan>,
    bulk_discard: Option<crate::persistence::bulk_discard::Batch>,
    bulk_report: Option<(
        crate::persistence::bulk_discard::Scope,
        crate::persistence::bulk_discard::Summary,
    )>,
    focus_bulk_cancel: bool,
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
        app.repository_dialog = app.repository_dialog.clone().set_parent(cc);
        #[cfg(target_os = "macos")]
        {
            app.native_about = about_macos::NativeAbout::install();
            quit_macos::connect(&cc.egui_ctx);
        }
        app
    }
    fn with_context(ctx: &egui::Context, store: Store, state: AppState, runtime: Runtime) -> Self {
        theme::install(ctx);
        ctx.set_theme(theme::preference(state.appearance));
        let (tx, rx) = mpsc::channel();
        let (events_tx, events_rx) = async_mpsc::channel(256);
        let selected_run = state.runs.first().map(|r| r.id);
        let manager = {
            let _entered = runtime.enter();
            RunManager::with_worktree_directory(
                state.global_concurrency,
                events_tx,
                store.directory().join("worktrees"),
            )
        };
        let repaint = ctx.clone();
        let cli_checks = agents::availability::CliChecks::new(
            store.directory().to_owned(),
            runtime.handle().clone(),
            move || repaint.request_repaint(),
        );
        let mut app = Self {
            store,
            state,
            runtime: Some(runtime),
            selected: HashSet::new(),
            draft_message: String::new(),
            focus_draft: false,
            library_editor: None,
            attachment_work: attachments_ui::AttachmentWork::default(),
            repository_input: String::new(),
            picked_repository_path: None,
            repository_dialog: rfd::AsyncFileDialog::new()
                .set_title("Choose a Git repository root")
                .set_can_create_directories(false),
            browsing_repository: false,
            focus_repository_input: false,
            repository_states: HashMap::new(),
            busy: false,
            cli_checks,
            cli_search: None,
            next_cli_search: 0,
            about_open: false,
            #[cfg(target_os = "macos")]
            native_about: None,
            execution_height: 184.0,
            session_runs: HashSet::new(),
            notice: String::new(),
            orphan_notice: String::new(),
            output_view: text_view::TextView::default(),
            diff_view: text_view::TextView::default(),
            tx,
            rx,
            events_rx,
            manager,
            prepared: None,
            prepared_provenance: None,
            followup_pending: false,
            review_selected: HashSet::new(),
            dirty_ack: false,
            selected_run,
            selected_job: 0,
            tab: Tab::Review,
            review_pending: None,
            result_operation: None,
            discard_confirmation: None,
            focus_discard_cancel: false,
            bulk_confirmation: None,
            bulk_discard: None,
            bulk_report: None,
            focus_bulk_cancel: false,
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
        app.sync_cli_checks();
        app.reconcile_results(ctx.clone());
        app
    }
    fn reconcile_results(&self, ctx: egui::Context) {
        let referenced: HashSet<_> = self
            .state
            .runs
            .iter()
            .flat_map(|r| &r.jobs)
            .filter_map(|j| j.worktree.as_ref().map(|m| m.path.clone()))
            .collect();
        let root = self.store.directory().join("worktrees");
        self.dispatch(ctx, async move {
            let orphans = tokio::task::spawn_blocking(move || {
                crate::worktrees::recovery::orphans(&root, &referenced)
            })
            .await;
            Message::Orphans(match orphans {
                Ok(result) => result.map_err(|e| format!("{e:#}")),
                Err(e) => Err(e.to_string()),
            })
        });
    }
    fn runtime(&self) -> &Runtime {
        // All methods run before Drop takes ownership of the runtime.
        self.runtime
            .as_ref()
            .expect("Application runtime exists until Drop")
    }

    fn sync_cli_checks(&mut self) {
        if self.closing {
            return;
        }
        for agent in domain::AgentId::ALL {
            let options = if agent == self.state.draft.agent {
                Some(&self.state.draft.options)
            } else {
                self.state.agent_options.get(&agent)
            };
            self.cli_checks.ensure(
                agent,
                options
                    .and_then(|options| options.get("executable"))
                    .cloned(),
            );
        }
    }

    fn current_detection(
        &self,
    ) -> Option<&Result<agents::availability::CliInfo, diagnostics::CliError>> {
        self.cli_checks.result(
            self.state.draft.agent,
            self.state
                .draft
                .options
                .get("executable")
                .map(String::as_str),
        )
    }
    fn current_cli_check(&self) -> bool {
        self.cli_checks.checking(self.state.draft.agent)
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
                                    self.state.appearance = theme::appearance(preference);
                                    self.dirty = true;
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
                            let (suffix, color) = match self.current_detection()
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
                if !self.orphan_notice.is_empty() {
                    ui.colored_label(theme::Palette::of(ui).warning, diagnostics::summary(&self.orphan_notice));
                    diagnostics::details(ui, "orphan_diagnostics", &self.orphan_notice);
                }
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
        self.runtime().spawn(async move {
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
        if self.closing || self.quit_requested || self.busy || self.prepared.is_some() {
            return;
        }
        if self.attachment_work.pending {
            return;
        }
        if let Some(error) = self.attachment_error() {
            self.notice = error;
            return;
        }
        // Discovery must be applied before the immutable preflight snapshot.
        // Also reconcile executable edits made since the last UI poll.
        self.sync_cli_checks();
        if self.current_cli_check() {
            return;
        }
        let task = self.state.draft.clone();
        self.prepared_provenance = self.state.draft_provenance.clone();
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
        let provenance = self.prepared_provenance.take();
        let retry = provenance
            .as_ref()
            .is_some_and(crate::continuation::Provenance::is_retry);
        if prepared.repositories.iter().any(|p| {
            !self
                .state
                .repositories
                .iter()
                .any(|r| r.path == p.repository.path)
        }) {
            self.notice =
                "Launch blocked: a selected repository is no longer registered. Review it again."
                    .into();
            return;
        }
        let backend = match agents::backend(prepared.task.agent) {
            Ok(backend) => backend,
            Err(e) => {
                self.notice = e.to_string();
                return;
            }
        };
        let id = self.state.next_run;
        self.state.next_run = id.saturating_add(1);
        let mut run = prepared.snapshot(id);
        if let Some(origin) = &provenance {
            for job in &mut run.jobs {
                job.log
                    .append(&format!("[CodeConvoy] {}\n", origin.label()));
            }
        }
        run.provenance = provenance;
        self.state.runs.insert(0, run);
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
        } else if !retry {
            self.state.draft.prompt.clear();
            self.state.draft_provenance = None;
            self.state.draft.attachments.clear();
            self.attachment_work.invalidate();
            self.selected.clear();
            self.draft_message = format!("Convoy #{id} launched. Ready for your next task.");
            self.focus_draft = true;
        }
        self.session_runs.insert(id);
        self.state.trim_history();
        self.dirty = true;
        self.select_run(Some(id));
        self.tab = Tab::Activity;
        self.dirty_ack = false;
    }
    fn poll(&mut self) {
        // Reconcile edits before receiving results, including task reuse and
        // edits made while a previous generation's completion was queued.
        self.sync_cli_checks();
        for resolved in self.cli_checks.poll() {
            self.apply_resolved_executable(resolved);
        }
        let mut recovered = false;
        while let Ok(message) = self.rx.try_recv() {
            match message {
                Message::RepositoryFolder(path) => self.repository_folder_selected(path),
                Message::Attachments(request, files, errors) => {
                    self.attachments_added(request, files, errors)
                }
                Message::ReusedAttachments(request, files, errors) => {
                    self.attachments_reused(request, files, errors)
                }
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
                        Err(e) => {
                            self.notice = e;
                            self.prepared_provenance = None;
                        }
                    }
                }
                Message::RetryPrepared(result) => {
                    self.busy = false;
                    match result {
                        Ok((prepared, origin)) => {
                            self.prepared = Some(prepared);
                            self.prepared_provenance = Some(origin);
                            self.dirty_ack = false;
                        }
                        Err(e) => self.notice = e,
                    }
                }
                Message::FollowUp(draft) => {
                    self.busy = false;
                    self.followup_pending = false;
                    self.apply_followup(*draft);
                }
                Message::ReviewStats(run, index, result) => {
                    self.review_pending = None;
                    if let Some(job) = self
                        .state
                        .runs
                        .iter_mut()
                        .find(|r| r.id == run)
                        .and_then(|r| r.jobs.get_mut(index))
                    {
                        job.review = Some(result);
                    }
                }
                Message::Resolved(op, completion) => {
                    self.result_operation = None;
                    if let Some(batch) = &mut self.bulk_discard {
                        batch.finish(&self.store, &mut self.state, &op, completion);
                    } else if let Err(e) =
                        self.store
                            .finish_result_operation(&mut self.state, &op, completion)
                    {
                        self.notice = format!("{e:#}");
                    }
                    self.diff = None;
                    self.diff_target = None;
                    self.dirty = true;
                    self.repository_states.remove(&op.job.repository.path);
                    for job in self.state.runs.iter_mut().flat_map(|r| &mut r.jobs) {
                        if job.execution_mode == domain::ExecutionMode::Direct
                            && job.repository.path == op.job.repository.path
                        {
                            job.review = None;
                        }
                    }
                }
                Message::FoundCli(request, result) => self.cli_search_completed(request, result),
                Message::Orphans(result) => {
                    self.orphan_notice = match result {
                        Ok(paths) if paths.is_empty() => String::new(),
                        Ok(paths) => format!(
                            "Unreferenced worktree resources found ({}{}). They were preserved; inspect these locations before removing anything.\n{}",
                            paths.len(),
                            if paths.len() == 100 { "+" } else { "" },
                            paths
                                .iter()
                                .map(|p| p.display().to_string())
                                .collect::<Vec<_>>()
                                .join("\n")
                        ),
                        Err(e) => format!("Could not inspect worktree storage. {e}"),
                    };
                }
                Message::Reconciled(run, index, metadata, report, with_diff) => {
                    if self.review_pending == Some((run, index)) {
                        self.review_pending = None;
                    }
                    if let Some(job) = self
                        .state
                        .runs
                        .iter_mut()
                        .find(|r| r.id == run)
                        .and_then(|r| r.jobs.get_mut(index))
                        && self.result_operation != Some((run, index))
                        && job.worktree.as_ref() == Some(&metadata)
                        && (job.status.is_terminal() || with_diff)
                    {
                        report.apply(job);
                        if self.diff_target.as_ref() == Some(&metadata.path) {
                            if with_diff {
                                self.diff = Some(report.diff.ok_or_else(|| report.detail.clone()));
                            } else if report.availability != domain::ResultAvailability::Available {
                                self.diff = Some(Err(report.detail));
                            }
                        }
                        recovered = true;
                    }
                }
                Message::Diff(path, result) => {
                    if self.diff_target.as_ref() == Some(&path) {
                        self.diff = Some(result);
                    }
                }
            }
        }
        if recovered {
            self.dirty = true;
            self.save();
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
    fn apply_resolved_executable(&mut self, resolved: agents::availability::ResolvedExecutable) {
        let options = if resolved.agent == self.state.draft.agent {
            &mut self.state.draft.options
        } else {
            self.state.agent_options.entry(resolved.agent).or_default()
        };
        if options.get("executable") == resolved.configured.as_ref() {
            // A startup resolution may finish while the user is choosing among
            // installations. Keep that chooser open; actual edits still reject
            // its snapshot through the usual stale-search checks.
            if let Some(search) = self
                .cli_search
                .as_mut()
                .filter(|search| search.agent == resolved.agent && search.options == *options)
            {
                search
                    .options
                    .insert("executable".into(), resolved.path.clone());
            }
            options.insert("executable".into(), resolved.path);
            self.dirty = true;
        }
    }
    fn select_run(&mut self, id: Option<u64>) {
        if self.selected_run != id {
            self.selected_run = id;
            self.selected_job = 0;
            self.review_selected.clear();
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
        if self.bulk_discard.is_some() {
            return;
        }
        if self
            .state
            .runs
            .iter()
            .any(|r| id.is_none_or(|id| r.id == id) && r.unresolved_results())
        {
            self.notice = "Convoys with unresolved isolated results stay in history. Use Discard convoy… to resolve them. Applied retained copies need explicit Clean up retained copy before history removal.".into();
        }
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
        self.state.draft_provenance = None;
        self.validate_reused_attachments();
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
            | Event::Preparing { run, job, .. }
            | Event::Result { run, job, .. }
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
            Event::Preparing { worktree, .. } => {
                job.status = JobStatus::Preparing;
                job.queue_reason = None;
                job.worktree = worktree;
                self.dirty = true;
            }
            Event::Result { result, detail, .. } => {
                job.result_availability = match result.as_ref() {
                    Some(r) if r.exists && r.changed.is_some() => {
                        domain::ResultAvailability::Available
                    }
                    Some(r) if !r.exists => domain::ResultAvailability::Missing,
                    _ => domain::ResultAvailability::Stale,
                };
                job.result_checked = true;
                job.worktree_result = result;
                job.worktree_detail = detail;
                self.dirty = true;
            }
            Event::Started { before, .. } => {
                job.status = JobStatus::Running;
                job.queue_reason = None;
                self.repository_states.remove(&job.repository.path);
                job.started_at = Some(domain::now());
                if job.execution_mode == domain::ExecutionMode::Direct {
                    job.before = Some(before);
                }
                self.dirty = true;
            }
            Event::Output { text, raw, .. } => {
                job.log.append(&text);
                job.raw_log.append(&raw);
            }
            Event::Finished {
                status,
                exit_code,
                detail,
                ..
            } => {
                job.finish(status, exit_code, detail);
                job.review = None;
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
        self.pump_bulk_discard(ctx);
        self.pump_review(ctx);
        self.handle_close(ctx);
        ctx.request_repaint_after(Duration::from_millis(
            if !self.manager.is_idle()
                || self.closing
                || self.state.runs.iter().any(Run::active)
                || self.busy
                || self.attachment_work.pending
                || self.cli_checks.any_checking()
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
            self.library_window(ctx);
        }
        self.quit_window(ctx);
        self.discard_window(ctx);
        self.bulk_discard_window(ctx);
        if self.dirty && self.last_save.elapsed() > Duration::from_secs(2) {
            self.save();
        }
    }
    fn on_exit(&mut self, _: Option<&eframe::glow::Context>) {
        self.cli_checks.stop();
        self.manager.shutdown();
        // Continue draining lifecycle/output events while process trees stop.
        let final_events = self.runtime.as_ref().expect("Runtime exists before Drop").block_on(async {
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

impl Drop for App {
    fn drop(&mut self) {
        self.cli_checks.stop();
        if let Some(runtime) = self.runtime.take() {
            // A stuck filesystem discovery worker must not hold application
            // shutdown open. Async probe drops retain ManagedChild cleanup.
            runtime.shutdown_background();
        }
    }
}
