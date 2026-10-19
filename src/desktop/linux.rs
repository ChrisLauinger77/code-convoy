//! SNI over session D-Bus, with no GTK loop and no X11 window-management code.
use super::{Action, Backend, Event, Events, MenuState};
use ksni::TrayMethods;
use std::{
    num::NonZeroUsize,
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
enum Change {
    State(MenuState),
    Icon(ksni::Icon),
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
        let mut appearance = Appearance::read().await;
        let icon = match sni_icon(appearance.light()) {
            Ok(icon) => icon,
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
        loop {
            let change = tokio::select! {
                changed = receiver.changed() => {
                    if changed.is_err() {
                        break;
                    }
                    Change::State(*receiver.borrow_and_update())
                }
                color_scheme = appearance.changed() => {
                    appearance.color_scheme = Some(color_scheme);
                    // The glyph was validated at startup; keep the current one if not.
                    let Ok(icon) = sni_icon(appearance.light()) else {
                        continue;
                    };
                    Change::Icon(icon)
                }
            };
            let update = handle.0.update(|item| match change {
                Change::State(state) => item.state = state,
                Change::Icon(icon) => item.icon = icon,
            });
            if !matches!(
                tokio::time::timeout(Duration::from_secs(5), update).await,
                Ok(Some(()))
            ) {
                active.store(false, Ordering::Release);
                events.emit(Event::Unavailable(
                    "Linux tray service stopped responding.".into(),
                ));
                break;
            }
        }
    });
    Ok(Box::new(LinuxTray {
        updates,
        task,
        online,
    }))
}
/// SNI pixmaps are ARGB in network byte order.
fn sni_icon(light: bool) -> Result<ksni::Icon, String> {
    let image = super::tray_glyph(light)?;
    let mut argb = image.rgba;
    for pixel in argb.as_chunks_mut::<4>().0 {
        pixel.rotate_right(1);
    }
    Ok(ksni::Icon {
        width: image.width as i32,
        height: image.height as i32,
        data: argb,
    })
}

/// SNI pixmaps are not recolored by hosts. GNOME Shell's top bar stays dark in
/// either style; other panels follow the desktop color scheme. Only an explicit
/// light preference selects the black glyph.
fn light_glyph(gnome_shell: bool, color_scheme: Option<u32>) -> bool {
    const PREFER_LIGHT: u32 = 2;
    gnome_shell || color_scheme != Some(PREFER_LIGHT)
}

const SETTINGS: &str = "org.freedesktop.portal.Settings";
const APPEARANCE: &str = "org.freedesktop.appearance";
const COLOR_SCHEME: &str = "color-scheme";

/// Panel contrast inputs. GNOME Shell is detected by its bus name because
/// `XDG_CURRENT_DESKTOP` may list `GNOME` only as a compatibility entry (for
/// example `Budgie:GNOME`). Elsewhere the XDG Settings portal `color-scheme` is
/// read once and then followed through `SettingChanged`.
struct Appearance {
    gnome_shell: bool,
    color_scheme: Option<u32>,
    changes: Option<zbus::MessageStream>,
}
impl Appearance {
    fn light(&self) -> bool {
        light_glyph(self.gnome_shell, self.color_scheme)
    }

    async fn read() -> Self {
        let read = async {
            let connection = zbus::Connection::session().await.ok()?;
            let gnome_shell = async {
                let reply = connection
                    .call_method(
                        Some("org.freedesktop.DBus"),
                        "/org/freedesktop/DBus",
                        Some("org.freedesktop.DBus"),
                        "NameHasOwner",
                        &("org.gnome.Shell",),
                    )
                    .await
                    .ok()?;
                reply.body().deserialize::<bool>().ok()
            }
            .await
            .unwrap_or(false);
            if gnome_shell {
                return Some(Self {
                    gnome_shell,
                    color_scheme: None,
                    changes: None,
                });
            }
            // Subscribe before reading so a change in between is not lost.
            let changes = async {
                let rule = zbus::MatchRule::builder()
                    .msg_type(zbus::message::Type::Signal)
                    .interface(SETTINGS)
                    .member("SettingChanged")
                    .arg(0, APPEARANCE)
                    .arg(1, COLOR_SCHEME)
                    .build()
                    .ok()?;
                zbus::MessageStream::for_match_rule(rule, &connection, NonZeroUsize::new(4))
                    .await
                    .ok()
            }
            .await;
            let color_scheme = async {
                let reply = connection
                    .call_method(
                        Some("org.freedesktop.portal.Desktop"),
                        "/org/freedesktop/portal/desktop",
                        Some(SETTINGS),
                        "ReadOne",
                        &(APPEARANCE, COLOR_SCHEME),
                    )
                    .await
                    .ok()?;
                let value: zbus::OwnedVariant = reply.body().deserialize().ok()?;
                u32::try_from(value).ok()
            }
            .await;
            Some(Self {
                gnome_shell,
                color_scheme,
                changes,
            })
        };
        tokio::time::timeout(Duration::from_secs(2), read)
            .await
            .ok()
            .flatten()
            .unwrap_or(Self {
                gnome_shell: false,
                color_scheme: None,
                changes: None,
            })
    }

    /// Next portal `color-scheme` value. Pending forever without a subscription.
    async fn changed(&mut self) -> u32 {
        use futures_lite::StreamExt;
        while let Some(changes) = &mut self.changes {
            match changes.next().await {
                Some(Ok(message)) => {
                    if let Ok((_, _, value)) = message
                        .body()
                        .deserialize::<(String, String, zbus::OwnedVariant)>()
                        && let Ok(value) = u32::try_from(value)
                    {
                        return value;
                    }
                }
                Some(Err(_)) => {}
                None => self.changes = None,
            }
        }
        std::future::pending().await
    }
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
fn glyph_contrasts_with_gnome_shell_and_explicit_light_panels_only() {
    assert!(light_glyph(true, Some(2)));
    assert!(light_glyph(true, None));
    assert!(!light_glyph(false, Some(2)));
    assert!(light_glyph(false, Some(1)));
    assert!(light_glyph(false, Some(0)));
    assert!(light_glyph(false, None));
}
