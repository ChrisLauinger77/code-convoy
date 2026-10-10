#![allow(clippy::unwrap_used)]
use super::*;
use std::sync::Mutex;

struct Mock(Arc<Mutex<Vec<MenuState>>>);
impl Backend for Mock {
    fn update(&mut self, state: MenuState) -> Result<(), String> {
        self.0.lock().unwrap().push(state);
        Ok(())
    }
}
impl Service {
    pub(crate) fn mock_available() -> Self {
        let mut service = Self::new(Events::new(|_, _| {}));
        service.sync_with(
            Preferences {
                enabled: true,
                ..Default::default()
            },
            0,
            |_, _| Ok(Box::new(Mock(Arc::default()))),
        );
        service.event(service.generation, Event::Ready);
        service
    }
}

#[test]
fn settings_defaults_legacy_and_store_roundtrip() {
    use crate::{domain::AppState, persistence::Store};
    let legacy: AppState =
        serde_json::from_str(r#"{"version":1,"notifications":{"enabled":false},"next_run":42}"#)
            .unwrap();
    assert_eq!(legacy.desktop, Preferences::default());
    assert_eq!(legacy.desktop.close_behavior, CloseBehavior::Quit);
    assert!(!legacy.desktop.enabled);
    assert!(legacy.desktop.show_running_count);
    let temp = tempfile::tempdir().unwrap();
    let store = Store::open(temp.path()).unwrap();
    for enabled in [true, false] {
        for close_behavior in [CloseBehavior::Quit, CloseBehavior::MinimizeToTray] {
            let state = AppState {
                desktop: Preferences {
                    enabled,
                    close_behavior,
                    show_running_count: false,
                },
                ..legacy.clone()
            };
            store.save(&state).unwrap();
            let restored = store.load().unwrap();
            assert_eq!(restored.desktop, state.desktop);
            assert_eq!(restored.next_run, 42);
            assert!(!restored.notifications.enabled);
        }
    }
}

#[test]
fn no_host_or_initialization_failure_attempts_once_until_explicit_reenable() {
    for error in ["No StatusNotifierHost", "Native icon creation failed"] {
        let mut service = Service::new(Events::new(|_, _| {}));
        let mut preferences = Preferences::default();
        service.sync_with(preferences, 0, |_, _| panic!("disabled tray initialized"));
        preferences.enabled = true;
        preferences.close_behavior = CloseBehavior::MinimizeToTray;
        service.sync_with(preferences, 0, |_, _| Err(error.into()));
        assert_eq!(service.status, Status::Unavailable(error.into()));
        for count in 0..4 {
            service.sync_with(preferences, count, |_, _| panic!("automatic retry"));
        }
        assert!(!service.can_minimize(preferences));
        service.event(service.generation, Event::Ready); // Late success cannot resurrect a failed service.
        assert!(!service.available());
        preferences.enabled = false;
        service.sync_with(preferences, 0, |_, _| panic!("disabled"));
        preferences.enabled = true;
        service.sync_with(preferences, 0, |_, _| Ok(Box::new(Mock(Arc::default()))));
        assert_eq!(service.status, Status::Starting);
        assert!(!service.can_minimize(preferences));
        service.event(service.generation, Event::Ready);
        assert!(service.can_minimize(preferences));
    }
}

#[test]
fn counts_toggle_and_actions_are_event_driven_and_generation_scoped() {
    let updates = Arc::new(Mutex::new(Vec::new()));
    let mut service = Service::new(Events::new(|_, _| {}));
    let mut preferences = Preferences {
        enabled: true,
        ..Default::default()
    };
    service.sync_with(preferences, 2, |state, _| {
        updates.lock().unwrap().push(state);
        Ok(Box::new(Mock(updates.clone())))
    });
    let generation = service.generation;
    service.event(generation, Event::Ready);
    for count in [2, 2, 3, 0, 0] {
        service.sync_with(preferences, count, |_, _| panic!("reinit"));
    }
    preferences.show_running_count = false;
    service.sync_with(preferences, 0, |_, _| panic!("reinit"));
    assert_eq!(
        updates
            .lock()
            .unwrap()
            .iter()
            .map(|state| (state.running, state.show_count))
            .collect::<Vec<_>>(),
        [(2, true), (3, true), (0, true), (0, false)]
    );
    for action in [Action::Show, Action::ShowActive, Action::Quit] {
        assert_eq!(
            service.event(generation, Event::Action(action)),
            Some(action)
        );
    }
    service.event(generation, Event::Unavailable("Host disappeared".into()));
    assert_eq!(service.event(generation, Event::Action(Action::Quit)), None);
    service.stop();
    assert_eq!(service.event(generation, Event::Action(Action::Show)), None);
    service.event(generation, Event::Ready);
    assert_eq!(service.status, Status::Disabled);
}

#[test]
fn restoration_coalesces_and_focus_follows_unminimize() {
    use eframe::egui::{Context, RawInput, ViewportCommand as Command, ViewportId};
    let ctx = Context::default();
    let mut window = Window::default();
    window.request_restore();
    window.request_restore();
    let first = ctx.run_logic(&RawInput::default(), |ctx| window.restore(ctx));
    assert_eq!(
        first.viewport_commands[&ViewportId::ROOT],
        [Command::Visible(true), Command::Minimized(false)]
    );
    let second = ctx.run_logic(&RawInput::default(), |ctx| window.restore(ctx));
    assert_eq!(
        second.viewport_commands[&ViewportId::ROOT],
        [Command::Focus]
    );
    let third = ctx.run_logic(&RawInput::default(), |ctx| window.restore(ctx));
    assert!(third.viewport_commands.values().all(Vec::is_empty));
}

#[test]
fn backend_update_and_health_failures_disable_minimization_without_retry() {
    struct Broken;
    impl Backend for Broken {
        fn update(&mut self, _: MenuState) -> Result<(), String> {
            Err("Menu update failed".into())
        }
        fn check(&self) -> Result<(), String> {
            Err("Icon registration lost".into())
        }
    }
    let preferences = Preferences {
        enabled: true,
        close_behavior: CloseBehavior::MinimizeToTray,
        ..Default::default()
    };
    for health_check in [true, false] {
        let mut service = Service::new(Events::new(|_, _| {}));
        service.sync_with(preferences, 0, |_, _| Ok(Box::new(Broken)));
        service.event(service.generation, Event::Ready);
        if health_check {
            assert!(!service.can_minimize(preferences));
        } else {
            service.sync_with(preferences, 1, |_, _| panic!("reinitialized"));
        }
        assert!(matches!(service.status, Status::Unavailable(_)));
        assert!(!service.can_minimize(preferences));
        service.sync_with(preferences, 2, |_, _| panic!("automatic retry"));
    }
}
