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

#[test]
fn folder_results_only_fill_the_draft_and_do_not_release_git_work() {
    let (_temp, mut app) = app();
    app.dirty = false;
    app.busy = true; // A refresh may complete independently of the picker.
    app.repository_input = "manually entered path".into();
    app.notice = "Existing diagnostic".into();
    let state = serde_json::to_value(&app.state).unwrap();

    for selection in [Some(PathBuf::from("/chosen/repo with spaces")), None] {
        app.browsing_repository = true;
        app.tx.send(Message::RepositoryFolder(selection)).unwrap();
        app.poll();
        assert_eq!(app.repository_input, "/chosen/repo with spaces");
        assert!(!app.browsing_repository);
        assert!(app.focus_repository_input);
        assert!(app.busy);
        assert!(!app.dirty);
        assert!(app.selected.is_empty());
        assert!(app.repository_states.is_empty());
        assert!(app.manager.is_idle());
        assert_eq!(serde_json::to_value(&app.state).unwrap(), state);
        assert_eq!(app.notice, "Existing diagnostic");
    }

    // Keep edits made while a portal is open, including whitespace, on cancel.
    app.repository_input = "  another manual path  ".into();
    app.tx.send(Message::RepositoryFolder(None)).unwrap();
    app.poll();
    assert_eq!(app.repository_input, "  another manual path  ");
}

#[cfg(unix)]
#[test]
fn unrepresentable_folder_path_does_not_silently_select_a_different_directory() {
    use std::os::unix::ffi::OsStringExt;
    let (_temp, mut app) = app();
    app.repository_input = "keep this path".into();
    app.browsing_repository = true;
    let path = std::ffi::OsString::from_vec(b"/tmp/invalid-\xff".to_vec());
    app.tx
        .send(Message::RepositoryFolder(Some(path.into())))
        .unwrap();
    app.poll();
    assert_eq!(app.repository_input, "keep this path");
    assert!(!app.browsing_repository);
    assert!(app.notice.contains("Unicode path"));
    assert!(app.state.repositories.is_empty());
}

fn add_repository_and_wait(app: &mut App) {
    app.register(egui::Context::default());
    assert!(app.busy);
    let message = app.runtime.block_on(async {
        tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                if let Ok(message) = app.rx.try_recv() {
                    break message;
                }
                tokio::time::sleep(Duration::from_millis(1)).await;
            }
        })
        .await
        .unwrap()
    });
    assert!(matches!(message, Message::Registered(_)));
    app.tx.send(message).unwrap();
    app.poll();
    assert!(!app.busy);
}

#[test]
fn selected_and_manual_paths_share_explicit_registration_and_git_validation() {
    let (temp, mut app) = app();
    let valid = temp.path().join("repository with spaces ü");
    assert!(
        std::process::Command::new("git")
            .args(["init", "--quiet"])
            .arg(&valid)
            .output()
            .unwrap()
            .status
            .success()
    );
    let untouched = valid.join("untracked.txt");
    std::fs::write(&untouched, "Preserve existing changes").unwrap();
    let non_git = temp.path().join("ordinary folder");
    std::fs::create_dir(&non_git).unwrap();
    let nested = valid.join("subdirectory");
    std::fs::create_dir(&nested).unwrap();

    app.tx
        .send(Message::RepositoryFolder(Some(valid.clone())))
        .unwrap();
    app.poll();
    assert!(app.state.repositories.is_empty());
    assert!(!app.busy);
    add_repository_and_wait(&mut app);
    assert_eq!(app.state.repositories.len(), 1);
    assert_eq!(
        app.state.repositories[0].path,
        valid.canonicalize().unwrap()
    );
    assert!(app.repository_input.is_empty());
    assert!(app.notice.is_empty());

    for path in [&valid, &non_git, &nested] {
        app.tx
            .send(Message::RepositoryFolder(Some(path.clone())))
            .unwrap();
        app.poll();
        add_repository_and_wait(&mut app);
        assert_eq!(app.state.repositories.len(), 1);
        assert_eq!(app.repository_input, path.to_str().unwrap());
        assert!(!app.notice.is_empty());
        if path == &valid {
            assert_eq!(app.notice, "This repository is already registered.");
        }
    }

    app.repository_input = temp.path().join("does not exist").display().to_string();
    let manual = app.repository_input.clone();
    add_repository_and_wait(&mut app);
    assert_eq!(app.repository_input, manual);
    assert!(app.notice.contains("Repository path does not exist"));
    assert_eq!(app.state.repositories.len(), 1);

    // The original manual whitespace and canonical duplicate behavior remains.
    app.repository_input = format!("  {}  ", valid.join(".").display());
    add_repository_and_wait(&mut app);
    assert_eq!(app.notice, "This repository is already registered.");
    assert_eq!(app.state.repositories.len(), 1);
    app.state.repositories.clear();
    add_repository_and_wait(&mut app);
    assert_eq!(app.state.repositories.len(), 1);
    assert!(app.repository_input.is_empty());
    assert!(app.notice.is_empty());
    assert_eq!(
        std::fs::read_to_string(untouched).unwrap(),
        "Preserve existing changes"
    );
    assert!(app.manager.is_idle());
}

#[cfg(unix)]
#[test]
fn picked_paths_with_trailing_whitespace_are_not_trimmed_to_another_repository() {
    let (temp, mut app) = app();
    let picked = temp.path().join("repo ");
    let other = temp.path().join("repo");
    for path in [&picked, &other] {
        assert!(
            std::process::Command::new("git")
                .args(["init", "--quiet"])
                .arg(path)
                .output()
                .unwrap()
                .status
                .success()
        );
    }
    app.tx
        .send(Message::RepositoryFolder(Some(picked.clone())))
        .unwrap();
    app.poll();
    add_repository_and_wait(&mut app);
    assert_eq!(
        app.state.repositories[0].path,
        picked.canonicalize().unwrap()
    );
    assert!(app.picked_repository_path.is_none());

    // Editing the field after a pick must use the edited value, not a stale pick.
    app.tx
        .send(Message::RepositoryFolder(Some(picked)))
        .unwrap();
    app.poll();
    app.repository_input = other.display().to_string();
    add_repository_and_wait(&mut app);
    assert_eq!(app.state.repositories.len(), 2);
    assert_eq!(
        app.state.repositories[1].path,
        other.canonicalize().unwrap()
    );
}

#[test]
fn bulk_selection_includes_dirty_and_nested_repositories_without_mutating_them() {
    let (_temp, mut app) = app();
    for count in [0, 1, 5, 10, 24, MAX_REPOSITORIES] {
        app.state.repositories = (0..count)
            .map(|i| repository(&format!("parent/repo{i}")))
            .collect();
        let registered = app.state.repositories.clone();
        app.selected.insert("/stale".into());
        app.select_repositories(true);
        assert_eq!(app.selected.len(), count);
        assert!(registered.iter().all(|r| app.selected.contains(&r.path)));
        app.select_repositories(false);
        assert!(app.selected.is_empty());
        assert_eq!(app.state.repositories, registered);
        assert!(app.manager.is_idle());
    }
}

#[test]
fn cli_check_is_configuration_scoped_and_does_not_release_repository_work() {
    let (_temp, mut app) = app();
    app.busy = true; // Repository refresh is independently in progress.
    let checked = app.state.draft.options.clone();
    app.checking_cli = Some((AgentId::Codex, checked.clone()));
    assert!(app.current_cli_check());
    app.state.select_agent(AgentId::Claude);
    assert!(!app.current_cli_check());
    app.tx
        .send(Message::Detected(
            AgentId::Codex,
            checked,
            Ok("fixture".into()),
        ))
        .unwrap();
    app.poll();
    assert!(app.busy);
    assert!(app.checking_cli.is_none());
    assert!(app.current_detection().is_none());
    app.state.select_agent(AgentId::Codex);
    assert!(app.current_detection().is_some());
}

#[test]
fn editor_scroll_and_execution_fit_small_and_large_panes() {
    let (_temp, mut app) = app();
    let ctx = egui::Context::default();
    theme::install(&ctx);
    app.state.repositories = (0..24)
        .map(|i| repository(&format!("long-repository-name-{i}")))
        .collect();
    for agent in AgentId::ALL {
        app.state.select_agent(agent);
        for (width, height) in [(310.0, 450.0), (360.0, 710.0), (520.0, 890.0)] {
            // The footer measures once and reserves its actual height next frame.
            for frame in 0..3 {
                let mut output = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::vec2(width, height),
                        )),
                        ..Default::default()
                    },
                    |ui| {
                        egui::CentralPanel::default()
                            .frame(egui::Frame::NONE.inner_margin(theme::PANEL_MARGIN))
                            .show(ui, |ui| {
                                let bounds = ui.available_rect_before_wrap();
                                let result = ui.scope(|ui| app.editor_pane(ui, &ctx));
                                if frame > 0 {
                                    assert!(
                                        result.response.rect.right() <= bounds.right() + 1.0,
                                        "{agent:?} width {width}"
                                    );
                                    assert!(
                                        result.response.rect.bottom() <= bounds.bottom() + 1.0,
                                        "{agent:?} height {height}"
                                    );
                                }
                            });
                    },
                );
                output.textures_delta.clear();
            }
        }
    }
}

#[test]
fn result_actions_fit_the_narrowest_allowed_results_pane() {
    let (_temp, mut app) = app();
    let ctx = egui::Context::default();
    theme::install(&ctx);
    app.state.runs = vec![run(
        1,
        &[JobStatus::Running, JobStatus::Queued, JobStatus::Succeeded],
    )];
    app.selected_run = Some(1);
    for width in [350.0, 600.0, 1100.0] {
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(width, 720.0),
                )),
                ..Default::default()
            },
            |ui| {
                egui::CentralPanel::default()
                    .frame(egui::Frame::NONE.inner_margin(theme::PANEL_MARGIN))
                    .show(ui, |ui| {
                        let right = ui.available_rect_before_wrap().right();
                        let response = ui.scope(|ui| app.results(ui, &ctx));
                        assert!(
                            response.response.rect.right() <= right + 1.0,
                            "results width {width}"
                        );
                    });
            },
        );
        output.textures_delta.clear();
    }
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
        options: [(
            "model".into(),
            if agent == AgentId::OpenCode {
                "provider/selected-model"
            } else {
                "selected-model"
            }
            .into(),
        )]
        .into(),
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
    for agent in AgentId::ALL {
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
        assert_eq!(app.state.draft.options, app.state.runs[0].task.options);
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
fn failed_preflight_save_or_admission_preserves_draft_and_selection() {
    for failure in ["preflight", "save", "admission"] {
        let (_temp, mut app) = app();
        prepare(&mut app, AgentId::Codex);
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

#[test]
fn opencode_reuse_restores_backend_options_without_launching_or_mutating_history() {
    let (_temp, mut app) = app();
    prepare(&mut app, AgentId::Codex);
    app.prepared = None;
    let mut history = run(30, &[JobStatus::Succeeded]);
    history.task.agent = AgentId::OpenCode;
    history.task.options = [
        ("model".into(), "provider/model".into()),
        ("agent".into(), "build".into()),
        ("variant".into(), "high".into()),
        ("permissions".into(), "auto".into()),
    ]
    .into();
    history.jobs[0].repository = app.state.repositories[0].clone();
    let expected = serde_json::to_value(&history.task).unwrap();
    app.state.runs.push(history);
    app.reuse_convoy(30);
    assert_eq!(serde_json::to_value(&app.state.draft).unwrap(), expected);
    assert_eq!(
        app.selected,
        [app.state.repositories[0].path.clone()].into()
    );
    assert!(app.manager.is_idle() && app.prepared.is_none());
    assert_eq!(app.state.global_concurrency, 6);
    app.state
        .draft
        .options
        .insert("variant".into(), "low".into());
    assert_eq!(
        serde_json::to_value(&app.state.runs[0].task).unwrap(),
        expected
    );
    app.save();
    assert_eq!(app.store.load().unwrap().draft.options["variant"], "low");
}

#[test]
fn backend_controls_fit_compact_editor_in_both_themes_and_detection_is_snapshot_scoped() {
    let (_temp, mut app) = app();
    let ctx = egui::Context::default();
    theme::install(&ctx);
    for dark in [false, true] {
        ctx.set_visuals(if dark {
            egui::Visuals::dark()
        } else {
            egui::Visuals::light()
        });
        for agent in AgentId::ALL {
            app.state.select_agent(agent);
            for width in [310.0, 360.0, 520.0] {
                // Include margins used by the real left pane; widgets must fit
                // horizontally even when the vertical editor needs scrolling.
                let mut output = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::vec2(width, 820.0),
                        )),
                        ..Default::default()
                    },
                    |ui| {
                        egui::CentralPanel::default()
                            .frame(egui::Frame::NONE.inner_margin(theme::PANEL_MARGIN))
                            .show(ui, |ui| {
                                let available = ui.available_width();
                                let inner = ui.scope(|ui| app.editor(ui, &ctx));
                                assert!(
                                    inner.response.rect.width() <= available + 1.0,
                                    "{} at width {width}: {} > {available}",
                                    agent.label(),
                                    inner.response.rect.width()
                                );
                            });
                    },
                );
                output.textures_delta.clear();
                assert!(!output.shapes.is_empty());
            }
        }
    }
    app.state.select_agent(AgentId::OpenCode);
    app.detection = Some((
        AgentId::OpenCode,
        app.state.draft.options.clone(),
        Ok("version detected".into()),
    ));
    assert!(app.current_detection().is_some());
    app.state.select_agent(AgentId::Codex);
    assert!(app.current_detection().is_none());
    app.state.select_agent(AgentId::OpenCode);
    assert!(app.current_detection().is_some());
    app.state
        .draft
        .options
        .insert("agent".into(), "plan".into());
    assert!(app.current_detection().is_none());
}
#[test]
fn claude_reuse_restores_backend_options_without_launching_or_mutating_history() {
    let (_temp, mut app) = app();
    prepare(&mut app, AgentId::Codex);
    app.prepared = None;
    let mut history = run(30, &[JobStatus::Succeeded]);
    history.task.agent = AgentId::Claude;
    history.task.options = [
        ("executable".into(), "claude".into()),
        ("model".into(), "sonnet".into()),
        ("max_turns".into(), "10".into()),
        ("effort".into(), "high".into()),
        ("permission_mode".into(), "acceptEdits".into()),
    ]
    .into();
    history.jobs[0].repository = app.state.repositories[0].clone();
    let expected = serde_json::to_value(&history.task).unwrap();
    app.state.runs.push(history);
    app.reuse_convoy(30);
    assert_eq!(serde_json::to_value(&app.state.draft).unwrap(), expected);
    assert_eq!(
        app.selected,
        [app.state.repositories[0].path.clone()].into()
    );
    assert!(app.manager.is_idle() && app.prepared.is_none());
    assert_eq!(app.state.global_concurrency, 6);
    app.state
        .draft
        .options
        .insert("effort".into(), "low".into());
    assert_eq!(
        serde_json::to_value(&app.state.runs[0].task).unwrap(),
        expected
    );
    app.save();
    assert_eq!(app.store.load().unwrap().draft.options["effort"], "low");
}
