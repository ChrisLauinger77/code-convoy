use super::*;
use crate::persistence::bulk_discard::{Batch, Plan, Scope};

fn modal_frame(app: &mut App, ctx: &egui::Context, key: Option<egui::Key>) {
    ctx.run_ui(
        egui::RawInput {
            events: key
                .into_iter()
                .flat_map(|key| {
                    [true, false].map(|pressed| egui::Event::Key {
                        key,
                        physical_key: None,
                        pressed,
                        repeat: false,
                        modifiers: egui::Modifiers::NONE,
                    })
                })
                .collect(),
            ..Default::default()
        },
        |ui| app.bulk_discard_window(ui.ctx()),
    )
    .textures_delta
    .clear();
}

#[test]
fn bulk_discard_confirmation_counts_terminal_results_and_defaults_to_cancel() {
    for scope in [Scope::Convoy(1), Scope::All] {
        for key in [egui::Key::Enter, egui::Key::Escape] {
            let (_temp, mut app) = app();
            app.state.runs = vec![
                isolated_history(1, JobStatus::Succeeded, Some(true)),
                isolated_history(2, JobStatus::Failed, Some(false)),
                isolated_history(3, JobStatus::Running, Some(true)),
            ];
            app.offer_bulk_discard(scope);
            let plan = app.bulk_confirmation.as_ref().unwrap();
            assert_eq!(plan.len(), if scope == Scope::All { 2 } else { 1 });
            assert_eq!(plan.convoys(), plan.len());
            let before = serde_json::to_value(&app.state).unwrap();
            let ctx = egui::Context::default();
            modal_frame(&mut app, &ctx, None);
            assert!(ctx.memory(|m| m.focused().is_some()));
            modal_frame(&mut app, &ctx, Some(key));
            assert!(app.bulk_confirmation.is_none());
            assert!(app.bulk_discard.is_none() && app.result_operation.is_none());
            assert_eq!(serde_json::to_value(&app.state).unwrap(), before);
        }
    }
}

#[test]
fn bulk_discard_keyboard_confirmation_freezes_targets_and_waits_for_review() {
    let (_temp, mut app) = app();
    app.state.runs = vec![isolated_history(1, JobStatus::Succeeded, Some(true))];
    app.offer_bulk_discard(Scope::All);
    app.state
        .runs
        .push(isolated_history(2, JobStatus::Succeeded, Some(true)));
    let ctx = egui::Context::default();
    modal_frame(&mut app, &ctx, None);
    modal_frame(&mut app, &ctx, Some(egui::Key::Tab));
    modal_frame(&mut app, &ctx, None);
    modal_frame(&mut app, &ctx, Some(egui::Key::Enter));
    assert!(app.bulk_confirmation.is_none());
    assert_eq!(app.bulk_discard.as_ref().unwrap().plan.len(), 1);
    app.review_pending = Some((1, 0));
    app.pump_bulk_discard(&ctx);
    assert!(app.result_operation.is_none());
    app.pump_review(&ctx);
    assert_eq!(app.review_pending, Some((1, 0)));
    assert!(
        app.state.runs[0].jobs[0]
            .log
            .text
            .contains("[CodeConvoy] Discarding all unresolved results")
    );
}

#[test]
fn bulk_discard_never_offers_active_convoy_or_starts_during_another_operation() {
    let (_temp, mut app) = app();
    for status in [JobStatus::Queued, JobStatus::Preparing, JobStatus::Running] {
        app.state.runs = vec![isolated_history(1, JobStatus::Succeeded, Some(true))];
        app.state.runs[0]
            .jobs
            .push(Job::queued(repository("active")));
        app.state.runs[0].jobs[1].status = status;
        app.offer_bulk_discard(Scope::Convoy(1));
        assert!(app.bulk_confirmation.is_none());
        app.offer_bulk_discard(Scope::All);
        assert!(app.bulk_confirmation.is_none());
    }
    app.state.runs[0].jobs.pop();
    app.result_operation = Some((1, 0));
    app.offer_bulk_discard(Scope::All);
    assert!(app.bulk_confirmation.is_none());
}

#[test]
fn bulk_discard_quit_waits_for_current_result_and_leaves_remaining_unattempted() {
    let (_temp, mut app) = app();
    app.state.runs = vec![
        isolated_history(1, JobStatus::Succeeded, Some(true)),
        isolated_history(2, JobStatus::Succeeded, Some(true)),
    ];
    let plan = Plan::new(&app.state, Scope::All);
    app.bulk_discard = Some(Batch::new(plan, &mut app.state));
    assert!(!app.request_quit());
    app.cancel_quit();
    assert!(app.bulk_discard.is_some());
    app.confirm_quit();
    app.result_operation = Some((1, 0));
    app.pump_bulk_discard(&egui::Context::default());
    assert!(app.bulk_discard.is_some());
    app.result_operation = None;
    app.pump_bulk_discard(&egui::Context::default());
    assert!(app.bulk_discard.is_none());
    let (_, summary) = app.bulk_report.as_ref().unwrap();
    assert_eq!(summary.not_attempted, 2);
    assert_eq!(summary.discarded, 0);
    assert!(app.state.runs.iter().all(Run::history_protected));
}

#[test]
fn bulk_discard_ui_pump_merges_real_cleanup_and_enables_history_removal() {
    let (temp, mut app) = app();
    let source = temp.path().join("source");
    std::fs::create_dir(&source).unwrap();
    // Exercise an aliased selection on every platform, not only macOS's /var
    // alias or Windows paths whose canonical spelling has a verbatim prefix.
    let source = source.join("..").join("source");
    let git = |args: &[&str]| {
        let output = std::process::Command::new("git")
            .arg("-C")
            .arg(&source)
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    };
    git(&["init", "--quiet"]);
    std::fs::write(source.join("tracked"), "base\n").unwrap();
    git(&["add", "tracked"]);
    git(&[
        "-c",
        "commit.gpgsign=false",
        "-c",
        "user.name=Fixture",
        "-c",
        "user.email=fixture@example.invalid",
        "commit",
        "-qm",
        "base",
    ]);
    let repository = app.runtime().block_on(git::register(&source)).unwrap();
    let summary = app
        .runtime()
        .block_on(git::status(&repository.path))
        .unwrap()
        .summary;
    app.state.repositories = vec![repository.clone()];
    let mut paths = Vec::new();
    for id in [1, 2] {
        let metadata = crate::worktrees::reserve(
            &app.store.directory().join("worktrees"),
            id,
            0,
            repository.clone(),
            summary.common_dir.clone().unwrap(),
            summary.head.clone().unwrap(),
        )
        .unwrap();
        app.runtime()
            .block_on(crate::worktrees::create(
                &metadata,
                &crate::process::Cancellation::default(),
                &std::sync::atomic::AtomicBool::new(true),
            ))
            .unwrap();
        std::fs::write(metadata.path.join("tracked"), "retained changes\n").unwrap();
        paths.push(metadata.path.clone());
        let mut run = isolated_history(id, JobStatus::Succeeded, Some(true));
        run.jobs[0].repository = repository.clone();
        run.jobs[0].worktree = Some(metadata);
        app.state.runs.push(run);
    }
    app.state.next_run = 3;
    app.store.save(&app.state).unwrap();
    let ctx = egui::Context::default();
    app.offer_bulk_discard(Scope::All);
    modal_frame(&mut app, &ctx, None);
    modal_frame(&mut app, &ctx, Some(egui::Key::Tab));
    modal_frame(&mut app, &ctx, None);
    modal_frame(&mut app, &ctx, Some(egui::Key::Enter));
    let deadline = Instant::now() + Duration::from_secs(10);
    while app.bulk_discard.is_some() {
        assert!(Instant::now() < deadline, "bulk operation did not complete");
        app.pump_bulk_discard(&ctx);
        app.runtime().block_on(async {
            tokio::time::sleep(Duration::from_millis(1)).await;
        });
        app.poll();
    }
    let (_, summary) = app.bulk_report.as_ref().unwrap();
    assert_eq!(summary.discarded, 2);
    assert!(summary.failures.is_empty(), "{:?}", summary.failures);
    assert!(app.result_operation.is_none());
    assert!(paths.iter().all(|p| !p.exists()));
    assert_eq!(
        std::fs::read_to_string(source.join("tracked")).unwrap(),
        "base\n"
    );
    assert!(app.state.runs.iter().all(|r| !r.history_protected()));
    assert!(
        app.store
            .load()
            .unwrap()
            .runs
            .iter()
            .all(|r| r.jobs[0].resolution == domain::ResultResolution::Discarded)
    );
    app.remove_history(None);
    assert!(app.state.runs.is_empty());
    assert_eq!(app.state.next_run, 3);
    assert!(app.store.load().unwrap().runs.is_empty());
}
