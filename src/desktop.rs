//! Optional desktop integration. No scheduler, process or repository ownership.
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(any(target_os = "macos", windows))]
mod native;
#[cfg(test)]
mod tests;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CloseBehavior {
    #[default]
    Quit,
    MinimizeToTray,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Preferences {
    pub enabled: bool,
    pub close_behavior: CloseBehavior,
    pub show_running_count: bool,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            enabled: false,
            close_behavior: CloseBehavior::Quit,
            show_running_count: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Show,
    ShowActive,
    Quit,
}

#[derive(Debug, Clone)]
pub enum Event {
    Ready,
    Unavailable(String),
    Action(Action),
}

/// Every callback is tagged by the service instance. Late events from a disabled
/// or replaced tray cannot restore a window or initiate shutdown.
#[derive(Clone)]
pub struct Events {
    generation: u64,
    send: Arc<dyn Fn(u64, Event) + Send + Sync>,
}
impl Events {
    pub fn new(send: impl Fn(u64, Event) + Send + Sync + 'static) -> Self {
        Self {
            generation: 0,
            send: Arc::new(send),
        }
    }
    fn emit(&self, event: Event) {
        (self.send)(self.generation, event);
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Status {
    #[default]
    Disabled,
    Starting,
    Available,
    Unavailable(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct MenuState {
    running: usize,
    show_count: bool,
}
impl MenuState {
    fn count_label(self) -> String {
        format!("Running Convoys: {}", self.running)
    }
    fn tooltip(self) -> String {
        if self.show_count {
            format!("CodeConvoy · {}", self.count_label())
        } else {
            "CodeConvoy".into()
        }
    }
}

trait Backend {
    fn update(&mut self, state: MenuState) -> Result<(), String>;
    // Used only immediately before a close-to-tray decision, never on a timer.
    fn check(&self) -> Result<(), String> {
        Ok(())
    }
}

pub struct Service {
    pub status: Status,
    generation: u64,
    enabled: bool,
    events: Events,
    backend: Option<Box<dyn Backend>>,
    menu: Option<MenuState>,
}
impl Service {
    pub fn new(events: Events) -> Self {
        Self {
            status: Status::Disabled,
            generation: 0,
            enabled: false,
            events,
            backend: None,
            menu: None,
        }
    }
    pub fn available(&self) -> bool {
        self.status == Status::Available
    }
    pub fn sync(
        &mut self,
        preferences: Preferences,
        running: usize,
        runtime: &tokio::runtime::Handle,
    ) {
        self.sync_with(preferences, running, |state, events| {
            #[cfg(target_os = "linux")]
            {
                linux::start(state, events, runtime)
            }
            #[cfg(any(target_os = "macos", windows))]
            {
                let _ = runtime;
                native::start(state, events)
            }
            #[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
            {
                let _ = (state, events, runtime);
                Err("System tray is unsupported on this platform.".into())
            }
        });
    }
    fn sync_with(
        &mut self,
        preferences: Preferences,
        running: usize,
        start: impl FnOnce(MenuState, Events) -> Result<Box<dyn Backend>, String>,
    ) {
        let menu = MenuState {
            running,
            show_count: preferences.show_running_count,
        };
        if self.enabled != preferences.enabled {
            self.stop();
            self.enabled = preferences.enabled;
            if self.enabled {
                self.status = Status::Starting;
                let mut events = self.events.clone();
                events.generation = self.generation;
                match start(menu, events) {
                    Ok(backend) => {
                        self.backend = Some(backend);
                        self.menu = Some(menu);
                    }
                    Err(error) => self.fail(error),
                }
            }
        }
        if self.menu != Some(menu)
            && let Some(backend) = &mut self.backend
        {
            match backend.update(menu) {
                Ok(()) => self.menu = Some(menu),
                Err(error) => self.fail(error),
            }
        }
    }
    pub fn event(&mut self, generation: u64, event: Event) -> Option<Action> {
        if generation != self.generation || !self.enabled {
            return None;
        }
        match event {
            Event::Ready if self.status == Status::Starting => self.status = Status::Available,
            Event::Unavailable(error) if !matches!(self.status, Status::Unavailable(_)) => {
                self.fail(error)
            }
            Event::Action(action) if self.available() => return Some(action),
            _ => {}
        }
        None
    }
    fn fail(&mut self, error: String) {
        self.status = Status::Unavailable(error);
        self.backend = None;
        self.menu = None;
    }
    pub fn can_minimize(&mut self, preferences: Preferences) -> bool {
        if !preferences.enabled
            || preferences.close_behavior != CloseBehavior::MinimizeToTray
            || !self.available()
        {
            return false;
        }
        if let Some(backend) = &self.backend
            && let Err(error) = backend.check()
        {
            self.fail(error);
            return false;
        }
        true
    }
    pub fn stop(&mut self) {
        self.generation += 1;
        self.enabled = false;
        self.backend = None;
        self.menu = None;
        self.status = Status::Disabled;
    }
}

/// One restore path for tray, native Quit and notifications. Coalesces a batch
/// of activations and defers focus until the OS has processed unminimization.
#[derive(Default)]
pub struct Window {
    pub tray_minimized: bool,
    restore: bool,
    focus_pending: bool,
}
impl Window {
    pub fn request_restore(&mut self) {
        self.restore = true;
    }
    pub fn minimize(&mut self, ctx: &eframe::egui::Context) {
        self.tray_minimized = true;
        self.restore = false;
        self.focus_pending = false;
        ctx.send_viewport_cmd(eframe::egui::ViewportCommand::Minimized(true));
    }
    pub fn restore(&mut self, ctx: &eframe::egui::Context) {
        use eframe::egui::ViewportCommand;
        if self.restore {
            ctx.send_viewport_cmd(ViewportCommand::Visible(true));
            ctx.send_viewport_cmd(ViewportCommand::Minimized(false));
            self.tray_minimized = false;
            self.restore = false;
            self.focus_pending = true;
            ctx.request_repaint();
        } else if self.focus_pending {
            ctx.send_viewport_cmd(ViewportCommand::Focus);
            self.focus_pending = false;
        }
    }
}
