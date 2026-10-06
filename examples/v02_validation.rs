//! Native UI validation using disposable repositories and the test CLI only.
use anyhow::{Context, Result, ensure};
use codeconvoy::{domain::*, persistence::Store};

#[path = "../src/graphics.rs"]
mod graphics;

fn main() -> Result<()> {
    #[cfg(target_os = "macos")]
    codeconvoy::ui::init_native_application()?;
    let fixture = std::env::current_exe()?
        .parent()
        .and_then(|p| p.parent())
        .context("Cannot find target directory")?
        .join(format!(
            "codeconvoy-test-agent{}",
            std::env::consts::EXE_SUFFIX
        ));
    ensure!(
        fixture.is_file(),
        "Build codeconvoy-test-agent with --features test-support first"
    );
    let directory = tempfile::Builder::new()
        .prefix("codeconvoy-v02-validation-")
        .tempdir()?;
    let root = directory.path().canonicalize()?;
    let mut state = AppState::default();
    for index in 0..24 {
        let name = match index {
            0 => "gnome-dock".into(),
            1 => "gnome-panel".into(),
            2 => "gnome-menu".into(),
            3 => "homebrew-cask".into(),
            4 => "scoop-bucket".into(),
            _ => format!("repository-{:02}", index + 1),
        };
        let path = root.join(&name);
        ensure!(
            std::process::Command::new("git")
                .args(["init", "--quiet"])
                .arg(&path)
                .status()?
                .success(),
            "Cannot create disposable repository"
        );
        std::fs::write(
            path.join(".git/codeconvoy-workflow.json"),
            br#"{"delay_ms":15000}"#,
        )?;
        state.repositories.push(Repository { name, path });
    }
    for (name, range) in [
        ("GNOME Extensions", 0..3),
        ("Package Repositories", 3..5),
        ("Overlap", 1..5),
        ("1 repository", 0..1),
        ("5 repositories", 0..5),
        ("10 repositories", 0..10),
        ("24 repositories", 0..24),
    ] {
        state.save_group(
            None,
            RepositoryGroup {
                name: name.into(),
                repositories: state.repositories[range]
                    .iter()
                    .map(|r| r.path.clone())
                    .collect(),
            },
        )?;
    }
    for (name, prompt) in [
        (
            "GNOME compatibility review",
            "Review compatibility using the attached specification. Report findings for this repository only.",
        ),
        (
            "Packaged application release",
            "Review this repository's package definition and README for a future application release. Describe repository-specific updates; do not commit or push.",
        ),
    ] {
        state.save_template(
            None,
            TaskTemplate {
                name: name.into(),
                prompt: prompt.into(),
            },
        )?;
    }
    std::fs::write(
        root.join("requirements.md"),
        "Fixture specification: review context independently in each repository. Token: convoy-blue-lantern.\n",
    )?;
    std::fs::write(
        root.join("screenshot.png"),
        include_bytes!("../assets/screenshot.png"),
    )?;
    std::fs::write(
        root.join("unsupported.pdf"),
        "Unsupported fixture; deliberately not an attachment",
    )?;
    for agent in AgentId::ALL {
        state.agent_options.insert(
            agent,
            [(
                "executable".into(),
                fixture
                    .to_str()
                    .context("Fixture path must be Unicode")?
                    .into(),
            )]
            .into(),
        );
    }
    state.draft.options = state.agent_options[&AgentId::Codex].clone();
    state.draft.prompt = state.templates[0].prompt.clone();
    let store = Store::open(&root.join("state"))?;
    store.save(&state)?;
    println!("Disposable validation workspace: {}", root.display());
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()?;
    let size = if std::env::args().any(|arg| arg == "--compact") {
        [780.0, 560.0]
    } else {
        [1180.0, 820.0]
    };
    let options = graphics::configure(eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("CodeConvoy Validation — fixture agents only")
            .with_inner_size(size)
            .with_min_inner_size([780.0, 560.0]),
        ..Default::default()
    })?;
    eframe::run_native(
        "CodeConvoy Validation",
        options,
        Box::new(move |cc| {
            Ok(Box::new(codeconvoy::ui::App::new(
                cc, store, state, runtime,
            )))
        }),
    )
    .map_err(graphics::startup_error)?;
    drop(directory);
    Ok(())
}
