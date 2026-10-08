//! Presentation-only summaries. Original diagnostics remain available verbatim.
use super::*;

pub(super) use crate::agents::availability::CliError;
impl CliError {
    pub fn label(&self) -> &'static str {
        if self.missing {
            "Unavailable"
        } else {
            "Invalid configuration"
        }
    }
    pub fn hint(&self) -> &'static str {
        if self.missing {
            "Executable not found. Use Find CLI, enter its absolute path, or install the CLI separately."
        } else {
            "Check this backend's settings and executable compatibility."
        }
    }
}

pub(super) fn summary(detail: &str) -> &str {
    if detail.starts_with("Retry blocked") {
        if detail.contains("Cannot locate attachment")
            || detail.contains("Attachment is missing or unavailable")
        {
            "Retry blocked: an original attachment is missing. Restore it before retrying; context cannot be omitted."
        } else if detail.contains("Cannot read attachment") {
            "Retry blocked: an original attachment cannot be read. Restore file access before retrying."
        } else if detail.contains("Attachment changed since") {
            "Retry blocked: an original attachment changed. Restore the original file, or use Reuse convoy to review an edited task."
        } else if detail.contains("cannot supply the requested image attachments") {
            "Retry blocked: this CLI cannot accept the original images. Restore a compatible CLI before retrying."
        } else if detail.contains("repository is unavailable") {
            "Retry blocked: repository is unavailable. Restore its registered location and Git state."
        } else if detail.contains("no longer registered") {
            "Retry blocked: repository is no longer registered. Register it again before retrying."
        } else {
            "Retry blocked. Check the original backend settings, executable and context in Diagnostics, then try again."
        }
    } else if detail.starts_with("Apply blocked:")
        || detail.starts_with("Apply preflight passed")
        || detail.starts_with("Cleanup completed")
        || detail.starts_with("Result discarded")
    {
        detail.lines().next().unwrap_or(detail)
    } else if detail.starts_with("Convoys with unresolved isolated results") {
        "Convoys with unresolved isolated results stay in history. Their files are preserved."
    } else if detail.starts_with("Unreferenced worktree resources found") {
        detail.split('\n').next().unwrap_or(detail)
    } else if detail.starts_with("Could not clean isolated result")
        || detail.starts_with("Cleanup did not finish")
    {
        "Cleanup did not complete. Result metadata is preserved; inspect Diagnostics before retrying."
    } else if detail.starts_with("Isolated result unavailable")
        && detail.contains("Base commit is unavailable")
    {
        "Isolated base commit is unavailable. Restore repository objects before inspecting or cleaning this result."
    } else if detail.contains("Base commit is unavailable") {
        "Base commit is unavailable. Check the repository, then launch a fresh convoy."
    } else if detail.contains("worktree creation failed")
        || detail.contains("Could not prepare isolated worktree")
    {
        "Could not create isolated worktree. Check Git, storage permissions and free space in Diagnostics. Any partial state is retained."
    } else if detail.contains("worktree storage") || detail.contains("worktree directory") {
        "Worktree storage is unavailable. Check the data directory permissions and free space."
    } else if detail.contains("ownership")
        || detail.contains("Worktree path now resolves")
        || detail.contains("Worktree belongs")
    {
        "Isolated result could not be verified. Inspect its location and Git state; no cleanup was performed."
    } else if detail.starts_with("Some attachments were not added") {
        "Some files were not added. Check file access, supported types and size limits in Diagnostics, then add them again."
    } else if detail.starts_with("Reused convoy has missing")
        || detail.starts_with("Attachment needs attention")
    {
        "Reused attachments need attention. Restore and re-add them, or explicitly remove them before launching."
    } else if detail.contains("Attachment changed since") {
        "An attachment changed. Remove and re-add it to accept the new content, then review the convoy again."
    } else if detail.contains("Attachment is missing or unavailable")
        || detail.contains("Cannot locate attachment")
    {
        "An attachment disappeared. Restore it, or remove its reference before reviewing the convoy again."
    } else if detail.contains("Cannot read attachment") {
        "An attachment cannot be read. Check file permissions, then remove and re-add it."
    } else if detail.contains("cannot supply the requested image attachments") {
        "This CLI cannot accept the selected images. Update the CLI, choose a compatible backend, or remove the images."
    } else if detail.starts_with("Group ") && detail.contains("unavailable or unregistered") {
        "Some group members were not selected. Restore and refresh their repositories, or repair membership in Manage groups."
    } else if detail.contains("Repository state changed after preflight") {
        "Repository changed since review. Inspect it, then launch a fresh convoy."
    } else if detail.starts_with("Cannot start ") {
        "Unable to start the process. Check the executable and repository path."
    } else if detail.starts_with("Repository path does not exist")
        || detail.starts_with("Git inspection failed")
    {
        "Repository unavailable. Check its path and Git state, then refresh."
    } else if detail.contains("completion was not confirmed") {
        "Agent completion could not be verified. Inspect Activity, Raw output and diagnostics."
    } else if detail.starts_with("Cannot save run") || detail.starts_with("Could not save state") {
        "Local state could not be saved. Check the data directory and diagnostics."
    } else {
        detail
            .split('\n')
            .next()
            .unwrap_or(detail)
            .split(": ")
            .next()
            .unwrap_or(detail)
    }
}

pub(super) fn details(ui: &mut egui::Ui, id: impl egui::AsIdSalt, detail: &str) {
    egui::CollapsingHeader::new("Diagnostics")
        .id_salt(id)
        .show(ui, |ui| {
            egui::ScrollArea::vertical()
                .max_height(120.0)
                .show(ui, |ui| {
                    ui.add(
                        egui::Label::new(egui::RichText::new(detail).monospace())
                            .wrap()
                            .selectable(true),
                    );
                });
            if ui.add(theme::quiet("Copy diagnostics").small()).clicked() {
                ui.ctx().copy_text(detail.to_owned());
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn attachment_summaries_explain_recovery_without_showing_os_error_chains() {
        for (detail, action) in [
            (
                "Base commit is unavailable. Git: fatal: private diagnostic",
                "fresh convoy",
            ),
            (
                "Git worktree creation failed at /private/path: private diagnostic",
                "free space",
            ),
            (
                "Cannot create worktree storage: Permission denied (os error 13)",
                "permissions",
            ),
            (
                "Worktree ownership record does not match this job. private diagnostic",
                "no cleanup",
            ),
            (
                "Cannot locate attachment /fixture/missing.md.: No such file (os error 2)",
                "Restore",
            ),
            (
                "Cannot read attachment /fixture/private.md. Check access permissions.: Permission denied (os error 13)",
                "permissions",
            ),
            (
                "Attachment changed since it was added: /fixture/requirements.md.",
                "re-add",
            ),
            (
                "Some attachments were not added:\nUnsupported attachment type: /fixture/report.pdf.",
                "supported types",
            ),
            (
                "OpenAI Codex CLI cannot supply the requested image attachments: screenshot.png. The installed CLI must support --image",
                "compatible backend",
            ),
            (
                "Reused convoy has missing, unreadable or changed attachments.\nprivate diagnostic",
                "explicitly remove",
            ),
            (
                "Group GNOME Extensions: 1 unavailable or unregistered member(s) were not selected.",
                "Manage groups",
            ),
        ] {
            let summary = summary(detail);
            assert!(summary.contains(action), "{summary}");
            assert!(!summary.contains("os error") && !summary.contains("private diagnostic"));
        }
    }
    #[test]
    fn missing_executables_and_invalid_configuration_have_different_presentations() {
        let error = anyhow::Error::new(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "fixture missing",
        ))
        .context("Cannot start fixture");
        let missing = CliError::from_error(error);
        assert_eq!(missing.label(), "Unavailable");
        assert!(missing.detail.contains("fixture missing"));
        let invalid = CliError::from_error(anyhow::anyhow!("Unsupported value for effort"));
        assert_eq!(invalid.label(), "Invalid configuration");
        assert_eq!(
            summary("Repository state changed after preflight. Raw detail"),
            "Repository changed since review. Inspect it, then launch a fresh convoy."
        );
        assert_eq!(
            summary("Claude Code completion was not confirmed (exit status: 0)."),
            "Agent completion could not be verified. Inspect Activity, Raw output and diagnostics."
        );
    }
}

#[cfg(test)]
mod recovery_tests {
    #[test]
    fn history_and_orphan_notices_are_not_misreported_as_ownership_errors() {
        assert!(super::summary("Convoys with unresolved isolated results stay in history. Their files and ownership metadata are preserved.").contains("stay in history"));
        assert!(
            super::summary("Unreferenced worktree resources found (1). Preserved.\n/some/path")
                .contains("Unreferenced")
        );
        assert!(
            super::summary("Could not clean isolated result. ownership error")
                .contains("did not complete")
        );
    }
}

#[cfg(test)]
mod result_tests {
    use super::*;
    #[test]
    fn apply_reason_and_successful_cleanup_are_not_reclassified() {
        for detail in [
            "Apply blocked: Registered working tree has local changes.",
            "Cleanup completed; ownership record retained.",
        ] {
            assert_eq!(summary(detail), detail);
        }
    }
}
