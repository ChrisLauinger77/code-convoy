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
    if detail.starts_with("Some attachments were not added") {
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
