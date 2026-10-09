use super::*;

#[test]
fn health_refresh_is_independent_and_superseded_removed_and_active_replies_are_rejected() {
    let (_temp, mut app) = app();
    app.state.repositories = vec![repository("alpha")];
    let path = app.state.repositories[0].path.clone();
    let ctx = egui::Context::default();
    app.busy = true;
    app.refresh(ctx.clone());
    let first = app.repository_refresh;
    let cancellation = app.repository_health_cancel.clone();
    app.refresh(ctx);
    let second = app.repository_refresh;
    assert!(cancellation.is_cancelled());
    app.repository_refreshed(first, path.clone(), Err("obsolete".into()));
    assert!(app.repository_states.is_empty());
    assert_eq!(app.repository_pending.get(&path), Some(&second));
    app.repository_refreshed(second, path.clone(), Err("current".into()));
    assert_eq!(
        app.repository_states[&path].as_ref().unwrap_err(),
        "current"
    );
    assert!(app.busy); // Does not clear another operation's pending state.
    app.repository_states.clear();
    app.repository_pending.insert(path.clone(), second);
    app.state.runs = vec![run(1, &[JobStatus::Running])];
    app.state.runs[0].jobs[0].repository.path = path.clone();
    app.repository_refreshed(second, path.clone(), Err("during execution".into()));
    assert!(app.repository_states.is_empty());
    app.state.runs.clear();
    app.repository_pending.insert(path.clone(), second);
    app.state.repositories.clear();
    app.repository_refreshed(second, path, Err("unregistered".into()));
    assert!(app.repository_states.is_empty());
}

#[test]
fn health_refresh_during_result_operations_only_suppresses_overlapping_repositories() {
    use crate::persistence::bulk_discard::{Batch, Plan, Scope};
    for bulk in [false, true] {
        let (_temp, mut app) = app();
        app.state.runs = vec![
            run(1, &[JobStatus::Succeeded]),
            isolated_history(42, JobStatus::Succeeded, Some(true)),
        ];
        let source = app.state.runs[1].jobs[0].repository.path.clone();
        let paths = [
            source.clone(),
            source.join("nested"),
            source.parent().unwrap().to_path_buf(),
            PathBuf::from("/unrelated/repo"),
        ];
        app.state.repositories = paths
            .iter()
            .map(|path| Repository {
                name: "fixture".into(),
                path: path.clone(),
            })
            .collect();
        let ctx = egui::Context::default();
        app.refresh(ctx.clone());
        let request = app.repository_refresh;
        if bulk {
            let plan = Plan::new(&app.state, Scope::All);
            assert_eq!(plan.len(), 1);
            app.bulk_discard = Some(Batch::new(plan, &mut app.state));
        }
        app.result_operation = Some((42, 0));
        for path in &paths[..3] {
            assert!(app.repository_in_use(path));
            app.repository_refreshed(request, path.clone(), Err("overlapping work".into()));
            assert!(!app.repository_states.contains_key(path));
        }
        assert!(!app.repository_in_use(&paths[3]));
        app.repository_refreshed(request, paths[3].clone(), Err("independent result".into()));
        assert_eq!(
            app.repository_states[&paths[3]].as_ref().unwrap_err(),
            "independent result"
        );
        app.refresh(ctx.clone());
        assert_eq!(app.repository_pending.len(), 1);
        assert!(app.repository_pending.contains_key(&paths[3]));
        app.result_operation = None;
        // A bulk batch between operations has no Git work in flight.
        app.refresh(ctx);
        assert_eq!(app.repository_pending.len(), paths.len());
    }
}

#[test]
fn result_operation_start_and_finish_invalidate_only_overlapping_health() {
    use crate::persistence::results::{Action, Completion, Operation};
    for action in [Action::Apply, Action::Discard] {
        let (_temp, mut app) = app();
        app.state.runs = vec![isolated_history(42, JobStatus::Succeeded, Some(true))];
        let job = app.state.runs[0].jobs[0].clone();
        let source = &job.repository.path;
        let paths = [
            source.clone(),
            source.join("nested"),
            source.parent().unwrap().to_path_buf(),
            PathBuf::from("/unrelated/repo"),
        ];
        app.state.repositories = paths
            .iter()
            .map(|path| Repository {
                name: "fixture".into(),
                path: path.clone(),
            })
            .collect();
        for path in &paths {
            app.repository_states
                .insert(path.clone(), Err("before operation".into()));
            app.repository_pending.insert(path.clone(), 1);
        }
        let operation = || Operation {
            run: 42,
            index: 0,
            action,
            job: job.clone(),
        };
        app.dispatch_result_operation(&egui::Context::default(), operation());
        assert_eq!(app.repository_states.len(), 1);
        assert_eq!(app.repository_pending.len(), 1);
        assert!(app.repository_states.contains_key(&paths[3]));
        assert!(app.repository_pending.contains_key(&paths[3]));
        // A check that overlaps a write must not be accepted after completion.
        for path in &paths[..3] {
            app.repository_pending.insert(path.clone(), 2);
        }
        let completion = match action {
            Action::Apply => {
                Completion::Apply(crate::worktrees::apply::Outcome::Blocked("fixture".into()))
            }
            _ => Completion::Cleanup(Err("fixture".into())),
        };
        app.tx
            .send(Message::Resolved(Box::new(operation()), completion))
            .unwrap();
        app.poll();
        assert!(app.result_operation.is_none());
        for path in &paths[..3] {
            app.repository_refreshed(2, path.clone(), Err("late overlapping result".into()));
            assert!(!app.repository_states.contains_key(path));
            assert!(!app.repository_pending.contains_key(path));
        }
        app.repository_refreshed(1, paths[3].clone(), Err("independent result".into()));
        assert_eq!(
            app.repository_states[&paths[3]].as_ref().unwrap_err(),
            "independent result"
        );
    }
}

#[test]
fn comparison_filters_preserve_detail_selection_and_snapshots_survive_live_review() {
    let (_temp, mut app) = app();
    app.state.runs = vec![run(12, &[JobStatus::Failed, JobStatus::Succeeded])];
    app.select_run(Some(12));
    app.selected_job = 1;
    let changes = crate::visibility::Changes {
        changed: Some(true),
        files: Some(2),
    };
    app.apply_event(Event::Changes {
        run: 12,
        job: 1,
        changes,
    });
    app.tx
        .send(Message::ReviewStats(
            12,
            1,
            Ok(crate::review::Statistics::default()),
        ))
        .unwrap();
    app.poll();
    app.history_search.query = "no match".into();
    assert!(app.history_search.matching(&app.state.runs).is_empty());
    app.comparison_filter = crate::visibility::RepositoryFilter::Failed;
    assert_eq!(app.selected_run, Some(12));
    assert_eq!(app.selected_job, 1);
    assert_eq!(app.state.runs[0].jobs[1].completion_changes, Some(changes));
    app.history_search.clear();
    assert_eq!(app.history_search.matching(&app.state.runs), vec![12]);
    app.select_run(None);
    assert_eq!(
        app.comparison_filter,
        crate::visibility::RepositoryFilter::All
    );
}
#[test]
fn visibility_controls_fit_supported_sizes_and_themes() {
    let (_temp, mut app) = app();
    app.state.runs = vec![run(
        1,
        &[
            JobStatus::Succeeded,
            JobStatus::Failed,
            JobStatus::Cancelled,
        ],
    )];
    app.select_run(Some(1));
    app.state.runs[0].jobs[0].repository.name =
        "Very long repository name that should truncate at the pane boundary".into();
    app.state.runs[0].jobs[0].completion_changes = Some(crate::visibility::Changes {
        changed: Some(true),
        files: Some(123),
    });
    let ctx = egui::Context::default();
    theme::install(&ctx);
    for appearance in [
        domain::Appearance::System,
        domain::Appearance::Dark,
        domain::Appearance::Light,
    ] {
        ctx.set_theme(theme::preference(appearance));
        for (width, height) in [(780.0, 560.0), (1180.0, 820.0), (1600.0, 1000.0)] {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(width, height),
                    )),
                    ..Default::default()
                },
                |ui| {
                    egui::Panel::left("test_editor")
                        .exact_size(width * 0.43)
                        .show(ui, |ui| {
                            let right = ui.available_rect_before_wrap().right();
                            let response = ui.scope(|ui| app.repositories_section(ui, &ctx));
                            assert!(response.response.rect.right() <= right + 1.0);
                        });
                    egui::CentralPanel::default().show(ui, |ui| {
                        let right = ui.available_rect_before_wrap().right();
                        let response = ui.scope(|ui| app.results(ui, &ctx));
                        assert!(
                            response.response.rect.right() <= right + 1.0,
                            "{appearance:?}, {width}x{height}"
                        );
                    });
                },
            );
            output.textures_delta.clear();
        }
    }
}

#[test]
fn isolated_completion_observation_is_saved_and_nested_health_replies_are_invalidated() {
    let (_temp, mut app) = app();
    app.state.runs = vec![run(7, &[JobStatus::Running])];
    let path = app.state.runs[0].jobs[0].repository.path.clone();
    let nested = path.join("nested");
    app.repository_pending.insert(nested.clone(), 1);
    app.apply_event(Event::Result {
        run: 7,
        job: 0,
        result: Some(domain::WorktreeResult {
            exists: true,
            changed: Some(true),
            observed_this_session: true,
        }),
        detail: String::new(),
    });
    app.apply_event(Event::Finished {
        run: 7,
        job: 0,
        status: JobStatus::Cancelled,
        exit_code: None,
        detail: String::new(),
    });
    assert!(!app.repository_pending.contains_key(&nested));
    let saved: domain::Run =
        serde_json::from_value(serde_json::to_value(&app.state.runs[0]).unwrap()).unwrap();
    assert_eq!(
        saved.jobs[0].completion_changes.unwrap().changed,
        Some(true)
    );
    assert_eq!(saved.jobs[0].status, JobStatus::Cancelled);
}
