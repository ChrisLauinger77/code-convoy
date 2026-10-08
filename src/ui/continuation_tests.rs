use super::*;
use crate::continuation::{FollowUp, Provenance};

#[test]
fn retry_launch_keeps_draft_selection_and_original_logs_and_saves_origin() {
    let (_temp, mut app) = app();
    prepare(&mut app, AgentId::Codex);
    let mut original = app.prepared.as_ref().unwrap().snapshot(41);
    original.jobs[0].finish(JobStatus::Failed, Some(7), "historical failure".into());
    original.jobs[0].log.append("original activity");
    original.jobs[0].raw_log.append("original raw");
    app.state.runs.push(original);
    app.state.next_run = 42;
    app.prepared_provenance = Some(Provenance::Retry {
        run: 41,
        job: 0,
        attempt: 2,
    });
    app.state.draft.prompt = "do not replace my draft".into();
    app.state.draft_provenance = Some(Provenance::FollowUp {
        run: 7,
        jobs: vec![1],
    });
    let draft = serde_json::to_value(&app.state.draft).unwrap();
    let origin = app.state.draft_provenance.clone();
    let selection = app.selected.clone();
    let old = serde_json::to_value(&app.state.runs[0]).unwrap();
    app.start();
    assert_eq!(app.selected, selection);
    assert_eq!(serde_json::to_value(&app.state.draft).unwrap(), draft);
    assert_eq!(app.state.draft_provenance, origin);
    assert_eq!(app.selected_run, Some(42));
    let original = app.state.runs.iter().find(|r| r.id == 41).unwrap();
    assert_eq!(serde_json::to_value(original).unwrap(), old);
    assert_eq!(original.jobs[0].log.text, "original activity");
    assert_eq!(original.jobs[0].raw_log.text, "original raw");
    let attempt = app.state.runs.iter().find(|r| r.id == 42).unwrap();
    assert!(
        attempt.jobs[0]
            .log
            .text
            .starts_with("[CodeConvoy] Retry · attempt 2")
    );
    assert!(attempt.jobs[0].raw_log.text.is_empty());
    let saved = app.store.load().unwrap();
    assert_eq!(saved.runs[0].provenance, attempt.provenance);
    assert!(saved.runs[0].jobs[0].interrupted);
}

#[test]
fn followup_draft_is_editable_independent_and_never_starts_work() {
    let (_temp, mut app) = app();
    prepare(&mut app, AgentId::Claude);
    let original = app.prepared.take().unwrap().snapshot(41);
    app.state.runs.push(original);
    let origin = serde_json::to_value(&app.state.runs).unwrap();
    let request = FollowUp::from_state(&app.state, 41, &[0].into()).unwrap();
    app.apply_followup(request);
    assert!(app.prepared.is_none() && app.manager.is_idle());
    assert!(app.state.draft.prompt.is_empty() && app.state.draft.attachments.is_empty());
    assert_eq!(app.selected.len(), 1);
    assert!(app.focus_draft && app.dirty);
    assert!(app.draft_message.contains("nothing has started"));
    assert_eq!(app.state.draft.agent, AgentId::Claude);
    assert_eq!(app.state.draft.concurrency, 3);
    assert_eq!(app.state.global_concurrency, 6);
    app.state.draft.prompt = "explicit new task".into();
    app.state.draft.execution_mode = domain::ExecutionMode::IsolatedWorktree;
    app.state.runs[0].jobs[0].status = JobStatus::Succeeded;
    assert!(app.state.remove_from_history(41));
    assert_eq!(
        app.state.draft_provenance,
        Some(Provenance::FollowUp {
            run: 41,
            jobs: vec![0]
        })
    );
    assert_eq!(app.state.draft.prompt, "explicit new task");
    assert_ne!(origin, serde_json::to_value(&app.state.runs).unwrap());
    assert!(app.manager.is_idle());
}

#[test]
fn retry_start_rechecks_registration_and_quit_guards_continuation() {
    let (_temp, mut app) = app();
    prepare(&mut app, AgentId::Codex);
    app.prepared_provenance = Some(Provenance::Retry {
        run: 41,
        job: 0,
        attempt: 2,
    });
    app.state.repositories.clear();
    app.start();
    assert!(app.manager.is_idle() && app.state.runs.is_empty());
    assert!(app.notice.contains("no longer registered"));
    app.closing = true;
    let ctx = egui::Context::default();
    app.retry(&ctx, 41, 0);
    app.followup(&ctx, 41);
    assert!(!app.busy && !app.followup_pending);
}

#[test]
fn review_selection_and_retry_have_accessible_controls_and_followup_remains_separate() {
    let (_temp, mut app) = app();
    app.state.runs = vec![run(
        1,
        &[
            JobStatus::Succeeded,
            JobStatus::Failed,
            JobStatus::Cancelled,
        ],
    )];
    app.selected_run = Some(1);
    for job in 0..3 {
        app.selected_job = job;
        for selected in [false, true] {
            app.review_selected = if selected {
                [0, 2].into()
            } else {
                HashSet::new()
            };
            let ctx = egui::Context::default();
            ctx.enable_accesskit();
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1000.0, 900.0),
                    )),
                    ..Default::default()
                },
                |ui| {
                    app.continuation_controls(ui, &ctx);
                    app.review_view(ui, &ctx, 900.0);
                },
            );
            output.textures_delta.clear();
            let nodes = output.platform_output.accesskit_update.unwrap().nodes;
            assert_eq!(
                nodes.iter().any(|(_, n)| n.label() == Some("Retry")),
                job != 0
            );
            let followup = nodes
                .iter()
                .find(|(_, n)| n.label() == Some("New convoy from selected"))
                .unwrap();
            assert_eq!(followup.1.is_disabled(), !selected);
            assert_eq!(app.selected_job, job);
        }
    }
    app.select_run(None);
    assert!(app.review_selected.is_empty());
}

#[test]
fn retry_diagnostics_keep_context_errors_actionable_without_allowing_context_loss() {
    for (detail, expected) in [
        (
            "Attachment is missing or unavailable: /file: os error 2",
            "missing",
        ),
        ("Attachment changed since it was added: /file", "changed"),
        (
            "Cannot read attachment /file: os error 13",
            "cannot be read",
        ),
        (
            "repository is unavailable: Git error",
            "repository is unavailable",
        ),
    ] {
        let full = format!("Retry blocked: {detail}");
        let summary = diagnostics::summary(&full);
        assert!(summary.starts_with("Retry blocked") && summary.contains(expected));
        assert!(!summary.contains("os error") && !summary.contains("remove"));
    }
}

#[test]
fn saved_appearance_restores_each_theme_without_affecting_followup() {
    for appearance in [
        domain::Appearance::System,
        domain::Appearance::Dark,
        domain::Appearance::Light,
    ] {
        let state = AppState {
            appearance,
            ..Default::default()
        };
        let (temp, app) = app_with_state(state);
        app.store.save(&app.state).unwrap();
        let loaded = app.store.load().unwrap();
        assert_eq!(loaded.appearance, appearance);
        let ctx = egui::Context::default();
        ctx.set_theme(theme::preference(loaded.appearance));
        assert_eq!(
            theme::appearance(ctx.options(|o| o.theme_preference)),
            appearance
        );
        drop(app);
        drop(temp);
    }
}
