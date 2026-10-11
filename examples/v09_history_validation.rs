//! Disposable native history smoke fixture. No installed agent is executed.
use anyhow::{Context, Result};
use codeconvoy::{domain::*, persistence::Store};
#[path = "../src/graphics.rs"]
mod graphics;

/// Optional native smoke run: exercise the ordinary close path without relying
/// on desktop automation (which compositors may restrict).
struct SmokeApp {
    app: codeconvoy::ui::App,
    close_at: Option<std::time::Instant>,
}
impl eframe::App for SmokeApp {
    fn logic(&mut self, ctx: &eframe::egui::Context, frame: &mut eframe::Frame) {
        self.app.logic(ctx, frame);
        if self
            .close_at
            .is_some_and(|at| std::time::Instant::now() >= at)
        {
            self.close_at = None;
            ctx.send_viewport_cmd(eframe::egui::ViewportCommand::Close);
        }
    }
    fn ui(&mut self, ui: &mut eframe::egui::Ui, frame: &mut eframe::Frame) {
        self.app.ui(ui, frame);
    }
    fn on_exit(&mut self, gl: Option<&eframe::glow::Context>) {
        self.app.on_exit(gl);
    }
}

fn main() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let root = temp.path().canonicalize()?;
    let store = Store::open(&root.join("state"))?;
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()?;
    let repo = root.join("source");
    std::fs::create_dir(&repo)?;
    for args in [
        vec!["init", "--quiet"],
        vec![
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=fixture@example.invalid",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "--allow-empty",
            "--quiet",
            "-m",
            "fixture",
        ],
    ] {
        anyhow::ensure!(
            std::process::Command::new("git")
                .current_dir(&repo)
                .args(args)
                .status()?
                .success(),
            "fixture Git initialization failed"
        );
    }
    let repository = runtime.block_on(codeconvoy::git::register(&repo))?;
    let git = runtime.block_on(codeconvoy::git::status(&repo))?;
    let metadata = codeconvoy::worktrees::reserve(
        &store.directory().join("worktrees"),
        1001,
        0,
        repository.clone(),
        git.summary.common_dir.context("missing common directory")?,
        git.summary.head.context("missing HEAD")?,
    )?;
    runtime.block_on(codeconvoy::worktrees::create(
        &metadata,
        &Default::default(),
        &std::sync::atomic::AtomicBool::new(true),
    ))?;
    std::fs::write(
        metadata.path.join("retained.txt"),
        "Retain this worktree and its ownership record.\n",
    )?;
    let now = now();
    let mut state = AppState {
        next_run: 1003,
        repositories: vec![repository.clone()],
        ..Default::default()
    };
    for agent in AgentId::ALL {
        state.agent_options.insert(
            agent,
            [(
                "executable".into(),
                root.join("missing-cli").display().to_string(),
            )]
            .into(),
        );
    }
    state.draft.options = state.agent_options[&AgentId::Codex].clone();
    state.history_retention.max_entries = std::num::NonZeroUsize::new(250);
    state.history_retention.max_age_days = std::num::NonZeroU32::new(180);
    state.history_retention.automatic = std::env::args().any(|a| a == "--automatic")
        || std::env::var_os("CODECONVOY_HISTORY_FIXTURE_AUTOMATIC").is_some();
    for id in (1..=1002).rev() {
        let mut job = Job::queued(repository.clone());
        job.status = if id == 1002 {
            JobStatus::Queued
        } else {
            JobStatus::Succeeded
        };
        let finished = now - (1002 - id) * 86_400;
        job.finished_at = Some(finished);
        if id == 1001 {
            job.execution_mode = ExecutionMode::IsolatedWorktree;
            job.worktree = Some(metadata.clone());
        }
        state.runs.push(Run {
            id,
            provenance: None,
            created_at: finished - 10,
            task: TaskConfig {
                prompt: format!("History fixture {id}"),
                ..Default::default()
            },
            jobs: vec![job],
        });
    }
    store.save(&state)?;
    // Reload saved history to exercise compatibility and interrupted-job recovery.
    state = store.load()?;
    println!("History fixture: {}", root.display());
    println!("Retained worktree: {}", metadata.path.display());
    #[cfg(target_os = "macos")]
    codeconvoy::ui::init_native_application()?;
    let options = graphics::configure(eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("CodeConvoy — History fixtures")
            .with_inner_size([780.0, 560.0]),
        ..Default::default()
    })?;
    eframe::run_native(
        "CodeConvoy History",
        options,
        Box::new(move |cc| {
            Ok(Box::new(SmokeApp {
                app: codeconvoy::ui::App::new(cc, store, state, runtime),
                close_at: std::env::args()
                    .any(|a| a == "--smoke")
                    .then(|| std::time::Instant::now() + std::time::Duration::from_secs(3)),
            }))
        }),
    )
    .map_err(graphics::startup_error)?;
    anyhow::ensure!(
        metadata.path.join("retained.txt").exists(),
        "history cleanup removed worktree contents"
    );
    anyhow::ensure!(
        metadata.path.join(".git").exists(),
        "history cleanup removed Git backlink"
    );
    println!("Retained worktree contents and Git backlink preserved.");
    Ok(())
}
