//! Application-local shortcuts. Text editing and modal dismissal stay with egui.
use super::*;
use egui::{Event, Key, KeyboardShortcut, Modifiers};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Action {
    Run,
    FocusRepositories,
    View(Tab),
    DismissSearch,
}

struct Binding {
    shortcut: KeyboardShortcut,
    action: Action,
    label: &'static str,
}

impl Binding {
    fn display(&self, ctx: &egui::Context) -> String {
        if ctx.os().is_mac() {
            // The bundled font has ⌘, but not every modifier/arrow symbol tested
            // by Context::format_shortcut. These bindings only need Command.
            self.shortcut.format(
                &egui::ModifierNames {
                    mac_cmd: "⌘",
                    concat: " ",
                    ..egui::ModifierNames::NAMES
                },
                true,
            )
        } else {
            ctx.format_shortcut(&self.shortcut)
        }
    }
}

const fn command(key: Key, action: Action, label: &'static str) -> Binding {
    Binding {
        shortcut: KeyboardShortcut::new(Modifiers::COMMAND, key),
        action,
        label,
    }
}

const BINDINGS: &[Binding] = &[
    command(Key::Enter, Action::Run, "Run Convoy (review first)"),
    command(Key::F, Action::FocusRepositories, "Focus repository search"),
    command(Key::Num1, Action::View(Tab::Activity), "Activity"),
    command(Key::Num2, Action::View(Tab::Diff), "Diff"),
    command(Key::Num3, Action::View(Tab::Raw), "Raw Output"),
    command(Key::Num4, Action::View(Tab::Task), "Task & Settings"),
    Binding {
        shortcut: KeyboardShortcut::new(Modifiers::NONE, Key::Escape),
        action: Action::DismissSearch,
        label: "Close dialog / dismiss search focus",
    },
];

fn mapping(key: Key, modifiers: Modifiers) -> Option<Action> {
    // egui supplies `command` as Ctrl on Linux/Windows and Command on macOS.
    // matches_exact rejects Alt/Shift; reject an extra physical Ctrl on macOS too.
    if modifiers.ctrl && modifiers.mac_cmd {
        return None;
    }
    BINDINGS
        .iter()
        .find(|b| b.shortcut.logical_key == key && modifiers.matches_exact(b.shortcut.modifiers))
        .map(|b| b.action)
}

#[derive(Clone, Copy, Default)]
pub(super) struct Scope {
    pub blocked: bool,
    pub text_edit: bool,
    pub run_available: bool,
    pub filter_available: bool,
    pub filter_focused: bool,
    pub result_available: bool,
}

impl Scope {
    fn allows(&self, action: Action) -> bool {
        if self.blocked {
            return false;
        }
        match action {
            Action::Run => self.run_available && !self.text_edit,
            Action::FocusRepositories => self.filter_available,
            Action::View(_) => self.result_available && !self.text_edit,
            Action::DismissSearch => self.filter_focused,
        }
    }
}

#[derive(Default)]
pub(super) struct Dispatcher {
    frame: Option<u64>,
}

impl Dispatcher {
    pub fn take(&mut self, ctx: &egui::Context, scope: Scope) -> Option<Action> {
        let frame = ctx.cumulative_frame_nr();
        let first_pass = self.frame != Some(frame);
        self.frame = Some(frame);
        let mut action = None;
        ctx.input_mut(|input| {
            input.events.retain(|event| {
                let Event::Key {
                    key,
                    modifiers,
                    pressed: true,
                    repeat,
                    ..
                } = event
                else {
                    return true;
                };
                // One Escape must not cascade through stacked dialogs on key repeat
                // or a discarded layout pass. First presses use Modal::should_close.
                if *key == Key::Escape && (*repeat || !first_pass) {
                    return false;
                }
                if let Some(mapped) = mapping(*key, *modifiers)
                    && scope.allows(mapped)
                {
                    if first_pass && !repeat && action.is_none() {
                        action = Some(mapped);
                    }
                    return false;
                }
                // egui buttons activate on Enter regardless of modifiers. Swallow
                // modified Enter outside editors even when launch is unavailable,
                // especially while a confirmation button has focus. Leave all text
                // editor events (including native macOS Ctrl shortcuts) untouched.
                !(*key == Key::Enter && modifiers.any() && !scope.text_edit)
            });
        });
        action
    }
}

pub(super) fn repository_filter_id() -> egui::Id {
    egui::Id::new("repository_filter")
}

impl App {
    fn shortcut_modal_open(&self) -> bool {
        self.prepared.is_some()
            || self.about_open
            || self.shortcuts_open
            || self.cli_search.is_some()
            || self.library_editor.is_some()
            || self.validation_editor.is_some()
            || self.discard_confirmation.is_some()
            || self.bulk_confirmation.is_some()
            || self.quit_requested
            || self.closing
            || self.browsing_repository
            || self.attachment_work.pending
            || self.worktree_location.pending
    }

    pub(super) fn repository_filter_present(&self) -> bool {
        !self.state.repositories.is_empty() || !self.repository_sections.query.is_empty()
    }

    pub(super) fn can_run_convoy(&self) -> bool {
        !self.shortcut_modal_open()
            && !self.busy
            && !self.followup_pending
            && self.attachment_error().is_none()
            && !self.current_cli_check()
            && !self.selected.is_empty()
            && !self.state.draft.prompt.trim().is_empty()
            && agents::backend(self.state.draft.agent).is_ok()
    }

    /// Both the button and shortcut enter the unchanged asynchronous preflight.
    pub(super) fn run_convoy(&mut self, ctx: &egui::Context) {
        if self.can_run_convoy() {
            self.preflight(ctx.clone());
        }
    }

    pub(super) fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        let scope = Scope {
            blocked: self.shortcut_modal_open()
                || ctx.memory(|m| m.top_modal_layer().is_some())
                || egui::Popup::is_any_open(ctx)
                || !ctx.input(|i| i.focused),
            text_edit: ctx.text_edit_focused(),
            run_available: self.can_run_convoy(),
            filter_available: self.repository_filter_present() && !self.followup_pending,
            filter_focused: ctx.memory(|m| m.has_focus(repository_filter_id())),
            result_available: self
                .state
                .runs
                .iter()
                .any(|r| Some(r.id) == self.selected_run),
        };
        match self.shortcuts.take(ctx, scope) {
            Some(Action::Run) => self.run_convoy(ctx),
            Some(Action::FocusRepositories) => {
                self.focus_draft = false;
                self.focus_repository_input = false;
                self.focus_repository_filter = true;
                // Move focus and selection before any editor processes this frame's
                // events: text typed immediately after the chord belongs here.
                let id = repository_filter_id();
                ctx.memory_mut(|m| m.request_focus(id));
                let mut state = egui::TextEdit::load_state(ctx, id).unwrap_or_default();
                state
                    .cursor
                    .set_char_range(Some(egui::text::CCursorRange::two(
                        egui::text::CCursor::new(0),
                        egui::text::CCursor::new(self.repository_sections.query.chars().count()),
                    )));
                state.store(ctx, id);
            }
            Some(Action::View(tab)) => self.tab = tab,
            Some(Action::DismissSearch) => {
                ctx.memory_mut(|m| m.surrender_focus(repository_filter_id()));
            }
            None => {}
        }
    }

    pub(super) fn shortcuts_window(&mut self, ctx: &egui::Context) {
        if !self.shortcuts_open {
            return;
        }
        let mut close = false;
        let response = egui::Modal::new(egui::Id::new("keyboard_shortcuts")).show(ctx, |ui| {
            ui.set_width(420.0_f32.min((ctx.content_rect().width() - 64.0).max(240.0)));
            ui.heading("Keyboard Shortcuts");
            egui::Grid::new("shortcut_reference").spacing([18.0, 6.0]).show(ui, |ui| {
                for binding in BINDINGS {
                    ui.label(binding.display(ctx));
                    ui.label(binding.label);
                    ui.end_row();
                }
            });
            ui.separator();
            ui.small("Run and view shortcuts pause while editing text. Run opens the usual review and requires a ready task and selected repositories.");
            ui.small("Search requires the repository filter. Views require a selected convoy. Dialogs and open menus take precedence; Escape cancels safely and keeps the search query.");
            ui.small("Shortcuts work while this window is focused. Your desktop may reserve a key combination.");
            close = ui.button("Close shortcuts").clicked();
        });
        if close || response.should_close() {
            self.shortcuts_open = false;
        }
    }
}

#[cfg(test)]
#[path = "shortcut_mapping_tests.rs"]
mod tests;
