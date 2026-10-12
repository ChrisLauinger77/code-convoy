//! Opt-in layout measurements, not wall-clock assertions in CI.
use super::*;

fn measure(name: &str, app: &mut App, mut draw: impl FnMut(&mut App, &mut egui::Ui)) {
    let ctx = egui::Context::default();
    theme::install(&ctx);
    dialog::install(&ctx);
    let mut samples = Vec::new();
    for frame in 0..40 {
        let start = Instant::now();
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1180.0, 820.0),
                )),
                time: Some(frame as f64 / 60.0),
                ..Default::default()
            },
            |ui| draw(app, ui),
        );
        output.textures_delta.clear();
        if frame >= 10 {
            samples.push(start.elapsed().as_secs_f64() * 1000.0);
        }
    }
    samples.sort_by(f64::total_cmp);
    println!(
        "{name}: median {:.3} ms, p95 {:.3} ms",
        samples[15], samples[28]
    );
}

#[test]
#[ignore = "repeatable performance measurement; no timing threshold"]
fn desktop_layout_performance() {
    let (_temp, mut app) = app();
    for count in [128, 512] {
        // 128 is the registration limit; 512 is an intentionally oversized stress case.
        app.state.repositories = (0..count)
            .map(|i| repository(&format!("service-{i}")))
            .collect();
        measure(&format!("repositories {count}"), &mut app, |app, ui| {
            let ctx = ui.ctx().clone();
            egui::CentralPanel::default().show(ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| app.repositories_section(ui, &ctx));
            });
        });
    }
    app.state.repositories.truncate(MAX_REPOSITORIES);
    app.state.runs = (1..=5000)
        .rev()
        .map(|id| run(id, &[JobStatus::Succeeded; 4]))
        .collect();
    app.select_run(Some(5000));
    measure("history 5000 open", &mut app, |app, ui| {
        app.show_active_navigation = true;
        egui::CentralPanel::default().show(ui, |ui| app.run_navigation(ui));
    });
    app.state.runs[0] = run(5000, &[JobStatus::Succeeded; MAX_REPOSITORIES]);
    measure("results 128 review", &mut app, |app, ui| {
        let ctx = ui.ctx().clone();
        egui::CentralPanel::default().show(ui, |ui| app.results(ui, &ctx));
    });
    let text = "Large output 日本語: completed a unit of work.\n".repeat(12_000);
    app.state.runs[0].jobs[0].raw_log.append(&text);
    app.tab = Tab::Raw;
    measure("results 128 raw 512KiB", &mut app, |app, ui| {
        let ctx = ui.ctx().clone();
        egui::CentralPanel::default().show(ui, |ui| app.results(ui, &ctx));
    });
    app.state.runs[0].jobs[0].validation = Some(crate::validation::Record {
        command: crate::validation::Command {
            executable: "fixture".into(),
            arguments: vec![],
        },
        directory: repository("repo0").path,
        status: crate::validation::Status::Passed,
        started_at: 1,
        finished_at: Some(2),
        duration_ms: 1000,
        exit_code: Some(0),
        output: "Validation output: completed a unit of work.\n".repeat(1500),
        truncated: true,
        detail: String::new(),
    });
    app.tab = Tab::Validation;
    measure("results 128 validation 64KiB", &mut app, |app, ui| {
        let ctx = ui.ctx().clone();
        egui::CentralPanel::default().show(ui, |ui| app.results(ui, &ctx));
    });
    app.tab = Tab::Task;
    measure("results 128 task settings", &mut app, |app, ui| {
        let ctx = ui.ctx().clone();
        egui::CentralPanel::default().show(ui, |ui| app.results(ui, &ctx));
    });
    app.state.runs[0].jobs.truncate(4);
    for run in app.state.runs.iter_mut().take(4) {
        for job in &mut run.jobs {
            job.status = JobStatus::Running;
        }
    }
    app.tab = Tab::Raw;
    measure("full UI 5000 history 4 active", &mut app, |app, ui| {
        app.main_ui(ui)
    });
    app.history_settings = Some(history::Settings::new(&app.state.history_retention));
    measure("full UI history settings", &mut app, |app, ui| {
        app.main_ui(ui)
    });
}

#[test]
fn repository_usage_tracks_nested_jobs_validation_and_result_operations() {
    let (_temp, mut app) = app();
    app.state.repositories = vec![
        repository("parent"),
        repository("parent/child"),
        repository("parent-other"),
        repository("unrelated"),
    ];
    let mut history = run(1, &[JobStatus::Succeeded; 3]);
    history.jobs[0].repository = repository("parent/child");
    history.jobs[1].repository = repository("parent-other");
    history.jobs[2].repository = repository("unrelated");
    app.state.runs.push(history);
    assert_eq!(app.repository_usage(), [false; 4]);
    app.state.runs[0].jobs[0].status = JobStatus::Queued;
    assert_eq!(app.repository_usage(), [true, true, false, false]);
    app.state.runs[0].jobs[0].status = JobStatus::Preparing;
    assert_eq!(app.repository_usage(), [true, true, false, false]);
    app.state.runs[0].jobs[0].status = JobStatus::Succeeded;
    let job = &mut app.state.runs[0].jobs[1];
    job.validation = Some(crate::validation::Record::pending(
        crate::validation::Command {
            executable: "fixture".into(),
            arguments: vec![],
        },
        job,
    ));
    app.result_operation = Some((1, 2));
    assert_eq!(app.repository_usage(), [false, false, true, true]);
    for (repo, in_use) in app.state.repositories.iter().zip(app.repository_usage()) {
        assert_eq!(in_use, app.repository_in_use(&repo.path));
    }
    app.state.runs[0].jobs[1]
        .validation
        .as_mut()
        .unwrap()
        .status = crate::validation::Status::Passed;
    app.result_operation = None;
    assert_eq!(app.repository_usage(), [false; 4]);
}

#[test]
#[ignore = "repeatable buffer retention measurement; no timing threshold"]
fn output_retention_performance() {
    let mut state = AppState::default();
    state
        .runs
        .push(run(1, &[JobStatus::Succeeded; MAX_REPOSITORIES]));
    let chunk = "x".repeat(128 * 1024);
    for index in 0..MAX_REPOSITORIES {
        for _ in 0..4 {
            state.runs[0].jobs[index].log.append(&chunk);
            state.runs[0].jobs[index].raw_log.append(&chunk);
        }
        state.trim_logs();
    }
    let (retained, allocated) = state.runs[0].jobs.iter().fold((0, 0), |(len, cap), job| {
        (
            len + job.log.text.len() + job.raw_log.text.len(),
            cap + job.log.text.capacity() + job.raw_log.text.capacity(),
        )
    });
    println!("output buffers: retained={retained} allocated={allocated} bytes");
}
