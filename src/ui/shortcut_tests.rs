//! Real egui widgets, focus, modals and the shared launch entry point.
use super::*;
use crate::ui::shortcuts::repository_filter_id;
use egui::{Event, Key, Modifiers};

fn chord(keys: &mut Keyboard, app: &mut App, key: Key, modifiers: Modifiers) {
    keys.frame(
        app,
        [true, false]
            .into_iter()
            .map(|pressed| Event::Key {
                key,
                physical_key: None,
                pressed,
                repeat: false,
                modifiers,
            })
            .collect(),
    );
    keys.frame(app, vec![]);
}

fn ready(app: &mut App) {
    app.state.draft.prompt = "Review the repository".into();
    app.state.repositories = vec![repository("registered")];
    app.select_repositories(true);
    complete_cli_checks(app);
    assert!(app.can_run_convoy());
}

fn execution() -> Keyboard {
    Keyboard::new(|app, ui, ctx| {
        app.handle_shortcuts(ctx);
        app.execution_section(ui, ctx);
        app.preflight_window(ctx);
        app.shortcuts_window(ctx);
        app.quit_window(ctx);
        app.discard_window(ctx);
        app.bulk_discard_window(ctx);
    })
}

#[test]
fn button_and_shortcut_share_preflight_and_backend_validation() {
    for shortcut in [false, true] {
        let (_temp, mut app) = app();
        ready(&mut app);
        let task = serde_json::to_value(&app.state.draft).unwrap();
        let mut keys = execution();
        keys.frame(&mut app, vec![]);
        if shortcut {
            // Also prove modified Enter on the button doesn't activate it twice.
            keys.focus(&mut app, "Run Convoy");
            chord(&mut keys, &mut app, Key::Enter, Modifiers::COMMAND);
        } else {
            keys.activate(&mut app, "Run Convoy");
        }
        assert!(app.busy);
        assert!(app.state.runs.is_empty() && app.prepared.is_none());
        for _ in 0..3 {
            keys.frame(
                &mut app,
                vec![Event::Key {
                    key: Key::Enter,
                    physical_key: None,
                    pressed: true,
                    repeat: true,
                    modifiers: Modifiers::COMMAND,
                }],
            );
        }
        // The fixture CLI is missing. Both entry points must fail normal preflight.
        let deadline = Instant::now() + Duration::from_secs(5);
        while app.busy {
            assert!(Instant::now() < deadline);
            app.runtime
                .as_ref()
                .unwrap()
                .block_on(async { tokio::time::sleep(Duration::from_millis(5)).await });
            app.poll();
        }
        assert!(!app.notice.is_empty());
        assert!(app.prepared.is_none() && app.state.runs.is_empty() && app.manager.is_idle());
        assert_eq!(serde_json::to_value(&app.state.draft).unwrap(), task);
        assert!(app.rx.try_recv().is_err());
    }
}

#[test]
fn run_shortcut_respects_button_prerequisites_and_current_cli_probe() {
    for gate in 0..7 {
        let (_temp, mut app) = app();
        ready(&mut app);
        match gate {
            0 => app.state.draft.prompt = "  ".into(),
            1 => app.selected.clear(),
            2 => app.busy = true,
            3 => app.attachment_work.pending = true,
            4 => app.followup_pending = true,
            5 => app.check_cli(egui::Context::default()),
            _ => app.closing = true,
        }
        let busy = app.busy;
        let mut keys = execution();
        assert!(!app.can_run_convoy());
        chord(&mut keys, &mut app, Key::Enter, Modifiers::COMMAND);
        assert_eq!(app.busy, busy);
        assert!(app.prepared.is_none() && app.state.runs.is_empty() && app.manager.is_idle());
    }
}

#[test]
fn repository_shortcut_selects_existing_query_and_escape_preserves_selection_and_groups() {
    let (_temp, mut app) = app();
    app.state.repositories = vec![repository("alpha"), repository("beta")];
    app.state.groups = vec![domain::RepositoryGroup {
        name: "Group".into(),
        repositories: vec![repository("beta").path],
    }];
    app.select_repositories(true);
    let selection = app.selected.clone();
    let mut keys = Keyboard::new(|app, ui, ctx| {
        app.handle_shortcuts(ctx);
        app.repositories_section(ui, ctx);
    });
    keys.frame(&mut app, vec![]);
    let group = app.repository_sections.sections[1].id;
    egui::collapsing_header::CollapsingState::load_with_default_open(&keys.ctx, group, false)
        .store(&keys.ctx);
    app.repository_sections.query = "béta 🌱".into();
    chord(&mut keys, &mut app, Key::F, Modifiers::COMMAND);
    assert_eq!(
        keys.ctx.memory(|m| m.focused()),
        Some(repository_filter_id())
    );
    let state = egui::TextEdit::load_state(&keys.ctx, repository_filter_id()).unwrap();
    assert_eq!(
        state.cursor.char_range().unwrap().as_sorted_char_range(),
        0.into()..6.into()
    );
    assert_eq!(app.repository_sections.query, "béta 🌱");
    keys.key(&mut app, Key::Escape);
    assert_ne!(
        keys.ctx.memory(|m| m.focused()),
        Some(repository_filter_id())
    );
    assert_eq!(app.repository_sections.query, "béta 🌱");
    assert_eq!(app.selected, selection);
    assert!(
        !egui::collapsing_header::CollapsingState::load(&keys.ctx, group)
            .unwrap()
            .is_open()
    );
    chord(&mut keys, &mut app, Key::F, Modifiers::COMMAND);
    keys.frame(&mut app, vec![Event::Text("alpha".into())]);
    assert_eq!(app.repository_sections.query, "alpha");
    assert_eq!(app.selected, selection);
}

#[test]
fn search_is_unhandled_without_filter_and_does_not_target_history() {
    let (_temp, mut app) = app();
    let ctx = egui::Context::default();
    let mut output = ctx.run_ui(
        egui::RawInput {
            events: vec![Event::Key {
                key: Key::F,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::COMMAND,
            }],
            ..Default::default()
        },
        |_| {
            app.handle_shortcuts(&ctx);
            assert!(ctx.input(|i| i.key_pressed(Key::F)));
        },
    );
    output.textures_delta.clear();
    assert!(!app.focus_repository_filter);
    assert!(app.history_search.query.is_empty());
}

#[test]
fn navigation_reuses_tabs_and_retains_run_job_diff_filters_and_focus() {
    let (_temp, mut app) = app();
    app.state.runs = vec![run(42, &[JobStatus::Succeeded, JobStatus::Succeeded])];
    app.selected_run = Some(42);
    app.selected_job = 1;
    app.diff_target = Some(repository("cached").path);
    app.diff = Some(Ok("cached diff".into()));
    app.history_search.query = "outside selected history".into();
    app.comparison_filter = crate::visibility::RepositoryFilter::Changed;
    let before = serde_json::to_value(&app.state).unwrap();
    let mut keys = Keyboard::new(|app, ui, ctx| {
        app.handle_shortcuts(ctx);
        let _ = ui.button("Focus anchor");
    });
    keys.focus(&mut app, "Focus anchor");
    let focus = keys.ctx.memory(|m| m.focused());
    for (key, tab) in [
        (Key::Num1, Tab::Activity),
        (Key::Num2, Tab::Diff),
        (Key::Num3, Tab::Raw),
        (Key::Num4, Tab::Task),
    ] {
        chord(&mut keys, &mut app, key, Modifiers::COMMAND);
        assert_eq!(app.tab, tab);
        assert_eq!(keys.ctx.memory(|m| m.focused()), focus);
        assert_eq!(app.selected_run, Some(42));
        assert_eq!(app.selected_job, 1);
        assert_eq!(app.diff, Some(Ok("cached diff".into())));
        assert_eq!(app.diff_target, Some(repository("cached").path));
        assert_eq!(
            app.comparison_filter,
            crate::visibility::RepositoryFilter::Changed
        );
    }
    app.selected_run = None;
    chord(&mut keys, &mut app, Key::Num1, Modifiers::COMMAND);
    assert_eq!(app.tab, Tab::Task);
    assert_eq!(serde_json::to_value(&app.state).unwrap(), before);
}

#[test]
fn text_focus_blocks_run_and_views_and_preserves_copy_paste_select_all() {
    let (_temp, mut app) = app();
    ready(&mut app);
    app.state.runs = vec![run(42, &[JobStatus::Succeeded])];
    app.selected_run = Some(42);
    app.tab = Tab::Review;
    let mut keys = Keyboard::new(|app, ui, ctx| {
        app.handle_shortcuts(ctx);
        let edit = ui.add(
            egui::TextEdit::multiline(&mut app.state.draft.prompt).id(egui::Id::new("test_task")),
        );
        if app.focus_draft {
            edit.request_focus();
            app.focus_draft = false;
        }
        app.repositories_section(ui, ctx);
    });
    app.focus_draft = true;
    keys.frame(&mut app, vec![]);
    chord(&mut keys, &mut app, Key::Enter, Modifiers::COMMAND);
    chord(&mut keys, &mut app, Key::Num2, Modifiers::COMMAND);
    assert!(!app.busy && app.prepared.is_none());
    assert_eq!(app.tab, Tab::Review);
    chord(&mut keys, &mut app, Key::A, Modifiers::COMMAND);
    keys.frame(&mut app, vec![Event::Copy]);
    assert!(
        matches!(keys.commands.last(), Some(egui::OutputCommand::CopyText(s)) if s == "Review the repository")
    );
    keys.frame(&mut app, vec![Event::Paste("Edited task".into())]);
    assert_eq!(app.state.draft.prompt, "Edited task");
    app.repository_sections.query = "replace me".into();
    app.focus_draft = true;
    keys.frame(&mut app, vec![]);
    keys.frame(
        &mut app,
        vec![
            Event::Key {
                key: Key::F,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::COMMAND,
            },
            Event::Key {
                key: Key::F,
                physical_key: None,
                pressed: false,
                repeat: false,
                modifiers: Modifiers::COMMAND,
            },
            Event::Text("new query".into()),
        ],
    );
    assert_eq!(app.repository_sections.query, "new query");
    assert_eq!(app.state.draft.prompt, "Edited task");
    chord(&mut keys, &mut app, Key::F, Modifiers::COMMAND);
    assert_eq!(
        keys.ctx.memory(|m| m.focused()),
        Some(repository_filter_id())
    );
    assert_eq!(app.state.draft.prompt, "Edited task");
}

#[test]
fn every_dialog_and_native_picker_blocks_background_shortcuts() {
    for modal in 0..12 {
        let (_temp, mut app) = app();
        ready(&mut app);
        app.state.runs = vec![isolated_history(42, JobStatus::Succeeded, Some(true))];
        app.selected_run = Some(42);
        app.tab = Tab::Review;
        match modal {
            0 => prepare(&mut app, AgentId::Codex),
            1 => app.about_open = true,
            2 => app.shortcuts_open = true,
            3 => app.find_cli(egui::Context::default()),
            4 => {
                app.library_editor = Some(library::LibraryEditor::Template {
                    index: None,
                    value: domain::TaskTemplate {
                        name: String::new(),
                        prompt: String::new(),
                    },
                    error: String::new(),
                })
            }
            5 => {
                app.validation_editor = Some(crate::ui::validation::Editor::new(
                    &app.state.repositories[0],
                    None,
                ))
            }
            6 => {
                app.discard_confirmation =
                    Some((42, 0, crate::persistence::results::Action::Discard))
            }
            7 => app.offer_bulk_discard(crate::persistence::bulk_discard::Scope::All),
            8 => app.quit_requested = true,
            9 => app.browsing_repository = true,
            10 => app.worktree_location.pending = true,
            _ => app.attachment_work.pending = true,
        }
        let mut keys = Keyboard::new(|app, _, ctx| app.handle_shortcuts(ctx));
        for key in [Key::Enter, Key::F, Key::Num2] {
            chord(&mut keys, &mut app, key, Modifiers::COMMAND);
        }
        assert!(!app.busy && !app.focus_repository_filter);
        assert_eq!(app.tab, Tab::Review);
        assert_eq!(app.state.runs.len(), 1);
        assert!(app.manager.is_idle());
    }
}

#[test]
fn review_retains_dirty_ack_and_modified_enter_cannot_confirm() {
    let (_temp, mut app) = app();
    prepare(&mut app, AgentId::Codex);
    app.prepared.as_mut().unwrap().repositories[0]
        .state
        .summary
        .changed = 1;
    let mut keys = execution();
    keys.frame(&mut app, vec![]);
    chord(&mut keys, &mut app, Key::Enter, Modifiers::COMMAND);
    assert!(app.prepared.is_some() && !app.dirty_ack);
    assert!(app.state.runs.is_empty());
    app.dirty_ack = true;
    keys.focus(&mut app, "Start convoy");
    chord(&mut keys, &mut app, Key::Enter, Modifiers::COMMAND);
    assert!(app.prepared.is_some() && app.state.runs.is_empty());
    keys.key(&mut app, Key::Escape);
    assert!(app.prepared.is_none() && app.prepared_provenance.is_none());
    assert!(app.manager.is_idle());
}

#[test]
fn escape_cancels_destructive_dialogs_without_operating_and_only_closes_top_modal() {
    for modal in 0..3 {
        let (_temp, mut app) = app();
        app.state.runs = vec![isolated_history(42, JobStatus::Succeeded, Some(true))];
        app.about_open = true;
        let mut keys = Keyboard::new(|app, _, ctx| {
            app.handle_shortcuts(ctx);
            if !app.quit_requested {
                app.about_window(ctx);
            }
            app.quit_window(ctx);
            app.discard_window(ctx);
            app.bulk_discard_window(ctx);
        });
        keys.frame(&mut app, vec![]);
        match modal {
            0 => {
                app.quit_requested = true;
                app.focus_quit_cancel = true;
            }
            1 => {
                app.discard_confirmation =
                    Some((42, 0, crate::persistence::results::Action::Discard));
                app.focus_discard_cancel = true;
            }
            _ => app.offer_bulk_discard(crate::persistence::bulk_discard::Scope::All),
        }
        keys.frame(&mut app, vec![]);
        keys.frame(&mut app, vec![]);
        chord(&mut keys, &mut app, Key::Enter, Modifiers::COMMAND);
        keys.key(&mut app, Key::Escape);
        assert!(app.about_open);
        assert!(
            !app.quit_requested
                && app.discard_confirmation.is_none()
                && app.bulk_confirmation.is_none()
        );
        assert!(!app.closing && app.result_operation.is_none() && app.bulk_discard.is_none());
        assert_eq!(
            app.state.runs[0].jobs[0].resolution,
            domain::ResultResolution::Unresolved
        );
    }
}

#[test]
fn shortcut_help_is_keyboard_accessible_and_escape_is_safe() {
    let (_temp, mut app) = app();
    ready(&mut app);
    let mut keys = Keyboard::new(|app, ui, ctx| {
        app.handle_shortcuts(ctx);
        app.header(ui, ctx);
        app.shortcuts_window(ctx);
    });
    keys.activate(&mut app, "Settings");
    keys.activate(&mut app, "Keyboard Shortcuts…");
    assert!(app.shortcuts_open);
    chord(&mut keys, &mut app, Key::Enter, Modifiers::COMMAND);
    assert!(app.shortcuts_open && !app.busy);
    keys.key(&mut app, Key::Escape);
    assert!(!app.shortcuts_open && !app.busy);
    assert!(app.state.runs.is_empty());
}

#[test]
fn held_escape_does_not_dismiss_the_underlying_dialog() {
    let (_temp, mut app) = app();
    let mut keys = Keyboard::new(|app, _, ctx| {
        app.handle_shortcuts(ctx);
        app.about_window(ctx);
        app.shortcuts_window(ctx);
    });
    app.about_open = true;
    keys.frame(&mut app, vec![]);
    app.shortcuts_open = true;
    keys.frame(&mut app, vec![]);
    keys.frame(&mut app, vec![]);
    for repeat in [false, true, true] {
        keys.frame(
            &mut app,
            vec![Event::Key {
                key: Key::Escape,
                physical_key: None,
                pressed: true,
                repeat,
                modifiers: Modifiers::NONE,
            }],
        );
        assert!(app.about_open && !app.shortcuts_open);
    }
}

#[test]
fn full_ui_help_fits_compact_sizes_and_all_themes() {
    for preference in [
        domain::Appearance::System,
        domain::Appearance::Dark,
        domain::Appearance::Light,
    ] {
        for size in [egui::vec2(780.0, 560.0), egui::vec2(1180.0, 820.0)] {
            let (_temp, mut app) = app();
            let ctx = egui::Context::default();
            theme::install(&ctx);
            ctx.enable_accesskit();
            ctx.set_theme(theme::preference(preference));
            app.shortcuts_open = true;
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
                    ..Default::default()
                },
                |ui| app.main_ui(ui),
            );
            output.textures_delta.clear();
            let nodes = output.platform_output.accesskit_update.unwrap().nodes;
            for label in [
                "Keyboard Shortcuts",
                "Close shortcuts",
                "Run Convoy (review first)",
            ] {
                let node = nodes
                    .iter()
                    .find(|(_, n)| n.label() == Some(label) || n.value() == Some(label))
                    .unwrap();
                let bounds = node.1.bounds().unwrap();
                assert!(
                    bounds.x0 >= 0.0
                        && bounds.y0 >= 0.0
                        && bounds.x1 <= f64::from(size.x)
                        && bounds.y1 <= f64::from(size.y),
                    "{label}: {bounds:?}"
                );
            }
        }
    }
}
