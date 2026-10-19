use super::*;
use crate::storage::Status;

fn finish(app: &mut App) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while app.storage.running() {
        assert!(Instant::now() < deadline, "storage worker did not finish");
        app.runtime()
            .block_on(async { tokio::time::sleep(Duration::from_millis(5)).await });
        app.poll();
    }
}
#[test]
fn initial_and_manual_scan_follow_persisted_cleanup_without_mutating_active_work() {
    let (_temp, mut app) = app();
    app.state.runs = vec![
        run(3, &[JobStatus::Running]),
        run(2, &[JobStatus::Succeeded]),
        run(1, &[JobStatus::Failed]),
    ];
    app.automatic_retention_pending = false;
    app.store.save(&app.state).unwrap();
    let before = serde_json::to_value(&app.state).unwrap();
    let bytes = std::fs::read(app.store.history_path()).unwrap();
    assert_eq!(app.storage.status, Status::NotMeasured);
    app.open_storage(egui::Context::default());
    app.refresh_storage();
    app.refresh_storage();
    finish(&mut app);
    assert_eq!(
        app.storage.snapshot.as_ref().unwrap().historical_convoys,
        Some(2)
    );
    assert_eq!(serde_json::to_value(&app.state).unwrap(), before);
    assert_eq!(std::fs::read(app.store.history_path()).unwrap(), bytes);
    assert!(app.history_write.is_none() && app.prepared.is_none());
    assert!(app.manager.is_idle() && app.validations.is_idle());
    assert!(app.repository_pending.is_empty());
    app.remove_history(Some(1));
    finish_history_write(&mut app);
    // No automatic storage rescan after cleanup.
    assert_eq!(
        app.storage.snapshot.as_ref().unwrap().historical_convoys,
        Some(2)
    );
    app.refresh_storage();
    finish(&mut app);
    assert_eq!(
        app.storage.snapshot.as_ref().unwrap().historical_convoys,
        Some(1)
    );
    assert_eq!(app.state.runs[0].status(), JobStatus::Running);
}
#[test]
fn compact_storage_keyboard_refresh_close_and_history_navigation() {
    for appearance in [
        domain::Appearance::System,
        domain::Appearance::Dark,
        domain::Appearance::Light,
    ] {
        let (_temp, mut app) = app();
        app.store.save(&app.state).unwrap();
        app.open_storage(egui::Context::default());
        finish(&mut app);
        let mut keys = Keyboard::new(|app, _, ctx| {
            app.handle_shortcuts(ctx);
            app.storage_window(ctx);
        });
        keys.ctx.set_theme(theme::preference(appearance));
        for _ in 0..3 {
            keys.input(
                &mut app,
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(780.0, 560.0),
                    )),
                    ..Default::default()
                },
            );
        }
        for label in ["Storage Overview", "Refresh", "Manage History…", "Close"] {
            let node = keys
                .nodes
                .values()
                .find(|n| keys.label(n) == label)
                .unwrap();
            let b = node.bounds().unwrap();
            assert!(
                b.x0 >= 0.0 && b.x1 <= 780.0 && b.y0 >= 0.0 && b.y1 <= 560.0,
                "{label}: {b:?}"
            );
        }
        keys.activate(&mut app, "Refresh");
        assert_eq!(app.storage.status, Status::Measuring);
        keys.key(&mut app, egui::Key::Escape);
        assert!(!app.storage_open);
        finish(&mut app);
        app.open_storage(keys.ctx.clone());
        finish(&mut app);
        keys.frame(&mut app, vec![]);
        keys.activate(&mut app, "Manage History…");
        assert!(!app.storage_open && app.history_settings.is_some());
        assert!(app.history_write.is_none());
    }
}
#[test]
fn quitting_signals_storage_cancellation_and_does_not_treat_it_as_agent_work() {
    let (_temp, mut app) = app();
    app.store.save(&app.state).unwrap();
    app.open_storage(egui::Context::default());
    assert!(app.request_quit());
    assert!(app.closing && !app.quit_requested);
    finish(&mut app);
    assert!(app.storage.snapshot.is_none());
}

#[cfg(unix)]
fn managed_fixture(app: &mut App, parent: &std::path::Path) -> domain::WorktreeMetadata {
    let source = parent.canonicalize().unwrap().join("source ü");
    std::fs::create_dir(&source).unwrap();
    let git = |args: &[&str]| {
        let out = std::process::Command::new("git")
            .arg("-C")
            .arg(&source)
            .args([
                "-c",
                "commit.gpgsign=false",
                "-c",
                "user.name=Fixture",
                "-c",
                "user.email=f@invalid",
            ])
            .args(args)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
    };
    git(&["init", "--quiet"]);
    git(&["config", "core.autocrlf", "false"]);
    std::fs::write(source.join("tracked"), "base\n").unwrap();
    git(&["add", "."]);
    git(&["commit", "-qm", "base"]);
    let repository = app
        .runtime()
        .block_on(crate::git::register(&source))
        .unwrap();
    let summary = app
        .runtime()
        .block_on(crate::git::status(&source))
        .unwrap()
        .summary;
    let m = crate::worktrees::reserve(
        &app.store.directory().join("worktrees"),
        77,
        0,
        repository.clone(),
        summary.common_dir.unwrap(),
        summary.head.unwrap(),
    )
    .unwrap();
    app.runtime()
        .block_on(crate::worktrees::create(
            &m,
            &crate::process::Cancellation::default(),
            &std::sync::atomic::AtomicBool::new(true),
        ))
        .unwrap();
    let mut run = run(77, &[JobStatus::Succeeded]);
    run.jobs[0].repository = repository.clone();
    run.jobs[0].execution_mode = domain::ExecutionMode::IsolatedWorktree;
    run.jobs[0].worktree = Some(m.clone());
    run.jobs[0].resolution = domain::ResultResolution::Applied;
    run.jobs[0].result_availability = domain::ResultAvailability::Available;
    run.jobs[0].result_checked = true;
    app.state.repositories = vec![repository];
    app.state.runs = vec![run];
    app.automatic_retention_pending = false;
    app.store.save(&app.state).unwrap();
    m
}

#[cfg(unix)]
#[test]
fn managed_worktree_keyboard_selection_cancel_and_parent_focus_are_safe() {
    let (temp, mut app) = app();
    let metadata = managed_fixture(&mut app, temp.path());
    let before = serde_json::to_value(&app.state).unwrap();
    app.open_storage(egui::Context::default());
    finish(&mut app);
    let mut keys = Keyboard::new(|app, _, ctx| {
        app.handle_shortcuts(ctx);
        app.storage_window(ctx);
        app.worktree_cleanup_window(ctx);
    });
    keys.frame(&mut app, vec![]);
    keys.activate(
        &mut app,
        &format!("{} · Convoy #77 · Job 1", metadata.repository.name),
    );
    assert_eq!(app.worktree_management.selected.len(), 1);
    keys.activate(&mut app, "Clean Up Selected…");
    assert!(app.worktree_management.confirmation.is_some());
    assert_eq!(
        super::accessibility::focused_name(&keys).as_deref(),
        Some("Cancel")
    );
    keys.key(&mut app, egui::Key::Escape);
    assert!(app.worktree_management.confirmation.is_none() && app.storage_open);
    assert_eq!(
        super::accessibility::focused_name(&keys).as_deref(),
        Some("Clean Up Selected…")
    );
    assert_eq!(serde_json::to_value(&app.state).unwrap(), before);
    assert!(metadata.path.is_dir());
    assert!(app.result_operation.is_none() && app.worktree_management.batch.is_none());
}

#[cfg(unix)]
#[test]
fn managed_cleanup_confirmation_defaults_to_cancel_and_success_refreshes_sizes() {
    let (temp, mut app) = app();
    let metadata = managed_fixture(&mut app, temp.path());
    app.open_storage(egui::Context::default());
    finish(&mut app);
    assert_eq!(app.storage.snapshot.as_ref().unwrap().entries.len(), 1);
    app.offer_worktree_cleanup();
    assert!(
        app.worktree_management.confirmation.is_none(),
        "Empty selection must not open confirmation"
    );
    app.worktree_management
        .selected
        .insert(crate::persistence::worktree_cleanup::identity(&metadata));
    app.dirty = true;
    app.save_background();
    let mut storage_keys = Keyboard::new(|app, _, ctx| app.storage_window(ctx));
    storage_keys.settle_new_area(&mut app);
    assert!(
        storage_keys
            .nodes
            .values()
            .any(|node| { node.label() == Some("Clean Up Selected…") && node.is_disabled() })
    );
    finish_state_write(&mut app);
    storage_keys.nodes.clear();
    storage_keys.frame(&mut app, vec![]);
    assert!(
        storage_keys
            .nodes
            .values()
            .any(|node| { node.label() == Some("Clean Up Selected…") && !node.is_disabled() })
    );
    app.offer_worktree_cleanup();
    assert_eq!(
        app.worktree_management
            .confirmation
            .as_ref()
            .unwrap()
            .eligible(),
        1
    );
    let before = serde_json::to_value(&app.state).unwrap();
    let mut keys = Keyboard::new(|app, _, ctx| {
        app.storage_window(ctx);
        app.worktree_cleanup_window(ctx);
    });
    keys.frame(&mut app, vec![]);
    keys.frame(&mut app, vec![]);
    assert!(
        keys.nodes
            .values()
            .any(|n| keys.label(n) == metadata.path.display().to_string())
    );
    keys.key(&mut app, egui::Key::Enter);
    assert!(app.worktree_management.confirmation.is_none());
    assert!(app.worktree_management.batch.is_none());
    assert_eq!(serde_json::to_value(&app.state).unwrap(), before);
    assert!(metadata.path.exists());
    app.offer_worktree_cleanup();
    keys.frame(&mut app, vec![]);
    keys.key(&mut app, egui::Key::Escape);
    assert!(app.worktree_management.confirmation.is_none());
    app.offer_worktree_cleanup();
    keys.frame(&mut app, vec![]);
    keys.focus(&mut app, "Clean Up 1 Worktrees");
    app.dirty = true;
    app.save_background();
    keys.nodes.clear();
    keys.frame(&mut app, vec![]);
    assert!(
        keys.nodes
            .values()
            .any(|node| { node.label() == Some("Clean Up 1 Worktrees") && node.is_disabled() })
    );
    keys.key(&mut app, egui::Key::Enter);
    assert!(app.worktree_management.confirmation.is_some());
    assert!(app.worktree_management.batch.is_none());
    assert!(metadata.path.exists());
    finish_state_write(&mut app);
    keys.nodes.clear();
    keys.frame(&mut app, vec![]);
    assert!(
        keys.nodes
            .values()
            .any(|node| { node.label() == Some("Clean Up 1 Worktrees") && !node.is_disabled() })
    );
    assert!(app.worktree_management.confirmation.is_some());
    assert!(app.worktree_management.batch.is_none());
    keys.activate(&mut app, "Clean Up 1 Worktrees");
    assert!(app.worktree_management.batch.is_some());
    // Starting a second operation while the batch is active is blocked.
    app.offer_worktree_cleanup();
    assert!(app.worktree_management.confirmation.is_none());
    let deadline = Instant::now() + Duration::from_secs(15);
    while app.worktree_management.batch.is_some() || app.result_operation.is_some() {
        assert!(Instant::now() < deadline);
        app.pump_worktree_cleanup(&keys.ctx);
        app.runtime()
            .block_on(async { tokio::time::sleep(Duration::from_millis(5)).await });
        app.poll();
    }
    finish(&mut app);
    assert!(!metadata.path.exists());
    assert!(app.storage.snapshot.as_ref().unwrap().entries.is_empty());
    assert_eq!(app.state.runs.len(), 1);
    assert_eq!(
        app.worktree_management
            .report
            .as_ref()
            .unwrap()
            .count(crate::persistence::worktree_cleanup::Outcome::Removed),
        1
    );
}

#[cfg(unix)]
#[test]
fn managed_cleanup_quit_stops_pending_batch_without_removing_worktree() {
    let (temp, mut app) = app();
    let metadata = managed_fixture(&mut app, temp.path());
    app.open_storage(egui::Context::default());
    finish(&mut app);
    app.worktree_management
        .selected
        .insert(crate::persistence::worktree_cleanup::identity(&metadata));
    app.offer_worktree_cleanup();
    let plan = app.worktree_management.confirmation.take().unwrap();
    app.worktree_management.batch = Some(crate::persistence::worktree_cleanup::Batch::new(plan));
    assert!(!app.request_quit());
    assert!(app.quit_requested);
    app.confirm_quit();
    app.pump_worktree_cleanup(&egui::Context::default());
    assert!(app.worktree_management.batch.is_none());
    assert!(metadata.path.exists());
    assert_eq!(
        app.worktree_management
            .report
            .as_ref()
            .unwrap()
            .count(crate::persistence::worktree_cleanup::Outcome::Skipped),
        1
    );
}
