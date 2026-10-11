use super::*;
use crate::history::RetentionPolicy;
const CLOCK: u64 = 2_000_000_000;
fn historical(id: u64) -> Run {
    let mut r = run(id, &[JobStatus::Succeeded]);
    r.created_at = CLOCK - 100_000 + id;
    r.jobs[0].finished_at = Some(CLOCK - 1000 + id);
    r
}
fn fixture() -> (tempfile::TempDir, App) {
    let (temp, mut app) = app();
    app.state.runs = (1..=4).rev().map(historical).collect();
    app.state.history_retention.max_entries = std::num::NonZeroUsize::new(2);
    app.automatic_retention_pending = false;
    (temp, app)
}
fn ids(app: &App) -> Vec<u64> {
    app.state.runs.iter().map(|r| r.id).collect()
}
fn keyboard() -> Keyboard {
    Keyboard::new(|app, _, ctx| {
        app.handle_shortcuts(ctx);
        app.history_settings_window(ctx);
        app.history_window(ctx);
    })
}
#[test]
fn history_preview_is_cancel_safe_and_keyboard_confirmation_is_explicit() {
    let (_temp, mut app) = fixture();
    app.offer_history_cleanup(CLOCK);
    let p = &app.history_dialog.as_ref().unwrap().preview;
    assert_eq!((p.stored, p.retained(), p.remove.len()), (4, 2, 2));
    assert_eq!(p.remove, [1, 2].into());
    let mut keys = keyboard();
    keys.frame(&mut app, vec![]);
    let focus = keys.ctx.memory(|m| m.focused()).unwrap();
    assert_eq!(keys.nodes[&focus.accesskit_id()].label(), Some("Cancel"));
    keys.key(&mut app, egui::Key::Escape);
    assert!(app.history_dialog.is_none());
    assert_eq!(ids(&app), [4, 3, 2, 1]);
    assert!(!app.store.directory().join("state.json").exists());
    app.offer_history_cleanup(CLOCK);
    app.confirm_history_cleanup(CLOCK);
    assert!(app.history_dialog.is_none());
    assert_eq!(ids(&app), [4, 3, 2, 1]); // until persisted
    finish_history_write(&mut app);
    assert_eq!(ids(&app), [4, 3]);
    assert!(app.notice.contains("Removed 2"));
    assert!(
        !app.dirty,
        "acknowledgement must not request a duplicate save"
    );
}
#[test]
fn history_changed_during_confirmation_requires_a_new_review() {
    for change in 0..4 {
        let (_temp, mut app) = fixture();
        app.offer_history_cleanup(CLOCK);
        match change {
            0 => app.state.runs.insert(0, historical(5)), // newly completed
            1 => app.state.runs[2].jobs[0].status = JobStatus::Running,
            2 => {
                app.state.runs.remove(3);
            }
            _ => app.state.history_retention.max_entries = std::num::NonZeroUsize::new(1),
        }
        let before = ids(&app);
        app.confirm_history_cleanup(CLOCK);
        assert_eq!(ids(&app), before);
        assert!(app.history_write.is_none());
        assert!(app.history_dialog.as_ref().unwrap().refreshed);
        let confirmed = app.history_dialog.as_ref().unwrap().preview.remove.clone();
        app.confirm_history_cleanup(CLOCK);
        finish_history_write(&mut app);
        assert!(confirmed.iter().all(|id| !ids(&app).contains(id)));
        if change == 0 {
            assert!(ids(&app).contains(&5));
        }
        if change == 1 {
            assert!(ids(&app).contains(&2));
        }
    }
}
#[test]
fn selection_and_comparison_are_reconciled_only_when_removed() {
    for selected in [1, 4] {
        let (_temp, mut app) = fixture();
        app.select_run(Some(selected));
        app.tab = Tab::Validation;
        app.diff = Some(Ok("cached".into()));
        app.diff_target = Some("/old-target".into());
        app.review_selected.insert(0);
        app.history_search.query = "Task".into();
        app.offer_history_cleanup(CLOCK);
        app.confirm_history_cleanup(CLOCK);
        finish_history_write(&mut app);
        assert_eq!(app.selected_run, Some(4));
        assert_eq!(app.diff.is_some(), selected == 4);
        assert_eq!(app.review_selected.is_empty(), selected == 1);
        assert_eq!(app.history_search.matching(&app.state.runs), [4, 3]);
        assert!(crate::ui::result_markdown::convoy(&app.state.runs[0]).contains("Task 4"));
    }
}
#[test]
fn failed_history_save_preserves_selection_and_never_retries_retention() {
    let (_temp, mut app) = fixture();
    app.select_run(Some(1));
    std::fs::create_dir(app.store.directory().join("state.json")).unwrap();
    app.state.history_retention.automatic = true;
    app.automatic_retention_pending = true;
    app.apply_startup_retention(CLOCK);
    finish_history_write(&mut app);
    assert_eq!(ids(&app), [4, 3, 2, 1]);
    assert_eq!(app.selected_run, Some(1));
    assert!(app.notice.contains("failed"));
    for _ in 0..5 {
        app.apply_startup_retention(CLOCK);
    }
    assert!(app.history_write.is_none());
    assert_eq!(ids(&app), [4, 3, 2, 1]);
}
#[test]
fn automatic_history_retention_is_one_shot_opt_in_and_avoids_noop_writes() {
    for (enabled, count, expected) in [(false, 2, 4), (true, 2, 2), (true, 10, 4)] {
        let (_temp, mut app) = fixture();
        app.state.history_retention.automatic = enabled;
        app.state.history_retention.max_entries = std::num::NonZeroUsize::new(count);
        app.automatic_retention_pending = true;
        app.apply_startup_retention(CLOCK);
        finish_history_write(&mut app);
        assert_eq!(app.state.runs.len(), expected);
        if expected == 4 {
            assert!(!app.store.directory().join("state.json").exists());
        }
        for _ in 0..5 {
            app.apply_startup_retention(CLOCK);
        }
        assert!(app.history_write.is_none());
        // Enabling later doesn't unexpectedly delete in the settings frame.
        app.state.history_retention.automatic = true;
        app.state.history_retention.max_entries = std::num::NonZeroUsize::new(1);
        app.apply_startup_retention(CLOCK);
        assert!(app.history_write.is_none());
    }
    assert_eq!(
        AppState::default().history_retention,
        RetentionPolicy::default()
    );
}
#[test]
fn history_save_keeps_lifecycle_changes_and_blocks_a_second_writer_or_launch() {
    let (_temp, mut app) = fixture();
    app.offer_history_cleanup(CLOCK);
    app.confirm_history_cleanup(CLOCK);
    assert!(app.store.save(&app.state).is_err());
    assert!(!app.request_quit());
    app.state.runs[0].jobs[0].validation = Some(crate::validation::Record::pending(
        crate::validation::Command {
            executable: "never-execute".into(),
            arguments: vec![],
        },
        &app.state.runs[0].jobs[0],
    ));
    app.dirty = true;
    app.save(); // deferred while the worker owns persistence
    finish_history_write(&mut app);
    assert_eq!(ids(&app), [4, 3]);
    assert!(app.state.runs[0].jobs[0].validation.is_some());
    assert!(app.dirty);
    app.save();
    assert!(
        app.store.load().unwrap().runs[0].jobs[0]
            .validation
            .is_some()
    );
}
#[test]
fn history_dialogs_fit_compact_window_and_all_themes_including_large_preview() {
    for appearance in [
        domain::Appearance::System,
        domain::Appearance::Dark,
        domain::Appearance::Light,
    ] {
        for settings in [false, true] {
            let (_temp, mut app) = fixture();
            app.state.runs = (1..=1000).map(historical).collect();
            if settings {
                app.history_settings = Some(crate::ui::history::Settings::new(
                    &app.state.history_retention,
                ));
            } else {
                app.offer_history_cleanup(CLOCK + 1000);
            }
            let mut keys = keyboard();
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
            let layer = keys.ctx.memory(|m| m.top_modal_layer()).unwrap();
            let rect = keys.ctx.memory(|m| m.area_rect(layer.id)).unwrap();
            assert!(
                rect.left() >= -1.0
                    && rect.top() >= -1.0
                    && rect.right() <= 781.0
                    && rect.bottom() <= 561.0,
                "{appearance:?} settings={settings}: {rect:?}"
            );
            keys.key(&mut app, egui::Key::Escape);
            assert!(app.history_settings.is_none() && app.history_dialog.is_none());
        }
    }
}
#[test]
fn settings_validate_custom_count_and_only_save_explicit_preferences() {
    let (_temp, mut app) = fixture();
    app.history_settings = Some(crate::ui::history::Settings::new(
        &app.state.history_retention,
    ));
    let mut keys = keyboard();
    keys.frame(&mut app, vec![]);
    keys.activate(&mut app, "Maximum entries");
    keys.activate(&mut app, "Custom positive integer");
    keys.replace_text(&mut app, "Custom maximum", "0");
    keys.activate(&mut app, "Save settings");
    assert!(app.history_settings.is_some());
    assert_eq!(app.state.history_retention.max_entries.unwrap().get(), 2);
    keys.replace_text(&mut app, "Custom maximum", "17");
    keys.activate(&mut app, "Automatically apply retention policy");
    keys.activate(&mut app, "Save settings");
    assert!(app.history_settings.is_none());
    assert_eq!(app.state.history_retention.max_entries.unwrap().get(), 17);
    assert!(app.state.history_retention.automatic);
    assert_eq!(
        app.store.load().unwrap().history_retention,
        app.state.history_retention
    );
    assert_eq!(ids(&app), [4, 3, 2, 1]);
}

#[test]
fn history_cleanup_handles_empty_history_and_unlimited_policy_without_writes() {
    let (_temp, mut app) = fixture();
    for empty in [false, true] {
        if empty {
            app.state.runs.clear();
        }
        app.state.history_retention = RetentionPolicy::default();
        app.offer_history_cleanup(CLOCK);
        let preview = &app.history_dialog.as_ref().unwrap().preview;
        assert!(preview.remove.is_empty());
        assert_eq!(preview.stored, app.state.runs.len());
        let mut keys = keyboard();
        keys.frame(&mut app, vec![]);
        assert!(
            keys.nodes
                .values()
                .any(|n| n.label() == Some("Clean Up History") && n.is_disabled())
        );
        keys.key(&mut app, egui::Key::Escape);
        assert!(app.history_write.is_none());
        assert!(!app.store.directory().join("state.json").exists());
    }
}

#[test]
fn late_read_only_review_does_not_pin_history_or_restore_removed_results() {
    let (_temp, mut app) = fixture();
    app.review_pending = Some((1, 0));
    app.offer_history_cleanup(CLOCK);
    assert!(
        app.history_dialog
            .as_ref()
            .unwrap()
            .preview
            .remove
            .contains(&1)
    );
    app.confirm_history_cleanup(CLOCK);
    finish_history_write(&mut app);
    app.tx
        .send(Message::ReviewStats(1, 0, Err("late read".into())))
        .unwrap();
    app.poll();
    assert!(app.review_pending.is_none());
    assert_eq!(ids(&app), [4, 3]);
}

#[test]
fn manager_owned_terminal_and_cancelling_convoys_cannot_be_removed() {
    let (_temp, mut app) = app();
    prepare(&mut app, AgentId::Codex);
    app.start();
    let id = app.state.runs[0].id;
    // Deliberately leave the runtime undriven: the manager owns queued work even
    // if presentation state is already terminal. No fixture agent executes.
    for job in &mut app.state.runs[0].jobs {
        job.finish(JobStatus::Succeeded, Some(0), String::new());
    }
    app.state.history_retention.max_age_days = std::num::NonZeroU32::new(1);
    for cancelling in [false, true] {
        if cancelling {
            app.manager.cancel_run(id);
        }
        assert!(app.manager.is_active(id));
        app.offer_history_cleanup(domain::now());
        assert_eq!(app.history_dialog.as_ref().unwrap().preview.protected, 1);
        app.history_dialog = None;
        app.remove_history(Some(id));
        app.remove_history(None);
        assert!(app.history_write.is_none());
        assert_eq!(app.state.runs.len(), 1);
        assert!(app.manager.is_active(id));
    }
}

#[test]
fn background_save_serializes_writers_preserves_new_edits_and_delays_quit() {
    let (_temp, mut app) = fixture();
    app.dirty = true;
    app.state.draft.prompt = "captured".into();
    app.save_background();
    assert!(app.state_write.is_some());
    assert!(!app.dirty);
    assert!(app.store.save(&app.state).is_err());
    assert!(!app.history_available());
    assert!(!app.request_quit());
    assert!(!app.closing);
    // Worker owns an independent metadata snapshot while the UI keeps editing.
    app.state.draft.prompt = "newer edit".into();
    app.dirty = true;
    app.state.runs[0].jobs[0].validation = Some(crate::validation::Record::pending(
        crate::validation::Command {
            executable: "never-executed".into(),
            arguments: vec![],
        },
        &app.state.runs[0].jobs[0],
    ));
    app.save_background(); // no second writer
    finish_state_write(&mut app);
    assert!(app.dirty);
    assert_eq!(app.state.draft.prompt, "newer edit");
    assert_eq!(app.store.load().unwrap().draft.prompt, "captured");
    app.save_background();
    assert!(
        app.state_write.is_none(),
        "a pending quit must not be starved by another save"
    );
    assert!(app.request_quit()); // Final save includes newer lifecycle/settings state.
    assert!(!app.dirty);
    let saved = app.store.load().unwrap();
    assert_eq!(saved.draft.prompt, "newer edit");
    assert!(saved.runs[0].jobs[0].validation.is_some());
}

#[test]
fn background_save_failure_retains_dirty_state_and_never_starts_a_launch() {
    let (_temp, mut app) = fixture();
    std::fs::create_dir(app.store.history_path()).unwrap();
    app.dirty = true;
    app.save_background();
    prepare(&mut app, AgentId::Codex);
    app.start();
    assert!(app.prepared.is_some());
    assert!(app.manager.is_idle());
    finish_state_write(&mut app);
    assert!(app.notice.contains("Could not save state"));
    assert!(app.dirty);
    assert_eq!(ids(&app), [4, 3, 2, 1]);
}

#[test]
fn virtual_history_rows_stay_bounded_and_keyboard_reaches_both_ends() {
    let (_temp, mut app) = fixture();
    app.state.runs = (1..=5000).rev().map(historical).collect();
    app.select_run(Some(5000));
    let mut keys = Keyboard::new(|app, ui, _| {
        app.run_navigation(ui);
    });
    app.show_active_navigation = true;
    for _ in 0..3 {
        keys.frame(&mut app, vec![]);
    }
    // AccessKit sees only the visible widgets, independent of total history.
    assert!(
        keys.nodes.len() < 150,
        "{} widgets created",
        keys.nodes.len()
    );
    keys.key(&mut app, egui::Key::End);
    assert_eq!(app.selected_run, Some(1));
    keys.key(&mut app, egui::Key::ArrowUp);
    assert_eq!(app.selected_run, Some(2));
    keys.key(&mut app, egui::Key::Home);
    assert_eq!(app.selected_run, Some(5000));
    keys.key(&mut app, egui::Key::ArrowDown);
    assert_eq!(app.selected_run, Some(4999));
    app.history_search.query = "Task 1".into();
    keys.frame(&mut app, vec![]);
    assert_eq!(app.selected_run, Some(4999));
}

#[test]
fn exhausted_run_counter_cannot_reuse_a_stable_id() {
    let (_temp, mut app) = fixture();
    app.state.next_run = u64::MAX;
    prepare(&mut app, AgentId::Codex);
    app.start();
    assert!(app.manager.is_idle());
    assert_eq!(ids(&app), [4, 3, 2, 1]);
    assert_eq!(app.state.next_run, u64::MAX);
    assert!(app.notice.contains("exhausted"));
}
