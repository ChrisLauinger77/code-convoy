//! Timers only for active lifecycle polling and the next eligible save.
use super::*;

impl App {
    fn repaint_delay(&self) -> Option<Duration> {
        if self.explicit_quit
            && self.history_write.is_none()
            && self.state_write.is_none()
            && self.export.pending.is_none()
        {
            return Some(Duration::ZERO);
        }
        // Runner events are bounded and validation updates are coalesced. Keep
        // their existing polling cadence, including hidden-window shutdown.
        // Metadata workers use messages::Sender and need no polling timer.
        let progress = (!self.manager.is_idle()
            || !self.validations.is_idle()
            || self.closing
            || self.state.runs.iter().any(Run::active)
            || self.result_operation.is_some()
            || self.bulk_discard.is_some()
            || self.worktree_management.batch.is_some())
        .then_some(Duration::from_millis(100));
        let save = self
            .can_save_background()
            .then(|| Duration::from_secs(2).saturating_sub(self.last_save.elapsed()));
        progress.into_iter().chain(save).min()
    }

    pub(super) fn schedule_repaint(&self, ctx: &egui::Context) {
        if let Some(delay) = self.repaint_delay() {
            ctx.request_repaint_after(delay);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::tests::{app, finish_state_write, run};

    #[test]
    fn idle_has_no_timer_and_dirty_state_has_a_save_deadline() {
        let (_temp, mut app) = app();
        app.dirty = false;
        assert_eq!(app.repaint_delay(), None);
        app.dirty = true;
        assert!(
            app.repaint_delay()
                .is_some_and(|delay| delay <= Duration::from_secs(2))
        );
        app.last_save = Instant::now() - Duration::from_secs(3);
        assert_eq!(app.repaint_delay(), Some(Duration::ZERO));
        app.save_background();
        // A reserved writer must not cause an overdue-save busy loop.
        app.dirty = true;
        app.last_save = Instant::now() - Duration::from_secs(3);
        assert_eq!(app.repaint_delay(), None);
        app.explicit_quit = true;
        assert_eq!(app.repaint_delay(), None);
        app.explicit_quit = false;
        finish_state_write(&mut app);
        assert_eq!(app.repaint_delay(), Some(Duration::ZERO));
    }

    #[test]
    fn active_work_and_quit_keep_lifecycle_polling() {
        let (_temp, mut app) = app();
        app.dirty = false;
        app.state.runs.push(run(1, &[JobStatus::Running]));
        assert_eq!(app.repaint_delay(), Some(Duration::from_millis(100)));
        app.state.runs[0].jobs[0].status = JobStatus::Succeeded;
        assert_eq!(app.repaint_delay(), None);
        app.explicit_quit = true;
        assert_eq!(app.repaint_delay(), Some(Duration::ZERO));
        app.explicit_quit = false;
        app.closing = true;
        assert_eq!(app.repaint_delay(), Some(Duration::from_millis(100)));
    }

    #[test]
    fn background_metadata_waits_for_messages_not_a_polling_timer() {
        let (_temp, mut app) = app();
        app.dirty = false;
        app.busy = true;
        app.attachment_work.pending = true;
        app.refresh_storage();
        assert!(app.storage.running());
        assert!(app.cli_checks.any_checking());
        assert_eq!(app.repaint_delay(), None);
    }
}
