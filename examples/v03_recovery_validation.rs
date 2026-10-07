//! Persistent disposable native fixture. `--prepare <directory>` seeds it and
//! records its path beside this test executable. `--cleanup` invokes the internal
//! primitive for the newest result. No arguments opens/reopens the native UI.
use anyhow::{Context, Result, ensure};
use codeconvoy::{domain::*, persistence::Store};
#[path = "../src/graphics.rs"]
mod graphics;
fn git(path: &std::path::Path, args: &[&std::ffi::OsStr]) -> Result<()> {
    ensure!(
        std::process::Command::new("git")
            .arg("-C")
            .arg(path)
            .args(args)
            .status()?
            .success(),
        "Fixture Git setup failed"
    );
    Ok(())
}
fn main() -> Result<()> {
    let exe = std::env::current_exe()?.canonicalize()?;
    let pointer = exe.with_extension("workspace");
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.first().is_some_and(|a| a == "--prepare") {
        let root =
            std::path::PathBuf::from(args.get(1).context("Provide a new disposable directory")?);
        ensure!(!root.exists(), "Use a new disposable directory");
        std::fs::create_dir_all(&root)?;
        let root = root.canonicalize()?;
        let source = root.join("source ü with spaces");
        std::fs::create_dir(&source)?;
        git(&source, &["init".as_ref(), "--quiet".as_ref()])?;
        std::fs::write(source.join("tracked.txt"), "committed native base\n")?;
        git(&source, &["add".as_ref(), "tracked.txt".as_ref()])?;
        git(
            &source,
            &[
                "-c".as_ref(),
                "commit.gpgsign=false".as_ref(),
                "-c".as_ref(),
                "user.name=Fixture".as_ref(),
                "-c".as_ref(),
                "user.email=fixture@example.invalid".as_ref(),
                "commit".as_ref(),
                "-qm".as_ref(),
                "base".as_ref(),
            ],
        )?;
        let unrelated = root.join("unrelated user worktree");
        git(
            &source,
            &[
                "worktree".as_ref(),
                "add".as_ref(),
                "--detach".as_ref(),
                unrelated.as_os_str(),
            ],
        )?;
        let control = root.join("control");
        std::fs::create_dir(&control)?;
        std::fs::write(control.join("native-tree.release"), "ready")?;
        let fixture = exe
            .parent()
            .and_then(|p| p.parent())
            .context("Fixture binary directory")?
            .join(format!(
                "codeconvoy-test-agent{}",
                std::env::consts::EXE_SUFFIX
            ));
        ensure!(fixture.is_file(), "Build the fixture binary first");
        let mut state = AppState::default();
        state.repositories.push(Repository {
            name: "Native recovery source".into(),
            path: source,
        });
        state.draft.execution_mode = ExecutionMode::IsolatedWorktree;
        state.draft.prompt = format!(
            "codeconvoy-fixture-gate\n{}",
            serde_json::json!({"control": control, "ticket":"native", "edit_before":true,"fail":false})
        );
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
        Store::open(&root.join("state"))?.save(&state)?;
        std::fs::write(pointer, root.to_str().context("Unicode workspace path")?)?;
        println!("Prepared disposable workspace: {}", root.display());
        return Ok(());
    }
    let root = std::path::PathBuf::from(
        std::fs::read_to_string(&pointer).context("Run --prepare <directory> first")?,
    );
    let store = Store::open(&root.join("state"))?;
    let mut state = store.load()?;
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()?;
    if args.first().is_some_and(|a| a == "--cleanup") {
        runtime.block_on(async {
            let (tx, _rx) = tokio::sync::mpsc::channel(256);
            let mut manager = codeconvoy::runner::RunManager::with_worktree_directory(
                2,
                tx,
                store.directory().join("worktrees"),
            );
            let run = state
                .runs
                .first()
                .context("Launch a fixture convoy first")?
                .id;
            let result = store.cleanup_result(&mut state, &manager, run, 0).await;
            manager.shutdown();
            (&mut manager.join).await?;
            result
        })?;
        println!(
            "Internal cleanup completed; fixture workspace: {}",
            root.display()
        );
        return Ok(());
    }
    #[cfg(target_os = "macos")]
    codeconvoy::ui::init_native_application()?;
    let options = graphics::configure(eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("CodeConvoy Recovery Validation — fixtures only")
            .with_inner_size([1180.0, 820.0]),
        ..Default::default()
    })?;
    eframe::run_native(
        "CodeConvoy Recovery Validation",
        options,
        Box::new(move |cc| {
            Ok(Box::new(codeconvoy::ui::App::new(
                cc, store, state, runtime,
            )))
        }),
    )
    .map_err(graphics::startup_error)
}
