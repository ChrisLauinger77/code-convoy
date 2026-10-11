//! Repeatable measurements, deliberately without timing assertions.
use anyhow::Result;
use codeconvoy::{
    export, history, persistence::Store, process::Cancellation, storage, visibility::HistorySearch,
};
use std::{hint::black_box, time::Instant};
#[path = "../tests/support/history_scale.rs"]
mod fixture;

fn measure(mut operation: impl FnMut() -> Result<()>, iterations: usize) -> Result<f64> {
    let mut samples = Vec::new();
    for _ in 0..iterations {
        let start = Instant::now();
        operation()?;
        samples.push(start.elapsed().as_secs_f64() * 1000.0);
    }
    samples.sort_by(f64::total_cmp);
    Ok(samples[samples.len() / 2])
}
fn main() -> Result<()> {
    println!(
        "convoys,bytes,save_ms,load_ms,cold_search_ms,warm_search_ms,retention_ms,capture_ms,json_ms,markdown_ms,storage_ms"
    );
    let sizes: Vec<usize> = std::env::args()
        .skip(1)
        .map(|s| s.parse())
        .collect::<Result<_, _>>()?;
    for count in if sizes.is_empty() {
        vec![100, 1000, 5000]
    } else {
        sizes
    } {
        let temp = tempfile::tempdir()?;
        let store = Store::open(temp.path())?;
        let state = fixture::state(count);
        let save = measure(|| store.save(black_box(&state)), 3)?;
        let bytes = std::fs::metadata(store.history_path())?.len();
        let load = measure(
            || {
                black_box(store.load()?);
                Ok(())
            },
            5,
        )?;
        let cold = measure(
            || {
                let mut search = HistorySearch::default();
                search.query = "日本語".into();
                black_box(search.matching(&state.runs));
                Ok(())
            },
            5,
        )?;
        let mut search = HistorySearch::default();
        search.query = "日本語".into();
        search.matching(&state.runs);
        let warm = measure(
            || {
                black_box(search.matching(&state.runs));
                Ok(())
            },
            25,
        )?;
        let policy = history::RetentionPolicy {
            max_entries: std::num::NonZeroUsize::new(250),
            max_age_days: std::num::NonZeroU32::new(90),
            automatic: true,
        };
        let retention = measure(
            || {
                black_box(history::evaluate(
                    &state.runs,
                    &policy,
                    &Default::default(),
                    fixture::NOW,
                ));
                Ok(())
            },
            5,
        )?;
        let scope = export::Scope::Filtered(state.runs.iter().map(|r| r.id).collect());
        let capture = measure(
            || {
                black_box(export::Snapshot::capture(&state.runs, &scope)?);
                Ok(())
            },
            5,
        )?;
        let snapshot = export::Snapshot::capture(&state.runs, &scope)?;
        let json = measure(|| snapshot.write(export::Format::Json, std::io::sink()), 5)?;
        let markdown = measure(
            || snapshot.write(export::Format::Markdown, std::io::sink()),
            5,
        )?;
        let scan = measure(
            || {
                black_box(storage::measure(
                    storage::Request {
                        history_path: store.history_path(),
                        worktree_root: temp.path().join("worktrees"),
                        references: vec![],
                    },
                    &Cancellation::default(),
                ));
                Ok(())
            },
            5,
        )?;
        println!(
            "{count},{bytes},{save:.3},{load:.3},{cold:.3},{warm:.3},{retention:.3},{capture:.3},{json:.3},{markdown:.3},{scan:.3}"
        );
    }
    Ok(())
}
