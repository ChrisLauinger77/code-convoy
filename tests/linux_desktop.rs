#![cfg(all(feature = "test-support", target_os = "linux"))]
#![allow(clippy::unwrap_used)]
//! Opt-in real D-Bus transport check; never run against the user's session bus.
use codeconvoy::{desktop, notifications};
use std::time::Duration;

#[tokio::test]
#[ignore = "Requires an isolated dbus-run-session; see docs/platform-validation.md"]
async fn missing_desktop_services_fail_once_without_enabling_close_to_tray() {
    assert_eq!(
        std::env::var("CODECONVOY_PRIVATE_BUS_TEST").as_deref(),
        Ok("1")
    );
    let connection = zbus::Connection::session().await.unwrap();
    let bus = zbus::fdo::DBusProxy::new(&connection).await.unwrap();
    // Also reject an accidentally reused desktop bus before submitting anything.
    for name in [
        "org.freedesktop.Notifications",
        "org.kde.StatusNotifierWatcher",
    ] {
        assert!(!bus.name_has_owner(name.try_into().unwrap()).await.unwrap());
    }
    let runtime = tokio::runtime::Handle::current();
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let mut tray = desktop::Service::new(desktop::Events::new(move |generation, event| {
        let _ = tx.send((generation, event));
    }));
    let preferences = desktop::Preferences {
        enabled: true,
        close_behavior: desktop::CloseBehavior::MinimizeToTray,
        ..Default::default()
    };
    tray.sync(preferences, 0, &runtime);
    tokio::time::timeout(Duration::from_secs(10), async {
        while !matches!(tray.status, desktop::Status::Unavailable(_)) {
            let (generation, event) = rx.recv().await.unwrap();
            assert!(tray.event(generation, event).is_none());
        }
    })
    .await
    .unwrap();
    assert!(!tray.can_minimize(preferences));
    tray.sync(preferences, 2, &runtime);
    assert!(matches!(tray.status, desktop::Status::Unavailable(_)));
    tray.stop();

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let mut service = notifications::Service::native(notifications::Events::new(move |event| {
        let _ = tx.send(event);
    }));
    service.submit(
        &runtime,
        notifications::Completion {
            run: 42,
            outcome: notifications::Outcome::Success,
            title: "CodeConvoy isolated bus test".into(),
            body: "No task or repository data".into(),
        },
    );
    let event = tokio::time::timeout(Duration::from_secs(15), rx.recv())
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(event, notifications::Event::Failed(42, error) if !error.is_empty()));
    service.stop();
}
