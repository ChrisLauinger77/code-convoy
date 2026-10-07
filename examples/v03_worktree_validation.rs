//! Native v0.3 core validation. Only disposable repositories and fixture CLIs.
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
        .context("Cannot locate fixture executable")?
        .join(format!(
            "codeconvoy-test-agent{}",
            std::env::consts::EXE_SUFFIX
        ));
    ensure!(
        fixture.is_file(),
        "Build codeconvoy-test-agent with --features test-support first"
    );
    let directory = tempfile::Builder::new()
        .prefix("codeconvoy-v03-validation-")
        .tempdir()?;
    let root = directory.path().canonicalize()?;
    let mut state = AppState::default();
    for name in ["dirty source ü", "clean source", "preparation source"] {
        let path = root.join(name);
        std::fs::create_dir(&path)?;
        for args in [
            vec!["init", "--quiet"],
            vec!["config", "commit.gpgsign", "false"],
        ] {
            ensure!(
                std::process::Command::new("git")
                    .arg("-C")
                    .arg(&path)
                    .args(args)
                    .status()?
                    .success(),
                "Fixture Git setup failed"
            );
        }
        std::fs::write(path.join("tracked.txt"), "committed base\n")?;
        ensure!(
            std::process::Command::new("git")
                .arg("-C")
                .arg(&path)
                .args(["add", "tracked.txt"])
                .status()?
                .success(),
            "Fixture add failed"
        );
        ensure!(
            std::process::Command::new("git")
                .arg("-C")
                .arg(&path)
                .args([
                    "-c",
                    "user.name=Fixture",
                    "-c",
                    "user.email=fixture@example.invalid",
                    "commit",
                    "-qm",
                    "base"
                ])
                .status()?
                .success(),
            "Fixture commit failed"
        );
        if name.starts_with("dirty") {
            std::fs::write(path.join("tracked.txt"), "dirty local changes excluded\n")?;
        }
        state.repositories.push(Repository {
            path,
            name: name.into(),
        });
    }
    let control = root.join("control");
    std::fs::create_dir(&control)?;
    let preparation = &state.repositories[2].path;
    std::fs::write(
        preparation.join(".gitattributes"),
        "tracked.txt filter=convoygate\n",
    )?;
    let quote = |value: &std::path::Path| -> Result<String> {
        Ok(format!(
            "'{}'",
            value
                .to_str()
                .context("Fixture path must be Unicode")?
                .replace('\'', "'\\''")
        ))
    };
    let binary = quote(&fixture)?;
    for args in [
        vec!["add".into(), ".gitattributes".into()],
        vec![
            "-c".into(),
            "user.name=Fixture".into(),
            "-c".into(),
            "user.email=fixture@example.invalid".into(),
            "commit".into(),
            "-qm".into(),
            "filter fixture".into(),
        ],
        vec![
            "config".into(),
            "filter.convoygate.smudge".into(),
            format!("{binary} smudge-gate {}", quote(&control)?),
        ],
        vec![
            "config".into(),
            "filter.convoygate.clean".into(),
            format!("{binary} passthrough"),
        ],
        vec![
            "config".into(),
            "filter.convoygate.required".into(),
            "true".into(),
        ],
    ] {
        ensure!(
            std::process::Command::new("git")
                .arg("-C")
                .arg(preparation)
                .args(args)
                .status()?
                .success(),
            "Preparation fixture setup failed"
        );
    }
    for (name, ticket, edit, fail) in [
        ("Failed result", "failed", true, true),
        ("Peer one", "one", true, false),
        ("Peer two", "two", true, false),
    ] {
        state.save_template(None, TaskTemplate {
            name: name.into(),
            prompt: format!("codeconvoy-fixture-gate\n{}", serde_json::json!({"control":control,"ticket":ticket,"edit_before":edit,"fail":fail})),
        })?;
    }
    let context = root.join("outside context 日本語.md");
    std::fs::write(
        &context,
        "Native isolated attachment sentinel: convoy-a2-context.\n",
    )?;
    state
        .draft
        .attachments
        .push(codeconvoy::attachments::Attachment::inspect(&context)?);
    for agent in AgentId::ALL {
        state.agent_options.insert(
            agent,
            [(
                "executable".into(),
                fixture
                    .to_str()
                    .context("Fixture path is not Unicode")?
                    .into(),
            )]
            .into(),
        );
    }
    state.draft.options = state.agent_options[&AgentId::Codex].clone();
    state.draft.execution_mode = ExecutionMode::IsolatedWorktree;
    state.draft.prompt = "Native worktree fixture: record input and complete.".into();
    let store = Store::open(&root.join("state"))?;
    store.save(&state)?;
    println!("Disposable validation workspace: {}", root.display());
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()?;
    let options = graphics::configure(eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("CodeConvoy Worktree Validation — fixture agents only")
            .with_inner_size([1180.0, 820.0])
            .with_min_inner_size([780.0, 560.0]),
        ..Default::default()
    })?;
    eframe::run_native(
        "CodeConvoy Worktree Validation",
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
