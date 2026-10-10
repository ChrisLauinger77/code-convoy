use super::*;
use crate::validation::{Command, Record, Status};

fn configured() -> (tempfile::TempDir, App) {
    let (temp, mut app) = app();
    app.state.runs = vec![run(7, &[JobStatus::Succeeded])];
    app.state.repositories = vec![app.state.runs[0].jobs[0].repository.clone()];
    app.state
        .save_validation(
            app.state.repositories[0].path.clone(),
            Some(Command {
                executable: "cargo".into(),
                arguments: vec![
                    "test".into(),
                    "a long literal argument with spaces 日本語".into(),
                ],
            }),
        )
        .unwrap();
    app.select_run(Some(7));
    (temp, app)
}
#[test]
fn validation_views_fit_required_sizes_and_all_themes() {
    for appearance in [
        domain::Appearance::System,
        domain::Appearance::Dark,
        domain::Appearance::Light,
    ] {
        for (width, height) in [(780.0, 560.0), (1180.0, 820.0), (1600.0, 1000.0)] {
            for tab in [Tab::Validation, Tab::Review] {
                let (_temp, mut app) = configured();
                app.tab = tab;
                let job = &mut app.state.runs[0].jobs[0];
                let mut record = Record::pending(
                    app.state.repository_validation[&job.repository.path].clone(),
                    job,
                );
                record.status = Status::Passed;
                record.output = "validation output 日本語\n".repeat(100);
                job.validation = Some(record);
                let ctx = egui::Context::default();
                theme::install(&ctx);
                ctx.set_theme(theme::preference(appearance));
                let mut output = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::vec2(width, height),
                        )),
                        ..Default::default()
                    },
                    |ui| {
                        egui::Panel::left("editor")
                            .exact_size(310.0)
                            .show(ui, |ui| {
                                app.repositories_section(ui, &ctx);
                            });
                        egui::CentralPanel::default().show(ui, |ui| {
                            let right = ui.available_rect_before_wrap().right();
                            let response = ui.scope(|ui| app.results(ui, &ctx));
                            assert!(
                                response.response.rect.right() <= right + 1.0,
                                "{appearance:?} {width}x{height}"
                            );
                        });
                    },
                );
                output.textures_delta.clear();
            }
        }
    }
}
#[test]
fn validation_is_explicit_and_does_not_rewrite_agent_outcomes_or_start_during_render() {
    let (_temp, mut app) = configured();
    let ctx = egui::Context::default();
    let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
        egui::CentralPanel::default().show(ui, |ui| app.validation_controls(ui, &ctx));
    });
    output.textures_delta.clear();
    assert!(app.validations.is_idle());
    assert!(app.state.runs[0].jobs[0].validation.is_none());
    assert_eq!(app.state.runs[0].jobs[0].status, JobStatus::Succeeded);
    assert_eq!(app.state.runs[0].jobs[0].exit_code, None);
}
#[test]
fn configuration_escape_closes_without_saving() {
    let (_temp, mut app) = configured();
    let before = app.state.repository_validation.clone();
    app.validation_editor = Some(super::super::validation::Editor::new(
        &app.state.repositories[0],
        None,
    ));
    let ctx = egui::Context::default();
    for pressed in [false, true] {
        let mut output = ctx.run_ui(
            egui::RawInput {
                events: if pressed {
                    vec![egui::Event::Key {
                        key: egui::Key::Escape,
                        physical_key: None,
                        pressed: true,
                        repeat: false,
                        modifiers: egui::Modifiers::NONE,
                    }]
                } else {
                    vec![]
                },
                ..Default::default()
            },
            |_| app.validation_settings_window(&ctx),
        );
        output.textures_delta.clear();
    }
    assert!(app.validation_editor.is_none());
    assert_eq!(app.state.repository_validation, before);
}
