//! SNI over session D-Bus, with no GTK loop and no X11 window-management code.
use super::{Action, Backend, Event, Events, MenuState};
use ksni::TrayMethods;
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use tokio::sync::watch;

struct Item {
    state: MenuState,
    events: Events,
    online: Arc<AtomicBool>,
    icon: ksni::Icon,
}
impl ksni::Tray for Item {
    fn category(&self) -> ksni::Category {
        ksni::Category::ApplicationStatus
    }
    fn id(&self) -> String {
        "codeconvoy".into()
    }
    fn title(&self) -> String {
        self.state.tooltip()
    }
    fn tool_tip(&self) -> ksni::ToolTip {
        ksni::ToolTip {
            title: self.state.tooltip(),
            ..Default::default()
        }
    }
    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        vec![self.icon.clone()]
    }
    fn activate(&mut self, _: i32, _: i32) {
        self.events.emit(Event::Action(Action::Show));
    }
    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        use ksni::menu::StandardItem;
        let action = |label: &str, action| {
            StandardItem {
                label: label.into(),
                activate: Box::new(move |item: &mut Self| item.events.emit(Event::Action(action))),
                ..Default::default()
            }
            .into()
        };
        let mut menu = vec![action("Show CodeConvoy", Action::Show)];
        if self.state.show_count {
            menu.push(
                StandardItem {
                    label: self.state.count_label(),
                    enabled: false,
                    ..Default::default()
                }
                .into(),
            );
        }
        menu.push(action("Show Active Convoys", Action::ShowActive));
        menu.push(action("Quit CodeConvoy", Action::Quit));
        menu
    }
    fn watcher_offline(&self, reason: ksni::OfflineReason) -> bool {
        self.online.store(false, Ordering::Release);
        self.events.emit(Event::Unavailable(format!(
            "Linux tray host unavailable: {reason:?}."
        )));
        false // Stop once. Re-enabling the setting is the only initialization retry.
    }
}

struct LinuxTray {
    updates: watch::Sender<MenuState>,
    task: tokio::task::JoinHandle<()>,
    online: Arc<AtomicBool>,
}
struct Shutdown(ksni::Handle<Item>);
impl Drop for Shutdown {
    fn drop(&mut self) {
        drop(self.0.shutdown());
    }
}

pub(super) fn start(
    state: MenuState,
    events: Events,
    runtime: &tokio::runtime::Handle,
) -> Result<Box<dyn Backend>, String> {
    let image =
        eframe::icon_data::from_png_bytes(include_bytes!("../../assets/codeconvoy-256.png"))
            .map_err(|e| e.to_string())?;
    let mut argb = image.rgba;
    for pixel in argb.as_chunks_mut::<4>().0 {
        pixel.rotate_right(1);
    }
    let icon = ksni::Icon {
        width: image.width as i32,
        height: image.height as i32,
        data: argb,
    };
    let (updates, mut receiver) = watch::channel(state);
    let online = Arc::new(AtomicBool::new(false));
    let active = online.clone();
    let task = runtime.spawn(async move {
        let item = Item {
            state,
            events: events.clone(),
            online: active.clone(),
            icon,
        };
        // Never assume SNI support; bare GNOME must fail safely and quietly.
        let handle = match tokio::time::timeout(Duration::from_secs(5), item.spawn()).await {
            Ok(Ok(handle)) => Shutdown(handle),
            result => {
                let error = match result {
                    Ok(Err(e)) => e.to_string(),
                    _ => "Timed out connecting to the Linux tray host".into(),
                };
                events.emit(Event::Unavailable(error));
                return;
            }
        };
        active.store(true, Ordering::Release);
        events.emit(Event::Ready);
        while receiver.changed().await.is_ok() {
            let state = *receiver.borrow_and_update();
            match tokio::time::timeout(
                Duration::from_secs(5),
                handle.0.update(|item| item.state = state),
            )
            .await
            {
                Ok(Some(())) => {}
                _ => {
                    active.store(false, Ordering::Release);
                    events.emit(Event::Unavailable(
                        "Linux tray service stopped responding.".into(),
                    ));
                    break;
                }
            }
        }
    });
    Ok(Box::new(LinuxTray {
        updates,
        task,
        online,
    }))
}
impl Backend for LinuxTray {
    fn update(&mut self, state: MenuState) -> Result<(), String> {
        self.updates
            .send(state)
            .map_err(|_| "Linux tray service stopped.".into())
    }
    fn check(&self) -> Result<(), String> {
        if self.online.load(Ordering::Acquire) && !self.task.is_finished() {
            Ok(())
        } else {
            Err("Linux tray host unavailable.".into())
        }
    }
}
impl Drop for LinuxTray {
    fn drop(&mut self) {
        self.task.abort();
    }
}
