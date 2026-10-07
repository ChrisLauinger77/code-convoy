#![allow(clippy::unwrap_used)]
use codeconvoy::{attachments::Attachment, domain::*, persistence::Store};
use std::fs;

#[test]
fn populated_v01_fixture_retains_registrations_history_and_all_preferences() {
    let temp = tempfile::tempdir().unwrap();
    let store = Store::open(temp.path()).unwrap();
    let bytes = include_bytes!("fixtures/state-v0.1.json");
    fs::write(temp.path().join("state.json"), bytes).unwrap();
    let original: serde_json::Value = serde_json::from_slice(bytes).unwrap();
    let state = store.load().unwrap();
    let loaded = serde_json::to_value(&state).unwrap();
    for field in [
        "version",
        "next_run",
        "repositories",
        "agent_options",
        "global_concurrency",
    ] {
        assert_eq!(loaded[field], original[field], "{field}");
    }
    assert_eq!(state.next_run, 42);
    assert!(state.groups.is_empty() && state.templates.is_empty());
    assert!(state.draft.attachments.is_empty());
    let mut expected = original.clone();
    expected["groups"] = serde_json::json!([]);
    expected["templates"] = serde_json::json!([]);
    expected["draft"]["attachments"] = serde_json::json!([]);
    expected["runs"][0]["task"]["attachments"] = serde_json::json!([]);
    expected["draft"]["execution_mode"] = serde_json::json!("direct");
    expected["runs"][0]["task"]["execution_mode"] = serde_json::json!("direct");
    expected["runs"][0]["jobs"][0]["execution_mode"] = serde_json::json!("direct");
    expected["runs"][0]["jobs"][0]["worktree"] = serde_json::Value::Null;
    expected["runs"][0]["jobs"][0]["worktree_result"] = serde_json::Value::Null;
    expected["runs"][0]["jobs"][0]["result_availability"] = serde_json::json!("Unchecked");
    expected["runs"][0]["jobs"][0]["before"]["common_dir"] = serde_json::Value::Null;
    assert_eq!(loaded, expected);
    store.save(&state).unwrap();
    assert_eq!(
        serde_json::to_value(store.load().unwrap()).unwrap(),
        expected
    );
}

#[test]
fn old_version_one_state_defaults_new_features_without_changing_run_ids() {
    let temp = tempfile::tempdir().unwrap();
    let store = Store::open(temp.path()).unwrap();
    fs::write(temp.path().join("state.json"), r#"{"version":1,"next_run":42,"draft":{"prompt":"old draft","agent":"codex","options":{},"concurrency":2},"repositories":[],"runs":[]}"#).unwrap();
    let state = store.load().unwrap();
    assert!(state.groups.is_empty());
    assert!(state.templates.is_empty());
    assert!(state.draft.attachments.is_empty());
    assert_eq!(state.next_run, 42);
    assert_eq!(state.draft.prompt, "old draft");
}

#[test]
fn library_crud_retains_missing_members_and_cannot_change_run_snapshots() {
    let mut state = AppState::default();
    let repository = Repository {
        name: "repo".into(),
        path: "/fixture/repo".into(),
    };
    let group = RepositoryGroup {
        name: " Maintenance ".into(),
        repositories: vec![repository.path.clone(), repository.path.clone()],
    };
    state.save_group(None, group).unwrap();
    assert_eq!(state.groups[0].name, "Maintenance");
    assert_eq!(
        state.groups[0].repositories.as_slice(),
        std::slice::from_ref(&repository.path)
    );
    assert!(state.save_group(None, state.groups[0].clone()).is_err());
    state
        .save_template(
            None,
            TaskTemplate {
                name: "Review".into(),
                prompt: "Review README".into(),
            },
        )
        .unwrap();
    state.runs.push(Run {
        id: 8,
        created_at: now(),
        task: TaskConfig {
            prompt: state.templates[0].prompt.clone(),
            ..Default::default()
        },
        jobs: vec![Job::queued(repository.clone())],
    });
    let before = serde_json::to_value(&state.runs).unwrap();
    state
        .save_group(
            Some(0),
            RepositoryGroup {
                name: "Renamed".into(),
                repositories: vec![repository.path],
            },
        )
        .unwrap();
    state
        .save_template(
            Some(0),
            TaskTemplate {
                name: "Changed template".into(),
                prompt: "Different task".into(),
            },
        )
        .unwrap();
    assert!(!state.groups[0].repositories.is_empty()); // Registration is absent, membership stays repairable.
    assert!(
        state
            .save_template(
                None,
                TaskTemplate {
                    name: "Empty".into(),
                    prompt: " ".into()
                }
            )
            .is_err()
    );
    assert!(state.save_group(Some(8), state.groups[0].clone()).is_err());
    state.groups.clear();
    state.templates.clear();
    assert_eq!(serde_json::to_value(&state.runs).unwrap(), before);
}

#[test]
fn state_saves_library_and_attachment_metadata_but_never_contents_or_output() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("specification.txt");
    fs::write(&path, "private attached specification content").unwrap();
    let attachment = Attachment::inspect(&path).unwrap();
    let store = Store::open(&temp.path().join("state")).unwrap();
    let mut state = AppState::default();
    state.draft.attachments.push(attachment.clone());
    state
        .save_group(
            None,
            RepositoryGroup {
                name: "Missing members".into(),
                repositories: vec![temp.path().join("unregistered")],
            },
        )
        .unwrap();
    state
        .save_template(
            None,
            TaskTemplate {
                name: "Review".into(),
                prompt: "Review changes".into(),
            },
        )
        .unwrap();
    let mut job = Job::queued(Repository {
        name: "repo".into(),
        path: temp.path().join("repo"),
    });
    job.log.append("private activity");
    job.raw_log.append("private raw output");
    state.runs.push(Run {
        id: 1,
        created_at: now(),
        task: state.draft.clone(),
        jobs: vec![job],
    });
    store.save(&state).unwrap();
    let saved = fs::read_to_string(store.directory().join("state.json")).unwrap();
    for contents in [
        "private attached specification content",
        "private activity",
        "private raw output",
    ] {
        assert!(!saved.contains(contents));
    }
    let loaded = store.load().unwrap();
    assert_eq!(loaded.groups, state.groups);
    assert_eq!(loaded.templates, state.templates);
    assert_eq!(
        loaded.draft.attachments.as_slice(),
        std::slice::from_ref(&attachment)
    );
    fs::remove_file(&path).unwrap();
    assert_eq!(store.load().unwrap().runs[0].task.attachments, [attachment]);
    assert!(loaded.runs[0].jobs[0].log.text.is_empty());
    assert!(loaded.runs[0].jobs[0].raw_log.text.is_empty());
    let template = serde_json::to_value(&loaded.templates[0]).unwrap();
    assert_eq!(template.as_object().unwrap().len(), 2);
}

#[test]
fn session_memory_budget_includes_both_activity_and_raw_output() {
    let mut state = AppState::default();
    let jobs = (0..40)
        .map(|i| {
            let mut job = Job::queued(Repository {
                name: i.to_string(),
                path: format!("/fixture/{i}").into(),
            });
            job.log.append(&"a".repeat(LOG_LIMIT));
            job.raw_log.append(&"r".repeat(LOG_LIMIT));
            job
        })
        .collect();
    state.runs.push(Run {
        id: 1,
        created_at: now(),
        task: TaskConfig::default(),
        jobs,
    });
    state.trim_logs();
    assert!(
        state.runs[0]
            .jobs
            .iter()
            .map(|job| job.log.text.len() + job.raw_log.text.len())
            .sum::<usize>()
            <= TOTAL_LOG_LIMIT
    );
    assert!(state.runs[0].jobs[0].log.truncated);
    assert!(state.runs[0].jobs[0].raw_log.truncated);
}
