//! Presentation regressions: real widgets, with no agents or Git dispatched.
use super::*;

const SIZES: [(f32, f32); 3] = [(780.0, 560.0), (1180.0, 820.0), (1600.0, 1000.0)];
const THEMES: [domain::Appearance; 3] = [
    domain::Appearance::System,
    domain::Appearance::Dark,
    domain::Appearance::Light,
];

#[test]
fn empty_output_distinguishes_running_validation_and_fixed_base_results() {
    use crate::validation::Status;
    assert_eq!(
        format::validation_output_empty(Status::Running),
        "Validation running; no output received yet."
    );
    for status in [Status::Failed, Status::Unavailable] {
        assert!(format::validation_output_empty(status).contains("Diagnostics"));
    }
    for status in [Status::Passed, Status::Cancelled] {
        assert!(format::validation_output_empty(status).contains("not restored"));
    }
    assert!(format::diff_empty(domain::ExecutionMode::Direct).contains("current Git diff"));
    assert!(
        format::diff_empty(domain::ExecutionMode::IsolatedWorktree).contains("saved base commit")
    );
    let mut job = isolated_history(1, JobStatus::Succeeded, Some(false))
        .jobs
        .remove(0);
    job.result_checked = true;
    job.result_availability = domain::ResultAvailability::Available;
    assert!(format::output_empty(&job, true).contains("saved base"));
    job.status = JobStatus::Cancelled;
    job.started_at = Some(1);
    assert!(format::output_empty(&job, true).contains("retained isolated worktree"));
}

#[test]
fn isolated_output_hints_follow_verified_availability_and_resolution() {
    use domain::{ResultAvailability as A, ResultResolution as R};
    for status in [JobStatus::Succeeded, JobStatus::Cancelled] {
        let mut job = isolated_history(1, status, Some(true)).jobs.remove(0);
        job.started_at = Some(1);
        for (resolution, availability, checked, expected) in [
            (R::Unresolved, A::Available, true, "saved base"),
            (R::Applied, A::Available, true, "saved base"),
            (R::Discarded, A::Available, true, "Inspect Review"),
            (R::Discarded, A::Cleaned, true, "was removed"),
            (R::Applied, A::Cleaned, true, "was removed"),
            (R::Unresolved, A::Cleaned, true, "was removed"),
            (R::Discarded, A::Cleaned, false, "Inspect Review"),
            (R::Applied, A::Cleaned, false, "Inspect Review"),
            (R::Unresolved, A::Available, false, "Inspect Review"),
            (R::Discarded, A::CleanupFailed, true, "Cleanup failed"),
            (R::Applied, A::CleanupFailed, true, "Cleanup failed"),
            (R::DiscardPending, A::CleanupFailed, true, "Cleanup failed"),
            (R::Discarded, A::CleanupFailed, false, "Inspect Review"),
            (R::Discarded, A::Unchecked, true, "Inspect Review"),
            (R::Discarded, A::Missing, true, "Inspect Review"),
            (R::Discarded, A::Stale, true, "Inspect Review"),
            (R::Discarded, A::Invalid, true, "Inspect Review"),
            (R::DiscardPending, A::CleanupPending, true, "Inspect Review"),
            (R::ApplyPending, A::Available, true, "Inspect Review"),
        ] {
            job.resolution = resolution;
            job.result_availability = availability;
            job.result_checked = checked;
            let text = format::output_empty(&job, true);
            assert!(
                text.contains(expected),
                "{resolution:?} {availability:?} {checked}: {text}"
            );
            if expected != "was removed" {
                assert!(!text.contains("was removed"), "{text}");
            }
        }
        job.resolution = R::Unresolved;
        job.result_availability = A::Available;
        job.result_checked = true;
        job.worktree = None;
        assert!(format::output_empty(&job, true).contains("Inspect Review"));
        assert!(format::output_empty(&job, false).contains("Logs are session-only"));
    }
}

#[test]
fn discarded_empty_output_follows_reconciled_cleanup_failures() {
    use crate::worktrees::recovery::Report;
    use domain::{ResultAvailability as A, ResultResolution as R};
    for status in [JobStatus::Succeeded, JobStatus::Cancelled] {
        let mut job = isolated_history(1, status, Some(true)).jobs.remove(0);
        job.started_at = Some(1);
        job.resolution = R::DiscardPending;
        Report::unavailable(A::Cleaned, "Cleanup completed").apply(&mut job);
        assert_eq!(job.resolution, R::Discarded);
        assert!(format::output_empty(&job, true).contains("was removed"));

        Report::unavailable(A::CleanupFailed, "The resource reappeared").apply(&mut job);
        assert!(job.result_checked);
        assert_eq!(job.resolution, R::Discarded);
        assert_eq!(job.status, status);
        let text = format::output_empty(&job, true);
        assert!(text.contains("Cleanup failed"), "{text}");
        assert!(text.contains("Review"), "{text}");
        assert!(!text.contains("was removed"), "{text}");
        assert!(!text.contains("Diff is no longer available"), "{text}");
    }
}

#[test]
fn diff_empty_state_uses_the_loaded_observation_not_headers_or_job_status() {
    for mode in [
        domain::ExecutionMode::Direct,
        domain::ExecutionMode::IsolatedWorktree,
    ] {
        for changed in [Some(false), Some(true), None] {
            let (_temp, mut app) = app();
            app.state.runs = vec![if mode == domain::ExecutionMode::Direct {
                run(1, &[JobStatus::Failed])
            } else {
                // A stale completion observation must not override the loaded diff.
                isolated_history(1, JobStatus::Failed, Some(true))
            }];
            app.select_run(Some(1));
            app.tab = Tab::Diff;
            app.diff = Some(Ok(git::Diff {
                text: "Diff headers and diagnostic details".into(),
                changed,
            }));
            let ctx = egui::Context::default();
            ctx.enable_accesskit();
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1180.0, 820.0),
                    )),
                    ..Default::default()
                },
                |ui| {
                    egui::CentralPanel::default().show(ui, |ui| app.results(ui, &ctx));
                },
            );
            output.textures_delta.clear();
            let nodes = output.platform_output.accesskit_update.unwrap().nodes;
            assert_eq!(
                nodes
                    .iter()
                    .any(|(_, n)| n.value() == Some(format::diff_empty(mode))),
                changed == Some(false),
                "{mode:?} {changed:?}"
            );
            assert_eq!(
                nodes
                    .iter()
                    .any(|(_, n)| n.value() == Some("Diff headers and diagnostic details")),
                changed != Some(false)
            );
            assert!(nodes.iter().any(|(_, n)| n.label() == Some("Copy diff")));
            assert_eq!(app.state.runs[0].jobs[0].status, JobStatus::Failed);
        }
    }
}

#[test]
fn settings_menu_fits_the_window_with_diagnostics() {
    for appearance in THEMES {
        for (width, height) in SIZES {
            let (_temp, mut app) = app();
            app.notification_error =
                "Notification service unavailable: ".to_owned() + &"long diagnostic ".repeat(30);
            app.desktop.status =
                crate::desktop::Status::Unavailable("Tray host unavailable".into());
            let ctx = egui::Context::default();
            theme::install(&ctx);
            ctx.set_theme(theme::preference(appearance));
            ctx.enable_accesskit();
            let mut nodes = HashMap::new();
            let mut events = Vec::new();
            for frame in 0..5 {
                let mut output = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::vec2(width, height),
                        )),
                        events: std::mem::take(&mut events),
                        ..Default::default()
                    },
                    |ui| app.header(ui, &ctx),
                );
                output.textures_delta.clear();
                nodes.extend(output.platform_output.accesskit_update.unwrap().nodes);
                if frame == 1 {
                    let bounds = nodes
                        .values()
                        .find(|n| n.label() == Some("Settings (!)"))
                        .unwrap()
                        .bounds()
                        .unwrap();
                    let pos = egui::pos2(
                        ((bounds.x0 + bounds.x1) / 2.0) as f32,
                        ((bounds.y0 + bounds.y1) / 2.0) as f32,
                    );
                    events.push(egui::Event::PointerMoved(pos));
                    for pressed in [true, false] {
                        events.push(egui::Event::PointerButton {
                            pos,
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers: egui::Modifiers::NONE,
                        });
                    }
                }
            }
            assert!(egui::Popup::is_any_open(&ctx));
            ctx.memory(|m| {
                for layer in m
                    .areas()
                    .visible_layer_ids()
                    .into_iter()
                    .filter(|l| l.order == egui::Order::Foreground)
                {
                    let rect = m.area_rect(layer.id).unwrap();
                    assert!(
                        rect.left() >= 0.0
                            && rect.right() <= width + 1.0
                            && rect.top() >= 0.0
                            && rect.bottom() <= height + 1.0,
                        "{appearance:?} {width}x{height}: {rect:?}"
                    );
                }
            });
        }
    }
}

#[test]
fn result_job_controls_stay_inside_resized_panes_in_every_tab() {
    for appearance in THEMES {
        for (width, height) in SIZES {
            for tab in [
                Tab::Review,
                Tab::Activity,
                Tab::Diff,
                Tab::Raw,
                Tab::Validation,
                Tab::Task,
            ] {
                let (_temp, mut app) = app();
                app.state.runs = vec![run(
                    1,
                    &[JobStatus::Running, JobStatus::Queued, JobStatus::Succeeded],
                )];
                app.select_run(Some(1));
                app.tab = tab;
                for job in &mut app.state.runs[0].jobs {
                    job.repository.name =
                        "Very long repository name with Unicode – Änderungen ".repeat(3);
                }
                let ctx = egui::Context::default();
                theme::install(&ctx);
                ctx.set_theme(theme::preference(appearance));
                ctx.enable_accesskit();
                // Exercise the maximum supported editor width, leaving the
                // results pane at its narrowest, including panel margins.
                let left = width * 0.55;
                let mut nodes = HashMap::new();
                for _ in 0..3 {
                    let mut output = ctx.run_ui(
                        egui::RawInput {
                            screen_rect: Some(egui::Rect::from_min_size(
                                egui::Pos2::ZERO,
                                egui::vec2(width, height),
                            )),
                            ..Default::default()
                        },
                        |ui| {
                            egui::Panel::left("editor")
                                .exact_size(left)
                                .show(ui, |_| {});
                            egui::CentralPanel::default()
                                .frame(egui::Frame::NONE.inner_margin(theme::PANEL_MARGIN))
                                .show(ui, |ui| app.results(ui, &ctx));
                        },
                    );
                    output.textures_delta.clear();
                    nodes.extend(output.platform_output.accesskit_update.unwrap().nodes);
                }
                for node in nodes
                    .values()
                    .filter(|node| node.role() == egui::accesskit::Role::Button)
                {
                    let bounds = node.bounds().unwrap();
                    assert!(
                        bounds.x0 >= f64::from(left) && bounds.x1 <= f64::from(width - 16.0) + 1.0,
                        "{appearance:?} {width}x{height} {tab:?}: {:?} {bounds:?}",
                        node.label()
                    );
                }
            }
        }
    }
}

#[test]
fn group_dialog_with_long_names_keeps_actions_on_screen() {
    for appearance in THEMES {
        for (width, height) in SIZES {
            let (_temp, mut app) = app();
            let name = "Long repository group name – Änderungen ".repeat(12);
            app.state.groups.push(domain::RepositoryGroup {
                name: name.clone(),
                repositories: Vec::new(),
            });
            app.state.repositories = (0..24).map(|i| repository(&format!("{name}{i}"))).collect();
            app.library_editor = Some(library::LibraryEditor::Group {
                index: Some(0),
                value: app.state.groups[0].clone(),
                error: String::new(),
            });
            let ctx = egui::Context::default();
            theme::install(&ctx);
            ctx.set_theme(theme::preference(appearance));
            for _ in 0..3 {
                ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::vec2(width, height),
                        )),
                        ..Default::default()
                    },
                    |ui| app.library_window(ui.ctx()),
                )
                .textures_delta
                .clear();
            }
            let rect = ctx
                .memory(|m| m.area_rect(egui::Id::new("task_library")))
                .unwrap();
            assert!(
                rect.left() >= 0.0
                    && rect.right() <= width + 1.0
                    && rect.top() >= 0.0
                    && rect.bottom() <= height + 1.0,
                "{appearance:?} {width}x{height}: {rect:?}"
            );
        }
    }
}
