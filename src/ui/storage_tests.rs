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
