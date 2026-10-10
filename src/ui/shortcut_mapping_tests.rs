use super::*;

fn frame(ctx: &egui::Context, input: egui::RawInput, ui: impl FnMut(&mut egui::Ui)) {
    ctx.run_ui(input, ui).textures_delta.clear();
}

fn key(key: Key, modifiers: Modifiers, repeat: bool) -> Event {
    Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat,
        modifiers,
    }
}

fn available() -> Scope {
    Scope {
        run_available: true,
        filter_available: true,
        filter_focused: true,
        result_available: true,
        ..Default::default()
    }
}

#[test]
fn native_modifiers_and_all_bindings_match_exactly() {
    for native in [Modifiers::CTRL, Modifiers::MAC_CMD] {
        let modifiers = native | Modifiers::COMMAND;
        for binding in &BINDINGS[..6] {
            assert_eq!(
                mapping(binding.shortcut.logical_key, modifiers),
                Some(binding.action)
            );
            for wrong in [
                Modifiers::NONE,
                native,
                modifiers | Modifiers::ALT,
                modifiers | Modifiers::SHIFT,
                Modifiers::CTRL | Modifiers::MAC_CMD | Modifiers::COMMAND,
            ] {
                assert_eq!(mapping(binding.shortcut.logical_key, wrong), None);
            }
        }
        for unsupported in [
            Key::A,
            Key::C,
            Key::V,
            Key::X,
            Key::Z,
            Key::Num5,
            Key::Q,
            Key::Escape,
        ] {
            assert_eq!(mapping(unsupported, modifiers), None);
        }
    }
    assert_eq!(
        mapping(Key::Escape, Modifiers::NONE),
        Some(Action::DismissSearch)
    );
}

#[test]
fn help_uses_egui_platform_labels() {
    let ctx = egui::Context::default();
    for os in [
        egui::os::OperatingSystem::Mac,
        egui::os::OperatingSystem::Windows,
        egui::os::OperatingSystem::Nix,
    ] {
        ctx.set_os(os);
        frame(&ctx, egui::RawInput::default(), |_| {
            let label = BINDINGS[1].display(&ctx);
            if os.is_mac() {
                assert!(ctx.fonts_mut(|f| f.has_glyph(&egui::FontId::proportional(14.0), '⌘')));
                assert_eq!(label, "⌘ F");
            } else {
                assert_eq!(label, "Ctrl+F");
            }
        });
    }
}

#[test]
fn contexts_gate_actions_and_leave_editing_shortcuts_alone() {
    for binding in BINDINGS {
        assert!(available().allows(binding.action));
        assert!(
            !Scope {
                blocked: true,
                ..available()
            }
            .allows(binding.action)
        );
        assert!(!Scope::default().allows(binding.action));
    }
    let editing = Scope {
        text_edit: true,
        ..available()
    };
    assert!(!editing.allows(Action::Run));
    assert!(!editing.allows(Action::View(Tab::Activity)));
    assert!(editing.allows(Action::FocusRepositories));
    assert!(editing.allows(Action::DismissSearch));
}

#[test]
fn repeated_events_and_multiple_passes_dispatch_once() {
    let ctx = egui::Context::default();
    let mut dispatcher = Dispatcher::default();
    frame(
        &ctx,
        egui::RawInput {
            events: vec![
                key(Key::Enter, Modifiers::COMMAND, false),
                key(Key::Enter, Modifiers::COMMAND, true),
                key(Key::Num2, Modifiers::COMMAND, false),
            ],
            ..Default::default()
        },
        |_| {
            assert_eq!(dispatcher.take(&ctx, available()), Some(Action::Run));
            // Explicitly replay the input as a layout retry would.
            ctx.input_mut(|i| i.events.push(key(Key::Enter, Modifiers::COMMAND, false)));
            assert_eq!(dispatcher.take(&ctx, available()), None);
            assert!(ctx.input(|i| i.events.is_empty()));
        },
    );
    frame(
        &ctx,
        egui::RawInput {
            events: vec![key(Key::Enter, Modifiers::COMMAND, true)],
            ..Default::default()
        },
        |_| {
            assert_eq!(dispatcher.take(&ctx, available()), None);
            assert!(ctx.input(|i| i.events.is_empty()));
        },
    );
}

#[test]
fn unavailable_search_and_navigation_are_unhandled_and_modal_enter_cannot_click() {
    let ctx = egui::Context::default();
    let mut dispatcher = Dispatcher::default();
    for scope in [
        Scope::default(),
        Scope {
            blocked: true,
            ..available()
        },
    ] {
        let events = vec![
            key(Key::F, Modifiers::COMMAND, false),
            key(Key::Num1, Modifiers::COMMAND, false),
            key(Key::A, Modifiers::COMMAND, false),
        ];
        frame(
            &ctx,
            egui::RawInput {
                events: events.clone(),
                ..Default::default()
            },
            |_| {
                let before = ctx.input(|i| i.events.clone());
                assert_eq!(dispatcher.take(&ctx, scope), None);
                assert_eq!(ctx.input(|i| i.events.clone()), before);
            },
        );
    }
    frame(
        &ctx,
        egui::RawInput {
            events: vec![key(Key::Enter, Modifiers::COMMAND, false)],
            ..Default::default()
        },
        |_| {
            assert_eq!(
                dispatcher.take(
                    &ctx,
                    Scope {
                        blocked: true,
                        ..available()
                    }
                ),
                None
            );
            assert!(ctx.input(|i| i.events.is_empty()));
        },
    );
}

#[test]
fn text_editor_receives_original_events_and_escape_repeat_is_suppressed() {
    let ctx = egui::Context::default();
    let mut dispatcher = Dispatcher::default();
    let events = vec![
        Event::Copy,
        Event::Cut,
        Event::Paste("text".into()),
        key(Key::A, Modifiers::COMMAND, false),
        key(Key::Z, Modifiers::COMMAND, false),
        key(Key::F, Modifiers::CTRL, false),
        key(Key::Enter, Modifiers::COMMAND, false),
        key(Key::Num3, Modifiers::COMMAND, false),
    ];
    frame(
        &ctx,
        egui::RawInput {
            events: events.clone(),
            ..Default::default()
        },
        |_| {
            assert_eq!(
                dispatcher.take(
                    &ctx,
                    Scope {
                        text_edit: true,
                        ..available()
                    }
                ),
                None
            );
            assert_eq!(ctx.input(|i| i.events.clone()), events);
        },
    );
    frame(
        &ctx,
        egui::RawInput {
            events: vec![key(Key::Escape, Modifiers::NONE, false)],
            ..Default::default()
        },
        |_| {},
    );
    frame(
        &ctx,
        egui::RawInput {
            events: vec![key(Key::Escape, Modifiers::NONE, true)],
            ..Default::default()
        },
        |_| {
            assert_eq!(
                dispatcher.take(
                    &ctx,
                    Scope {
                        blocked: true,
                        ..available()
                    }
                ),
                None
            );
            assert!(ctx.input(|i| i.events.is_empty()));
        },
    );
}
