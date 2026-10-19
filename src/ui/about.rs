use super::*;
use crate::updates::{Failure, FailureKind, InstallSource, Outcome};

pub(super) const APPLICATION_NAME: &str = "CodeConvoy";
pub(super) const REPOSITORY_URL: &str = env!("CARGO_PKG_REPOSITORY");

/// Session-only. The single request starts from an explicit click and only
/// informs; nothing is saved, scheduled, downloaded or installed.
#[derive(Debug, Default)]
pub(super) enum UpdateCheck {
    #[default]
    Idle,
    Checking,
    Finished(Result<Outcome, Failure>),
}

pub(super) fn valid_revision(revision: Option<&str>) -> Option<&str> {
    revision.filter(|r| (7..=40).contains(&r.len()) && r.bytes().all(|b| b.is_ascii_hexdigit()))
}

pub(super) fn version_text(version: &str, revision: Option<&str>) -> String {
    match valid_revision(revision) {
        Some(revision) => format!("Version {version} ({revision})"),
        None => format!("Version {version}"),
    }
}

impl App {
    pub(super) fn about_window(&mut self, ctx: &egui::Context) {
        if !self.about_open {
            return;
        }
        let response = dialog::show(ctx, "about_codeconvoy", |ui, opening| {
            ui.set_width(360.0_f32.min(ctx.content_rect().width() - 64.0));
            ui.heading(APPLICATION_NAME);
            ui.label("One task. Multiple repositories. Your agent.");
            self.update_controls(ui);
            ui.small("The base commit identifies the checkout used at build time; local changes may be present.");
            update_result(ui, &self.update_check);
            ui.separator();
            ui.label("A local desktop task runner for coding-agent CLIs.");
            ui.label(concat!("License: ", env!("CARGO_PKG_LICENSE")));
            ui.hyperlink_to("Project on GitHub", REPOSITORY_URL);
            ui.weak("About is available offline. Check for updates contacts GitHub only when clicked; links open in your browser.");
            ui.add_space(theme::GAP);
            let button = ui.button("Close About");
            if opening {
                button.request_focus();
            }
            if button.clicked() {
                self.about_open = false;
            }
        });
        if response.should_close() {
            self.about_open = false;
        }
    }

    /// Starts a check unless one is pending or the application is closing.
    pub(super) fn begin_update_check(&mut self) -> bool {
        if self.closing || matches!(self.update_check, UpdateCheck::Checking) {
            return false;
        }
        self.update_check = UpdateCheck::Checking;
        true
    }

    fn check_for_updates(&mut self) {
        if !self.begin_update_check() {
            return;
        }
        self.dispatch(async {
            let result = tokio::task::spawn_blocking(|| {
                let executable = std::env::current_exe().ok();
                crate::updates::check(env!("CARGO_PKG_VERSION"), executable.as_deref())
            })
            .await
            .unwrap_or_else(|e| {
                Err(Failure::new(
                    FailureKind::Network,
                    format!("Update check worker failed: {e}"),
                ))
            });
            Message::UpdateChecked(result)
        });
    }

    fn update_controls(&mut self, ui: &mut egui::Ui) {
        let checking = matches!(self.update_check, UpdateCheck::Checking);
        ui.horizontal_wrapped(|ui| {
            ui.label(version_text(
                env!("CARGO_PKG_VERSION"),
                option_env!("CODECONVOY_GIT_REV"),
            ));
            let button = ui.add_enabled(
                !checking && !self.closing,
                theme::quiet("Check for updates"),
            );
            button.widget_info(|| {
                egui::WidgetInfo::labeled(
                    egui::Role::Button,
                    button.enabled(),
                    "Check for updates on GitHub",
                )
            });
            if button.clicked() {
                self.check_for_updates();
            }
            if checking {
                ui.spinner();
                ui.weak("Checking…");
            }
        });
    }
}

fn update_result(ui: &mut egui::Ui, check: &UpdateCheck) {
    let UpdateCheck::Finished(result) = check else {
        return;
    };
    match result {
        Ok(Outcome::UpToDate { latest }) => {
            ui.horizontal_wrapped(|ui| {
                theme::success_label(ui, "Up to date");
                ui.small(format!("Latest release: {latest}"));
            });
        }
        Ok(Outcome::Available { release, source }) => {
            ui.strong(format!("Version {} available", release.version));
            let link = ui.link("Open release page").on_hover_text(&release.url);
            if link.clicked() {
                ui.ctx().open_url(egui::OpenUrl::new_tab(&release.url));
            }
            if let Some((manager, command)) = source.update() {
                ui.small(match source {
                    InstallSource::ScoopGlobal => {
                        "Installed globally with Scoop. Update from an administrator terminal:"
                            .to_owned()
                    }
                    _ => format!("Installed with {manager}. Update from a terminal:"),
                });
                ui.add(
                    egui::Label::new(egui::RichText::new(command).monospace())
                        .wrap()
                        .selectable(true),
                );
                if ui
                    .add(theme::quiet("Copy update command").small())
                    .clicked()
                {
                    ui.ctx().copy_text(command.to_owned());
                }
                ui.small(format!(
                    "{manager} package definitions can lag a GitHub release. If {manager} offers no update yet, try again later."
                ));
            }
            ui.small("CodeConvoy does not download or install updates.");
        }
        Err(failure) => {
            ui.colored_label(theme::Palette::of(ui).error, "Update check failed");
            ui.small(failure.summary());
            diagnostics::details(ui, "update_check_diagnostics", &failure.detail);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn version_is_useful_without_git_and_rejects_invalid_metadata() {
        assert_eq!(
            version_text("0.1.0", Some("abcdef1")),
            "Version 0.1.0 (abcdef1)"
        );
        assert_eq!(
            version_text("0.1.0", Some("abcdef123456")),
            "Version 0.1.0 (abcdef123456)"
        );
        for revision in [
            None,
            Some(""),
            Some("0.1.0"),
            Some("abcdef"),
            Some("unknown\nmetadata"),
            Some("/Users/builder/workspace"),
        ] {
            assert_eq!(version_text("0.1.0", revision), "Version 0.1.0");
        }
    }
}
