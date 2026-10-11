//! Small focus lifecycle around egui's modal input barrier.
use eframe::egui;

#[derive(Default)]
struct Focus {
    stack: Vec<Session>,
    outside: Option<egui::Id>,
}

struct Session {
    id: egui::Id,
    opener: Option<egui::Id>,
    last_focus: Option<egui::Id>,
    shown: bool,
}

pub(super) fn install(ctx: &egui::Context) {
    ctx.add_plugin(Focus::default());
}

impl egui::plugin::Plugin for Focus {
    fn debug_name(&self) -> &'static str {
        "CodeConvoy dialog focus"
    }

    fn on_begin_pass(&mut self, _: &mut egui::Ui) {
        for session in &mut self.stack {
            session.shown = false;
        }
    }

    fn on_end_pass(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx();
        let mut restore = None;
        let mut closed = Vec::new();
        // Keep a hidden parent while its child is open (e.g. Storage cleanup).
        // Closing/replacing several dialogs unwinds to the surviving scope.
        while self.stack.last().is_some_and(|s| !s.shown) {
            if let Some(session) = self.stack.pop() {
                closed.push(session.id);
                restore = session.opener;
            }
        }
        let focused = ctx.memory(|m| m.focused());
        let current = focused.and_then(|id| ctx.read_response(id));
        let belongs = |response: &egui::Response| {
            self.stack
                .last()
                .map_or(response.layer_id.order != egui::Order::Foreground, |s| {
                    response.layer_id.id == s.id
                })
        };
        // Explicit focus changes made by the app or user take precedence.
        if !closed.is_empty()
            && current
                .as_ref()
                .is_none_or(|r| closed.contains(&r.layer_id.id))
            && let Some(response) = restore.and_then(|id| ctx.read_response(id))
            && response.enabled()
            && response.sense.is_focusable()
            && belongs(&response)
        {
            response.request_focus();
            ctx.request_repaint();
        }
        if let Some(response) = ctx
            .memory(|m| m.focused())
            .and_then(|id| ctx.read_response(id))
            && belongs(&response)
        {
            if let Some(session) = self.stack.last_mut() {
                session.last_focus = Some(response.id);
            } else {
                // Menu items disappear when they open a dialog. Remember their
                // launcher, rather than restoring a now-hidden popup item.
                self.outside = Some(response.id);
            }
        }
    }
}

pub(super) fn show<R>(
    ctx: &egui::Context,
    id: &'static str,
    contents: impl FnOnce(&mut egui::Ui, bool) -> R,
) -> egui::ModalResponse<R> {
    install(ctx);
    let id = egui::Id::new(id);
    let focused = ctx
        .memory(|m| m.focused())
        .and_then(|id| ctx.read_response(id));
    let opening = ctx.with_plugin(|focus: &mut Focus| {
        if let Some(session) = focus.stack.iter_mut().find(|s| s.id == id) {
            session.shown = true;
            return false;
        }
        let parent = focus.stack.last();
        let opener = focused
            .as_ref()
            .filter(|r| {
                parent.map_or(r.layer_id.order != egui::Order::Foreground, |s| {
                    r.layer_id.id == s.id
                })
            })
            .map(|r| r.id)
            .or_else(|| parent.and_then(|s| s.last_focus).or(focus.outside));
        focus.stack.push(Session {
            id,
            opener,
            last_focus: None,
            shown: true,
        });
        true
    });
    egui::Modal::new(id).show(ctx, |ui| contents(ui, opening == Some(true)))
}
