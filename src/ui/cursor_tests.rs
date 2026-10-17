//! Real-widget regressions; no installed CLI, account, model call or live repository.
use super::*;

#[test]
fn cursor_selection_settings_and_setup_are_keyboard_accessible_and_backend_local() {
    let (_temp, mut app) = app();
    let mut keys = Keyboard::new(|app, ui, ctx| app.agent_section(ui, ctx));
    keys.frame(&mut app, vec![]);
    keys.activate(&mut app, "Agent");
    keys.activate(&mut app, "Cursor CLI");
    assert_eq!(app.state.draft.agent, AgentId::Cursor);
    assert!(
        keys.nodes
            .values()
            .any(|node| keys.label(node) == agents::cursor::SETUP_HINT)
    );
    for (label, value) in [("Model", "cursor-model"), ("Time limit (seconds)", "120")] {
        keys.replace_text(&mut app, label, value);
    }
    for (label, choice) in [
        ("Permissions", "Force allow unless denied"),
        ("Workspace trust", "Trust assigned workspace"),
    ] {
        keys.activate(&mut app, label);
        keys.activate(&mut app, choice);
    }
    let cursor_options = app.state.draft.options.clone();
    assert_eq!(cursor_options["model"], "cursor-model");
    assert_eq!(cursor_options["permissions"], "force");
    assert_eq!(cursor_options["workspace_trust"], "trust");
    assert_eq!(cursor_options["timeout_seconds"], "120");
    keys.activate(&mut app, "Install Cursor CLI");
    keys.activate(&mut app, "Cursor sign-in help");
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
            agents::cursor::INSTALLATION_URL,
            agents::cursor::AUTHENTICATION_URL
        ]
    );

    keys.activate(&mut app, "Agent");
    keys.activate(&mut app, "Claude Code");
    keys = Keyboard::new(|app, ui, ctx| app.agent_section(ui, ctx));
    keys.frame(&mut app, vec![]);
    assert!(!keys.nodes.values().any(|node| matches!(
        keys.label(node).as_str(),
        "Workspace trust" | "Time limit (seconds)" | "Cursor sign-in help"
    )));
    keys.activate(&mut app, "Agent");
    keys.activate(&mut app, "Cursor CLI");
    assert_eq!(app.state.draft.options, cursor_options);
    app.save();
    assert_eq!(app.store.load().unwrap().draft.options, cursor_options);
    assert!(app.manager.is_idle() && app.state.runs.is_empty());
}

#[test]
fn cursor_results_and_history_keep_original_settings_when_the_draft_changes() {
    let (_temp, mut app) = app();
    let mut history = run(42, &[JobStatus::Queued, JobStatus::Queued]);
    history.task.agent = AgentId::Cursor;
    history.task.options = [
        ("model".into(), "original-cursor-model".into()),
        ("permissions".into(), "configured".into()),
        ("workspace_trust".into(), "configured".into()),
        ("timeout_seconds".into(), "120".into()),
    ]
    .into();
    history.jobs.iter_mut().for_each(|job| {
        job.log = Default::default();
    });
    app.state.repositories = history
        .jobs
        .iter()
        .map(|job| job.repository.clone())
        .collect();
    app.state.runs = vec![history];
    app.select_run(Some(42));
    app.apply_event(Event::Started {
        run: 42,
        job: 0,
        before: Default::default(),
    });
    assert_eq!(app.state.runs[0].status(), JobStatus::Running);
    let activity = "[CodeConvoy] Cursor initialized · model fixture\nGrüße 🌍\n";
    let raw = include_str!("../../tests/fixtures/cursor-success.jsonl");
    app.apply_event(Event::Output {
        run: 42,
        job: 0,
        text: activity.into(),
        raw: raw.into(),
    });
    for (job, status, detail) in [
        (
            0,
            JobStatus::Succeeded,
            "Cursor reported a successful terminal result.",
        ),
        (
            1,
            JobStatus::Failed,
            "Cursor authentication is required. Run agent status / agent login separately; CodeConvoy does not manage credentials.",
        ),
    ] {
        app.apply_event(Event::Finished {
            run: 42,
            job,
            status,
            exit_code: Some(if job == 0 { 0 } else { 1 }),
            detail: detail.into(),
        });
    }
    let saved_task = app.state.runs[0].task.clone();
    app.state.select_agent(AgentId::Codex);
    app.state.draft.prompt = "A different next task".into();
    app.state
        .draft
        .options
        .insert("model".into(), "next-model".into());
    let mut keys = Keyboard::new(|app, ui, ctx| app.results(ui, ctx));
    for (tab, expected) in [
        (Tab::Activity, "Grüße 🌍"),
        (Tab::Raw, "\"type\":\"result\""),
        (Tab::Task, "original-cursor-model"),
    ] {
        app.tab = tab;
        keys.nodes.clear();
        keys.frame(&mut app, vec![]);
        assert!(
            keys.nodes
                .values()
                .any(|node| keys.label(node).contains(expected)),
            "{tab:?}"
        );
    }
    app.tab = Tab::Diff;
    app.diff = Some(Ok(git::Diff {
        text: "-old\n+Cursor fixture edit".into(),
        changed: Some(true),
    }));
    keys.nodes.clear();
    keys.frame(&mut app, vec![]);
    assert!(
        keys.nodes
            .values()
            .any(|node| keys.label(node).contains("+Cursor fixture edit"))
    );
    // Copy/comparison summaries keep agent outcome separate from observed Git changes.
    let summary = result_markdown::convoy(&app.state.runs[0]);
    assert!(
        summary.contains("Cursor")
            && summary.contains("Succeeded: 1")
            && summary.contains("Failed: 1"),
        "{summary}"
    );
    assert!(diagnostics::summary(&app.state.runs[0].jobs[1].detail).contains("agent login"));
    app.save();
    assert_eq!(
        serde_json::to_value(&app.store.load().unwrap().runs[0].task).unwrap(),
        serde_json::to_value(&saved_task).unwrap()
    );
    app.reuse_convoy(42);
    assert_eq!(
        serde_json::to_value(&app.state.draft).unwrap(),
        serde_json::to_value(&saved_task).unwrap()
    );
    assert_eq!(
        serde_json::to_value(&app.state.runs[0].task).unwrap(),
        serde_json::to_value(&saved_task).unwrap()
    );
    assert_eq!(app.selected.len(), 2);
    assert!(app.manager.is_idle());
}
