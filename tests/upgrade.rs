#![allow(clippy::unwrap_used)]
//! Synthetic legacy saves, with field names checked against the release tags.
//! No registered path, attachment or retained checkout in these files is accessed.
use codeconvoy::{domain::AppState, persistence::Store};
use serde_json::Value;
use std::{fs, sync::Arc};

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
        include_bytes!("fixtures/state-v0.2.json").as_slice(),
        include_bytes!("fixtures/state-v0.3.json").as_slice(),
        include_bytes!("fixtures/state-v0.6.json").as_slice(),
        include_bytes!("fixtures/state-v0.7.json").as_slice(),
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
            if let Some(validation) = job.get_mut("validation") {
                validation["status"] = "Cancelled".into();
            }
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

#[test]
fn unknown_additive_fields_do_not_reset_known_settings_or_history() {
    let fixture = include_bytes!("fixtures/state-v0.9.json");
    let known: Value = serde_json::from_slice(fixture).unwrap();
    let mut extended = known.clone();
    extended["additional_metadata"] = serde_json::json!({"opaque": [1, 2, 3]});
    extended["draft"]["additional_metadata"] = true.into();
    extended["desktop"]["additional_metadata"] = "ignored".into();
    extended["runs"][0]["additional_metadata"] = false.into();
    let temp = tempfile::tempdir().unwrap();
    let store = Store::open(temp.path()).unwrap();
    let bytes = serde_json::to_vec(&extended).unwrap();
    fs::write(store.history_path(), &bytes).unwrap();
    let loaded = store.load().unwrap();
    assert_eq!(fs::read(store.history_path()).unwrap(), bytes);
    // Current known fields match a load without extensions, including recovery.
    let mut expected: AppState = serde_json::from_value(known).unwrap();
    expected.recover_interrupted();
    assert_eq!(
        serde_json::to_value(&loaded).unwrap(),
        serde_json::to_value(&expected).unwrap()
    );
    // Unknown fields are accepted, not preserved as an extension API. This is
    // explicitly separate from future version numbers, which Store rejects.
    store.save(&loaded).unwrap();
    assert_eq!(
        serde_json::to_value(store.load().unwrap()).unwrap(),
        serde_json::to_value(&expected).unwrap()
    );
}

#[test]
fn failed_load_requires_reopening_even_after_successful_reread() {
    // This test binary does not launch processes that can temporarily inherit
    // a Unix Store lock between fork and exec while we close and reopen it.
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(Store::open(dir.path()).unwrap());
    fs::write(store.history_path(), b"{").unwrap();
    assert!(store.load().is_err());
    // Simulate explicit recovery by the user; this instance stays read-only.
    let recovered = include_bytes!("fixtures/state-v0.9.json");
    fs::write(store.history_path(), recovered).unwrap();
    let state = store.load().unwrap();
    assert!(store.save(&state).is_err());
    assert!(store.begin_save(&state).is_err());
    assert_eq!(fs::read(store.history_path()).unwrap(), recovered);
    drop(store);
    let reopened = Store::open(dir.path()).unwrap();
    reopened.save(&reopened.load().unwrap()).unwrap();
}
