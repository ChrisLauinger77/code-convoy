use super::*;

#[test]
fn copy_actions_are_keyboard_accessible_and_copy_selected_history_without_mutation() {
    let (_temp, mut app) = app();
    app.state.runs = vec![
        run(42, &[JobStatus::Succeeded, JobStatus::Failed]),
        run(99, &[JobStatus::Succeeded]),
    ];
    app.selected_run = Some(42);
    app.selected_job = 1;
    // Copy is independent of both visible history and comparison filters.
    app.history_search.query = "no match".into();
    app.comparison_filter = crate::visibility::RepositoryFilter::Changed;
    let before = serde_json::to_value(&app.state).unwrap();
    let mut keys = Keyboard::new(|app, ui, ctx| app.results(ui, ctx));
    keys.frame(&mut app, vec![]);
    keys.activate(&mut app, "Copy Summary");
    let [egui::OutputCommand::CopyText(text)] = keys.commands.as_slice() else {
        panic!("Expected one native clipboard command: {:?}", keys.commands);
    };
    assert_eq!(text, &result_markdown::convoy(&app.state.runs[0]));
    assert!(
        keys.nodes
            .values()
            .any(|n| keys.label(n) == "Summary copied")
    );
    keys.commands.clear();
    keys.activate(&mut app, "Copy Repository Result");
    let expected = result_markdown::repository(&app.state.runs[0], &app.state.runs[0].jobs[1]);
    assert!(
        matches!(keys.commands.as_slice(), [egui::OutputCommand::CopyText(text)] if text == &expected)
    );
    assert!(
        keys.nodes
            .values()
            .any(|n| keys.label(n) == "Repository result copied")
    );
    for tab in [
        Tab::Activity,
        Tab::Diff,
        Tab::Raw,
        Tab::Validation,
        Tab::Task,
    ] {
        app.tab = tab;
        keys.commands.clear();
        keys.frame(&mut app, vec![]);
        keys.activate(&mut app, "Copy Repository Result");
        assert!(
            matches!(keys.commands.as_slice(), [egui::OutputCommand::CopyText(text)] if text == &expected)
        );
    }
    assert_eq!(serde_json::to_value(&app.state).unwrap(), before);
    assert!(app.manager.is_idle());
    assert!(app.diff_target.is_none());
    assert!(app.review_pending.is_none());
}

#[test]
fn running_or_missing_results_have_no_copy_action() {
    for statuses in [vec![], vec![JobStatus::Running], vec![JobStatus::Queued]] {
        let (_temp, mut app) = app();
        app.state.runs = vec![run(42, &statuses)];
        app.selected_run = Some(42);
        let mut keys = Keyboard::new(|app, ui, ctx| app.results(ui, ctx));
        keys.frame(&mut app, vec![]);
        assert!(!keys.nodes.values().any(|n| {
            matches!(
                keys.label(n).as_str(),
                "Copy Summary" | "Copy Repository Result"
            )
        }));
        assert!(keys.commands.is_empty());
    }
    let (_temp, mut app) = app();
    app.state.runs = vec![run(42, &[JobStatus::Succeeded, JobStatus::Running])];
    app.selected_run = Some(42);
    let mut keys = Keyboard::new(|app, ui, ctx| app.results(ui, ctx));
    keys.frame(&mut app, vec![]);
    assert!(!keys.nodes.values().any(|n| keys.label(n) == "Copy Summary"));
    keys.activate(&mut app, "Copy Repository Result");
    assert!(matches!(
        keys.commands.as_slice(),
        [egui::OutputCommand::CopyText(_)]
    ));
}

#[test]
fn copy_feedback_expires_and_buttons_fit_compact_theme_layouts() {
    for preference in [
        domain::Appearance::System,
        domain::Appearance::Dark,
        domain::Appearance::Light,
    ] {
        for size in [egui::vec2(390.0, 560.0), egui::vec2(590.0, 820.0)] {
            let (_temp, mut app) = app();
            app.state.runs = vec![run(42, &[JobStatus::Succeeded])];
            app.selected_run = Some(42);
            let mut keys = Keyboard::new(|app, ui, ctx| app.results(ui, ctx));
            // Theme install uses the same egui visuals as the application.
            keys.ctx.set_theme(theme::preference(preference));
            keys.input(
                &mut app,
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
                    time: Some(1.0),
                    ..Default::default()
                },
            );
            for label in ["Copy Summary", "Copy Repository Result"] {
                let node = keys
                    .nodes
                    .values()
                    .find(|n| keys.label(n) == label)
                    .unwrap();
                let bounds = node.bounds().unwrap();
                assert!(
                    bounds.x0 >= 0.0 && bounds.x1 <= f64::from(size.x),
                    "{label}: {bounds:?}"
                );
            }
        }
    }
    // Inspect fresh accessibility output so removed feedback isn't kept by the harness.
    let ctx = egui::Context::default();
    ctx.enable_accesskit();
    let run = run(42, &[JobStatus::Succeeded]);
    ctx.data_mut(|d| d.insert_temp(egui::Id::new(("copy_summary", 42u64, 0usize)), 4.0f64));
    for (time, visible) in [(2.0, true), (5.0, false)] {
        let mut output = ctx.run_ui(
            egui::RawInput {
                time: Some(time),
                ..Default::default()
            },
            |ui| {
                super::super::super::copy_results::summary(ui, &run);
            },
        );
        output.textures_delta.clear();
        let nodes = output.platform_output.accesskit_update.unwrap().nodes;
        assert_eq!(
            nodes
                .iter()
                .any(|(_, n)| n.value() == Some("Summary copied")
                    || n.label() == Some("Summary copied")),
            visible
        );
        assert!(output.platform_output.commands.is_empty());
    }
}
