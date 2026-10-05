#![allow(clippy::unwrap_used)]

use super::*;
use crate::domain::{AgentId, Job, TaskConfig};
use crate::runner::PreparedRepository;

fn app() -> (tempfile::TempDir, App) {
    let temp = tempfile::tempdir().unwrap();
    let store = Store::open(&temp.path().join("app")).unwrap();
    // Intentionally not driven: these lifecycle tests must never spawn agents.
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let app = App::with_context(
        &egui::Context::default(),
        store,
        AppState::default(),
        runtime,
    );
    (temp, app)
}

fn repository(name: &str) -> Repository {
    Repository {
        name: name.into(),
        path: PathBuf::from("/fixtures").join(name),
    }
}

fn run(id: u64, statuses: &[JobStatus]) -> Run {
    Run {
        id,
        created_at: domain::now(),
        task: TaskConfig {
            prompt: format!("Task {id}"),
            ..Default::default()
        },
        jobs: statuses
            .iter()
            .enumerate()
            .map(|(index, status)| {
                let mut job = domain::Job::queued(repository(&format!("repo{index}")));
                job.status = *status;
                job.log.append("session output");
                job
            })
            .collect(),
    }
}

fn prepare(app: &mut App, agent: AgentId) {
    app.state.draft = TaskConfig {
        prompt: "Make a focused change".into(),
        agent,
        concurrency: 3,
        options: [("model".into(), "selected-model".into())].into(),
    };
    app.state.global_concurrency = 6;
    app.state.agent_options.insert(
        AgentId::Claude,
        [("model".into(), "other-preference".into())].into(),
    );
    app.state.repositories = vec![repository("selected"), repository("unselected")];
    app.selected.insert(app.state.repositories[0].path.clone());
    app.prepared = Some(PreparedRun {
        task: app.state.draft.clone(),
        repositories: vec![PreparedRepository {
            repository: app.state.repositories[0].clone(),
            state: WorkingTree {
                summary: Default::default(),
                entries: vec![],
            },
        }],
    });
}

#[test]
fn accepted_launch_clears_only_task_and_selection_and_persists_preferences() {
    for agent in [AgentId::Codex, AgentId::Copilot] {
        let (_temp, mut app) = app();
        prepare(&mut app, agent);
        let expected = serde_json::to_value(app.prepared.as_ref().unwrap().task.clone()).unwrap();
        let preferences = app.state.agent_options.clone();
        let registered = app.state.repositories.clone();
        app.start();
        assert!(app.manager.is_active(1));
        assert!(app.state.draft.prompt.is_empty());
        assert!(app.selected.is_empty());
        assert_eq!(app.state.draft.agent, agent);
        assert_eq!(app.state.draft.options["model"], "selected-model");
        assert_eq!(app.state.draft.concurrency, 3);
        assert_eq!(app.state.global_concurrency, 6);
        assert_eq!(app.state.agent_options, preferences);
        assert_eq!(app.state.repositories, registered);
        assert_eq!(
            serde_json::to_value(&app.state.runs[0].task).unwrap(),
            expected
        );
        assert_eq!(app.state.runs[0].jobs[0].repository, registered[0]);
        assert!(app.focus_draft);
        app.save();
        let restored = app.store.load().unwrap();
        assert!(restored.draft.prompt.is_empty());
        assert_eq!(restored.draft.agent, agent);
        assert_eq!(restored.draft.concurrency, 3);
        assert_eq!(restored.global_concurrency, 6);
        assert_eq!(restored.repositories, registered);
        assert_eq!(
            serde_json::to_value(&restored.runs[0].task).unwrap(),
            expected
        );
    }
}

#[test]
fn failed_preflight_save_backend_or_admission_preserves_draft_and_selection() {
    for failure in ["preflight", "save", "backend", "admission"] {
        let (_temp, mut app) = app();
        prepare(
            &mut app,
            if failure == "backend" {
                AgentId::Claude
            } else {
                AgentId::Codex
            },
        );
        let draft = serde_json::to_value(&app.state.draft).unwrap();
        let selected = app.selected.clone();
        match failure {
            "preflight" => {
                app.prepared = None;
                app.tx
                    .send(Message::Prepared(Err("preflight failed".into())))
                    .unwrap();
                app.poll();
            }
            "save" => {
                std::fs::create_dir(app.store.directory().join("state.json")).unwrap();
                app.start();
            }
            "backend" => app.start(),
            "admission" => {
                app.manager.shutdown();
                app.start();
            }
            _ => unreachable!(),
        }
        assert_eq!(
            serde_json::to_value(&app.state.draft).unwrap(),
            draft,
            "{failure}"
        );
        assert_eq!(app.selected, selected, "{failure}");
        assert!(!app.manager.is_active(1));
        assert!(!app.focus_draft);
        if failure == "admission" {
            assert_eq!(app.state.runs[0].status(), JobStatus::Failed);
        } else {
            assert!(app.state.runs.is_empty());
        }
    }
}

#[test]
fn history_cleanup_is_terminal_only_and_resets_selection_without_touching_repositories() {
    let (temp, mut app) = app();
    let repo_path = temp.path().join("repository");
    std::fs::create_dir_all(repo_path.join(".git")).unwrap();
    std::fs::write(repo_path.join("dirty.txt"), "preserve edits").unwrap();
    std::fs::write(repo_path.join(".git/HEAD"), "ref: refs/heads/preserve\n").unwrap();
    app.state.repositories.push(Repository {
        path: repo_path.clone(),
        name: "repo".into(),
    });
    app.state.draft.prompt = "unrelated draft".into();
    app.selected.insert(repo_path.clone());
    app.state.next_run = 50;
    app.state.runs = vec![
        run(8, &[JobStatus::Succeeded]),
        run(7, &[JobStatus::Failed, JobStatus::Queued]),
        run(6, &[JobStatus::Cancelled]),
        run(5, &[JobStatus::Running]),
        run(4, &[JobStatus::Succeeded, JobStatus::Failed]),
    ];
    app.state.runs[0].jobs[0].repository = app.state.repositories[0].clone();
    app.state.runs[2].jobs[0].interrupted = true;
    app.selected_run = Some(8);
    app.selected_job = 1;
    app.diff = Some(Ok("old diff".into()));
    app.diff_target = Some(repo_path.clone());
    app.remove_history(Some(7)); // Even partially failed convoys remain active.
    assert_eq!(app.state.runs.len(), 5);
    app.remove_history(Some(8));
    assert_eq!(app.selected_run, Some(7));
    assert_eq!(app.selected_job, 0);
    assert!(app.diff.is_none() && app.diff_target.is_none());
    app.tx
        .send(Message::Diff(repo_path.clone(), Ok("late diff".into())))
        .unwrap();
    app.poll();
    assert!(app.diff.is_none());
    app.remove_history(None);
    assert_eq!(
        app.state.runs.iter().map(|r| r.id).collect::<Vec<_>>(),
        vec![7, 5]
    );
    assert_eq!(app.selected_run, Some(7));
    assert!(app.state.runs.iter().all(Run::active));
    assert_eq!(app.state.runs[0].jobs[0].log.text, "session output");
    assert_eq!(app.state.draft.prompt, "unrelated draft");
    assert_eq!(app.selected, [repo_path.clone()].into());
    assert_eq!(app.state.next_run, 50);
    let loaded = app.store.load().unwrap();
    assert_eq!(
        loaded.runs.iter().map(|r| r.id).collect::<Vec<_>>(),
        vec![7, 5]
    );
    assert_eq!(loaded.repositories, app.state.repositories);
    assert_eq!(
        std::fs::read_to_string(repo_path.join("dirty.txt")).unwrap(),
        "preserve edits"
    );
    assert_eq!(
        std::fs::read_to_string(repo_path.join(".git/HEAD")).unwrap(),
        "ref: refs/heads/preserve\n"
    );
    // After the jobs become terminal, removing the last histories has an empty fallback.
    app.state.recover_interrupted();
    app.remove_history(Some(7));
    assert_eq!(app.selected_run, Some(5));
    app.remove_history(None);
    assert!(app.selected_run.is_none());
    assert!(app.state.runs.is_empty());
    assert!(app.store.load().unwrap().runs.is_empty());
    app.remove_history(Some(999));
}

#[test]
fn reuse_copies_full_configuration_and_registered_selection_without_starting_jobs() {
    let (_temp, mut app) = app();
    prepare(&mut app, AgentId::Codex);
    app.prepared = None;
    let original_options = app.state.draft.options.clone();
    let mut history = run(20, &[JobStatus::Cancelled]);
    history.task = TaskConfig {
        prompt: "Historical task".into(),
        agent: AgentId::Copilot,
        concurrency: 1,
        options: [("tool_approvals".into(), "file-edits".into())].into(),
    };
    history.jobs[0].repository = app.state.repositories[1].clone();
    history
        .jobs
        .push(Job::queued(repository("no-longer-registered")));
    history.jobs[1].finish(JobStatus::Cancelled, None, String::new());
    let expected = serde_json::to_value(&history).unwrap();
    app.state.runs.push(history);
    app.reuse_convoy(20);
    assert_eq!(app.state.draft.prompt, "Historical task");
    assert_eq!(app.state.draft.agent, AgentId::Copilot);
    assert_eq!(app.state.draft.options["tool_approvals"], "file-edits");
    assert_eq!(app.state.draft.concurrency, 1);
    assert_eq!(app.state.global_concurrency, 6);
    assert_eq!(
        app.selected,
        [app.state.repositories[1].path.clone()].into()
    );
    assert_eq!(app.state.repositories.len(), 2);
    assert!(app.draft_message.contains("1 unregistered repository"));
    assert!(app.manager.is_idle());
    assert_eq!(app.state.next_run, 1);
    assert!(app.prepared.is_none());
    app.state.draft.prompt = "New edit".into();
    assert_eq!(serde_json::to_value(&app.state.runs[0]).unwrap(), expected);
    app.state.select_agent(AgentId::Codex);
    assert_eq!(app.state.draft.options, original_options);
    // A reuse click cannot replace a snapshot currently under review.
    prepare(&mut app, AgentId::Codex);
    app.reuse_convoy(20);
    assert_eq!(app.state.draft.prompt, "Make a focused change");
}
