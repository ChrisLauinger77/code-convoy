//! Keyboard behavior and AccessKit metadata from the real widgets.
use super::*;
use egui::{Event, Key, Modifiers};

pub(super) fn focused_name(keys: &Keyboard) -> Option<String> {
    keys.ctx.memory(|m| m.focused()).and_then(|id| {
        keys.nodes
            .get(&id.accesskit_id())
            .map(|node| keys.label(node))
    })
}

#[test]
fn about_opens_on_close_and_restores_keyboard_opener() {
    let (_temp, mut app) = app();
    let mut keys = Keyboard::new(|app, ui, ctx| {
        app.handle_shortcuts(ctx);
        if ui.button("Open About").clicked() {
            app.about_open = true;
        }
        app.about_window(ctx);
    });
    keys.activate(&mut app, "Open About");
    assert_eq!(focused_name(&keys).as_deref(), Some("Close About"));
    keys.key(&mut app, Key::Escape);
    assert!(!app.about_open);
    assert_eq!(focused_name(&keys).as_deref(), Some("Open About"));
    keys.key(&mut app, Key::Enter);
    assert!(app.about_open);
    keys.key(&mut app, Key::Enter);
    assert!(!app.about_open);
    assert_eq!(focused_name(&keys).as_deref(), Some("Open About"));
}

#[test]
fn review_starts_on_back_and_held_activation_cannot_launch() {
    for key in [Key::Enter, Key::Space] {
        let (_temp, mut app) = app();
        let mut keys = Keyboard::new(|app, ui, ctx| {
            app.handle_shortcuts(ctx);
            let _ = ui.button("Activation anchor");
            app.preflight_window(ctx);
        });
        keys.focus(&mut app, "Activation anchor");
        // egui derives repeat from held keys; seed a real press without release.
        keys.frame(
            &mut app,
            vec![Event::Key {
                key,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            }],
        );
        prepare(&mut app, AgentId::Codex);
        keys.settle_new_area(&mut app);
        assert_eq!(focused_name(&keys).as_deref(), Some("Back to task"));
        keys.focus(&mut app, "Start convoy");
        keys.frame(
            &mut app,
            vec![Event::Key {
                key,
                physical_key: None,
                pressed: true,
                repeat: true,
                modifiers: Modifiers::NONE,
            }],
        );
        assert!(app.prepared.is_some());
        assert!(app.state.runs.is_empty() && app.manager.is_idle());
    }
}

#[test]
fn draft_fields_have_accessible_names_after_typing() {
    let (_temp, mut app) = app();
    app.state.draft.prompt = "Task without a placeholder".into();
    app.repository_input = "/some/repository".into();
    app.worktree_location.input = "/some/worktrees".into();
    let mut keys = Keyboard::new(|app, ui, ctx| {
        app.editor(ui, ctx);
        app.execution_section(ui);
        app.worktree_settings(ui);
    });
    keys.frame(&mut app, vec![]);
    for label in [
        "Task",
        "Agent",
        "Repository path",
        "Execution",
        "Worktree base directory",
    ] {
        keys.focus(&mut app, label);
    }
}

#[test]
fn settings_dialog_restores_the_menu_launcher_after_escape() {
    let (_temp, mut app) = app();
    let mut keys = Keyboard::new(|app, ui, ctx| {
        app.handle_shortcuts(ctx);
        app.header(ui, ctx);
        app.shortcuts_window(ctx);
    });
    keys.activate(&mut app, "Settings");
    keys.activate(&mut app, "Keyboard Shortcuts…");
    assert_eq!(focused_name(&keys).as_deref(), Some("Close shortcuts"));
    keys.key(&mut app, Key::Escape);
    assert_eq!(focused_name(&keys).as_deref(), Some("Settings"));
    assert!(!egui::Popup::is_any_open(&keys.ctx));
}

#[test]
fn closing_a_dialog_preserves_an_explicit_move_to_the_task_editor() {
    let (_temp, mut app) = app();
    let mut keys = Keyboard::new(|app, ui, ctx| {
        app.handle_shortcuts(ctx);
        if ui.button("Open About").clicked() {
            app.about_open = true;
        }
        app.editor(ui, ctx);
        app.about_window(ctx);
    });
    keys.activate(&mut app, "Open About");
    app.about_open = false;
    app.focus_draft = true;
    keys.frame(&mut app, vec![]);
    keys.frame(&mut app, vec![]);
    assert_eq!(focused_name(&keys).as_deref(), Some("Task"));
}

#[test]
fn nested_confirmation_restores_hidden_parent_then_original_opener() {
    let (_temp, mut app) = app();
    let mut keys = Keyboard::new(|app, ui, ctx| {
        app.handle_shortcuts(ctx);
        if ui.button("Open About").clicked() {
            app.about_open = true;
        }
        if !app.quit_requested {
            app.about_window(ctx);
        }
        app.quit_window(ctx);
    });
    keys.activate(&mut app, "Open About");
    keys.focus(&mut app, "Project on GitHub");
    // Native Quit can interrupt any modal; cancelling must restore that modal.
    app.quit_requested = true;
    app.focus_quit_cancel = true;
    keys.frame(&mut app, vec![]);
    keys.frame(&mut app, vec![]);
    assert_eq!(focused_name(&keys).as_deref(), Some("Cancel"));
    keys.key(&mut app, Key::Escape);
    assert!(!app.quit_requested && app.about_open && !app.closing);
    assert_eq!(focused_name(&keys).as_deref(), Some("Project on GitHub"));
    keys.key(&mut app, Key::Escape);
    assert_eq!(focused_name(&keys).as_deref(), Some("Open About"));
}

#[test]
fn modal_tab_and_shift_tab_stay_inside_then_release_focus() {
    for modifiers in [Modifiers::NONE, Modifiers::SHIFT] {
        let (_temp, mut app) = app();
        let mut keys = Keyboard::new(|app, ui, ctx| {
            app.handle_shortcuts(ctx);
            if ui.button("Open About").clicked() {
                app.about_open = true;
            }
            let _ = ui.button("Background action");
            app.about_window(ctx);
        });
        keys.activate(&mut app, "Open About");
        let mut visited = HashSet::new();
        for _ in 0..8 {
            shortcuts::chord(&mut keys, &mut app, Key::Tab, modifiers);
            let label = focused_name(&keys).unwrap();
            assert!(matches!(
                label.as_str(),
                "Close About" | "Project on GitHub" | "Check for updates on GitHub"
            ));
            visited.insert(label);
        }
        assert_eq!(visited.len(), 3);
        keys.key(&mut app, Key::Escape);
        keys.key(&mut app, Key::Tab);
        assert_eq!(focused_name(&keys).as_deref(), Some("Background action"));
    }
}

#[test]
fn validation_popup_has_escape_priority_and_editor_restores_repository_action() {
    let (_temp, mut app) = app();
    app.state.repositories = vec![repository("registered")];
    let mut keys = Keyboard::new(|app, ui, ctx| {
        app.handle_shortcuts(ctx);
        if ui.button("Configure validation").clicked() {
            app.validation_editor = Some(crate::ui::validation::Editor::new(
                &app.state.repositories[0],
                None,
            ));
        }
    });
    keys.activate(&mut app, "Configure validation");
    assert_eq!(
        focused_name(&keys).as_deref(),
        Some("Executable name on PATH or absolute path")
    );
    keys.activate(&mut app, "Preset");
    assert!(egui::Popup::is_any_open(&keys.ctx));
    keys.key(&mut app, Key::Escape);
    assert!(!egui::Popup::is_any_open(&keys.ctx));
    assert!(app.validation_editor.is_some());
    keys.key(&mut app, Key::Escape);
    assert!(app.validation_editor.is_none());
    assert_eq!(focused_name(&keys).as_deref(), Some("Configure validation"));
    assert!(app.state.repository_validation.is_empty());
}

#[test]
fn history_filters_have_names_and_keep_their_values() {
    let (_temp, mut app) = app();
    app.state.runs = vec![run(42, &[JobStatus::Succeeded])];
    app.history_search.query = "search without placeholder".into();
    let mut keys = Keyboard::new(|app, ui, ctx| app.results(ui, ctx));
    keys.frame(&mut app, vec![]);
    for label in ["Search history", "History filter"] {
        keys.focus(&mut app, label);
    }
    assert_eq!(app.history_search.query, "search without placeholder");
    assert!(
        keys.nodes
            .values()
            .any(|node| keys.label(node) == "Search history"
                && node.value() == Some("search without placeholder"))
    );
}

#[test]
fn held_or_modified_activation_cannot_discard_a_result() {
    for key in [Key::Enter, Key::Space] {
        let (_temp, mut app) = app();
        app.state.runs = vec![isolated_history(42, JobStatus::Succeeded, Some(true))];
        let before = serde_json::to_value(&app.state).unwrap();
        let mut keys = Keyboard::new(|app, ui, ctx| {
            app.handle_shortcuts(ctx);
            let _ = ui.button("Activation anchor");
            app.discard_window(ctx);
        });
        keys.focus(&mut app, "Activation anchor");
        keys.frame(
            &mut app,
            vec![Event::Key {
                key,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            }],
        );
        app.discard_confirmation = Some((42, 0, crate::persistence::results::Action::Discard));
        app.focus_discard_cancel = true;
        keys.settle_new_area(&mut app);
        assert_eq!(focused_name(&keys).as_deref(), Some("Cancel"));
        keys.focus(&mut app, "Discard result");
        keys.frame(
            &mut app,
            vec![Event::Key {
                key,
                physical_key: None,
                pressed: true,
                repeat: true,
                modifiers: Modifiers::NONE,
            }],
        );
        keys.frame(
            &mut app,
            vec![Event::Key {
                key,
                physical_key: None,
                pressed: false,
                repeat: false,
                modifiers: Modifiers::NONE,
            }],
        );
        for modifiers in [Modifiers::COMMAND, Modifiers::ALT, Modifiers::SHIFT] {
            shortcuts::chord(&mut keys, &mut app, key, modifiers);
        }
        assert!(app.discard_confirmation.is_some() && app.result_operation.is_none());
        assert_eq!(serde_json::to_value(&app.state).unwrap(), before);
        keys.key(&mut app, Key::Escape);
        assert_eq!(focused_name(&keys).as_deref(), Some("Activation anchor"));
    }
}

#[test]
fn held_enter_cannot_activate_a_button_when_tab_leaves_an_editor() {
    let (_temp, mut app) = app();
    let mut keys = Keyboard::new(|app, ui, ctx| {
        app.handle_shortcuts(ctx);
        let label = ui.label("Editor");
        ui.text_edit_multiline(&mut app.state.draft.prompt)
            .labelled_by(label.id);
        if ui.button("Action after editor").clicked() {
            app.about_open = true;
        }
    });
    keys.focus(&mut app, "Editor");
    keys.frame(
        &mut app,
        vec![Event::Key {
            key: Key::Enter,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        }],
    );
    keys.frame(
        &mut app,
        vec![
            Event::Key {
                key: Key::Tab,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            },
            Event::Key {
                key: Key::Enter,
                physical_key: None,
                pressed: true,
                repeat: true,
                modifiers: Modifiers::NONE,
            },
        ],
    );
    assert!(!app.about_open);
    assert_eq!(focused_name(&keys).as_deref(), Some("Action after editor"));
}

#[test]
fn dialog_focus_and_bounds_scale_with_native_pixel_density() {
    for scale in [1.0, 1.5, 2.0] {
        for appearance in [
            domain::Appearance::System,
            domain::Appearance::Dark,
            domain::Appearance::Light,
        ] {
            let (_temp, mut app) = app();
            app.shortcuts_open = true;
            let mut keys = Keyboard::new(|app, _, ctx| {
                app.handle_shortcuts(ctx);
                app.shortcuts_window(ctx);
            });
            keys.ctx.set_theme(theme::preference(appearance));
            for _ in 0..3 {
                let mut input = egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(780.0, 560.0),
                    )),
                    ..Default::default()
                };
                input
                    .viewports
                    .get_mut(&egui::ViewportId::ROOT)
                    .unwrap()
                    .native_pixels_per_point = Some(scale);
                keys.input(&mut app, input);
            }
            assert_eq!(keys.ctx.pixels_per_point(), scale);
            assert_eq!(focused_name(&keys).as_deref(), Some("Close shortcuts"));
            let response = keys
                .ctx
                .read_response(keys.ctx.memory(|m| m.focused()).unwrap())
                .unwrap();
            assert!(keys.ctx.content_rect().contains_rect(response.rect));
            let bounds = keys.nodes[&response.id.accesskit_id()].bounds().unwrap();
            // Widget bounds are logical; the AccessKit root carries the scale.
            assert!((bounds.width() - f64::from(response.rect.width())).abs() < 1.0);
            assert!(keys.nodes.values().any(|node| {
                node.transform().is_some_and(|transform| {
                    transform.as_coeffs()
                        == [f64::from(scale), 0.0, 0.0, f64::from(scale), 0.0, 0.0]
                })
            }));
            assert!(bounds.x0 >= 0.0 && bounds.y0 >= 0.0);
            assert!(bounds.x1 <= 780.0 && bounds.y1 <= 560.0);
            keys.key(&mut app, Key::Escape);
            assert!(!app.shortcuts_open);
        }
    }
}
