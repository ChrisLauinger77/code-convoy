//! Disposable native startup/idle probe. No installed agents or real repositories.
//! Usage: desktop_performance [history count] [idle seconds]
use anyhow::Result;
use codeconvoy::{domain::*, persistence::Store};
use eframe::egui;
use std::time::{Duration, Instant};
#[path = "../tests/support/history_scale.rs"]
mod fixture;
#[path = "../src/graphics.rs"]
mod graphics;

struct Probe {
    app: codeconvoy::ui::App,
    start: Instant,
    idle_at: Instant,
    close_at: Instant,
    first: bool,
    idle_frames: usize,
}
impl eframe::App for Probe {
    fn logic(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        self.app.logic(ctx, frame);
        if Instant::now() >= self.close_at {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        } else {
            // One deadline, not a periodic timer that would hide idle repaint behavior.
            ctx.request_repaint_after(self.close_at.saturating_duration_since(Instant::now()));
        }
    }
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        self.app.ui(ui, frame);
        if self.first {
            println!(
                "first_ui_ms={:.3}",
                self.start.elapsed().as_secs_f64() * 1000.0
            );
            self.first = false;
        }
        let now = Instant::now();
        if now >= self.idle_at && now < self.close_at {
            self.idle_frames += 1;
        }
    }
    fn on_exit(&mut self, gl: Option<&eframe::glow::Context>) {
        println!("idle_frames={}", self.idle_frames);
        self.app.on_exit(gl);
    }
}
fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().collect();
    let count = args.get(1).map(|s| s.parse()).transpose()?.unwrap_or(0);
    let seconds = args.get(2).map(|s| s.parse()).transpose()?.unwrap_or(20);
    let temp = tempfile::tempdir()?;
    let store = Store::open(temp.path())?;
    let mut state = fixture::state(count);
    state.repositories.clear();
    for agent in AgentId::ALL {
        state.agent_options.insert(
            agent,
            [(
                "executable".into(),
                temp.path().join("missing-cli").display().to_string(),
            )]
            .into(),
        );
    }
    state.draft.options = state.agent_options[&state.draft.agent].clone();
    state.notifications = Default::default();
    store.save(&state)?;
    // Exclude fixture generation, include loading saved state and native initialization.
    let start = Instant::now();
    let mut state = store.load()?;
    // No Git inspection in this rendering/idle fixture. Real scanning has its own probe.
    for job in state.runs.iter_mut().flat_map(|r| &mut r.jobs) {
        job.review = Some(Err("Synthetic performance fixture".into()));
    }
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()?;
    let options = graphics::configure(eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("CodeConvoy performance fixture")
            .with_inner_size([1180.0, 820.0]),
        ..Default::default()
    })?;
    eframe::run_native(
        "CodeConvoy performance fixture",
        options,
        Box::new(move |cc| {
            let app = codeconvoy::ui::App::new(cc, store, state, runtime);
            let idle_at = Instant::now() + Duration::from_secs(5);
            println!(
                "pid={} idle_starts_after_seconds=5 idle_seconds={seconds}",
                std::process::id()
            );
            Ok(Box::new(Probe {
                app,
                start,
                idle_at,
                close_at: idle_at + Duration::from_secs(seconds),
                first: true,
                idle_frames: 0,
            }))
        }),
    )
    .map_err(graphics::startup_error)
}
