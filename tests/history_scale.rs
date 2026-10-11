#![allow(clippy::unwrap_used)]
#[path = "support/history_scale.rs"]
mod fixture;
use codeconvoy::{domain::*, export, history, persistence::Store, visibility::*};

#[test]
fn scale_load_search_order_retention_and_exports_preserve_metadata() {
    for count in [100, 1000, 5000] {
        let temp = tempfile::tempdir().unwrap();
        let store = Store::open(temp.path()).unwrap();
        let state = fixture::state(count);
        store.save(&state).unwrap();
        let original = std::fs::read(store.history_path()).unwrap();
        let mut loaded = store.load().unwrap();
        assert_eq!(loaded.runs.len(), count);
        assert_eq!(
            serde_json::to_vec(&loaded).unwrap(),
            serde_json::to_vec(&state).unwrap()
        );
        let mut search = HistorySearch::default();
        for query in ["", "日本語", " SERVICE-2 ", "#10", "gItHuB", "absent"] {
            search.query = query.into();
            for filter in HistoryFilter::ALL {
                search.filter = filter;
                let needle = query.trim().to_lowercase();
                let expected: Vec<_> = loaded
                    .runs
                    .iter()
                    .filter(|r| filter.matches(r))
                    .filter(|r| {
                        format!(
                            "#{}\n{}\n{}\n{}",
                            r.id,
                            r.task.prompt,
                            r.task.agent.label(),
                            r.jobs
                                .iter()
                                .map(|j| format!(
                                    "{}\n{}",
                                    j.repository.name,
                                    j.repository.path.display()
                                ))
                                .collect::<Vec<_>>()
                                .join("\n")
                        )
                        .to_lowercase()
                        .contains(&needle)
                    })
                    .map(|r| r.id)
                    .collect();
                assert_eq!(search.matching(&loaded.runs), expected);
                assert_eq!(search.matching(&loaded.runs), expected);
            }
        }
        let snapshot = export::Snapshot::capture(&loaded.runs, &export::Scope::All).unwrap();
        // Mutation after capture must not leak into either export format.
        loaded.runs[0].task.prompt = "AFTER_SNAPSHOT".into();
        loaded.runs[0].jobs[0].validation = None;
        for format in [export::Format::Json, export::Format::Markdown] {
            let mut bytes = Vec::new();
            snapshot.write(format, &mut bytes).unwrap();
            let text = String::from_utf8(bytes).unwrap();
            assert!(!text.contains("AFTER_SNAPSHOT"));
            assert!(!text.contains("/synthetic-codeconvoy/"));
            assert!(text.contains("日本語"));
            if format == export::Format::Json {
                let report: serde_json::Value = serde_json::from_str(&text).unwrap();
                assert_eq!(report["schema_version"], 1);
                assert_eq!(report["convoys"].as_array().unwrap().len(), count);
                assert_eq!(report["convoys"][0]["id"], count);
            }
        }
        let policy = history::RetentionPolicy {
            max_entries: std::num::NonZeroUsize::new(10),
            ..Default::default()
        };
        let protected = [1].into();
        let preview = history::evaluate(&state.runs, &policy, &protected, fixture::NOW);
        assert!(!preview.remove.contains(&1));
        assert_eq!(preview.retained(), 11);
        assert_eq!(std::fs::read(store.history_path()).unwrap(), original);
    }
}

#[test]
fn persistence_snapshot_keeps_all_saved_fields_without_copying_session_buffers() {
    let mut state = fixture::state(100);
    let job = &mut state.runs[1].jobs[0]; // id 99 includes validation
    job.log.append(&"activity".repeat(64 * 1024));
    job.raw_log.append(&"raw".repeat(128 * 1024));
    job.detail = "private diagnostics".repeat(4096);
    job.validation.as_mut().unwrap().output = "validation output".repeat(4096);
    job.result_checked = true;
    let snapshot = state.persistence_snapshot();
    assert_eq!(
        serde_json::to_vec(&snapshot).unwrap(),
        serde_json::to_vec(&state).unwrap()
    );
    let saved = &snapshot.runs[1].jobs[0];
    assert!(saved.result_checked);
    assert!(saved.log.text.is_empty() && saved.raw_log.text.is_empty() && saved.detail.is_empty());
    assert!(saved.validation.as_ref().unwrap().output.is_empty());
    assert!(!state.runs[1].jobs[0].log.text.is_empty());
}

#[test]
fn filters_recheck_outcomes_and_review_after_mutation_and_removal() {
    let mut state = fixture::state(100);
    let mut search = HistorySearch::default();
    search.filter = HistoryFilter::NeedsReview;
    assert!(search.matching(&state.runs).contains(&100));
    for job in &mut state.runs[0].jobs {
        job.resolution = ResultResolution::Applied;
    }
    assert!(!search.matching(&state.runs).contains(&100));
    search.clear();
    assert_eq!(search.matching(&state.runs).len(), 100);
    state.runs.retain(|r| r.id % 2 == 0);
    assert_eq!(
        search.matching(&state.runs),
        (1..=50).rev().map(|i| i * 2).collect::<Vec<_>>()
    );
    state.runs[0].jobs[0].status = JobStatus::Running;
    assert!(!search.matching(&state.runs).contains(&100));
    state.runs[0].jobs[0].status = JobStatus::Succeeded;
    assert!(search.matching(&state.runs).contains(&100));
}
