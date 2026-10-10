#![allow(clippy::unwrap_used)]

use super::*;
use crate::domain::{AgentId, Job, TaskConfig};
use crate::runner::PreparedRepository;

#[path = "bulk_discard_tests.rs"]
mod bulk_discard;
#[path = "continuation_tests.rs"]
mod continuation;
#[path = "desktop_tests.rs"]
mod desktop;
#[path = "notification_tests.rs"]
mod notifications;
#[path = "task_context_tests.rs"]
mod task_context;
#[path = "visibility_tests.rs"]
mod visibility;

fn app() -> (tempfile::TempDir, App) {
    app_with_state(AppState::default())
}

fn app_with_state(mut state: AppState) -> (tempfile::TempDir, App) {
    let temp = tempfile::tempdir().unwrap();
    let store = Store::open(&temp.path().join("app")).unwrap();
    // Intentionally not driven: these lifecycle tests must never spawn agents.
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    for agent in AgentId::ALL {
        let options = [(
            "executable".into(),
            temp.path().join("missing-cli").display().to_string(),
        )]
        .into();
        state.agent_options.insert(agent, options);
    }
    state.draft.options = state.agent_options[&state.draft.agent].clone();
    let app = App::with_context(&egui::Context::default(), store, state, runtime);
    (temp, app)
}

#[test]
fn group_selection_is_explicit_deduplicated_and_missing_members_remain_repairable() {
    let (_temp, mut app) = app();
    app.state.repositories = vec![repository("a"), repository("b"), repository("c")];
    let paths: Vec<_> = app
        .state
        .repositories
        .iter()
        .map(|r| r.path.clone())
        .collect();
    let missing = repository("missing").path;
    app.state.groups = vec![
        domain::RepositoryGroup {
            name: "First".into(),
            repositories: vec![paths[0].clone(), paths[1].clone(), missing.clone()],
        },
        domain::RepositoryGroup {
            name: "Overlap".into(),
            repositories: vec![paths[1].clone(), paths[2].clone()],
        },
    ];
    app.repository_states
        .insert(paths[2].clone(), Err("unavailable".into()));
    app.select_group(0, true);
    assert_eq!(app.selected, [paths[0].clone(), paths[1].clone()].into());
    assert!(app.notice.contains("Membership is retained"));
    app.select_group(1, true);
    assert_eq!(app.selected.len(), 2);
    assert!(app.state.groups[0].repositories.contains(&missing));
    assert!(app.state.groups[1].repositories.contains(&paths[2]));
    app.repository_states.remove(&paths[2]);
    app.select_group(1, true);
    assert_eq!(app.selected.len(), 3);
    app.selected.remove(&paths[0]); // Individual edits have no hidden group state.
    app.select_group(0, false);
    assert_eq!(app.selected, [paths[2].clone()].into());
    app.state.groups.clear();
    assert_eq!(app.selected, [paths[2].clone()].into());
    assert_eq!(app.state.repositories.len(), 3);
    assert!(app.manager.is_idle());
}

#[test]
fn template_load_changes_only_task_text_and_never_launches_or_mutates_history() {
    let (temp, mut app) = app();
    prepare(&mut app, AgentId::Copilot);
    app.state.draft.execution_mode = domain::ExecutionMode::IsolatedWorktree;
    app.prepared = None;
    let path = temp.path().join("context.md");
    std::fs::write(&path, "specification").unwrap();
    app.state
        .draft
        .attachments
        .push(crate::attachments::Attachment::inspect(&path).unwrap());
    app.state.templates.push(domain::TaskTemplate {
        name: "Review".into(),
        prompt: "Review README".into(),
    });
    app.state.runs.push(run(20, &[JobStatus::Succeeded]));
    let before = serde_json::to_value(&app.state).unwrap();
    let selection = app.selected.clone();
    app.load_template(0);
    let mut expected = before;
    expected["draft"]["prompt"] = "Review README".into();
    assert_eq!(serde_json::to_value(&app.state).unwrap(), expected);
    assert_eq!(app.selected, selection);
    assert!(app.focus_draft && app.dirty);
    assert!(app.manager.is_idle());
    assert!(app.prepared.is_none());
    app.state.draft.prompt = "User edit".into();
    assert_eq!(app.state.templates[0].prompt, "Review README");
}

#[test]
fn reused_attachment_validation_is_async_visible_and_stale_results_cannot_replace_draft() {
    let (temp, mut app) = app();
    let valid_path = temp.path().join("valid.md");
    let missing_path = temp.path().join("missing.txt");
    std::fs::write(&valid_path, "valid").unwrap();
    std::fs::write(&missing_path, "missing").unwrap();
    let files: Vec<_> = [&valid_path, &missing_path]
        .into_iter()
        .map(|p| crate::attachments::Attachment::inspect(p).unwrap())
        .collect();
    let mut history = run(20, &[JobStatus::Succeeded]);
    history.task.attachments = files.clone();
    app.state.runs.push(history);
    std::fs::remove_file(&missing_path).unwrap();
    app.reuse_convoy(20);
    assert!(app.attachment_work.pending);
    app.preflight(egui::Context::default());
    assert!(!app.busy && app.prepared.is_none());
    let messages = app.runtime.as_ref().unwrap().block_on(async {
        tokio::time::timeout(Duration::from_secs(5), async {
            let mut messages = Vec::new();
            loop {
                if let Ok(message) = app.rx.try_recv() {
                    let done = matches!(message, Message::ReusedAttachments(..));
                    messages.push(message);
                    if done {
                        return messages;
                    }
                } else {
                    tokio::time::sleep(Duration::from_millis(2)).await;
                }
            }
        })
        .await
        .unwrap()
    });
    for message in messages {
        app.tx.send(message).unwrap();
    }
    app.poll();
    assert!(!app.attachment_work.pending);
    assert_eq!(app.state.draft.attachments, files);
    assert!(app.notice.contains("missing.txt") && app.draft_message.contains("need attention"));
    assert_eq!(app.state.runs[0].task.attachments, files);
    app.attachment_work.invalidate();
    app.attachments_added(0, vec![files[1].clone()], vec![]);
    app.attachments_reused(0, vec![], vec![]);
    assert_eq!(app.state.draft.attachments, files);
    app.preflight(egui::Context::default());
    assert!(!app.busy && app.prepared.is_none());
    assert!(app.attachment_error().unwrap().contains("missing.txt"));
    assert!(app.manager.is_idle());
}

#[test]
fn new_library_and_attachment_controls_fit_compact_editor_in_both_themes() {
    let (temp, mut app) = app();
    app.state.repositories = vec![repository("registered")];
    app.state.groups.push(domain::RepositoryGroup {
        name: "A very long repository group name that must fit the compact editor".into(),
        repositories: vec![
            app.state.repositories[0].path.clone(),
            repository("unregistered").path,
        ],
    });
    let path = temp
        .path()
        .join("A very long attachment filename Grüße $(echo nope) that needs truncation.png");
    std::fs::write(&path, include_bytes!("../../assets/codeconvoy-256.png")).unwrap();
    app.state
        .draft
        .attachments
        .push(crate::attachments::Attachment::inspect(&path).unwrap());
    let ctx = egui::Context::default();
    theme::install(&ctx);
    for dark in [false, true] {
        ctx.set_visuals(if dark {
            egui::Visuals::dark()
        } else {
            egui::Visuals::light()
        });
        for width in [310.0, 360.0, 520.0] {
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
                            let width = ui.available_width();
                            let rendered = ui.scope(|ui| app.editor(ui, &ctx));
                            assert!(
                                rendered.response.rect.width() <= width + 1.0,
                                "{} > {width}",
                                rendered.response.rect.width()
                            );
                        });
                },
            );
            output.textures_delta.clear();
        }
    }
}

#[test]
fn idle_close_needs_no_confirmation_and_closes_launch_admission() {
    let (_temp, mut app) = app();
    app.state.runs.push(run(40, &[JobStatus::Succeeded]));
    assert!(app.request_quit());
    assert!(!app.quit_requested);
    assert!(app.closing && app.exit_ready);
    prepare(&mut app, AgentId::Codex);
    app.start();
    assert_eq!(app.state.runs.len(), 1);
    assert!(app.manager.is_idle());
}

#[test]
fn running_or_queued_close_is_a_single_side_effect_free_decision() {
    for status in [JobStatus::Running, JobStatus::Queued, JobStatus::Preparing] {
        let (_temp, mut app) = app();
        prepare(&mut app, AgentId::Codex);
        app.start(); // Undriven manager: no Git commands or agent processes yet.
        app.state.runs[0].jobs[0].status = status;
        app.state.runs.push(run(40, &[JobStatus::Succeeded]));
        let before = serde_json::to_value(&app.state).unwrap();
        for _ in 0..3 {
            assert!(!app.request_quit());
            assert!(app.quit_requested && !app.closing);
        }
        app.cancel_quit();
        assert!(!app.quit_requested && !app.closing);
        assert!(app.manager.is_active(1));
        assert_eq!(serde_json::to_value(&app.state).unwrap(), before);

        // Driving these deliberately nonexistent fixture repositories fails Git
        // validation. Cancelled instead would expose a signalled manager token.
        let events = app.runtime.as_ref().unwrap().block_on(async {
            tokio::time::timeout(Duration::from_secs(5), async {
                let mut events = Vec::new();
                while !events
                    .iter()
                    .any(|event| matches!(event, Event::Finished { .. }))
                {
                    events.push(app.events_rx.recv().await.unwrap());
                }
                events
            })
            .await
            .unwrap()
        });
        assert!(events.iter().any(|event| matches!(
            event,
            Event::Finished {
                status: JobStatus::Failed,
                ..
            }
        )));
        assert!(!events.iter().any(|event| matches!(
            event,
            Event::Finished {
                status: JobStatus::Cancelled,
                ..
            }
        )));
    }
}

#[test]
fn close_observes_manager_work_even_before_ui_metadata_arrives() {
    let (_temp, mut app) = app();
    prepare(&mut app, AgentId::Codex);
    let prepared = app.prepared.take().unwrap();
    app.manager
        .start(1, prepared, agents::backend(AgentId::Codex).unwrap())
        .unwrap();
    assert!(app.state.runs.is_empty());
    assert!(!app.request_quit());
    assert!(app.quit_requested);
}

#[test]
fn confirmed_quit_drains_and_persists_terminal_jobs_preserving_history() {
    let (_temp, mut app) = app();
    prepare(&mut app, AgentId::Codex);
    app.start();
    app.state
        .runs
        .push(run(40, &[JobStatus::Succeeded, JobStatus::Failed]));
    let history = serde_json::to_value(&app.state.runs[1]).unwrap();
    assert!(!app.request_quit());
    app.confirm_quit();
    app.confirm_quit();
    assert!(!app.request_quit()); // Cannot exit while cleanup is pending.
    prepare(&mut app, AgentId::Codex); // Includes a late preflight result.
    let before = serde_json::to_value(&app.state).unwrap();
    app.start();
    assert_eq!(serde_json::to_value(&app.state).unwrap(), before);
    app.runtime.as_ref().unwrap().block_on(async {
        tokio::time::timeout(Duration::from_secs(5), &mut app.manager.join)
            .await
            .unwrap()
            .unwrap();
    });
    // handle_close must drain Finished itself, even before the next poll.
    app.handle_close(&egui::Context::default());
    assert!(app.exit_ready);
    let saved = app.store.load().unwrap();
    assert_eq!(saved.runs[0].status(), JobStatus::Cancelled);
    assert!(!saved.runs[0].jobs[0].interrupted); // Cleanup completion was persisted.
    assert_eq!(serde_json::to_value(&saved.runs[1]).unwrap(), history);
    assert!(app.request_quit());
}

#[test]
fn close_event_is_cancelled_until_confirmation_and_escape_is_safe() {
    let (_temp, mut app) = app();
    app.state
        .runs
        .push(run(1, &[JobStatus::Running, JobStatus::Queued]));
    let before = serde_json::to_value(&app.state).unwrap();
    let ctx = egui::Context::default();
    let mut input = egui::RawInput::default();
    input
        .viewports
        .get_mut(&egui::ViewportId::ROOT)
        .unwrap()
        .events
        .push(egui::ViewportEvent::Close);
    let mut output = ctx.run_ui(input, |ui| {
        app.handle_close(ui.ctx());
        app.quit_window(ui.ctx());
    });
    output.textures_delta.clear();
    assert!(
        output.viewport_output[&egui::ViewportId::ROOT]
            .commands
            .contains(&egui::ViewportCommand::CancelClose)
    );
    assert!(ctx.memory(|m| m.focused().is_some())); // Cancel receives visible keyboard focus.
    let input = egui::RawInput {
        events: vec![egui::Event::Key {
            key: egui::Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::default(),
        }],
        ..Default::default()
    };
    ctx.run_ui(input, |ui| app.quit_window(ui.ctx()))
        .textures_delta
        .clear();
    assert!(!app.quit_requested && !app.closing);
    assert_eq!(serde_json::to_value(&app.state).unwrap(), before);
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

#[test]
fn hidden_window_close_is_cancelled_without_a_ui_pass() {
    let (_temp, mut app) = app();
    app.state.runs.push(run(1, &[JobStatus::Queued]));
    let ctx = egui::Context::default();
    let mut input = egui::RawInput::default();
    input
        .viewports
        .get_mut(&egui::ViewportId::ROOT)
        .unwrap()
        .events
        .push(egui::ViewportEvent::Close);
    let output = ctx.run_logic(&input, |ctx| app.handle_close(ctx));
    let commands = &output.viewport_commands[&egui::ViewportId::ROOT];
    assert!(commands.contains(&egui::ViewportCommand::CancelClose));
    assert!(commands.contains(&egui::ViewportCommand::Minimized(false)));
    assert!(app.quit_requested && !app.closing);
    assert_eq!(app.state.runs[0].status(), JobStatus::Queued);
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
    let message = app.runtime.as_ref().unwrap().block_on(async {
        tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                if let Ok(message) = app.rx.try_recv()
                    && matches!(message, Message::Registered(_))
                {
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
fn startup_cli_checks_are_independent_and_do_not_release_repository_work() {
    let (_temp, mut app) = app();
    app.busy = true; // Repository refresh is independently in progress.
    for agent in AgentId::ALL {
        assert!(app.cli_checks.checking(agent));
    }
    app.state.select_agent(AgentId::Claude);
    assert!(app.current_cli_check());
    let deadline = Instant::now() + Duration::from_secs(5);
    while app.cli_checks.any_checking() {
        assert!(Instant::now() < deadline);
        app.runtime.as_ref().unwrap().block_on(async {
            tokio::time::sleep(Duration::from_millis(5)).await;
        });
        app.poll();
    }
    assert!(app.busy);
    assert!(!app.cli_checks.any_checking());
    assert!(
        app.current_detection()
            .unwrap()
            .as_ref()
            .unwrap_err()
            .missing
    );
    app.state.select_agent(AgentId::Codex);
    assert!(app.current_detection().is_some());
    assert!(app.notice.is_empty());
    app.check_cli(egui::Context::default());
    assert!(app.current_cli_check());
    assert!(app.current_detection().is_none());
}

fn complete_cli_checks(app: &mut App) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while app.cli_checks.any_checking() {
        assert!(Instant::now() < deadline);
        app.runtime.as_ref().unwrap().block_on(async {
            tokio::time::sleep(Duration::from_millis(5)).await;
        });
        app.poll();
    }
}

#[test]
fn preflight_reconciles_executable_changes_before_snapshotting() {
    let (temp, mut app) = app();
    app.state.draft.prompt = "Review the repository".into();
    app.preflight(egui::Context::default());
    assert!(app.current_cli_check());
    assert!(!app.busy && app.prepared.is_none());

    complete_cli_checks(&mut app);
    app.state.draft.options.insert(
        "executable".into(),
        temp.path().join("new-codex").display().to_string(),
    );
    // An edit must gate preflight even before the next poll/render cycle.
    app.preflight(egui::Context::default());
    assert!(app.current_cli_check());
    assert!(!app.busy && app.prepared.is_none());
    assert_eq!(app.state.draft.prompt, "Review the repository");
    assert!(app.state.runs.is_empty() && app.manager.is_idle());

    complete_cli_checks(&mut app);
    app.check_cli(egui::Context::default());
    app.preflight(egui::Context::default());
    assert!(app.current_cli_check());
    assert!(!app.busy && app.prepared.is_none());
}

#[cfg(unix)]
#[test]
fn preflight_waits_for_discovered_cli_before_snapshotting() {
    use std::os::unix::fs::{PermissionsExt, symlink};

    const ROOT: &str = "CODECONVOY_TEST_PREFLIGHT_DISCOVERY_ROOT";
    let Ok(root) = std::env::var(ROOT) else {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path();
        for name in ["bin", "path", "repo"] {
            std::fs::create_dir(root.join(name)).unwrap();
        }
        let cli = root.join("bin/codex");
        // Only help is supported; this fixture cannot run an agent task.
        std::fs::write(
            &cli,
            "#!/bin/sh\n[ \"$1\" = '--help' ] || exit 2\n: > \"$0.started\"\nwhile [ ! -f \"$0.release\" ]; do /bin/sleep 0.01; done\nprintf '%s\\n' 'Codex exec --no-daemon --ask-for-approval'\n",
        )
        .unwrap();
        std::fs::set_permissions(&cli, std::fs::Permissions::from_mode(0o755)).unwrap();
        let git = std::env::split_paths(&std::env::var_os("PATH").unwrap())
            .map(|path| path.join("git"))
            .find(|path| path.is_file())
            .unwrap()
            .canonicalize()
            .unwrap();
        symlink(&git, root.join("path/git")).unwrap();
        assert!(
            std::process::Command::new(&git)
                .args(["init", "--quiet"])
                .current_dir(root.join("repo"))
                .status()
                .unwrap()
                .success()
        );
        // Isolate GUI-style PATH changes from all other tests. Git is available,
        // but the CLI is discoverable only in the common installation directory.
        assert!(
            std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "ui::tests::preflight_waits_for_discovered_cli_before_snapshotting",
                    "--nocapture",
                ])
                .env(ROOT, root)
                .env("PATH", root.join("path"))
                .env("XDG_BIN_DIR", root.join("bin"))
                .status()
                .unwrap()
                .success()
        );
        return;
    };
    let root = PathBuf::from(root);
    let store = Store::open(&root.join("app")).unwrap();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let mut state = AppState::default();
    for agent in [AgentId::Copilot, AgentId::OpenCode, AgentId::Claude] {
        state.agent_options.insert(
            agent,
            [(
                "executable".into(),
                root.join("missing").display().to_string(),
            )]
            .into(),
        );
    }
    let repository = runtime.block_on(git::register(&root.join("repo"))).unwrap();
    state.repositories.push(repository.clone());
    state.draft.prompt = "Review the repository".into();
    let ctx = egui::Context::default();
    let mut app = App::with_context(&ctx, store, state, runtime);
    app.selected.insert(repository.path.clone());
    let deadline = Instant::now() + Duration::from_secs(5);
    while !root.join("bin/codex.started").exists() || app.busy {
        assert!(Instant::now() < deadline);
        app.runtime().block_on(async {
            tokio::time::sleep(Duration::from_millis(5)).await;
        });
        app.poll();
    }
    assert!(app.current_cli_check());
    assert!(!app.state.draft.options.contains_key("executable"));
    app.preflight(ctx.clone());
    assert!(!app.busy && app.prepared.is_none());
    assert!(app.selected.contains(&repository.path));
    assert_eq!(app.state.draft.prompt, "Review the repository");

    std::fs::write(root.join("bin/codex.release"), "release").unwrap();
    complete_cli_checks(&mut app);
    let resolved = root.join("bin/codex").display().to_string();
    assert_eq!(app.state.draft.options["executable"], resolved);
    assert!(app.current_detection().unwrap().is_ok());

    // A different backend's refresh must not prevent the selected CLI's review.
    app.cli_checks.recheck(AgentId::Copilot);
    assert!(app.cli_checks.checking(AgentId::Copilot));
    app.preflight(ctx);
    assert!(app.busy);
    let deadline = Instant::now() + Duration::from_secs(5);
    while app.busy {
        assert!(Instant::now() < deadline);
        app.runtime().block_on(async {
            tokio::time::sleep(Duration::from_millis(5)).await;
        });
        app.poll();
    }
    assert!(app.notice.is_empty(), "{}", app.notice);
    assert_eq!(
        app.prepared.as_ref().unwrap().task.options["executable"],
        resolved
    );
    assert!(app.state.runs.is_empty() && app.manager.is_idle());
}

#[test]
fn agent_selection_starts_an_unchecked_backend_without_waiting_for_a_frame() {
    let (_temp, mut app) = app();
    // Model an unchecked backend without driving startup's scheduled tasks.
    app.cli_checks = agents::availability::CliChecks::new(
        app.store.directory().to_owned(),
        app.runtime().handle().clone(),
        || {},
    );
    app.busy = true;
    assert!(!app.cli_checks.any_checking());
    app.select_agent(AgentId::Copilot);
    assert_eq!(app.state.draft.agent, AgentId::Copilot);
    assert!(app.current_cli_check());
    assert!(app.current_detection().is_none());
    assert!(!app.cli_checks.checking(AgentId::Codex));
    assert!(app.busy && app.manager.is_idle());
}

#[test]
fn agent_selection_reuses_session_results_but_check_cli_forces_refresh() {
    let (_temp, mut app) = app();
    complete_cli_checks(&mut app);
    for agent in [
        AgentId::Copilot,
        AgentId::Codex,
        AgentId::Claude,
        AgentId::Copilot,
    ] {
        app.select_agent(agent);
        assert!(app.current_detection().is_some()); // Missing CLIs are cached too.
        assert!(!app.current_cli_check());
        assert!(!app.cli_checks.any_checking());
    }
    app.check_cli(egui::Context::default());
    assert!(app.current_cli_check());
    assert!(app.current_detection().is_none());
}

#[test]
fn agent_selection_checks_a_changed_executable_and_never_shows_its_old_result() {
    let (temp, mut app) = app();
    complete_cli_checks(&mut app);
    let changed = temp.path().join("new-copilot").display().to_string();
    app.state
        .agent_options
        .get_mut(&AgentId::Copilot)
        .unwrap()
        .insert("executable".into(), changed.clone());
    app.select_agent(AgentId::Copilot);
    assert_eq!(app.state.draft.options["executable"], changed);
    assert!(app.current_cli_check());
    assert!(app.current_detection().is_none());
    // Switching away leaves the selected backend's cached result independent.
    app.select_agent(AgentId::Codex);
    assert!(!app.current_cli_check());
    assert!(app.current_detection().is_some());
    complete_cli_checks(&mut app);
    app.select_agent(AgentId::Copilot);
    assert!(!app.current_cli_check());
    assert!(
        app.current_detection()
            .unwrap()
            .as_ref()
            .unwrap_err()
            .detail
            .contains("new-copilot")
    );
}

#[test]
fn startup_checks_leave_task_input_and_loaded_history_usable() {
    let mut state = AppState::default();
    state.runs.push(run(9, &[JobStatus::Succeeded]));
    state.repositories.push(repository("registered"));
    state.draft.prompt = "existing task".into();
    let history = serde_json::to_value(&state.runs).unwrap();
    let repositories = state.repositories.clone();
    let (_temp, mut app) = app_with_state(state);
    app.focus_draft = true;
    let ctx = egui::Context::default();
    theme::install(&ctx);
    for events in [
        vec![],
        vec![egui::Event::Text(" typing during validation".into())],
    ] {
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1180.0, 820.0),
                )),
                events,
                ..Default::default()
            },
            |ui| {
                app.header(ui, &ctx);
                egui::Panel::left("task_editor")
                    .default_size(360.0)
                    .show(ui, |ui| app.editor_pane(ui, &ctx));
                egui::CentralPanel::default().show(ui, |ui| app.results(ui, &ctx));
            },
        );
        assert!(!output.shapes.is_empty());
        output.textures_delta.clear();
    }
    assert!(app.state.draft.prompt.contains("typing during validation"));
    assert!(app.current_cli_check());
    assert!(!app.busy && app.manager.is_idle());
    assert!(!app.repository_pending.is_empty()); // Independent repository refresh.
    assert_eq!(serde_json::to_value(&app.state.runs).unwrap(), history);
    assert_eq!(app.state.repositories, repositories);
    assert_eq!(app.selected_run, Some(9));
    app.state
        .draft
        .options
        .insert("executable".into(), "/new/executable".into());
    app.poll();
    assert!(app.current_cli_check());
    assert!(app.current_detection().is_none());
}

#[test]
fn shutdown_does_not_wait_for_a_blocked_discovery_worker() {
    let (_temp, app) = app();
    let (started_tx, started_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    app.runtime().spawn_blocking(move || {
        started_tx.send(()).unwrap();
        release_rx.recv().unwrap();
    });
    started_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    let started = Instant::now();
    drop(app);
    assert!(started.elapsed() < Duration::from_secs(2));
    release_tx.send(()).unwrap();
}

#[test]
fn cli_discovery_requires_explicit_selection_and_preserves_custom_paths_and_cancellation() {
    let (temp, mut app) = app();
    app.busy = true;
    let ctx = egui::Context::default();
    for agent in AgentId::ALL {
        app.state.select_agent(agent);
        let custom = temp.path().join("my manually configured executable");
        app.state
            .draft
            .options
            .insert("executable".into(), custom.display().to_string());
        let before = serde_json::to_value(&app.state).unwrap();
        let paths = vec![
            temp.path().join("first candidate"),
            temp.path().join("second candidate"),
        ];
        for result in [
            Ok(vec![]),
            Ok(vec![paths[0].clone()]),
            Ok(paths.clone()),
            Err("fixture search error".into()),
        ] {
            app.dirty = false;
            pending_cli_search(&mut app, 1);
            app.tx.send(Message::FoundCli(1, result)).unwrap();
            app.poll();
            assert_eq!(serde_json::to_value(&app.state).unwrap(), before);
            assert!(!app.dirty);
            assert!(app.current_cli_check());
            assert!(app.busy);
            assert!(app.manager.is_idle());
            app.cli_search = None; // Cancel, then receive a late completion.
            app.cli_search_completed(1, Ok(paths.clone()));
            assert!(app.cli_search.is_none());
            assert_eq!(serde_json::to_value(&app.state).unwrap(), before);
        }
        pending_cli_search(&mut app, 2);
        app.cli_search_completed(2, Ok(paths));
        assert!(app.cli_search.as_ref().unwrap().selected.is_none());
        app.use_discovered_cli(ctx.clone()); // No implicit choice among multiple matches.
        assert_eq!(serde_json::to_value(&app.state).unwrap(), before);
        assert!(app.current_cli_check());
    }
}

fn pending_cli_search(app: &mut App, request: u64) {
    app.cli_search = Some(cli_discovery::CliSearch {
        request,
        agent: app.state.draft.agent,
        options: app.state.draft.options.clone(),
        result: None,
        selected: None,
    });
}

#[test]
fn startup_resolution_preserves_an_open_chooser_and_never_overwrites_manual_edits() {
    let (temp, mut app) = app();
    app.state.draft.options.remove("executable");
    pending_cli_search(&mut app, 2);
    let first = temp.path().join("first-codex");
    let second = temp.path().join("second-codex");
    app.apply_resolved_executable(agents::availability::ResolvedExecutable {
        agent: AgentId::Codex,
        configured: None,
        path: first.display().to_string(),
    });
    app.cli_search_completed(2, Ok(vec![first.clone(), second.clone()]));
    assert_eq!(
        app.cli_search.as_ref().unwrap().options,
        app.state.draft.options
    );
    app.cli_search.as_mut().unwrap().selected = Some(1);
    app.use_discovered_cli(egui::Context::default());
    assert_eq!(
        app.state.draft.options["executable"],
        second.display().to_string()
    );
    app.apply_resolved_executable(agents::availability::ResolvedExecutable {
        agent: AgentId::Codex,
        configured: None,
        path: first.display().to_string(),
    });
    assert_eq!(
        app.state.draft.options["executable"],
        second.display().to_string()
    );
}

#[test]
fn cli_discovery_ignores_stale_requests_agent_switches_and_manual_edits() {
    let (temp, mut app) = app();
    let candidate = temp.path().join("candidate");
    pending_cli_search(&mut app, 2);
    app.cli_search_completed(1, Ok(vec![candidate.clone()]));
    assert!(app.cli_search.as_ref().unwrap().result.is_none());
    app.state.select_agent(AgentId::Claude);
    app.cli_search_completed(2, Ok(vec![candidate.clone()]));
    assert!(app.cli_search.is_none());
    assert!(app.state.draft.options["executable"].ends_with("missing-cli"));

    pending_cli_search(&mut app, 3);
    app.state
        .draft
        .options
        .insert("executable".into(), "manual edit".into());
    app.cli_search_completed(3, Ok(vec![candidate.clone()]));
    assert!(app.cli_search.is_none());
    assert_eq!(app.state.draft.options["executable"], "manual edit");

    pending_cli_search(&mut app, 4);
    app.cli_search_completed(4, Ok(vec![candidate]));
    app.state
        .draft
        .options
        .insert("model".into(), "new configuration".into());
    app.use_discovered_cli(egui::Context::default());
    assert_eq!(app.state.draft.options["executable"], "manual edit");
    assert!(app.current_cli_check());
}

#[test]
fn choosing_a_cli_saves_only_the_executable_and_automatically_checks_it() {
    let (temp, mut app) = app();
    app.busy = true;
    for agent in AgentId::ALL {
        app.state.select_agent(agent);
        app.state.draft.prompt = "unchanged task".into();
        let candidate = temp.path().join("chosen CLI with spaces");
        pending_cli_search(&mut app, 1);
        app.cli_search_completed(1, Ok(vec![candidate.clone()]));
        assert_eq!(app.cli_search.as_ref().unwrap().selected, Some(0));
        app.use_discovered_cli(egui::Context::default());
        assert!(app.cli_search.is_none());
        assert_eq!(
            app.state.draft.options["executable"],
            candidate.display().to_string()
        );
        assert_eq!(app.state.draft.prompt, "unchanged task");
        assert!(app.current_cli_check());
        assert!(app.current_detection().is_none());
        assert!(app.busy);
        assert!(app.manager.is_idle());
        assert!(app.state.runs.is_empty());
        app.save();
        assert_eq!(
            app.store.load().unwrap().draft.options,
            app.state.draft.options
        );
        // The current-thread runtime is not driven, so this never runs a CLI.
    }
}

#[test]
fn editor_scroll_and_execution_fit_small_and_large_panes() {
    let (_temp, mut app) = app();
    let ctx = egui::Context::default();
    theme::install(&ctx);
    for count in [1, 5, 10, 24] {
        for visuals in [egui::Visuals::dark(), egui::Visuals::light()] {
            ctx.set_visuals(visuals);
            app.state.repositories = (0..count)
                .map(|i| repository(&format!("long-repository-name-{i}")))
                .collect();
            app.state.groups = vec![domain::RepositoryGroup {
                name: "Maintenance repositories".into(),
                repositories: app
                    .state
                    .repositories
                    .iter()
                    .map(|r| r.path.clone())
                    .collect(),
            }];
            app.state.templates = vec![domain::TaskTemplate {
                name: "Review".into(),
                prompt: "Review requirements".into(),
            }];
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
                                                result.response.rect.right()
                                                    <= bounds.right() + 1.0,
                                                "{agent:?} width {width}"
                                            );
                                            assert!(
                                                result.response.rect.bottom()
                                                    <= bounds.bottom() + 1.0,
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
        provenance: None,
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
        execution_mode: domain::ExecutionMode::Direct,
        attachments: Vec::new(),
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
        let (temp, mut app) = app();
        prepare(&mut app, agent);
        let path = temp.path().join("specification.md");
        std::fs::write(&path, "task context").unwrap();
        app.state
            .draft
            .attachments
            .push(crate::attachments::Attachment::inspect(&path).unwrap());
        app.prepared.as_mut().unwrap().task.attachments = app.state.draft.attachments.clone();
        app.state.groups.push(domain::RepositoryGroup {
            name: "Keep group".into(),
            repositories: vec![app.state.repositories[0].path.clone()],
        });
        app.state.templates.push(domain::TaskTemplate {
            name: "Keep template".into(),
            prompt: "Reusable text".into(),
        });
        let groups = app.state.groups.clone();
        let templates = app.state.templates.clone();
        let expected = serde_json::to_value(app.prepared.as_ref().unwrap().task.clone()).unwrap();
        let preferences = app.state.agent_options.clone();
        let registered = app.state.repositories.clone();
        app.start();
        assert!(app.manager.is_active(1));
        assert!(app.state.draft.prompt.is_empty());
        assert!(app.state.draft.attachments.is_empty());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "task context");
        assert_eq!(app.state.groups, groups);
        assert_eq!(app.state.templates, templates);
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
        assert!(restored.draft.attachments.is_empty());
        assert_eq!(restored.draft.agent, agent);
        assert_eq!(restored.draft.concurrency, 3);
        assert_eq!(restored.global_concurrency, 6);
        assert_eq!(restored.repositories, registered);
        assert_eq!(restored.groups, groups);
        assert_eq!(restored.templates, templates);
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
        execution_mode: domain::ExecutionMode::IsolatedWorktree,
        attachments: Vec::new(),
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
    assert_eq!(
        app.state.draft.execution_mode,
        domain::ExecutionMode::IsolatedWorktree
    );
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
fn backend_controls_fit_compact_editor_in_both_themes_while_startup_checks_run() {
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
    assert!(app.cli_checks.any_checking());
    assert!(!app.busy);
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

fn isolated_history(id: u64, status: JobStatus, changed: Option<bool>) -> Run {
    let mut run = run(id, &[status]);
    run.task.execution_mode = domain::ExecutionMode::IsolatedWorktree;
    run.jobs[0].execution_mode = domain::ExecutionMode::IsolatedWorktree;
    run.jobs[0].worktree = Some(domain::WorktreeMetadata {
        owner: "codeconvoy.worktree.v1".into(),
        run: id,
        job: 0,
        repository: run.jobs[0].repository.clone(),
        common_dir: PathBuf::from("/fixtures/repo0/.git"),
        path: PathBuf::from("/fixtures/owned/result/tree"),
        base_commit: "1234567890abcdef1234567890abcdef12345678".into(),
        execution_mode: domain::ExecutionMode::IsolatedWorktree,
    });
    run.jobs[0].worktree_result = changed.map(|changed| domain::WorktreeResult {
        observed_this_session: true,
        exists: true,
        changed: Some(changed),
    });
    run
}

#[test]
fn isolated_result_presentation_distinguishes_execution_observation_and_history() {
    for status in [
        JobStatus::Succeeded,
        JobStatus::Failed,
        JobStatus::Cancelled,
    ] {
        for changed in [false, true] {
            let run = isolated_history(1, status, Some(changed));
            let label = format::isolated_result(&run.jobs[0]).unwrap();
            assert!(label.contains(if changed {
                "changes retained"
            } else {
                "No repository changes"
            }));
            let loaded: Run = serde_json::from_value(serde_json::to_value(&run).unwrap()).unwrap();
            let label = format::isolated_result(&loaded.jobs[0]).unwrap();
            assert!(label.contains("not checked after restart"));
            assert!(label.contains(if changed {
                "changes retained"
            } else {
                "no changes"
            }));
            assert_eq!(loaded.jobs[0].status, status);
        }
    }
    let unknown = isolated_history(1, JobStatus::Cancelled, None);
    assert!(
        format::isolated_result(&unknown.jobs[0])
            .unwrap()
            .contains("result not validated")
    );
    assert!(format::isolated_result(&run(1, &[JobStatus::Succeeded]).jobs[0]).is_none());
}

#[test]
fn retained_results_allow_exit_and_cancel_quit_preserves_preparation_metadata() {
    for status in [
        JobStatus::Succeeded,
        JobStatus::Failed,
        JobStatus::Cancelled,
    ] {
        let (_temp, mut app) = app();
        app.state.runs.push(isolated_history(1, status, Some(true)));
        let before = serde_json::to_value(&app.state.runs).unwrap();
        assert!(app.request_quit());
        assert_eq!(
            serde_json::to_value(&app.store.load().unwrap().runs).unwrap(),
            before
        );
    }
    for status in [JobStatus::Preparing, JobStatus::Running] {
        let (_temp, mut app) = app();
        app.state.runs.push(isolated_history(1, status, None));
        let before = serde_json::to_value(&app.state.runs).unwrap();
        assert!(!app.request_quit());
        app.cancel_quit();
        assert!(!app.closing && !app.exit_ready);
        assert_eq!(serde_json::to_value(&app.state.runs).unwrap(), before);
    }
}

#[test]
fn confirmed_quit_persists_preparation_cancellation_and_retained_results() {
    let (_temp, mut app) = app();
    prepare(&mut app, AgentId::Codex);
    app.prepared.as_mut().unwrap().task.execution_mode = domain::ExecutionMode::IsolatedWorktree;
    app.start();
    let metadata = isolated_history(1, JobStatus::Preparing, None).jobs[0]
        .worktree
        .clone();
    app.apply_event(Event::Preparing {
        run: 1,
        job: 0,
        worktree: metadata.clone(),
    });
    app.state
        .runs
        .push(isolated_history(40, JobStatus::Failed, Some(true)));
    let retained = serde_json::to_value(&app.state.runs[1]).unwrap();
    assert!(!app.request_quit());
    app.confirm_quit();
    app.runtime.as_ref().unwrap().block_on(async {
        tokio::time::timeout(Duration::from_secs(5), &mut app.manager.join)
            .await
            .unwrap()
            .unwrap();
    });
    app.handle_close(&egui::Context::default());
    let saved = app.store.load().unwrap();
    assert!(app.exit_ready);
    assert_eq!(saved.runs[0].jobs[0].status, JobStatus::Cancelled);
    assert!(!saved.runs[0].jobs[0].interrupted);
    assert_eq!(saved.runs[0].jobs[0].worktree, metadata);
    assert_eq!(serde_json::to_value(&saved.runs[1]).unwrap(), retained);
}

#[test]
fn isolated_groups_templates_and_reuse_produce_only_fresh_explicit_jobs() {
    let (_temp, mut app) = app();
    prepare(&mut app, AgentId::Copilot);
    app.prepared = None;
    app.state.draft.execution_mode = domain::ExecutionMode::IsolatedWorktree;
    app.state.groups = vec![
        domain::RepositoryGroup {
            name: "all".into(),
            repositories: app
                .state
                .repositories
                .iter()
                .map(|r| r.path.clone())
                .collect(),
        },
        domain::RepositoryGroup {
            name: "overlap".into(),
            repositories: vec![app.state.repositories[0].path.clone()],
        },
    ];
    app.select_group(0, true);
    app.select_group(1, true);
    app.state.templates.push(domain::TaskTemplate {
        name: "task only".into(),
        prompt: "Original task".into(),
    });
    app.load_template(0);
    let prepared = PreparedRun {
        task: app.state.draft.clone(),
        repositories: app
            .state
            .repositories
            .iter()
            .filter(|r| app.selected.contains(&r.path))
            .map(|r| PreparedRepository {
                repository: r.clone(),
                state: WorkingTree {
                    summary: domain::GitSummary::default(),
                    entries: vec![],
                },
            })
            .collect(),
    };
    let run = prepared.snapshot(42);
    assert_eq!(run.jobs.len(), 2);
    assert!(
        run.jobs
            .iter()
            .all(|j| j.execution_mode == domain::ExecutionMode::IsolatedWorktree)
    );
    let snapshot = serde_json::to_value(&run).unwrap();
    app.state.groups[0].repositories.clear();
    app.state.groups.clear();
    app.state.templates.clear();
    assert_eq!(serde_json::to_value(&run).unwrap(), snapshot);
    let mut history = isolated_history(42, JobStatus::Cancelled, Some(true));
    history.task = run.task;
    history.jobs[0].repository = app.state.repositories[0].clone();
    app.state.runs.push(history);
    app.reuse_convoy(42);
    assert_eq!(
        app.state.draft.execution_mode,
        domain::ExecutionMode::IsolatedWorktree
    );
    assert_eq!(app.state.draft.prompt, "Original task");
    let fresh = PreparedRun {
        task: app.state.draft.clone(),
        repositories: prepared.repositories,
    }
    .snapshot(43);
    assert!(fresh.jobs.iter().all(|j| j.worktree.is_none()
        && j.worktree_result.is_none()
        && j.status == JobStatus::Queued
        && j.started_at.is_none()));
    assert!(
        !serde_json::to_string(&app.state.draft)
            .unwrap()
            .contains("base_commit")
    );
    assert!(app.manager.is_idle());
}

#[test]
fn recovered_result_messages_update_history_once_and_do_not_change_raw_output() {
    use crate::{domain::ResultAvailability as A, worktrees::recovery::Report};
    let (_temp, mut app) = app();
    app.state.runs = vec![isolated_history(1, JobStatus::Failed, Some(true))];
    let metadata = app.state.runs[0].jobs[0].worktree.clone().unwrap();
    let report = Report::unavailable(
        A::Missing,
        "Isolated directory is missing; restore its location.",
    );
    app.state.runs[0].jobs[0]
        .raw_log
        .append("original backend output");
    app.selected_run = Some(1);
    app.diff_target = Some(metadata.path.clone());
    app.diff = Some(Ok("old diff".into()));
    for _ in 0..2 {
        app.tx
            .send(Message::Reconciled(
                1,
                0,
                metadata.clone(),
                report.clone(),
                false,
            ))
            .unwrap();
        app.poll();
    }
    let job = &app.state.runs[0].jobs[0];
    assert_eq!(job.status, JobStatus::Failed);
    assert_eq!(job.result_availability, A::Missing);
    assert_eq!(job.raw_log.text, "original backend output");
    assert_eq!(job.log.text.matches("Isolated result missing").count(), 1);
    assert!(format::isolated_result(job).unwrap().contains("missing"));
    assert!(matches!(app.diff, Some(Err(_))));
    assert_eq!(
        app.store.load().unwrap().runs[0].jobs[0].result_availability,
        A::Missing
    );
}

#[test]
fn history_actions_preserve_unresolved_results_with_an_explanation() {
    let (_temp, mut app) = app();
    app.state.runs = vec![
        isolated_history(1, JobStatus::Succeeded, Some(true)),
        run(2, &[JobStatus::Succeeded]),
    ];
    app.remove_history(Some(1));
    assert_eq!(app.state.runs.len(), 2);
    assert!(app.notice.contains("unresolved isolated results"));
    app.remove_history(None);
    assert_eq!(app.state.runs.len(), 1);
    assert_eq!(app.state.runs[0].id, 1);
}

#[test]
fn discard_confirmation_focuses_cancel_and_escape_preserves_everything() {
    let (_temp, mut app) = app();
    app.state.runs.push(run(1, &[JobStatus::Succeeded]));
    app.discard_confirmation = Some((1, 0, crate::persistence::results::Action::Discard));
    app.focus_discard_cancel = true;
    let before = serde_json::to_value(&app.state).unwrap();
    let ctx = egui::Context::default();
    ctx.run_ui(egui::RawInput::default(), |ui| app.discard_window(ui.ctx()))
        .textures_delta
        .clear();
    assert!(ctx.memory(|m| m.focused().is_some()));
    // Enter activates the safe initial button, not the destructive action.
    let key = |key| egui::RawInput {
        events: vec![egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::default(),
        }],
        ..Default::default()
    };
    ctx.run_ui(key(egui::Key::Enter), |ui| app.discard_window(ui.ctx()))
        .textures_delta
        .clear();
    assert!(app.discard_confirmation.is_none());
    app.discard_confirmation = Some((1, 0, crate::persistence::results::Action::Discard));
    ctx.run_ui(key(egui::Key::Escape), |ui| app.discard_window(ui.ctx()))
        .textures_delta
        .clear();
    assert!(app.discard_confirmation.is_none());
    assert!(app.result_operation.is_none());
    assert_eq!(serde_json::to_value(&app.state).unwrap(), before);
}

#[test]
fn review_renders_24_jobs_and_result_resolution_does_not_change_agent_status() {
    let (_temp, mut app) = app();
    app.state.runs.push(run(1, &[JobStatus::Failed; 24]));
    app.selected_run = Some(1);
    app.tab = Tab::Review;
    let ctx = egui::Context::default();
    for _ in 0..3 {
        ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(680.0, 760.0),
                )),
                ..Default::default()
            },
            |ui| app.results(ui, &ctx),
        )
        .textures_delta
        .clear();
    }
    assert_eq!(crate::review::totals(&app.state.runs[0]).failed, 24);
    assert!(app.review_pending.is_none()); // Rendering itself does not dispatch Git.
    app.state.runs[0].jobs[0].resolution = domain::ResultResolution::Applied;
    assert_eq!(app.state.runs[0].jobs[0].status, JobStatus::Failed);
}

#[test]
fn result_operation_keeps_quit_waiting_for_completion_merge() {
    let (_temp, mut app) = app();
    app.result_operation = Some((1, 0));
    assert!(!app.request_quit());
    assert!(app.quit_requested && !app.exit_ready);
    app.cancel_quit();
    assert!(app.result_operation.is_some());
}

#[test]
fn review_controls_ignore_unrelated_inspections_and_preserve_uncertain_results() {
    use domain::{ResultAvailability as A, ResultResolution as R};
    let (_temp, mut app) = app();
    app.state.runs = vec![isolated_history(1, JobStatus::Succeeded, Some(true))];
    app.selected_run = Some(1);
    app.selected_job = 0;
    let job = &mut app.state.runs[0].jobs[0];
    job.result_checked = true;
    job.result_availability = A::Available;
    job.review = Some(Ok(crate::review::Statistics {
        files: 1,
        ..Default::default()
    }));
    for resolution in [R::Unresolved, R::Applied, R::ApplyPending] {
        app.state.runs[0].jobs[0].resolution = resolution;
        for pending in [None, Some((1, 0)), Some((1, 1)), Some((2, 0))] {
            app.review_pending = pending;
            let ctx = egui::Context::default();
            ctx.enable_accesskit();
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1000.0, 900.0),
                    )),
                    ..Default::default()
                },
                |ui| app.review_view(ui, &ctx, 900.0),
            );
            output.textures_delta.clear();
            let tree = output.platform_output.accesskit_update.unwrap();
            for label in ["Apply result", "Discard result", "Clean up retained copy"] {
                let button = tree.nodes.iter().find(|(_, n)| n.label() == Some(label));
                let present = match resolution {
                    R::Unresolved => label != "Clean up retained copy",
                    R::Applied => label == "Clean up retained copy",
                    _ => false,
                };
                assert_eq!(button.is_some(), present, "{resolution:?}: {label}");
                if let Some((_, node)) = button {
                    assert_eq!(
                        node.is_disabled(),
                        pending == Some((1, 0)),
                        "{pending:?}: {label}"
                    );
                }
            }
        }
    }
}
