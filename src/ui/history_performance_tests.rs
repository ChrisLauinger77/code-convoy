use super::*;

#[test]
#[ignore = "repeatable performance measurement; no timing threshold"]
fn history_navigation_performance() {
    for count in [100, 1000, 5000] {
        let (_temp, mut app) = app();
        app.state.runs = (1..=count)
            .rev()
            .map(|id| {
                let mut r = run(
                    id,
                    &[
                        JobStatus::Succeeded,
                        JobStatus::Failed,
                        JobStatus::Cancelled,
                        JobStatus::Succeeded,
                    ],
                );
                r.task.prompt = format!(
                    "History task {id}: review 日本語\n{}",
                    "Detailed context. ".repeat(512)
                );
                r
            })
            .collect();
        app.select_run(Some(count));
        let ctx = egui::Context::default();
        theme::install(&ctx);
        let mut times = Vec::new();
        for frame in 0..12 {
            app.show_active_navigation = true;
            let start = Instant::now();
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(780.0, 560.0),
                    )),
                    ..Default::default()
                },
                |ui| {
                    egui::CentralPanel::default().show(ui, |ui| app.run_navigation(ui));
                },
            );
            output.textures_delta.clear();
            if frame >= 2 {
                times.push(start.elapsed().as_secs_f64() * 1000.0);
            }
        }
        times.sort_by(f64::total_cmp);
        println!(
            "history navigation {count}: median {:.3} ms",
            times[times.len() / 2]
        );
    }
}
