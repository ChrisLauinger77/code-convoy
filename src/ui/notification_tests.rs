use super::*;
use crate::notifications::{
    Backend, Completion, Delivery, Event as NotificationEvent, Events, Service,
};
use std::sync::{Arc, Mutex};

struct RecordingBackend(Arc<Mutex<Vec<Completion>>>);
impl Backend for RecordingBackend {
    fn deliver(&self, completion: Completion, _: Events) -> Delivery {
        let calls = self.0.clone();
        Box::pin(async move {
            calls.lock().unwrap().push(completion);
            Ok(())
        })
    }
}
fn record(app: &mut App) -> Arc<Mutex<Vec<Completion>>> {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let tx = app.tx.clone();
    app.notification_service = Service::new(
        Arc::new(RecordingBackend(calls.clone())),
        Events::new(move |event| {
            let _ = tx.send(Message::Notification(event));
        }),
    );
    calls
}
fn complete(app: &mut App, id: u64, job: usize, status: JobStatus) {
    app.apply_event(Event::Finished {
        run: id,
        job,
        status,
        exit_code: Some(0),
        detail: String::new(),
    });
}
fn receive_assessment(app: &mut App) {
    let messages = app.runtime().block_on(async {
        tokio::time::timeout(Duration::from_secs(5), async {
            let mut messages = Vec::new();
            loop {
                if let Ok(message) = app.rx.try_recv() {
                    let ready =
                        matches!(message, Message::Notification(NotificationEvent::Ready(_)));
                    messages.push(message);
                    if ready {
                        return messages;
                    }
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap()
    });
    for message in messages {
        app.tx.send(message).unwrap();
    }
    app.poll();
}
fn wait_for_calls(app: &App, calls: &Mutex<Vec<Completion>>, count: usize) {
    app.runtime().block_on(async {
        tokio::time::timeout(Duration::from_secs(5), async {
            while calls.lock().unwrap().len() < count {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
    });
}

#[test]
fn lifecycle_notifies_once_per_convoy_after_last_job_and_not_for_restored_history() {
    let (_temp, mut app) = app_with_state(AppState {
        runs: vec![run(5, &[JobStatus::Succeeded])],
        ..Default::default()
    });
    let calls = record(&mut app);
    app.state
        .runs
        .insert(0, run(9, &[JobStatus::Running, JobStatus::Running]));
    app.notification_tracker.launched(9);
    complete(&mut app, 9, 0, JobStatus::Failed);
    app.pump_notifications();
    assert!(calls.lock().unwrap().is_empty());
    complete(&mut app, 9, 1, JobStatus::Succeeded);
    app.pump_notifications();
    receive_assessment(&mut app);
    wait_for_calls(&app, &calls, 1);
    complete(&mut app, 9, 1, JobStatus::Succeeded);
    app.pump_notifications();
    app.pump_notifications();
    assert_eq!(calls.lock().unwrap().len(), 1);
    assert_eq!(calls.lock().unwrap()[0].run, 9);
    assert_eq!(app.state.runs[0].status(), JobStatus::Failed);
}

#[test]
fn foreground_policy_uses_current_preferences_and_does_not_replay_on_blur() {
    let (_temp, mut app) = app();
    let calls = record(&mut app);
    app.state.runs = vec![run(9, &[JobStatus::Running])];
    app.notification_tracker.launched(9);
    complete(&mut app, 9, 0, JobStatus::Cancelled);
    app.state.notifications.cancellation = true;
    app.notification_focused = true;
    app.pump_notifications();
    receive_assessment(&mut app);
    assert!(calls.lock().unwrap().is_empty());
    app.notification_focused = false;
    app.pump_notifications();
    assert!(calls.lock().unwrap().is_empty());
    app.state.runs.push(run(10, &[JobStatus::Running]));
    app.notification_tracker.launched(10);
    complete(&mut app, 10, 0, JobStatus::Failed);
    app.notification_focused = true;
    app.pump_notifications();
    receive_assessment(&mut app);
    wait_for_calls(&app, &calls, 1);
    assert_eq!(calls.lock().unwrap()[0].run, 10);
    app.state.runs.push(run(11, &[JobStatus::Running]));
    app.notification_tracker.launched(11);
    complete(&mut app, 11, 0, JobStatus::Failed);
    app.pump_notifications();
    app.state.notifications.enabled = false;
    receive_assessment(&mut app);
    assert_eq!(calls.lock().unwrap().len(), 1);
}

#[test]
fn activation_selects_stable_ids_review_and_handles_removed_or_unavailable_results() {
    let (_temp, mut app) = app();
    app.state.runs = vec![
        run(700, &[JobStatus::Succeeded]),
        run(9, &[JobStatus::Failed]),
    ];
    app.selected_run = Some(700);
    app.tab = Tab::Raw;
    app.selected_job = 20;
    app.diff = Some(Ok("stale".into()));
    app.notification_event(NotificationEvent::Activated(9));
    assert_eq!(app.selected_run, Some(9));
    assert!(matches!(app.tab, Tab::Review));
    assert_eq!(app.selected_job, 0);
    assert!(app.diff.is_none() && app.notification_activation);
    app.state.runs[0].jobs[0].result_availability = domain::ResultAvailability::Missing;
    app.notification_event(NotificationEvent::Activated(700));
    assert_eq!(app.selected_run, Some(700));
    app.state.runs.remove(1);
    app.notification_event(NotificationEvent::Activated(9));
    assert_eq!(app.selected_run, Some(700));
    assert!(app.notice.contains("#9 is no longer"));
    let ctx = egui::Context::default();
    let mut output = ctx.run_ui(egui::RawInput::default(), |ctx| {
        app.activate_notification_window(ctx)
    });
    output.textures_delta.clear();
    let commands = &output.viewport_output[&egui::ViewportId::ROOT].commands;
    assert!(commands.contains(&egui::ViewportCommand::Minimized(false)));
    assert!(!commands.contains(&egui::ViewportCommand::Focus));
    let mut output = ctx.run_ui(egui::RawInput::default(), |ctx| {
        app.activate_notification_window(ctx)
    });
    output.textures_delta.clear();
    assert!(
        output.viewport_output[&egui::ViewportId::ROOT]
            .commands
            .contains(&egui::ViewportCommand::Focus)
    );
    app.closing = true;
    app.notification_event(NotificationEvent::Activated(9));
    assert!(!app.notification_activation);
}

#[test]
fn minimized_viewport_is_not_foreground_even_with_stale_focus() {
    let (_temp, mut app) = app();
    let ctx = egui::Context::default();
    for (focused, minimized, expected) in [
        (true, false, true),
        (false, false, false),
        (true, true, false),
    ] {
        let mut input = egui::RawInput::default();
        let viewport = input.viewports.get_mut(&egui::ViewportId::ROOT).unwrap();
        viewport.focused = Some(focused);
        viewport.minimized = Some(minimized);
        let mut output = ctx.run_ui(input, |ui| app.update_notification_focus(ui.ctx()));
        output.textures_delta.clear();
        assert_eq!(app.notification_focused, expected);
    }
}

#[test]
fn backend_errors_are_separate_from_execution_and_do_not_persist() {
    let (_temp, mut app) = app();
    app.state.runs = vec![run(9, &[JobStatus::Succeeded])];
    let before = serde_json::to_value(&app.state).unwrap();
    app.notification_event(NotificationEvent::Failed(
        9,
        "Desktop service unavailable".into(),
    ));
    assert!(
        app.notification_error
            .contains("Desktop service unavailable")
    );
    assert_eq!(serde_json::to_value(&app.state).unwrap(), before);
}
