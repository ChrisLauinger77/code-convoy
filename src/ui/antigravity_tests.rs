//! Real-widget regressions; no installed CLI, account, model call or live repository.
use super::*;

#[test]
fn antigravity_selection_settings_and_setup_are_keyboard_accessible_and_backend_local() {
    let (_temp, mut app) = app();
    let mut keys = Keyboard::new(|app, ui, ctx| app.agent_section(ui, ctx));
    keys.frame(&mut app, vec![]);
    keys.activate(&mut app, "Agent");
    keys.activate(&mut app, "Antigravity CLI");
    assert_eq!(app.state.draft.agent, AgentId::Antigravity);
    assert!(
        keys.nodes
            .values()
            .any(|node| keys.label(node) == agents::antigravity::SETUP_HINT)
    );
    for (label, value) in [
        ("Model", "antigravity-model"),
        ("Time limit (seconds)", "120"),
    ] {
        keys.replace_text(&mut app, label, value);
    }
    for (label, choice) in [
        ("Effort", "High"),
        ("Permissions", "Approve all tool requests"),
        ("Terminal sandbox", "Enable for this job"),
    ] {
        keys.activate(&mut app, label);
        keys.activate(&mut app, choice);
    }
    let antigravity_options = app.state.draft.options.clone();
    assert_eq!(antigravity_options["model"], "antigravity-model");
    assert_eq!(antigravity_options["effort"], "high");
    assert_eq!(antigravity_options["permissions"], "skip");
    assert_eq!(antigravity_options["sandbox"], "enabled");
    assert_eq!(antigravity_options["timeout_seconds"], "120");
    keys.activate(&mut app, "Install Antigravity CLI");
    keys.activate(&mut app, "Antigravity headless help");
    let urls: Vec<_> = keys
        .commands
        .iter()
        .filter_map(|command| match command {
            egui::OutputCommand::OpenUrl(url) => Some(url.url.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(
        urls,
        [
            agents::antigravity::INSTALLATION_URL,
            agents::antigravity::HEADLESS_URL
        ]
    );

    keys.activate(&mut app, "Agent");
    keys.activate(&mut app, "Cursor CLI");
    keys = Keyboard::new(|app, ui, ctx| app.agent_section(ui, ctx));
    keys.frame(&mut app, vec![]);
    assert!(!keys.nodes.values().any(|node| matches!(
        keys.label(node).as_str(),
        "Terminal sandbox" | "Effort" | "Antigravity headless help"
    )));
    keys.activate(&mut app, "Agent");
    keys.activate(&mut app, "Antigravity CLI");
    assert_eq!(app.state.draft.options, antigravity_options);
    app.save();
    assert_eq!(app.store.load().unwrap().draft.options, antigravity_options);
    assert!(app.manager.is_idle() && app.state.runs.is_empty());
}
