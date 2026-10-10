//! Native menu objects live and are dropped on eframe's UI thread.
use super::{Action, Backend, Event, Events, MenuState};
use tray_icon::{
    Icon, TrayIcon, TrayIconBuilder,
    menu::{Menu, MenuEvent, MenuId, MenuItem},
};

use std::sync::{Mutex, Once};

// muda's handler is a OnceCell. Install once and replace only this routing table,
// so disable/re-enable never leaves callbacks bound to a dead tray generation.
#[derive(Clone)]
struct Routing {
    ids: [(MenuId, Action); 3],
    events: Events,
}
static ROUTING: Mutex<Option<Routing>> = Mutex::new(None);
static INSTALL: Once = Once::new();

fn route(event: MenuEvent) {
    let routing = ROUTING.lock().ok().and_then(|routing| routing.clone());
    if let Some(routing) = routing
        && let Some((_, action)) = routing.ids.iter().find(|(id, _)| *id == event.id)
    {
        routing.events.emit(Event::Action(*action));
    }
}

struct NativeTray {
    icon: TrayIcon,
    menu: Menu,
    count: MenuItem,
    count_visible: bool,
    #[cfg(windows)]
    tooltip: String,
}

pub(super) fn start(state: MenuState, events: Events) -> Result<Box<dyn Backend>, String> {
    let menu = Menu::new();
    let show = MenuItem::new("Show CodeConvoy", true, None);
    let count = MenuItem::new(state.count_label(), false, None);
    let active = MenuItem::new("Show Active Convoys", true, None);
    let quit = MenuItem::new("Quit CodeConvoy", true, None);
    menu.append(&show).map_err(|e| e.to_string())?;
    if state.show_count {
        menu.append(&count).map_err(|e| e.to_string())?;
    }
    menu.append_items(&[&active, &quit])
        .map_err(|e| e.to_string())?;
    let image =
        eframe::icon_data::from_png_bytes(include_bytes!("../../assets/codeconvoy-256.png"))
            .map_err(|e| e.to_string())?;
    let icon = Icon::from_rgba(image.rgba, image.width, image.height).map_err(|e| e.to_string())?;
    let icon = TrayIconBuilder::new()
        .with_menu(Box::new(menu.clone()))
        .with_tooltip(state.tooltip())
        .with_icon(icon)
        .build()
        .map_err(|e| e.to_string())?;
    // On Windows creation can succeed before Explorer has registered the icon.
    // NIM_MODIFY confirms registration; failure must not enable close-to-tray.
    icon.set_tooltip(Some(state.tooltip()))
        .map_err(|e| e.to_string())?;
    let ids = [
        (show.id().clone(), Action::Show),
        (active.id().clone(), Action::ShowActive),
        (quit.id().clone(), Action::Quit),
    ];
    INSTALL.call_once(|| MenuEvent::set_event_handler(Some(route)));
    *ROUTING
        .lock()
        .map_err(|_| "Tray event routing unavailable")? = Some(Routing {
        ids,
        events: events.clone(),
    });
    events.emit(Event::Ready);
    Ok(Box::new(NativeTray {
        icon,
        menu,
        count,
        count_visible: state.show_count,
        #[cfg(windows)]
        tooltip: state.tooltip(),
    }))
}
impl Backend for NativeTray {
    fn update(&mut self, state: MenuState) -> Result<(), String> {
        self.count.set_text(state.count_label());
        #[cfg(windows)]
        {
            self.tooltip = state.tooltip();
        }
        if state.show_count != self.count_visible {
            if state.show_count {
                self.menu.insert(&self.count, 1)
            } else {
                self.menu.remove(&self.count)
            }
            .map_err(|e| e.to_string())?;
            self.count_visible = state.show_count;
        }
        self.icon
            .set_tooltip(Some(state.tooltip()))
            .map_err(|e| e.to_string())
    }
    fn check(&self) -> Result<(), String> {
        // Event-driven health check at the point of minimization, not polling.
        #[cfg(windows)]
        self.icon
            .set_tooltip(Some(&self.tooltip))
            .map_err(|e| e.to_string())?;
        Ok(())
    }
}
impl Drop for NativeTray {
    fn drop(&mut self) {
        if let Ok(mut routing) = ROUTING.lock() {
            *routing = None;
        }
    }
}

#[cfg(test)]
#[test]
#[allow(clippy::unwrap_used)]
fn native_menu_routing_replaces_ids_across_enable_cycles() {
    let (tx, rx) = std::sync::mpsc::channel();
    for generation in [1, 2] {
        let tx = tx.clone();
        let mut events = Events::new(move |generation, event| {
            tx.send((generation, event)).unwrap();
        });
        events.generation = generation;
        let ids = [
            (MenuId::new(format!("show-{generation}")), Action::Show),
            (
                MenuId::new(format!("active-{generation}")),
                Action::ShowActive,
            ),
            (MenuId::new(format!("quit-{generation}")), Action::Quit),
        ];
        *ROUTING.lock().unwrap() = Some(Routing {
            ids: ids.clone(),
            events,
        });
        for (id, action) in ids {
            route(MenuEvent { id });
            let (received_generation, event) = rx.try_recv().unwrap();
            assert_eq!(received_generation, generation);
            assert!(matches!(event, Event::Action(received) if received == action));
        }
        *ROUTING.lock().unwrap() = None;
        route(MenuEvent {
            id: MenuId::new(format!("quit-{generation}")),
        });
        assert!(rx.try_recv().is_err());
    }
}
