use super::*;

pub(super) fn version_text(version: &str, revision: Option<&str>) -> String {
    let revision = revision
        .filter(|r| (7..=40).contains(&r.len()) && r.bytes().all(|b| b.is_ascii_hexdigit()));
    match revision {
        Some(revision) => format!("Version {version} · base commit {revision}"),
        None => format!("Version {version} · commit unavailable"),
    }
}

impl App {
    pub(super) fn about_window(&mut self, ctx: &egui::Context) {
        if !self.about_open {
            return;
        }
        let response = egui::Modal::new(egui::Id::new("about_codeconvoy")).show(ctx, |ui| {
            ui.set_width(360.0_f32.min(ctx.content_rect().width() - 64.0));
            ui.heading("CodeConvoy");
            ui.label("One task. Multiple repositories. Your agent.");
            ui.label(version_text(
                env!("CARGO_PKG_VERSION"),
                option_env!("CODECONVOY_GIT_REV"),
            ));
            ui.small("The base commit identifies the checkout used at build time; local changes may be present.");
            ui.separator();
            ui.label("A local desktop task runner for coding-agent CLIs.");
            ui.label(concat!("License: ", env!("CARGO_PKG_LICENSE")));
            ui.hyperlink_to(
                "Project on GitHub",
                "https://github.com/ChrisLauinger77/code-convoy",
            );
            ui.weak("About is available offline. Opening the project link uses your browser.");
            ui.add_space(theme::GAP);
            if ui.button("Close About").clicked() {
                self.about_open = false;
            }
        });
        if response.should_close() {
            self.about_open = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn version_is_useful_without_git_and_rejects_invalid_metadata() {
        assert_eq!(
            version_text("0.1.0", Some("abcdef123456")),
            "Version 0.1.0 · base commit abcdef123456"
        );
        for revision in [None, Some(""), Some("unknown\nmetadata")] {
            assert_eq!(
                version_text("0.1.0", revision),
                "Version 0.1.0 · commit unavailable"
            );
        }
    }
}
