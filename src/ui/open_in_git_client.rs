//! The one "Open in Git Client" action. Its directory comes from recorded
//! context: the registration in repository management, and the job's saved
//! execution snapshot in results and history. The user never chooses between
//! the original repository and a worktree, and nothing is substituted.
use super::*;
use crate::git_client::{Client, Error, Target};

/// Bundled fonts render this north-east arrow; see the glyph test.
pub(super) const ICON: char = '⬈';
const NAME: &str = "Open in Git Client";

/// Renders the action and returns whether it was activated. Hover text names
/// the exact directory, or why it cannot be opened.
pub(super) fn button(
    ui: &mut egui::Ui,
    client: Option<Client>,
    target: &Result<Target, Error>,
) -> bool {
    let (enabled, hint) = match (client, target) {
        (None, _) => (
            false,
            "Choose a Git client in Settings → Git client… to enable this action.".to_owned(),
        ),
        (Some(_), Err(error)) => (false, error.to_string()),
        (Some(client), Ok(target)) => (
            true,
            format!("Open the {} in {}.", target.describe(), client.label()),
        ),
    };
    let response = ui.add_enabled(enabled, theme::quiet(&format!("{ICON} {NAME}")).small());
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, enabled, NAME));
    let response = if enabled {
        response.on_hover_text(hint)
    } else {
        response.on_disabled_hover_text(hint)
    };
    if response.has_focus() {
        response.scroll_to_me(None);
    }
    response.clicked()
}

impl App {
    /// A job's recorded execution location, unless a result operation may be
    /// changing or removing it right now.
    pub(super) fn job_git_client_target(
        &self,
        run: u64,
        index: usize,
        job: &domain::Job,
    ) -> Result<Target, Error> {
        let target = Target::job(job)?;
        let busy = self.result_operation == Some((run, index))
            || (job.worktree.is_some()
                && (self.bulk_discard.is_some() || self.worktree_management.batch.is_some()));
        if busy && target.kind == crate::git_client::Kind::Worktree {
            return Err(Error::Target {
                path: Some(target.path),
                problem: "wait until the current Apply, Discard or cleanup finishes.".into(),
            });
        }
        Ok(target)
    }

    /// Verifies and launches off the UI thread; the outcome becomes a notice.
    pub(super) fn open_in_git_client(&self, target: Target) {
        let preferences = self.state.git_client.clone();
        let Some(client) = preferences.client.filter(|_| !self.closing) else {
            return;
        };
        let description = target.describe();
        self.dispatch(async move {
            Message::GitClientOpened(
                crate::git_client::open_target(preferences, target)
                    .await
                    .map(|()| format!("Opened the {description} in {}.", client.label()))
                    .map_err(|error| error.to_string()),
            )
        });
    }

    pub(super) fn git_client_opened(&mut self, result: Result<String, String>) {
        self.notice = match result {
            Ok(message) | Err(message) => message,
        };
    }
}
