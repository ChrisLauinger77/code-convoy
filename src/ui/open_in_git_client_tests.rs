//! The Open in Git Client action without starting any Git client: targets are
//! deliberately missing, so verification reports them before any launch.
use super::*;
use crate::domain::{ExecutionMode, Job, WorktreeMetadata};
use crate::git_client::{Client, Kind, Preferences};
use egui::accesskit::Node;

const ACTION: &str = "Open in Git Client";

fn configure(app: &mut App) {
    app.state.git_client = Preferences {
        client: Some(Client::Custom),
        executable: "/nonexistent/client".into(),
        arguments: vec!["{repo}".into()],
    };
}

fn isolated(job: &mut Job, path: PathBuf) {
    job.execution_mode = ExecutionMode::IsolatedWorktree;
    job.worktree = Some(WorktreeMetadata {
        owner: "test".into(),
        run: 7,
        job: 1,
        repository: job.repository.clone(),
        common_dir: PathBuf::from("/fixtures/common"),
        path,
        base_commit: "0".repeat(40),
        execution_mode: ExecutionMode::IsolatedWorktree,
    });
}

struct Convoy {
    repository: PathBuf,
    original: PathBuf,
    worktree: PathBuf,
}

/// Run 7: a Direct job whose repository moved, and an isolated job whose
/// worktree was deleted. Absolute on every platform, and all missing.
fn convoy(app: &mut App, temp: &std::path::Path) -> Convoy {
    let paths = Convoy {
        repository: temp.join("moved repository"),
        original: temp.join("original repository"),
        worktree: temp.join("deleted worktree ü"),
    };
    let mut run = run(7, &[JobStatus::Succeeded, JobStatus::Failed]);
    run.jobs[0].repository.path = paths.repository.clone();
    run.jobs[1].repository.path = paths.original.clone();
    isolated(&mut run.jobs[1], paths.worktree.clone());
    app.state.runs.push(run);
    app.selected_run = Some(7);
    paths
}

fn notice(app: &mut App) -> String {
    let deadline = Instant::now() + Duration::from_secs(5);
    while app.notice.is_empty() {
        assert!(Instant::now() < deadline, "no Git client notice");
        app.runtime().block_on(async {
            tokio::time::sleep(Duration::from_millis(5)).await;
        });
        app.poll();
    }
    std::mem::take(&mut app.notice)
}

fn action(keys: &Keyboard) -> Vec<&Node> {
    keys.nodes
        .values()
        .filter(|node| keys.label(node) == ACTION)
        .collect()
}

#[test]
fn the_action_icon_renders_with_the_bundled_fonts() {
    let ctx = egui::Context::default();
    theme::install(&ctx);
    let mut output = ctx.run_ui(egui::RawInput::default(), |_| {});
    output.textures_delta.clear();
    let font = egui::FontId::proportional(12.0);
    assert!(ctx.fonts_mut(|fonts| fonts.has_glyph(&font, open_in_git_client::ICON)));
}

#[test]
fn without_a_client_the_action_is_disabled_and_explains_why() {
    let (temp, mut app) = app();
    convoy(&mut app, temp.path());
    app.state.repositories = vec![repository("registered")];
    for tab in [Tab::Review, Tab::Activity] {
        app.tab = tab;
        let mut keys = Keyboard::new(|app, ui, ctx| app.results(ui, ctx));
        keys.frame(&mut app, vec![]);
        let nodes = action(&keys);
        assert_eq!(nodes.len(), 1, "one action, never one per location");
        assert!(nodes[0].is_disabled());
    }
    let mut keys = Keyboard::new(|app, ui, ctx| app.editor(ui, ctx));
    keys.frame(&mut app, vec![]);
    assert!(action(&keys).iter().all(|node| node.is_disabled()));
    assert!(app.notice.is_empty());
}

#[test]
fn results_open_each_jobs_recorded_location_in_both_views() {
    let (temp, mut app) = app();
    configure(&mut app);
    let paths = convoy(&mut app, temp.path());
    for tab in [Tab::Review, Tab::Activity] {
        app.tab = tab;
        let mut keys = Keyboard::new(|app, ui, ctx| app.results(ui, ctx));
        // Direct job: the repository the convoy ran in.
        app.selected_job = 0;
        keys.activate(&mut app, ACTION);
        let message = notice(&mut app);
        assert!(
            message.contains(&paths.repository.display().to_string())
                && message.contains("register it again"),
            "{message}"
        );
        // Isolated job: its recorded worktree, never the original repository.
        app.selected_job = 1;
        keys.frame(&mut app, vec![]);
        keys.activate(&mut app, ACTION);
        let message = notice(&mut app);
        assert!(
            message.contains(&paths.worktree.display().to_string())
                && message.contains("isolated worktree no longer exists")
                && !message.contains(&paths.original.display().to_string()),
            "{message}"
        );
    }
}

#[test]
fn repository_management_opens_the_registered_repository() {
    let (temp, mut app) = app();
    configure(&mut app);
    let registered = temp.path().join("registered ü");
    app.state.repositories = vec![Repository {
        name: "registered ü".into(),
        path: registered.clone(),
    }];
    let mut keys = Keyboard::new(|app, ui, ctx| app.editor(ui, ctx));
    keys.activate(&mut app, ACTION);
    let message = notice(&mut app);
    assert!(
        message.contains(&registered.display().to_string())
            && message.contains("register it again"),
        "{message}"
    );
}

#[test]
fn unavailable_results_explain_and_never_substitute_the_repository() {
    let (temp, mut app) = app();
    configure(&mut app);
    convoy(&mut app, temp.path());
    let run = &mut app.state.runs[0];
    run.jobs[1].worktree = None;
    let job = run.jobs[1].clone();
    let target = app
        .job_git_client_target(7, 1, &job)
        .unwrap_err()
        .to_string();
    assert!(
        target.contains("original repository is not opened instead"),
        "{target}"
    );
    app.tab = Tab::Review;
    app.selected_job = 1;
    let mut keys = Keyboard::new(|app, ui, ctx| app.results(ui, ctx));
    keys.frame(&mut app, vec![]);
    assert!(action(&keys)[0].is_disabled());
}

#[test]
fn result_operations_block_only_the_worktree_they_may_change() {
    let (temp, mut app) = app();
    configure(&mut app);
    convoy(&mut app, temp.path());
    let jobs = app.state.runs[0].jobs.clone();
    app.result_operation = Some((7, 1));
    let blocked = app.job_git_client_target(7, 1, &jobs[1]).unwrap_err();
    assert!(blocked.to_string().contains("wait until the current Apply"));
    let direct = app.job_git_client_target(7, 0, &jobs[0]).unwrap();
    assert_eq!(direct.kind, Kind::Repository);
    app.result_operation = None;
    assert_eq!(
        app.job_git_client_target(7, 1, &jobs[1]).unwrap().kind,
        Kind::Worktree
    );
}

#[test]
fn opening_changes_no_run_state_or_scheduling() {
    let (temp, mut app) = app();
    configure(&mut app);
    convoy(&mut app, temp.path());
    app.state.runs[0].jobs[0].status = JobStatus::Running;
    let before = serde_json::to_value(app.state.persistence_snapshot()).unwrap();
    let mut keys = Keyboard::new(|app, ui, ctx| app.results(ui, ctx));
    app.selected_job = 0;
    keys.activate(&mut app, ACTION);
    notice(&mut app);
    assert_eq!(
        serde_json::to_value(app.state.persistence_snapshot()).unwrap(),
        before
    );
    assert!(app.manager.is_idle(), "no convoy work was started");
}
