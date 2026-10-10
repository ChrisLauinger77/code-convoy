//! Disposable native configuration smoke fixture. Never uses provider agents.
use anyhow::Result;
use codeconvoy::{domain::*, persistence::Store, validation::Command};
#[path = "../src/graphics.rs"]
mod graphics;

fn main() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let root = temp.path().canonicalize()?;
    let store = Store::open(&root.join("state"))?;
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()?;
    let mut state = AppState::default();
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
    for name in ["Extension", "Rust", "Shared custom"] {
        let path = root.join(name);
        std::fs::create_dir(&path)?;
        let status = std::process::Command::new("git")
            .args(["init", "--quiet"])
            .arg(&path)
            .status()?;
        anyhow::ensure!(status.success(), "Fixture Git init failed");
        std::fs::write(
            path.join("unchanged.txt"),
            "configuration must not alter this file\n",
        )?;
        state.repositories.push(Repository {
            path,
            name: name.into(),
        });
    }
    state.groups = vec![
        RepositoryGroup {
            name: "GNOME Extensions".into(),
            repositories: vec![
                state.repositories[0].path.clone(),
                state.repositories[2].path.clone(),
            ],
        },
        RepositoryGroup {
            name: "Rust projects".into(),
            repositories: vec![
                state.repositories[1].path.clone(),
                state.repositories[2].path.clone(),
            ],
        },
    ];
    state.save_validation(
        state.repositories[2].path.clone(),
        Some(Command {
            executable: "custom-check".into(),
            arguments: vec!["literal spaces".into()],
        }),
    )?;
    store.save(&state)?;
    println!("Disposable validation preset fixture: {}", root.display());
    let size: Vec<f32> = std::env::args()
        .skip(1)
        .filter_map(|v| v.parse().ok())
        .collect();
    #[cfg(target_os = "macos")]
    codeconvoy::ui::init_native_application()?;
    let options = graphics::configure(eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("CodeConvoy — Validation preset fixtures")
            .with_inner_size([
                size.first().copied().unwrap_or(1180.0),
                size.get(1).copied().unwrap_or(820.0),
            ]),
        ..Default::default()
    })?;
    eframe::run_native(
        "CodeConvoy Validation Presets",
        options,
        Box::new(move |cc| {
            Ok(Box::new(codeconvoy::ui::App::new(
                cc, store, state, runtime,
            )))
        }),
    )
    .map_err(graphics::startup_error)?;
    drop(temp);
    Ok(())
}
