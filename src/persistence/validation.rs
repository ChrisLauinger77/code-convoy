use super::Store;
use crate::{
    domain::AppState,
    validation::{
        Command,
        configuration::{Assignment, Report},
    },
};
use anyhow::Result;
use std::path::PathBuf;

impl Store {
    pub fn save_validation_command(
        &self,
        state: &mut AppState,
        path: PathBuf,
        command: Option<Command>,
    ) -> Result<()> {
        let mut staged = state.clone();
        staged.save_validation(path, command)?;
        self.save(&staged)?;
        state.repository_validation = staged.repository_validation;
        Ok(())
    }

    /// One atomic state replacement for all eligible targets. A failed save never
    /// leaks staged changes into the live state or its later background saves.
    pub fn assign_validation(&self, state: &mut AppState, assignment: &Assignment) -> Report {
        let mut staged = state.clone();
        let mut report = assignment.stage(&mut staged);
        if !report.updated.is_empty() {
            match self.save(&staged) {
                Ok(()) => state.repository_validation = staged.repository_validation,
                Err(error) => {
                    // Store::save can also fail during directory sync after rename.
                    // Do not claim the on-disk outcome is known in that case.
                    let message = format!(
                        "Could not confirm saving configuration: {error:#}. Fix state-directory access and retry, or reload to check the saved state."
                    );
                    report
                        .failed
                        .extend(report.updated.drain(..).map(|r| (r, message.clone())));
                }
            }
        }
        report
    }
}
