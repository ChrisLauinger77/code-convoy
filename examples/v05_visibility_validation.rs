//! Disposable native visibility smoke fixture; never invokes an installed agent.
use anyhow::Result;
use codeconvoy::{domain::*, persistence::Store, visibility::Changes};
#[path = "../src/graphics.rs"]
mod graphics;
fn main() -> Result<()> {
    let temporary = tempfile::tempdir()?;
    let root = temporary.path().canonicalize()?;
    let store = Store::open(&root.join("state"))?;
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()?;
    let mut state = AppState::default();
    state.notifications.enabled = false;
    for agent in AgentId::ALL {
        state.agent_options.insert(
            agent,
            [(
                "executable".into(),
                root.join("missing-fixture-cli").display().to_string(),
            )]
            .into(),
        );
    }
    state.draft.options = state.agent_options[&AgentId::Codex].clone();
    for (index, name) in [
        "Clean service",
        "Changed service",
        "Failed service",
        "Missing repository",
    ]
    .into_iter()
    .enumerate()
    {
        let path = root.join(name);
        if index < 3 {
            std::fs::create_dir(&path)?;
            let output = std::process::Command::new("git")
                .args(["init", "--quiet"])
                .arg(&path)
                .output()?;
            anyhow::ensure!(output.status.success(), "Git fixture initialization failed");
            if index > 0 {
                std::fs::write(path.join("changed.txt"), "fixture change")?;
            }
        }
        state.repositories.push(Repository {
            name: name.into(),
            path,
        });
    }
    for id in (1..=6).rev() {
        let mut jobs = Vec::new();
        for (index, repo) in state.repositories.iter().enumerate() {
            let mut job = Job::queued(repo.clone());
            job.status = match index {
                2 => JobStatus::Failed,
                3 => JobStatus::Cancelled,
                _ => JobStatus::Succeeded,
            };
            job.started_at = Some(now() - 90);
            job.finished_at = Some(now() - 30);
            if id != 1 && index < 3 {
                job.completion_changes = Some(Changes {
                    changed: Some(index > 0),
                    files: Some(if index > 0 { 2 } else { 0 }),
                });
            }
            job.log
                .append("[CodeConvoy] Disposable visibility fixture; no agent executed.\n");
            jobs.push(job);
        }
        state.runs.push(Run {
            id,
            provenance: None,
            created_at: now() - 120,
            task: TaskConfig {
                prompt: format!("Visibility fixture {id}: compare repository results"),
                ..Default::default()
            },
            jobs,
        });
    }
    state.next_run = 7;
    let width = std::env::args()
        .nth(1)
        .and_then(|v| v.parse::<f32>().ok())
        .unwrap_or(1180.0);
    let height = std::env::args()
        .nth(2)
        .and_then(|v| v.parse::<f32>().ok())
        .unwrap_or(820.0);
    #[cfg(target_os = "macos")]
    codeconvoy::ui::init_native_application()?;
    let options = graphics::configure(eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("CodeConvoy 0.5 Visibility — disposable fixtures")
            .with_inner_size([width, height]),
        ..Default::default()
    })?;
    eframe::run_native(
        "CodeConvoy Visibility Validation",
        options,
        Box::new(move |cc| {
            Ok(Box::new(codeconvoy::ui::App::new(
                cc, store, state, runtime,
            )))
        }),
    )
    .map_err(graphics::startup_error)?;
    drop(temporary);
    Ok(())
}
