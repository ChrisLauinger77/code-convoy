//! Presentation-only summaries. Original diagnostics remain available verbatim.
use super::*;

#[derive(Debug)]
pub(super) struct CliError {
    pub missing: bool,
    pub detail: String,
}
impl CliError {
    pub fn from_error(error: anyhow::Error) -> Self {
        Self {
            missing: error.chain().any(|cause| {
                cause
                    .downcast_ref::<std::io::Error>()
                    .is_some_and(|e| e.kind() == std::io::ErrorKind::NotFound)
            }),
            detail: format!("{error:#}"),
        }
    }
    pub fn label(&self) -> &'static str {
        if self.missing {
            "Unavailable"
        } else {
            "Invalid configuration"
        }
    }
    pub fn hint(&self) -> &'static str {
        if self.missing {
            "Executable not found. Install the CLI separately or choose its absolute path."
        } else {
            "Check this backend's settings and executable compatibility."
        }
    }
}

pub(super) fn summary(detail: &str) -> &str {
    if detail.contains("Repository state changed after preflight") {
        "Repository changed since review. Inspect it, then launch a fresh convoy."
    } else if detail.starts_with("Cannot start ") {
        "Unable to start the process. Check the executable and repository path."
    } else if detail.starts_with("Repository path does not exist")
        || detail.starts_with("Git inspection failed")
    {
        "Repository unavailable. Check its path and Git state, then refresh."
    } else if detail.contains("completion was not confirmed") {
        "Agent completion could not be verified. Inspect Output and diagnostics."
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
            "Agent completion could not be verified. Inspect Output and diagnostics."
        );
    }
}
