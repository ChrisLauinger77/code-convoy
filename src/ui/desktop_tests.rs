use super::*;
use crate::desktop::{Action, CloseBehavior, Service};

fn tray(app: &mut App) {
    app.desktop = Service::mock_available();
    app.state.desktop.enabled = true;
    app.state.desktop.close_behavior = CloseBehavior::MinimizeToTray;
}
fn close(app: &mut App) -> Vec<egui::ViewportCommand> {
    let ctx = egui::Context::default();
    let mut input = egui::RawInput::default();
    input
        .viewports
        .get_mut(&egui::ViewportId::ROOT)
        .unwrap()
        .events
        .push(egui::ViewportEvent::Close);
    ctx.run_logic(&input, |ctx| app.handle_close(ctx))
        .viewport_commands
        .remove(&egui::ViewportId::ROOT)
        .unwrap_or_default()
}

#[test]
fn close_to_tray_preserves_running_and_queued_work_even_on_repeated_close() {
    let (_temp, mut app) = app();
    tray(&mut app);
    app.state
        .runs
        .push(run(17, &[JobStatus::Running, JobStatus::Queued]));
    let before = serde_json::to_value(&app.state).unwrap();
    for _ in 0..3 {
        let commands = close(&mut app);
        assert!(commands.contains(&egui::ViewportCommand::CancelClose));
        assert!(commands.contains(&egui::ViewportCommand::Minimized(true)));
        assert!(!app.quit_requested && !app.closing && !app.exit_ready);
    }
    assert_eq!(serde_json::to_value(&app.state).unwrap(), before);
    assert!(app.window.tray_minimized);
}

#[test]
fn unavailable_tray_close_falls_back_to_confirmation_and_cancel_preserves_jobs() {
    let (_temp, mut app) = app();
    tray(&mut app);
    app.desktop.stop();
    app.state
        .runs
        .push(run(17, &[JobStatus::Running, JobStatus::Queued]));
    let before = serde_json::to_value(&app.state).unwrap();
    let commands = close(&mut app);
    assert!(commands.contains(&egui::ViewportCommand::CancelClose));
    assert!(!commands.contains(&egui::ViewportCommand::Minimized(true)));
    assert!(app.quit_requested);
    app.cancel_quit();
    assert!(!app.closing && !app.quit_requested);
    assert_eq!(serde_json::to_value(&app.state).unwrap(), before);
}

#[test]
fn explicit_tray_quit_overrides_close_preference_and_cannot_hide_confirmation() {
    let (_temp, mut app) = app();
    tray(&mut app);
    app.state.runs.push(run(17, &[JobStatus::Preparing]));
    app.window.tray_minimized = true;
    for _ in 0..3 {
        app.desktop_action(Action::Quit);
        let commands = close(&mut app);
        assert!(!commands.contains(&egui::ViewportCommand::Minimized(true)));
        assert!(commands.contains(&egui::ViewportCommand::Minimized(false)));
        assert!(app.quit_requested && !app.closing);
    }
    app.cancel_quit();
    assert_eq!(app.state.runs[0].jobs[0].status, JobStatus::Preparing);
    app.state.runs[0].jobs[0].status = JobStatus::Succeeded;
    app.desktop_action(Action::Quit);
    let commands = close(&mut app);
    assert!(commands.contains(&egui::ViewportCommand::Close));
    assert!(app.closing && app.exit_ready);
}

#[test]
fn show_retains_view_active_opens_existing_group_and_tray_loss_restores() {
    let (_temp, mut app) = app();
    tray(&mut app);
    app.state.runs = vec![
        run(1, &[JobStatus::Succeeded]),
        run(2, &[JobStatus::Queued]),
    ];
    app.selected_run = Some(1);
    app.tab = Tab::Raw;
    app.desktop_action(Action::Show);
    assert_eq!(app.selected_run, Some(1));
    assert!(app.tab == Tab::Raw);
    app.desktop_action(Action::ShowActive);
    assert_eq!(app.selected_run, Some(2));
    assert!(app.tab == Tab::Review && app.show_active_navigation);
    let ctx = egui::Context::default();
    app.window.minimize(&ctx);
    app.desktop.stop();
    app.sync_desktop();
    let output = ctx.run_logic(&egui::RawInput::default(), |ctx| app.window.restore(ctx));
    assert!(
        output.viewport_commands[&egui::ViewportId::ROOT]
            .contains(&egui::ViewportCommand::Minimized(false))
    );
    assert!(!app.window.tray_minimized);
    assert_eq!(app.state.runs[1].jobs[0].status, JobStatus::Queued);
}

#[test]
fn notification_while_minimized_selects_convoy_and_duplicate_click_does_not_restore_twice() {
    let (_temp, mut app) = app();
    app.state.runs = vec![run(2, &[JobStatus::Succeeded])];
    app.window.tray_minimized = true;
    let ctx = egui::Context::default();
    let input = egui::RawInput::default();
    let _ = ctx.run_logic(&input, |ctx| app.update_notification_focus(ctx));
    assert!(!app.notification_focused);
    app.notification_event(crate::notifications::Event::Activated(2));
    app.notification_event(crate::notifications::Event::Activated(2));
    let output = ctx.run_logic(&input, |ctx| app.window.restore(ctx));
    assert_eq!(app.selected_run, Some(2));
    assert!(
        output.viewport_commands[&egui::ViewportId::ROOT]
            .contains(&egui::ViewportCommand::Minimized(false))
    );
    let _ = ctx.run_logic(&input, |ctx| app.window.restore(ctx));
    app.notification_event(crate::notifications::Event::Activated(2));
    let output = ctx.run_logic(&input, |ctx| app.window.restore(ctx));
    assert!(output.viewport_commands.values().all(Vec::is_empty));
}
