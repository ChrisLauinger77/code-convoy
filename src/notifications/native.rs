//! Small native adapter. Policy, run IDs and navigation stay platform-independent.
use super::{Backend, Completion, Delivery, Event, Events};

pub(super) struct Native;
impl Backend for Native {
    fn deliver(&self, completion: Completion, events: Events) -> Delivery {
        Box::pin(deliver(completion, events))
    }
}

#[cfg(all(unix, not(target_os = "macos")))]
async fn deliver(completion: Completion, events: Events) -> Result<(), String> {
    use notify_rust::{Hint, Notification, NotificationResponse};
    let mut notification = Notification::new();
    notification
        .appname("CodeConvoy")
        .summary(&completion.title)
        .body(&completion.body)
        .icon("codeconvoy")
        .hint(Hint::DesktopEntry("codeconvoy".into()))
        .action("default", "Open convoy");
    // Default urgency/expiration respect the desktop's own notification policy.
    let handle = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        notification.show_async(),
    )
    .await
    .map_err(|_| "Desktop notification service did not respond.".to_owned())?
    .map_err(|e| e.to_string())?;
    handle
        .wait_for_action_async(|response: &NotificationResponse| {
            if matches!(response, NotificationResponse::Default)
                || matches!(response, NotificationResponse::Action(action) if action == "default")
            {
                events.emit(Event::Activated(completion.run));
            }
        })
        .await;
    Ok(())
}

#[cfg(target_os = "macos")]
async fn deliver(completion: Completion, events: Events) -> Result<(), String> {
    use mac_usernotifications::{Action, Notification};
    // Serialize first-use OS authorization, including simultaneous completions.
    static AUTH: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
    {
        let _guard = AUTH.lock().await;
        let allowed = mac_usernotifications::request_auth()
            .await
            .map_err(|e| e.to_string())?;
        if !allowed {
            return Err(
                "Notifications are disabled for CodeConvoy in macOS System Settings.".into(),
            );
        }
    }
    // Uses the actual .app bundle identity; no impersonation of Finder/Terminal.
    let handle = Notification::new()
        .title(&completion.title)
        .message(&completion.body)
        .action(Action::button("open", "Open convoy"))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let response = handle.response().await.map_err(|e| e.to_string())?;
    if response.is_default_action() || response.action_identifier == "open" {
        events.emit(Event::Activated(completion.run));
    }
    Ok(())
}

#[cfg(windows)]
async fn deliver(completion: Completion, events: Events) -> Result<(), String> {
    // WinRT submission is synchronous; callbacks themselves never block or touch UI.
    tokio::task::spawn_blocking(move || -> Result<(), String> {
        const APP_ID: &str = "io.github.chrislauinger77.code-convoy";
        // Unpackaged/portable applications need their own per-user identity too.
        // This only registers display metadata, not a daemon or activation executable.
        let key = windows_registry::CURRENT_USER
            .create(format!(r"SOFTWARE\Classes\AppUserModelId\{APP_ID}"))
            .map_err(|e| e.to_string())?;
        key.set_string("DisplayName", "CodeConvoy")
            .map_err(|e| e.to_string())?;
        tauri_winrt_notification::Toast::new(APP_ID)
            .title(&completion.title)
            .text1(&completion.body)
            .on_activated(move |_| {
                events.emit(Event::Activated(completion.run));
                Ok(())
            })
            .show()
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}
