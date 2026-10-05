mod editor;
mod format;
mod results;
mod text_view;
mod theme;

use crate::{
    agents,
    domain::{self, AppState, Job, JobStatus, MAX_HISTORY, MAX_REPOSITORIES, Repository, Run},
    git::{self, WorkingTree},
    persistence::Store,
    runner::{self, Event, PreparedRun, RunHandle},
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
    Registered(Result<(Repository, WorkingTree), String>),
    Refreshed(Vec<(PathBuf, Result<WorkingTree, String>)>),
    Prepared(Result<PreparedRun, String>),
    Detected(
        domain::AgentId,
        domain::AgentOptions,
        Result<String, String>,
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
    repository_input: String,
    repository_states: HashMap<PathBuf, Result<WorkingTree, String>>,
    busy: bool,
    notice: String,
    detection: Option<(
        domain::AgentId,
        domain::AgentOptions,
        Result<String, String>,
    )>,
    output_view: text_view::TextView,
    diff_view: text_view::TextView,
    tx: mpsc::Sender<Message>,
    rx: mpsc::Receiver<Message>,
    events_tx: async_mpsc::Sender<Event>,
    events_rx: async_mpsc::Receiver<Event>,
    active: Option<RunHandle>,
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
}
impl App {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        store: Store,
        state: AppState,
        runtime: Runtime,
    ) -> Self {
        theme::install(&cc.egui_ctx);
        let (tx, rx) = mpsc::channel();
        let (events_tx, events_rx) = async_mpsc::channel(256);
        let selected_run = state.runs.first().map(|r| r.id);
        let mut app = Self {
            store,
            state,
            runtime,
            selected: HashSet::new(),
            repository_input: String::new(),
            repository_states: HashMap::new(),
            busy: false,
            notice: String::new(),
            detection: None,
            output_view: text_view::TextView::default(),
            diff_view: text_view::TextView::default(),
            tx,
            rx,
            events_tx,
            events_rx,
            active: None,
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
        };
        if !app.state.repositories.is_empty() {
            app.refresh(cc.egui_ctx.clone());
        }
        app
    }
    fn current_detection(
        &self,
    ) -> Option<&(
        domain::AgentId,
        domain::AgentOptions,
        Result<String, String>,
    )> {
        self.detection.as_ref().filter(|(agent, options, _)| {
            *agent == self.state.draft.agent && *options == self.state.draft.options
        })
    }

    fn header(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("header")
            .frame(
                egui::Frame::side_top_panel(&ctx.style())
                    .inner_margin(egui::Margin::symmetric(theme::PANEL_MARGIN, 10)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.heading("CodeConvoy");
                    if ui.available_width() > 650.0 {
                        ui.weak("One task. Multiple repositories.");
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
                        if self.active.is_some()
                            && ui.add(theme::quiet("Stop all jobs")).clicked()
                            && let Some(handle) = &self.active
                        {
                            handle.cancel_all();
                        }
                        let p = theme::Palette::of(ui);
                        if self.closing {
                            ui.colored_label(p.warning, "Stopping processes…");
                        } else {
                            let (suffix, color) = match self.current_detection().map(|(_, _, r)| r)
                            {
                                Some(Ok(_)) => ("available", p.success),
                                Some(Err(_)) => ("check failed", p.error),
                                None => ("not checked", p.muted),
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

    fn footer(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("footer")
            .frame(egui::Frame::side_top_panel(&ctx.style()).inner_margin(egui::Margin::symmetric(theme::PANEL_MARGIN, 5)))
            .show(ctx, |ui| {
                if !self.notice.is_empty() {
                    ui.horizontal_wrapped(|ui| {
                        ui.colored_label(theme::Palette::of(ui).error, &self.notice);
                        if ui.add(theme::quiet("Dismiss").small()).clicked() { self.notice.clear(); }
                    });
                }
                ui.horizontal(|ui| {
                    ui.small("Tab / Shift+Tab to navigate · Enter / Space to activate");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
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
        let path = PathBuf::from(self.repository_input.trim());
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
        let jobs = prepared
            .repositories
            .iter()
            .map(|r| {
                let mut job = Job::queued(r.repository.clone());
                job.before = Some(r.state.summary.clone());
                job
            })
            .collect();
        self.state.runs.insert(
            0,
            Run {
                id,
                created_at: domain::now(),
                task: prepared.task.clone(),
                jobs,
            },
        );
        self.state.runs.truncate(MAX_HISTORY);
        // Persist intent before an agent can touch a repository.
        if let Err(error) = self.store.save(&self.state) {
            self.state.runs.remove(0);
            self.notice = format!("Cannot save run; nothing was started: {error:#}");
            return;
        }
        let _entered = self.runtime.enter();
        self.active = Some(runner::start(id, prepared, backend, self.events_tx.clone()));
        self.selected_run = Some(id);
        self.selected_job = 0;
        self.tab = Tab::Output;
        self.diff = None;
        self.diff_target = None;
        self.dirty_ack = false;
    }
    fn poll(&mut self) {
        while let Ok(message) = self.rx.try_recv() {
            match message {
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
                Message::Detected(agent, options, result) => {
                    self.busy = false;
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
            let (id, job) = match &event {
                Event::Started { run, job, .. }
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
                continue;
            };
            match event {
                Event::Started { before, .. } => {
                    job.status = JobStatus::Running;
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
                    self.dirty = true;
                }
            }
        }
        self.state.trim_logs();
        if self.active.as_ref().is_some_and(|h| h.join.is_finished())
            && !self.state.runs.iter().any(Run::active)
        {
            self.active = None;
            // Cached registration status is stale after execution.
            self.repository_states.clear();
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
    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        self.poll();
        if ctx.input(|i| i.viewport().close_requested()) && self.active.is_some() {
            self.closing = true;
            if let Some(handle) = &self.active {
                handle.cancel_all();
            }
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
        }
        if self.closing && self.active.is_none() {
            self.save();
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        self.header(ctx);
        self.footer(ctx);
        egui::SidePanel::left("task_editor")
            .resizable(true)
            .default_width(360.0)
            .width_range(310.0..=(ctx.content_rect().width() * 0.55).max(310.0))
            .frame(egui::Frame::side_top_panel(&ctx.style()).inner_margin(theme::PANEL_MARGIN))
            .show(ctx, |ui| {
                egui::TopBottomPanel::bottom("execution_controls")
                    .frame(egui::Frame::NONE)
                    .show_inside(ui, |ui| {
                        ui.add_enabled_ui(self.prepared.is_none() && !self.closing, |ui| {
                            self.execution_section(ui, ctx)
                        });
                    });
                egui::ScrollArea::vertical()
                    .id_salt("editor_scroll")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.add_enabled_ui(self.prepared.is_none() && !self.closing, |ui| {
                            self.editor(ui, ctx)
                        });
                    });
            });
        egui::CentralPanel::default()
            .frame(egui::Frame::central_panel(&ctx.style()).inner_margin(theme::PANEL_MARGIN))
            .show(ctx, |ui| self.results(ui, ctx));
        self.preflight_window(ctx);
        if self.dirty && self.last_save.elapsed() > Duration::from_secs(2) {
            self.save();
        }
        ctx.request_repaint_after(Duration::from_millis(
            if self.active.is_some() || self.busy {
                100
            } else {
                1000
            },
        ));
    }
    fn on_exit(&mut self, _: Option<&eframe::glow::Context>) {
        if let Some(handle) = &self.active {
            handle.cancel_all();
        }
        // Ordinary close is delayed in update. This covers framework-driven exit.
        if let Some(handle) = self.active.take() {
            self.runtime.block_on(async {
                let deadline = tokio::time::sleep(Duration::from_secs(8));
                tokio::pin!(deadline);
                loop {
                    tokio::select! {
                        _ = &mut deadline => break,
                        _ = tokio::time::sleep(Duration::from_millis(20)) => {
                            while self.events_rx.try_recv().is_ok() {}
                            if handle.join.is_finished() { break; }
                        }
                    }
                }
            });
        }
        self.state.recover_interrupted();
        self.save();
    }
}
