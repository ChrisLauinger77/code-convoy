//! Persistent, disposable native Retry/follow-up scenarios, fixture agents only.
use anyhow::{Context, Result, ensure};
use codeconvoy::{
    domain::*,
    persistence::Store,
    runner::{self, Event, RunManager},
};
use std::path::Path;
#[path = "../src/graphics.rs"]
mod graphics;
fn git(path: &Path, args: &[&str]) -> Result<()> {
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(path)
        .args(args)
        .output()?;
    ensure!(
        output.status.success(),
        "Fixture Git failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(())
}
async fn seed(root: &Path, fixture: &Path) -> Result<()> {
    ensure!(!root.exists(), "Use a new disposable directory");
    std::fs::create_dir_all(root)?;
    let root = root.canonicalize()?;
    let store = Store::open(&root.join("state"))?;
    let mut state = AppState::default();
    let control = root.join("control");
    std::fs::create_dir(&control)?;
    for n in 0..5 {
        let name = match n {
            0 => "SmartAutoMoveNG",
            1 => "HeadsetControl",
            2 => "messagingmenu",
            3 => "homebrew-cask",
            _ => "scoop-bucket",
        };
        let path = root.join(name);
        std::fs::create_dir(&path)?;
        git(&path, &["init", "--quiet"])?;
        git(&path, &["config", "core.autocrlf", "false"])?;
        std::fs::write(path.join("tracked.txt"), "native committed base\n")?;
        git(&path, &["add", "."])?;
        git(
            &path,
            &[
                "-c",
                "commit.gpgsign=false",
                "-c",
                "user.name=Fixture",
                "-c",
                "user.email=f@invalid",
                "commit",
                "-qm",
                "base",
            ],
        )?;
        state
            .repositories
            .push(codeconvoy::git::register(&path).await?);
    }
    for agent in AgentId::ALL {
        state.agent_options.insert(
            agent,
            [(
                "executable".into(),
                fixture.to_str().context("Unicode fixture path")?.into(),
            )]
            .into(),
        );
    }
    state.draft.options = state.agent_options[&AgentId::Codex].clone();
    let (tx, mut rx) = tokio::sync::mpsc::channel(256);
    let mut manager =
        RunManager::with_worktree_directory(4, tx, store.directory().join("worktrees"));
    state.groups = vec![
        RepositoryGroup {
            name: "GNOME Extensions".into(),
            repositories: state.repositories[..3]
                .iter()
                .map(|r| r.path.clone())
                .collect(),
        },
        RepositoryGroup {
            name: "Package Repositories".into(),
            repositories: state.repositories[3..]
                .iter()
                .map(|r| r.path.clone())
                .collect(),
        },
    ];
    state.templates = vec![TaskTemplate {
        name: "Maintenance follow-up".into(),
        prompt: "Review the previous compatibility work and fix remaining issues.".into(),
    }];
    let context = root.join("maintenance-notes.md");
    std::fs::write(
        &context,
        "Fixture context: compatibility and packaging maintenance; disposable repositories only.",
    )?;
    for id in [1, 2] {
        let ticket = if id == 1 {
            "gnome-maintenance"
        } else {
            "package-review"
        };
        if id == 2 {
            std::fs::write(control.join(format!("{ticket}-tree.release")), "ready")?;
        }
        let task = TaskConfig {
            prompt: format!(
                "codeconvoy-fixture-gate\n{}",
                serde_json::json!({"control": control, "ticket":ticket, "edit_before":true, "fail":false})
            ),
            attachments: vec![codeconvoy::attachments::Attachment::inspect(&context)?],
            options: state.draft.options.clone(),
            execution_mode: ExecutionMode::IsolatedWorktree,
            concurrency: 4,
            ..Default::default()
        };
        let repos = if id == 1 {
            state.repositories[..3].to_vec()
        } else {
            state.repositories[3..].to_vec()
        };
        let prepared = runner::prepare(task, repos).await?;
        let mut run = prepared.snapshot(id);
        let mut ready = std::collections::HashSet::new();
        manager.start(id, prepared, codeconvoy::agents::backend(AgentId::Codex)?)?;
        while run.active() {
            match rx.recv().await.context("Missing fixture event")? {
                Event::Preparing { job, worktree, .. } => {
                    run.jobs[job].worktree = worktree;
                    run.jobs[job].status = JobStatus::Preparing;
                }
                Event::Result { job, result, .. } => run.jobs[job].worktree_result = result,
                Event::Started { job, .. } => {
                    run.jobs[job].status = JobStatus::Running;
                }
                Event::Output { job, text, raw, .. } => {
                    if text.contains("fixture gate ready") {
                        ready.insert(job);
                    }
                    if id == 1 && ready.len() == 3 {
                        manager.cancel(id, 2);
                        ready.clear();
                    }
                    run.jobs[job].log.append(&text);
                    run.jobs[job].raw_log.append(&raw);
                }
                Event::Finished {
                    job,
                    status,
                    exit_code,
                    detail,
                    ..
                } => {
                    if id == 1 && job == 2 && status == JobStatus::Cancelled {
                        // Release peers only after cancellation is confirmed; the
                        // shared gate must not let this job finish before Stop.
                        std::fs::write(control.join(format!("{ticket}-tree.release")), "ready")?;
                    }
                    run.jobs[job].finish(status, exit_code, detail);
                }
                _ => {}
            }
        }
        ensure!(
            run.jobs.iter().enumerate().all(|(i, j)| j.status
                == if id == 1 && i == 2 {
                    JobStatus::Cancelled
                } else {
                    JobStatus::Succeeded
                }),
            "Fixture status mismatch"
        );
        state.runs.push(run);
    }
    manager.shutdown();
    (&mut manager.join).await?;
    state.next_run = 3;
    store.save(&state)?;
    Ok(())
}
fn main() -> Result<()> {
    let exe = std::env::current_exe()?.canonicalize()?;
    let pointer = exe.with_extension("workspace");
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()?;
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.first().is_some_and(|a| a == "--prepare") {
        let root =
            std::path::PathBuf::from(args.get(1).context("Provide new disposable directory")?);
        let fixture = exe
            .parent()
            .and_then(|p| p.parent())
            .context("Binary path")?
            .join(format!(
                "codeconvoy-test-agent{}",
                std::env::consts::EXE_SUFFIX
            ));
        runtime.block_on(seed(&root, &fixture))?;
        std::fs::write(pointer, root.to_str().context("Unicode root")?)?;
        println!("Prepared {}", root.display());
        return Ok(());
    }
    let root =
        std::path::PathBuf::from(std::fs::read_to_string(pointer).context("Run --prepare first")?);
    let store = Store::open(&root.join("state"))?;
    let state = store.load()?;
    #[cfg(target_os = "macos")]
    codeconvoy::ui::init_native_application()?;
    let options = graphics::configure(eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("CodeConvoy Continuation Validation — fixtures only")
            .with_inner_size([1240.0, 860.0]),
        ..Default::default()
    })?;
    eframe::run_native(
        "CodeConvoy Continuation Validation",
        options,
        Box::new(move |cc| {
            Ok(Box::new(codeconvoy::ui::App::new(
                cc, store, state, runtime,
            )))
        }),
    )
    .map_err(graphics::startup_error)
}
