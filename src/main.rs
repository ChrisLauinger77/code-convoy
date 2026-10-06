#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

fn main() -> anyhow::Result<()> {
    let result = run();
    // Release GUI applications have no console; startup failures must stay visible.
    // Debug builds retain the ordinary console and its complete diagnostics.
    #[cfg(all(windows, not(debug_assertions)))]
    if let Err(error) = &result {
        rfd::MessageDialog::new()
            .set_title("CodeConvoy could not start")
            .set_description(format!("{error:#}"))
            .set_level(rfd::MessageLevel::Error)
            .show();
    }
    result
}

fn run() -> anyhow::Result<()> {
    #[cfg(target_os = "macos")]
    codeconvoy::ui::init_native_application()?;
    let store = codeconvoy::persistence::Store::open_default()?;
    let state = store.load()?;
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()?;
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("CodeConvoy")
            .with_app_id("codeconvoy")
            .with_icon(eframe::icon_data::from_png_bytes(include_bytes!(
                "../assets/codeconvoy-256.png"
            ))?)
            .with_inner_size([1180.0, 820.0])
            .with_min_inner_size([780.0, 560.0]),
        ..Default::default()
    };
    eframe::run_native(
        "CodeConvoy",
        options,
        Box::new(move |cc| {
            Ok(Box::new(codeconvoy::ui::App::new(
                cc, store, state, runtime,
            )))
        }),
    )
    .map_err(|error| anyhow::anyhow!("Unable to open CodeConvoy: {error}"))
}
