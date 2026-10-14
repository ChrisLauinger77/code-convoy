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
    // Validate the embedded glyph before reporting the tray as starting.
    super::tray_glyph(false)?;
    let (updates, mut receiver) = watch::channel(state);
    let online = Arc::new(AtomicBool::new(false));
    let active = online.clone();
    let task = runtime.spawn(async move {
        let desktop = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default();
        let light = light_glyph(&desktop, color_scheme().await);
        let icon = match super::tray_glyph(light) {
            Ok(image) => {
                let mut argb = image.rgba;
                for pixel in argb.as_chunks_mut::<4>().0 {
                    pixel.rotate_right(1);
                }
                ksni::Icon {
                    width: image.width as i32,
                    height: image.height as i32,
                    data: argb,
                }
            }
            Err(error) => {
                events.emit(Event::Unavailable(error));
                return;
            }
        };
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
/// SNI pixmaps are not recolored by hosts. GNOME Shell's top bar stays dark in
/// either style; other panels follow the desktop color scheme. Only an explicit
/// light preference selects the black glyph.
fn light_glyph(desktop: &str, color_scheme: Option<u32>) -> bool {
    const PREFER_LIGHT: u32 = 2;
    desktop
        .split(':')
        .any(|name| name.eq_ignore_ascii_case("GNOME"))
        || color_scheme != Some(PREFER_LIGHT)
}

/// XDG Settings portal `org.freedesktop.appearance` `color-scheme`, if readable.
async fn color_scheme() -> Option<u32> {
    let read = async {
        let connection = zbus::Connection::session().await.ok()?;
        let reply = connection
            .call_method(
                Some("org.freedesktop.portal.Desktop"),
                "/org/freedesktop/portal/desktop",
                Some("org.freedesktop.portal.Settings"),
                "ReadOne",
                &("org.freedesktop.appearance", "color-scheme"),
            )
            .await
            .ok()?;
        let value: zbus::zvariant::OwnedValue = reply.body().deserialize().ok()?;
        u32::try_from(value).ok()
    };
    tokio::time::timeout(Duration::from_secs(2), read)
        .await
        .ok()
        .flatten()
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

#[cfg(test)]
#[test]
fn glyph_contrasts_with_gnome_bar_and_explicit_light_panels_only() {
    assert!(light_glyph("ubuntu:GNOME", Some(2)));
    assert!(light_glyph("GNOME", None));
    assert!(!light_glyph("KDE", Some(2)));
    assert!(light_glyph("KDE", Some(1)));
    assert!(light_glyph("KDE", Some(0)));
    assert!(light_glyph("", None));
}
