use super::*;
use crate::export::{Format, Scope, Snapshot, file::Destination};
use crate::ui::export::{Choice, Pending};

fn fixture() -> (tempfile::TempDir, App) {
    let (temp, mut app) = app();
    app.state.runs = vec![
        run(4, &[JobStatus::Succeeded]),
        run(3, &[JobStatus::Running]),
        run(2, &[JobStatus::Failed]),
        run(1, &[JobStatus::Cancelled]),
    ];
    app.selected_run = Some(4);
    (temp, app)
}
fn pending(app: &mut App, format: Format) {
    app.export.pending = Some(Pending {
        snapshot: Some(Snapshot::capture(&app.state.runs, &Scope::Selected(vec![4])).unwrap()),
        format,
        cancel: Default::default(),
    });
}
fn finish(app: &mut App) {
    for _ in 0..300 {
        if app.export.pending.is_none() {
            return;
        }
        app.runtime().block_on(async {
            tokio::time::sleep(Duration::from_millis(10)).await;
        });
        app.poll();
    }
    panic!("export worker did not complete");
}
#[test]
fn export_selection_is_stable_and_scope_resolves_when_confirmed() {
    let (_temp, mut app) = fixture();
    app.offer_export(Some(4));
    app.selected_run = Some(1);
    assert_eq!(app.capture_export().unwrap().convoys[0].id, 4);
    app.state.runs.remove(0);
    assert!(
        app.capture_export()
            .unwrap_err()
            .to_string()
            .contains("no longer")
    );
    app.offer_export(None);
    app.history_search.filter = crate::visibility::HistoryFilter::Failed;
    assert_eq!(app.capture_export().unwrap().convoys[0].id, 2);
    app.export.dialog.as_mut().unwrap().choice = Choice::All;
    assert_eq!(
        app.capture_export()
            .unwrap()
            .convoys
            .iter()
            .map(|c| c.id)
            .collect::<Vec<_>>(),
        [2, 1]
    );
}
#[test]
fn save_cancel_duplicate_prevention_and_errors_do_not_change_history() {
    let (_temp, mut app) = fixture();
    let before = serde_json::to_value(&app.state).unwrap();
    pending(&mut app, Format::Json);
    assert!(!app.export_available());
    app.offer_export(Some(1));
    assert!(app.export.dialog.is_none());
    app.export_destination(Ok(None));
    assert!(app.export_available());
    assert!(app.export.feedback.contains("cancelled"));
    pending(&mut app, Format::Json);
    app.export_destination(Err("Permission denied".into()));
    assert!(
        app.export
            .feedback
            .contains("Export failed: Permission denied")
    );
    assert_eq!(serde_json::to_value(&app.state).unwrap(), before);
    assert!(app.history_write.is_none());
    assert!(app.manager.is_idle());
    assert!(app.validations.is_idle());
}
#[test]
fn background_exports_survive_cleanup_and_do_not_touch_active_work() {
    for format in [Format::Markdown, Format::Json] {
        let (temp, mut app) = fixture();
        let path = temp.path().join(format!("saved.{}", format.extension()));
        let active_before = serde_json::to_value(&app.state.runs[1]).unwrap();
        pending(&mut app, format);
        app.export_destination(Ok(Some(Destination::inspect(&path).unwrap())));
        // Cleanup can independently remove the source record after snapshot capture.
        app.remove_history(Some(4));
        finish_history_write(&mut app);
        finish(&mut app);
        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("Task 4"));
        assert!(app.export.feedback.contains("Exported 1 convoys to saved."));
        assert!(!app.state.runs.iter().any(|r| r.id == 4));
        assert_eq!(
            serde_json::to_value(&app.state.runs[0]).unwrap(),
            active_before
        );
        app.remove_history(None);
        finish_history_write(&mut app);
        assert_eq!(std::fs::read_to_string(path).unwrap(), content);
    }
}
#[test]
fn overwrite_confirmation_is_cancel_safe_and_keyboard_accessible() {
    let (temp, mut app) = fixture();
    let path = temp.path().join("existing.json");
    std::fs::write(&path, "original").unwrap();
    pending(&mut app, Format::Json);
    app.export_destination(Ok(Some(Destination::inspect(&path).unwrap())));
    assert!(app.export.overwrite.is_some());
    let mut keys = Keyboard::new(|app, _, ctx| {
        app.handle_shortcuts(ctx);
        app.export_window(ctx);
    });
    keys.settle_new_area(&mut app);
    let focus = keys.ctx.memory(|m| m.focused()).unwrap();
    assert_eq!(keys.nodes[&focus.accesskit_id()].label(), Some("Cancel"));
    keys.key(&mut app, egui::Key::Escape);
    assert!(app.export.pending.is_none());
    assert!(app.export.overwrite.is_none());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "original");
    pending(&mut app, Format::Json);
    app.export_destination(Ok(Some(Destination::inspect(&path).unwrap())));
    keys.frame(&mut app, vec![]);
    keys.activate(&mut app, "Replace file");
    finish(&mut app);
    let json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    assert_eq!(json["convoys"][0]["id"], 4);
}
#[test]
fn cancelled_picker_result_cannot_write_and_quit_waits_for_writer() {
    let (temp, mut app) = fixture();
    let path = temp.path().join("cancelled.md");
    pending(&mut app, Format::Markdown);
    app.cancel_export();
    app.export_destination(Ok(Some(Destination::inspect(&path).unwrap())));
    assert!(!path.exists());
    assert!(app.export.pending.is_none());
    pending(&mut app, Format::Markdown);
    app.export_destination(Ok(Some(Destination::inspect(&path).unwrap())));
    assert!(!app.request_quit());
    finish(&mut app);
    assert!(!path.exists());
    assert!(app.state.runs.iter().any(|r| r.active()));
}
#[test]
fn export_controls_and_dialog_fit_compact_themes_with_keyboard_escape() {
    for appearance in [
        domain::Appearance::System,
        domain::Appearance::Dark,
        domain::Appearance::Light,
    ] {
        let (_temp, mut app) = fixture();
        let mut keys = Keyboard::new(|app, ui, ctx| {
            app.results(ui, ctx);
            app.export_window(ctx);
        });
        keys.ctx.set_theme(theme::preference(appearance));
        keys.input(
            &mut app,
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(390.0, 560.0),
                )),
                ..Default::default()
            },
        );
        keys.activate(&mut app, "Export History…");
        for _ in 0..2 {
            keys.input(
                &mut app,
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(390.0, 560.0),
                    )),
                    ..Default::default()
                },
            );
        }
        assert!(app.export.dialog.is_some());
        for label in [
            "Export History",
            "Filtered history",
            "All retained history",
            "JSON",
            "Cancel",
        ] {
            let node = keys
                .nodes
                .values()
                .find(|n| keys.label(n) == label)
                .unwrap();
            let bounds = node.bounds().unwrap();
            assert!(
                bounds.x0 >= 0.0 && bounds.x1 <= 390.0 && bounds.y0 >= 0.0 && bounds.y1 <= 560.0,
                "{label}: {bounds:?}"
            );
        }
        keys.key(&mut app, egui::Key::Escape);
        assert!(app.export.dialog.is_none());
    }
}
