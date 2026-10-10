use super::*;
use crate::validation::{
    configuration::{Assignment, Existing},
    presets::BUILT_INS,
};

fn fixture() -> (tempfile::TempDir, App, Keyboard) {
    let (temp, mut app) = app();
    app.state.repositories = vec![
        repository("Extension"),
        repository("Rust"),
        repository("Shared"),
    ];
    app.state.groups = vec![
        domain::RepositoryGroup {
            name: "GNOME Extensions".into(),
            repositories: vec![
                app.state.repositories[0].path.clone(),
                app.state.repositories[2].path.clone(),
            ],
        },
        domain::RepositoryGroup {
            name: "Rust projects".into(),
            repositories: vec![
                app.state.repositories[1].path.clone(),
                app.state.repositories[2].path.clone(),
            ],
        },
    ];
    app.state.runs = vec![run(7, &[JobStatus::Succeeded])];
    app.selected.insert(app.state.repositories[0].path.clone());
    let keys = Keyboard::new(|app, ui, ctx| app.repositories_section(ui, ctx));
    (temp, app, keys)
}

#[test]
fn validation_preset_selection_edit_save_remove_and_cancel_are_keyboard_accessible() {
    let (_temp, mut app, mut keys) = fixture();
    let repo = app.state.repositories[0].clone();
    app.validation_editor = Some(super::super::super::validation::Editor::new(&repo, None));
    keys.frame(&mut app, vec![]);
    keys.activate(&mut app, "Preset");
    keys.activate(&mut app, "npm Lint");
    let editor = app.validation_editor.as_ref().unwrap();
    assert_eq!(editor.command().unwrap(), BUILT_INS[0].command());
    assert_eq!(editor.preset, Some("gnome-extension-lint"));
    assert!(app.state.repository_validation.is_empty());
    keys.replace_text(
        &mut app,
        "Arguments (JSON array; each string is one literal argument)",
        r#"["run", "validate", "literal spaces", ""]"#,
    );
    assert!(app.validation_editor.as_ref().unwrap().preset.is_none());
    keys.activate(&mut app, "Save command");
    let saved = app.store.load().unwrap().repository_validation[&repo.path].clone();
    assert_eq!(saved.arguments, ["run", "validate", "literal spaces", ""]);
    app.validation_editor = Some(super::super::super::validation::Editor::new(
        &repo,
        Some(&saved),
    ));
    keys.frame(&mut app, vec![]);
    keys.activate(&mut app, "Preset");
    keys.activate(&mut app, "Rust Tests");
    assert_eq!(
        app.validation_editor.as_ref().unwrap().command().unwrap(),
        BUILT_INS[1].command()
    );
    keys.activate(&mut app, "Preset");
    keys.activate(&mut app, "Custom");
    assert_eq!(
        app.validation_editor.as_ref().unwrap().command().unwrap(),
        BUILT_INS[1].command()
    );
    keys.key(&mut app, egui::Key::Escape);
    assert!(app.validation_editor.is_none());
    assert_eq!(app.state.repository_validation[&repo.path], saved);
    app.validation_editor = Some(super::super::super::validation::Editor::new(
        &repo,
        Some(&saved),
    ));
    keys.frame(&mut app, vec![]);
    keys.activate(&mut app, "Remove command");
    assert!(app.store.load().unwrap().repository_validation.is_empty());
    assert!(app.validations.is_idle() && app.manager.is_idle());
}

#[test]
fn validation_bulk_keyboard_groups_preserve_overwrite_and_safe_cancel() {
    let (_temp, mut app, mut keys) = fixture();
    let shared = app.state.repositories[2].path.clone();
    let custom = crate::validation::Command {
        executable: "custom-check".into(),
        arguments: vec!["two words".into()],
    };
    app.state
        .save_validation(shared.clone(), Some(custom.clone()))
        .unwrap();
    let groups = app.state.groups.clone();
    let selected = app.selected.clone();
    let runs = serde_json::to_value(&app.state.runs).unwrap();
    keys.frame(&mut app, vec![]);
    keys.activate(&mut app, "Assign Validation Preset…");
    keys.activate(&mut app, "GNOME Extensions");
    keys.activate(&mut app, "Rust projects");
    assert_eq!(
        app.validation_assignment
            .as_ref()
            .unwrap()
            .selection
            .paths
            .len(),
        3
    );
    keys.activate(&mut app, "Review assignment…");
    assert_eq!(app.state.repository_validation.len(), 1);
    assert_eq!(
        keys.label(&keys.nodes[&keys.ctx.memory(|m| m.focused()).unwrap().accesskit_id()]),
        "Cancel"
    );
    keys.activate(&mut app, "Confirm assignment");
    let report = app
        .validation_assignment
        .as_ref()
        .unwrap()
        .report
        .as_ref()
        .unwrap();
    assert_eq!(
        (
            report.updated.len(),
            report.skipped.len(),
            report.failed.len()
        ),
        (2, 1, 0)
    );
    assert_eq!(
        app.store.load().unwrap().repository_validation[&shared],
        custom
    );
    keys.activate(&mut app, "Close");

    // Explicit overwrite still needs a separate confirmation; Escape saves nothing.
    keys.activate(&mut app, "Assign Validation Preset…");
    keys.activate(&mut app, "Rust projects");
    keys.activate(&mut app, "Preset");
    keys.activate(&mut app, "Rust Check");
    keys.activate(&mut app, "Preset");
    keys.activate(&mut app, "Rust Tests");
    keys.activate(&mut app, "Overwrite 2 existing configurations");
    keys.activate(&mut app, "Review assignment…");
    keys.key(&mut app, egui::Key::Escape);
    assert!(app.validation_assignment.is_none());
    assert_eq!(app.state.repository_validation[&shared], custom);

    keys.activate(&mut app, "Assign Validation Preset…");
    keys.activate(&mut app, "Rust projects");
    keys.activate(&mut app, "Preset");
    keys.activate(&mut app, "Rust Tests");
    keys.activate(&mut app, "Overwrite 2 existing configurations");
    keys.activate(&mut app, "Review assignment…");
    keys.activate(&mut app, "Confirm overwrite");
    assert_eq!(
        app.store.load().unwrap().repository_validation[&shared],
        BUILT_INS[1].command()
    );
    assert_eq!(app.selected, selected);
    assert_eq!(app.state.groups, groups);
    assert_eq!(serde_json::to_value(&app.state.runs).unwrap(), runs);
    assert!(app.validations.is_idle() && app.manager.is_idle());
    assert!(!app.store.directory().join("worktrees").exists());
}

#[test]
fn validation_preset_dialogs_fit_required_sizes_and_themes() {
    for appearance in [
        domain::Appearance::System,
        domain::Appearance::Dark,
        domain::Appearance::Light,
    ] {
        for (width, height) in [(780.0, 560.0), (1180.0, 820.0), (1600.0, 1000.0)] {
            for step in 0..5 {
                let (_temp, mut app, mut keys) = fixture();
                keys.ctx.set_theme(theme::preference(appearance));
                if step == 0 || step == 4 {
                    app.validation_editor = Some(super::super::super::validation::Editor::new(
                        &app.state.repositories[0],
                        Some(&BUILT_INS[0].command()),
                    ));
                    if step == 4 {
                        app.validation_editor.as_mut().unwrap().arguments =
                            serde_json::to_string(&vec!["long literal argument ".repeat(40); 30])
                                .unwrap();
                    }
                } else {
                    let mut dialog = super::super::super::validation_assignment::Dialog::new();
                    // Exercise scrolling without widening or lengthening the dialog.
                    app.state.repositories.extend((0..100).map(|i| {
                        repository(&format!("Repository {i} {}", "long name ".repeat(15)))
                    }));
                    dialog
                        .selection
                        .paths
                        .extend(app.state.repositories.iter().map(|r| r.path.clone()));
                    let review = Assignment::review(
                        &app.state,
                        &dialog.selection,
                        &BUILT_INS[0],
                        Existing::Overwrite,
                    );
                    if step == 2 {
                        dialog.review = Some(review);
                    } else if step == 3 {
                        dialog.report = Some(app.store.assign_validation(&mut app.state, &review));
                    }
                    app.validation_assignment = Some(dialog);
                }
                for _ in 0..3 {
                    keys.input(
                        &mut app,
                        egui::RawInput {
                            screen_rect: Some(egui::Rect::from_min_size(
                                egui::Pos2::ZERO,
                                egui::vec2(width, height),
                            )),
                            ..Default::default()
                        },
                    );
                }
                let layer = keys.ctx.memory(|m| m.top_modal_layer()).unwrap();
                let rect = keys.ctx.memory(|m| m.area_rect(layer.id)).unwrap();
                assert!(
                    rect.left() >= -1.0
                        && rect.top() >= -1.0
                        && rect.right() <= width + 1.0
                        && rect.bottom() <= height + 1.0,
                    "{appearance:?} {width}x{height} step {step}: {rect:?}"
                );
                keys.key(&mut app, egui::Key::Escape);
                assert!(app.validation_editor.is_none() && app.validation_assignment.is_none());
            }
        }
    }
}

#[test]
fn validation_bulk_empty_selection_cannot_confirm_and_modal_blocks_shortcuts() {
    let (_temp, mut app, mut keys) = fixture();
    keys.frame(&mut app, vec![]);
    keys.activate(&mut app, "Assign Validation Preset…");
    assert!(
        keys.nodes
            .values()
            .any(|n| n.label() == Some("Review assignment…") && n.is_disabled())
    );
    let tab = app.tab;
    keys.input(
        &mut app,
        egui::RawInput {
            events: vec![egui::Event::Key {
                key: egui::Key::Num2,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::COMMAND,
            }],
            ..Default::default()
        },
    );
    app.handle_shortcuts(&keys.ctx);
    assert_eq!(app.tab, tab);
    assert!(app.validation_assignment.is_some());
    keys.key(&mut app, egui::Key::Escape);
    assert!(app.state.repository_validation.is_empty());
}
