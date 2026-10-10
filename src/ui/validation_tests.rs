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

#[test]
fn result_controls_and_transactions_reject_only_conflicting_validation_intents() {
    use crate::persistence::results::Action;
    use domain::{ExecutionMode, GitSummary, ResultAvailability as A, ResultResolution as R};
    for relation in [
        "unrelated",
        "same",
        "nested",
        "parent",
        "checkout",
        "shared git",
        "isolated git",
        "isolated checkout",
        "finished",
    ] {
        for (action, label) in [
            (Action::Apply, "Apply result"),
            (Action::Discard, "Discard result"),
            (Action::CleanupApplied, "Clean up retained copy"),
        ] {
            let (_temp, mut app) = app();
            app.state.runs = vec![
                isolated_history(1, JobStatus::Succeeded, Some(true)),
                run(2, &[JobStatus::Succeeded]),
            ];
            let target = &mut app.state.runs[0].jobs[0];
            target.result_checked = true;
            target.result_availability = A::Available;
            target.review = Some(Ok(crate::review::Statistics {
                files: 1,
                ..Default::default()
            }));
            if action == Action::CleanupApplied {
                target.resolution = R::Applied;
            }
            let metadata = target.worktree.clone().unwrap();
            app.state.repositories = vec![target.repository.clone()];
            let active = &mut app.state.runs[1].jobs[0];
            active.repository = repository("unrelated");
            match relation {
                "same" | "finished" => active.repository.path = metadata.repository.path.clone(),
                "nested" => active.repository.path = metadata.repository.path.join("nested"),
                "parent" => {
                    active.repository.path = metadata.repository.path.parent().unwrap().into()
                }
                "checkout" => active.repository.path = metadata.path.join("nested"),
                "shared git" => {
                    active.before = Some(GitSummary {
                        common_dir: Some(metadata.common_dir.clone()),
                        ..Default::default()
                    })
                }
                "isolated git" | "isolated checkout" => {
                    let mut m = metadata.clone();
                    m.run = 2;
                    m.repository = active.repository.clone();
                    m.path = PathBuf::from("/unrelated/retained/tree");
                    if relation == "isolated checkout" {
                        m.common_dir = active.repository.path.join(".git");
                        m.path = metadata.repository.path.join("nested");
                    }
                    active.execution_mode = ExecutionMode::IsolatedWorktree;
                    active.worktree = Some(m);
                }
                _ => {}
            }
            let mut record = Record::pending(
                Command {
                    executable: "unused-fixture".into(),
                    arguments: vec![],
                },
                active,
            );
            if relation == "finished" {
                record.status = Status::Passed;
            }
            active.validation = Some(record);
            app.select_run(Some(1));
            let blocked = !matches!(relation, "unrelated" | "finished");
            let ctx = egui::Context::default();
            ctx.enable_accesskit();
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1000.0, 1200.0),
                    )),
                    ..Default::default()
                },
                |ui| app.review_view(ui, &ctx, 1200.0),
            );
            output.textures_delta.clear();
            let tree = output.platform_output.accesskit_update.unwrap();
            let (_, button) = tree
                .nodes
                .iter()
                .find(|(_, node)| node.label() == Some(label))
                .unwrap();
            assert_eq!(button.is_disabled(), blocked, "{relation}: {label}");
            let before = serde_json::to_value(&app.state).unwrap();
            let result = app
                .store
                .begin_result_operation(&mut app.state, 1, 0, action);
            assert_eq!(result.is_err(), blocked, "{relation}: {label}");
            if blocked {
                assert_eq!(
                    serde_json::to_value(&app.state).unwrap(),
                    before,
                    "Blocked validation must not write operation intent"
                );
            }
        }
    }
}

#[test]
fn bulk_discard_skips_validation_conflicts_and_starts_unrelated_results() {
    use crate::persistence::bulk_discard::{Batch, Plan, Scope};
    let (_temp, mut app) = app();
    app.state.runs = vec![
        isolated_history(1, JobStatus::Succeeded, Some(true)),
        isolated_history(2, JobStatus::Succeeded, Some(true)),
        run(3, &[JobStatus::Succeeded]),
    ];
    let unrelated = &mut app.state.runs[1].jobs[0];
    unrelated.repository = repository("other");
    let m = unrelated.worktree.as_mut().unwrap();
    m.repository = unrelated.repository.clone();
    m.common_dir = unrelated.repository.path.join(".git");
    m.path = PathBuf::from("/fixtures/owned/other/tree");
    let active = &mut app.state.runs[2].jobs[0];
    active.validation = Some(Record::pending(
        Command {
            executable: "unused-fixture".into(),
            arguments: vec![],
        },
        active,
    ));
    let protected = serde_json::to_value(&app.state.runs[0].jobs[0]).unwrap();
    let mut batch = Batch::new(Plan::new(&app.state, Scope::All), &mut app.state);
    assert!(batch.begin_next(&app.store, &mut app.state).is_none());
    assert_eq!(batch.summary.failures.len(), 1);
    assert!(batch.summary.failures[0].contains("Wait for validation"));
    let job = &app.state.runs[0].jobs[0];
    assert_eq!(job.resolution, domain::ResultResolution::Unresolved);
    assert_eq!(
        serde_json::to_value(&job.worktree).unwrap(),
        protected["worktree"]
    );
    let operation = batch.begin_next(&app.store, &mut app.state).unwrap();
    assert_eq!((operation.run, operation.index), (2, 0));
    assert_eq!(
        app.state.runs[2].jobs[0]
            .validation
            .as_ref()
            .unwrap()
            .status,
        Status::Running
    );
}
