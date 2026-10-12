#![allow(clippy::unwrap_used)]
//! Synthetic legacy saves, with field names checked against the release tags.
//! No registered path, attachment or retained checkout in these files is accessed.
use codeconvoy::{domain::AppState, persistence::Store};
use serde_json::Value;
use std::fs;

fn assert_preserved(expected: &Value, actual: &Value, path: &str) {
    match expected {
        Value::Object(fields) => {
            for (key, value) in fields {
                assert_preserved(value, &actual[key], &format!("{path}.{key}"));
            }
        }
        Value::Array(entries) => {
            assert_eq!(actual.as_array().unwrap().len(), entries.len(), "{path}");
            for (index, entry) in entries.iter().enumerate() {
                assert_preserved(entry, &actual[index], &format!("{path}[{index}]"));
            }
        }
        _ => assert_eq!(expected, actual, "legacy field changed: {path}"),
    }
}

#[test]
fn legacy_settings_history_and_retained_ownership_survive_load_save_reload() {
    for fixture in [
        include_bytes!("fixtures/state-v0.1.json").as_slice(),
        include_bytes!("fixtures/state-v0.9.json").as_slice(),
    ] {
        let temporary = tempfile::tempdir().unwrap();
        let store = Store::open(temporary.path()).unwrap();
        fs::write(store.history_path(), fixture).unwrap();
        let loaded = store.load().unwrap();
        assert_eq!(fs::read(store.history_path()).unwrap(), fixture);

        // Only recovery of unfinished work may change saved execution fields.
        // Defaults added by later releases are allowed; every supplied legacy
        // setting and snapshot must still be present and equal.
        let mut expected: Value = serde_json::from_slice(fixture).unwrap();
        if let Some(run) = expected["runs"].as_array_mut().unwrap().get_mut(1) {
            let job = &mut run["jobs"][0];
            job["status"] = "cancelled".into();
            job["interrupted"] = true.into();
            job["validation"]["status"] = "Cancelled".into();
        }
        assert_preserved(&expected, &serde_json::to_value(&loaded).unwrap(), "state");
        for job in loaded.runs.iter().flat_map(|run| &run.jobs) {
            assert!(job.status.is_terminal());
            assert!(
                !job.result_checked,
                "saved ownership is not a fresh verification"
            );
            assert!(job.log.text.is_empty() && job.raw_log.text.is_empty());
        }
        store.save(&loaded.persistence_snapshot()).unwrap();
        drop(store);
        let reopened = Store::open(temporary.path()).unwrap();
        let reloaded: AppState = reopened.load().unwrap();
        assert_preserved(
            &expected,
            &serde_json::to_value(&reloaded).unwrap(),
            "state",
        );
        assert_eq!(
            serde_json::to_value(&loaded).unwrap(),
            serde_json::to_value(&reloaded).unwrap()
        );
    }
}
