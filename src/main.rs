fn main() -> anyhow::Result<()> {
    let store = codeconvoy::persistence::Store::open_default()?;
    let state = store.load()?;
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()?;
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("CodeConvoy")
            .with_inner_size([1180.0, 820.0])
            .with_min_inner_size([860.0, 600.0]),
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
