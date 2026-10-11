#![allow(clippy::unwrap_used)]
use codeconvoy::{domain::*, history::*, persistence::Store};
use std::{
    collections::BTreeSet,
    num::{NonZeroU32, NonZeroUsize},
    sync::Arc,
};
const NOW: u64 = 2_000_000_000;
const DAY: u64 = 86_400;
fn run(id: u64, age: u64) -> Run {
    let mut job = Job::queued(Repository {
        name: "fixture".into(),
        path: "/never-executed".into(),
    });
    job.status = JobStatus::Succeeded;
    job.started_at = Some(NOW - age * DAY - 1);
    job.finished_at = Some(NOW - age * DAY);
    Run {
        id,
        provenance: None,
        created_at: NOW - age * DAY - 2,
        task: TaskConfig::default(),
        jobs: vec![job],
    }
}
fn policy(count: usize, days: u32) -> RetentionPolicy {
    RetentionPolicy {
        max_entries: NonZeroUsize::new(count),
        max_age_days: NonZeroU32::new(days),
        automatic: false,
    }
}
fn preview(runs: &[Run], count: usize, days: u32) -> CleanupPreview {
    evaluate(runs, &policy(count, days), &BTreeSet::new(), NOW)
}
#[test]
fn limits_are_independent_inclusive_and_unlimited_by_default() {
    let runs = vec![run(1, 100), run(3, 0), run(2, 30)];
    assert!(preview(&runs, 0, 0).remove.is_empty());
    assert!(preview(&runs, 3, 0).remove.is_empty());
    assert_eq!(preview(&runs, 2, 0).remove, [1].into());
    assert_eq!(preview(&runs, 0, 30).remove, [1].into());
    assert_eq!(preview(&runs, 1, 30).remove, [1, 2].into());
    let p = preview(&runs, 1, 30);
    assert_eq!((p.stored, p.retained(), p.remove.len()), (3, 1, 2));
    assert!(preview(&[], 1, 30).remove.is_empty());
}
#[test]
fn completion_then_id_not_creation_or_visible_order_determines_newest() {
    let mut runs = vec![run(3, 1), run(1, 1), run(2, 1)];
    runs[1].created_at = 10;
    assert_eq!(preview(&runs, 1, 0).remove, [1, 2].into());
    runs.reverse();
    assert_eq!(preview(&runs, 1, 0).remove, [1, 2].into());
    runs[1].jobs[0].finished_at = Some(NOW);
    assert_eq!(preview(&runs, 1, 0).remove, [2, 3].into());
}
#[test]
fn incomplete_inconsistent_future_and_duplicate_records_are_preserved() {
    let mut runs = (1..=8).map(|id| run(id, 100)).collect::<Vec<_>>();
    runs[0].jobs[0].finished_at = None;
    runs[1].jobs[0].finished_at = Some(NOW + 1);
    runs[2].jobs[0].finished_at = Some(runs[2].created_at - 1);
    runs[3].jobs[0].started_at = Some(NOW + 1);
    runs[4].jobs.clear();
    let repository = runs[5].jobs[0].repository.clone();
    runs[5].jobs.push(Job::queued(repository));
    runs.push(runs[6].clone());
    let p = preview(&runs, 1, 30);
    assert_eq!(p.remove, [8].into());
    assert_eq!(p.undated, 5);
    assert_eq!(p.protected, 3);
    assert_eq!(p.retained(), 8);
}
#[test]
fn every_jobs_completion_is_required_and_last_completion_controls_age() {
    let mut r = run(1, 100);
    r.jobs.push(run(2, 1).jobs.remove(0));
    assert!(preview(&[r.clone()], 0, 30).remove.is_empty());
    r.jobs[1].finished_at = None;
    assert!(preview(&[r], 0, 30).remove.is_empty());
}
#[test]
fn active_validation_runtime_ownership_and_worktrees_are_protected() {
    let mut runs = (1..=8).map(|id| run(id, 100)).collect::<Vec<_>>();
    for (r, status) in
        runs.iter_mut()
            .zip([JobStatus::Queued, JobStatus::Preparing, JobStatus::Running])
    {
        r.jobs[0].status = status;
    }
    runs[3].jobs[0].validation = Some(codeconvoy::validation::Record::pending(
        codeconvoy::validation::Command {
            executable: "never-execute".into(),
            arguments: vec![],
        },
        &runs[3].jobs[0],
    ));
    runs[4].jobs[0].worktree = Some(WorktreeMetadata {
        owner: "ownership".into(),
        run: 5,
        job: 0,
        repository: runs[4].jobs[0].repository.clone(),
        common_dir: "/never-executed/.git".into(),
        path: "/owned-result".into(),
        base_commit: "base".into(),
        execution_mode: ExecutionMode::IsolatedWorktree,
    });
    runs[4].jobs[0].resolution = ResultResolution::Applied;
    let protected = [6, 7].into(); // cancelling/terminal workers and lifecycle leases
    let p = evaluate(&runs, &policy(1, 30), &protected, NOW);
    assert_eq!(p.remove, [8].into());
    assert_eq!(p.protected, 7);
    let ids = runs.iter().map(|r| r.id).collect();
    assert_eq!(remove(&mut runs, &ids, &protected), 1);
    assert!(runs[4].jobs[0].worktree.is_some());
}
#[test]
fn settings_compatibility_and_malformed_policies_fail_closed() {
    let old: AppState =
        serde_json::from_str(r#"{"version":1,"next_run":42,"desktop":{},"notifications":{}}"#)
            .unwrap();
    assert_eq!(old.history_retention, RetentionPolicy::default());
    for bad in ["0", "-1", "1.5", "\"bad\"", "184467440737095516160"] {
        let value = format!(
            r#"{{"next_run":42,"history_retention":{{"automatic":true,"max_entries":{bad},"max_age_days":30}}}}"#
        );
        let state: AppState = serde_json::from_str(&value).unwrap();
        assert_eq!(state.next_run, 42);
        assert_eq!(state.history_retention, RetentionPolicy::default());
    }
    for value in [
        r#"null"#,
        r#"{"automatic":"yes"}"#,
        r#"{"automatic":true,"max_age_days":0}"#,
    ] {
        let state: AppState =
            serde_json::from_str(&format!(r#"{{"history_retention":{value}}}"#)).unwrap();
        assert_eq!(state.history_retention, RetentionPolicy::default());
    }
    for count in [
        "0",
        "-1",
        "",
        "1.1",
        "overflow",
        "999999999999999999999999999",
    ] {
        assert!(RetentionPolicy::parse_count(count).is_err());
    }
    assert_eq!(RetentionPolicy::parse_count(" 250 ").unwrap().get(), 250);
    let state = AppState {
        history_retention: RetentionPolicy {
            automatic: true,
            ..policy(250, 90)
        },
        ..Default::default()
    };
    let restored: AppState = serde_json::from_value(serde_json::to_value(&state).unwrap()).unwrap();
    assert_eq!(restored.history_retention, state.history_retention);
}
#[test]
fn load_never_trims_or_runs_retention_during_recovery() {
    let temp = tempfile::tempdir().unwrap();
    let store = Store::open(temp.path()).unwrap();
    let mut state = AppState {
        runs: (1..=600).map(|id| run(id, 100)).collect(),
        next_run: 601,
        history_retention: RetentionPolicy {
            automatic: true,
            ..policy(1, 1)
        },
        ..Default::default()
    };
    state.runs[0].jobs[0].status = JobStatus::Running;
    state.runs[0].jobs[0].finished_at = None;
    store.save(&state).unwrap();
    let bytes = std::fs::read(temp.path().join("state.json")).unwrap();
    let loaded = store.load().unwrap();
    assert_eq!(loaded.runs.len(), 600);
    assert!(loaded.runs[0].jobs[0].interrupted);
    assert_eq!(loaded.runs[0].jobs[0].finished_at, None);
    assert_eq!(
        std::fs::read(temp.path().join("state.json")).unwrap(),
        bytes
    );
    assert!(!preview(&loaded.runs, 1, 1).remove.contains(&1));
}
#[test]
fn atomic_transaction_noop_failure_and_concurrent_saves_preserve_state() {
    let temp = tempfile::tempdir().unwrap();
    let store = Arc::new(Store::open(temp.path()).unwrap());
    let mut state = AppState {
        runs: vec![run(2, 1), run(1, 100)],
        next_run: 3,
        ..Default::default()
    };
    store.save(&state).unwrap();
    let bytes = std::fs::read(temp.path().join("state.json")).unwrap();
    assert!(
        store
            .begin_history_write(&state, &BTreeSet::new(), &BTreeSet::new())
            .unwrap()
            .is_none()
    );
    assert_eq!(
        std::fs::read(temp.path().join("state.json")).unwrap(),
        bytes
    );
    let (write, staged) = store
        .begin_history_write(&state, &[1].into(), &BTreeSet::new())
        .unwrap()
        .unwrap();
    assert!(store.save(&state).is_err());
    assert_eq!(state.runs.len(), 2);
    write.persist(&staged).unwrap();
    assert_eq!(store.load().unwrap().runs.len(), 1);
    assert!(store.save(&state).is_err()); // until live merge
    state.draft.prompt = "new event during save".into();
    write.publish(&mut state);
    drop(write);
    assert_eq!(state.draft.prompt, "new event during save");
    assert_eq!(state.next_run, 3);
    assert_eq!(state.runs[0].id, 2);
    store.save(&state).unwrap();
    std::fs::rename(
        temp.path().join("state.json"),
        temp.path().join("prior.json"),
    )
    .unwrap();
    std::fs::create_dir(temp.path().join("state.json")).unwrap();
    let (write, staged) = store
        .begin_history_write(&state, &[2].into(), &BTreeSet::new())
        .unwrap()
        .unwrap();
    assert!(write.persist(&staged).is_err());
    drop(write);
    assert_eq!(state.runs.len(), 1);
    let prior: AppState =
        serde_json::from_slice(&std::fs::read(temp.path().join("prior.json")).unwrap()).unwrap();
    assert_eq!(prior.runs.len(), 1);
    assert_eq!(std::fs::read_dir(temp.path()).unwrap().count(), 3); // no abandoned temp file
}

#[test]
fn legacy_interrupted_finish_times_are_not_trusted_as_observed_completion() {
    let mut interrupted = run(1, 100);
    interrupted.jobs[0].interrupted = true;
    interrupted.jobs[0].status = JobStatus::Cancelled;
    let p = preview(&[interrupted, run(2, 1)], 1, 30);
    assert!(p.remove.is_empty());
    assert_eq!(p.undated, 1);
}
