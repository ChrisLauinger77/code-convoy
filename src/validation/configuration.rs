//! Pure configuration policy. Canonical registered paths are repository identities.
//! No filesystem inspection, Git, execution, or convoy selection is involved.
use super::{Command, presets::Preset};
use crate::domain::{AppState, Repository, RepositoryGroup};
use std::{collections::BTreeSet, path::PathBuf};

#[derive(Default)]
pub struct Selection {
    pub paths: BTreeSet<PathBuf>,
}
impl Selection {
    // Groups only edit this dialog's explicit path set. Individual deselection
    // wins until the user explicitly selects a group again.
    pub fn select_group(&mut self, group: &RepositoryGroup, selected: bool) {
        for path in &group.repositories {
            if selected {
                self.paths.insert(path.clone());
            } else {
                self.paths.remove(path);
            }
        }
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub enum Existing {
    #[default]
    Preserve,
    Overwrite,
}

pub struct Target {
    pub repository: Repository,
    pub previous: Option<Command>,
    registered: bool,
}

/// Immutable review of both target identities and commands to be replaced.
pub struct Assignment {
    pub preset_name: &'static str,
    pub command: Command,
    pub targets: Vec<Target>,
    pub existing: Existing,
}
impl Assignment {
    pub fn review(
        state: &AppState,
        selection: &Selection,
        preset: &Preset,
        existing: Existing,
    ) -> Self {
        Self {
            preset_name: preset.name,
            command: preset.command(),
            targets: selection
                .paths
                .iter()
                .map(|path| {
                    let repository = state.repositories.iter().find(|r| r.path == *path);
                    Target {
                        registered: repository.is_some(),
                        repository: repository.cloned().unwrap_or_else(|| Repository {
                            path: path.clone(),
                            name: "Unregistered repository".into(),
                        }),
                        previous: state.repository_validation.get(path).cloned(),
                    }
                })
                .collect(),
            existing,
        }
    }
    pub fn existing_count(&self) -> usize {
        self.targets
            .iter()
            .filter(|t| t.registered && t.previous.is_some())
            .count()
    }
    pub fn missing_count(&self) -> usize {
        self.targets.iter().filter(|t| !t.registered).count()
    }
    /// Apply only to staged application state. The Store publishes it after saving.
    pub(crate) fn stage(&self, state: &mut AppState) -> Report {
        let mut report = Report::default();
        for target in &self.targets {
            let repo = &target.repository;
            let current = state.repository_validation.get(&repo.path);
            if !target.registered || !state.repositories.iter().any(|r| r.path == repo.path) {
                report.failed.push((
                    repo.clone(),
                    "Repository is no longer registered. Select targets again.".into(),
                ));
            } else if current.is_some() && self.existing == Existing::Preserve {
                report.skipped.push(repo.clone());
            } else if self.existing == Existing::Overwrite && current != target.previous.as_ref() {
                report.failed.push((
                    repo.clone(),
                    "Configuration changed since review. Review again before replacing it.".into(),
                ));
            } else {
                match state.save_validation(repo.path.clone(), Some(self.command.clone())) {
                    Ok(()) => report.updated.push(repo.clone()),
                    Err(error) => report.failed.push((repo.clone(), error.to_string())),
                }
            }
        }
        report
    }
}

#[derive(Default)]
pub struct Report {
    pub updated: Vec<Repository>,
    pub skipped: Vec<Repository>,
    pub failed: Vec<(Repository, String)>,
}
impl Report {
    pub fn summary(&self) -> String {
        format!(
            "{} repositories updated, {} existing configurations preserved, {} failed.",
            self.updated.len(),
            self.skipped.len(),
            self.failed.len()
        )
    }
}
