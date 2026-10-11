use super::*;
#[cfg(unix)]
use codeconvoy::persistence::worktree_cleanup::{Batch, Outcome, Summary, identity};
use codeconvoy::{
    persistence::worktree_cleanup::Plan,
    storage::{
        self,
        management::{Lifecycle, classify, protection},
    },
    validation,
};

async fn entry(f: &Fixture, m: &WorktreeMetadata) -> storage::Worktree {
    storage::Worktree {
        path: m.path.clone(),
        metadata: m.clone(),
        measurement: storage::Measurement {
            bytes: Some(120),
            status: storage::Status::Complete,
            ..Default::default()
        },
        inspection: Some(
            f.manager
                .lifecycle()
                .inspect_cleanup(m, &Cancellation::default())
                .await,
        ),
    }
}
#[cfg(unix)]
async fn resolved(f: &mut Fixture) {
    f.inspect(false).await;
    f.state.runs[0].jobs[0].resolution = ResultResolution::Applied;
}
#[cfg(unix)]
async fn plan(f: &Fixture) -> Plan {
    let m = f.metadata();
    let entry = entry(f, &m).await;
    Plan::new(&f.state, &[entry], &[identity(&m)].into())
}
#[cfg(unix)]
async fn execute(f: &mut Fixture, plan: Plan) -> Summary {
    let mut batch = Batch::new(plan);
    while !batch.done() {
        if let Some(op) = batch.begin_next(&f.store, &mut f.state) {
            let completion = op.execute(&f.manager.lifecycle()).await;
            batch.finish(&f.store, &mut f.state, &op, completion);
        }
    }
    batch.summary
}
#[cfg(unix)]
fn assert_prewrite_rejection(f: &Fixture) {
    let job = &f.state.runs[0].jobs[0];
    assert_eq!(job.result_availability, ResultAvailability::Available);
    assert!(!job.result_checked);
    assert!(job.worktree_detail.contains("Cleanup not started"));
    assert!(f.metadata().path.exists());
    assert!(f.admin().join("locked").exists());
    assert!(
        !f.metadata()
            .path
            .parent()
            .unwrap()
            .join("cleanup.json")
            .exists()
    );
}
fn validation_record(job: &Job) -> validation::Record {
    validation::Record::pending(
        validation::Command {
            executable: "git".into(),
            arguments: vec!["status".into()],
        },
        job,
    )
}

#[cfg(unix)]
#[tokio::test]
async fn ordinary_cleanup_preserves_history_validation_indexes_and_invalidates_inventory() {
    let mut f = Fixture::new().await;
    resolved(&mut f).await;
    let m = f.metadata();
    let job = &mut f.state.runs[0].jobs[0];
    let mut record = validation_record(job);
    record.status = validation::Status::Passed;
    record.finished_at = Some(10);
    job.validation = Some(record);
    job.completion_changes = Some(codeconvoy::visibility::Changes {
        changed: Some(false),
        files: Some(0),
    });
    let historical = serde_json::to_value(&f.state.runs[0]).unwrap();
    let index = fs::read(f.source().join(".git/index")).unwrap();
    let selected = plan(&f).await;
    assert_eq!(selected.eligible(), 1);
    let summary = execute(&mut f, selected).await;
    assert_eq!(
        summary.count(Outcome::Removed),
        1,
        "{:?}",
        summary
            .entries
            .iter()
            .map(|e| &e.detail)
            .collect::<Vec<_>>()
    );
    assert!(!m.path.exists());
    assert_eq!(fs::read(f.source().join(".git/index")).unwrap(), index);
    let job = &f.state.runs[0].jobs[0];
    assert_eq!(job.result_availability, ResultAvailability::Cleaned);
    assert_eq!(job.resolution, ResultResolution::Applied);
    for key in [
        "validation",
        "completion_changes",
        "worktree_result",
        "status",
        "before",
        "worktree",
        "resolved_at",
    ] {
        assert_eq!(
            serde_json::to_value(job).unwrap()[key],
            historical["jobs"][0][key],
            "{key}"
        );
    }
    let mut validations = validation::Service::default();
    assert!(
        validations
            .start(
                &tokio::runtime::Handle::current(),
                f.manager.lifecycle(),
                (1, 0),
                job.clone(),
                job.validation.as_ref().unwrap().command.clone()
            )
            .is_err()
    );
    let inventory = worktrees::inventory::discover(
        &f.store.directory().join("worktrees"),
        vec![m.clone()],
        &Cancellation::default(),
    );
    assert!(inventory.entries.is_empty());
    assert!(m.path.parent().unwrap().join("owner.json").exists());
    let loaded = f.store.load().unwrap();
    assert_eq!(loaded.runs.len(), 1);
    assert_eq!(
        loaded.runs[0].jobs[0].result_availability,
        ResultAvailability::Cleaned
    );
    assert_eq!(plan(&f).await.eligible(), 0);
}

#[tokio::test]
async fn ordinary_cleanup_classifies_active_retained_missing_and_unknown_without_mutation() {
    let mut f = Fixture::new().await;
    let m = f.metadata();
    let e = entry(&f, &m).await;
    // A completed, clean result is still retained until explicitly resolved.
    assert_eq!(protection(&f.state, &m).unwrap().0, Lifecycle::Retained);
    let (kind, detail) = classify(&f.state, &m, e.inspection.as_ref());
    if cfg!(unix) {
        assert_eq!(kind, Lifecycle::Retained, "{detail}");
    } else {
        assert_eq!(kind, Lifecycle::Unknown, "{detail}");
        assert!(detail.contains("disabled on this platform"), "{detail}");
        let inspection = e.inspection.as_ref().unwrap();
        assert_eq!(inspection.exists, Some(true));
        assert_eq!(inspection.registered, Some(true));
        assert!(inspection.proof.is_none());
    }
    for status in [JobStatus::Queued, JobStatus::Running, JobStatus::Preparing] {
        f.state.runs[0].jobs[0].status = status;
        assert_eq!(
            classify(&f.state, &m, e.inspection.as_ref()).0,
            Lifecycle::Active
        );
        assert_eq!(Plan::new(&f.state, &[], &HashSet::new()).eligible(), 0);
    }
    f.state.runs[0].jobs[0].status = JobStatus::Succeeded;
    f.state.runs[0].jobs[0].validation = Some(validation_record(&f.state.runs[0].jobs[0]));
    assert_eq!(
        classify(&f.state, &m, e.inspection.as_ref()).0,
        Lifecycle::Active
    );
    f.state.runs[0].jobs[0].validation = None;
    f.state.runs[0].jobs[0].resolution = ResultResolution::ApplyPending;
    assert_eq!(
        classify(&f.state, &m, e.inspection.as_ref()).0,
        Lifecycle::Unknown
    );
    f.state.runs.clear();
    assert_eq!(
        classify(&f.state, &m, e.inspection.as_ref()).0,
        Lifecycle::Unknown
    );
    fs::rename(&m.path, m.path.with_extension("missing")).unwrap();
    let missing = entry(&f, &m).await;
    assert_eq!(
        classify(&f.state, &m, missing.inspection.as_ref()).0,
        Lifecycle::Unavailable
    );
    assert!(m.path.parent().unwrap().join("owner.json").exists());
}

#[cfg(unix)]
#[tokio::test]
async fn ordinary_cleanup_protects_each_git_data_loss_condition_and_preserves_index() {
    for case in [
        "modified",
        "staged",
        "untracked",
        "ignored",
        "conflict",
        "committed",
        "assume",
        "sparse",
        "operation",
        "cached-edit",
        "nested",
    ] {
        let mut f = Fixture::new().await;
        resolved(&mut f).await;
        let m = f.metadata();
        match case {
            "modified" => f.edit(),
            "staged" => {
                f.edit();
                git_args(&m.path, &["add", "tracked.txt"]);
            }
            "untracked" => fs::write(m.path.join("valuable"), "keep").unwrap(),
            "ignored" => {
                fs::write(m.common_dir.join("info/exclude"), "ignored\n").unwrap();
                fs::create_dir(m.path.join("ignored")).unwrap();
                fs::write(m.path.join("ignored/valuable"), "keep").unwrap();
            }
            "committed" => {
                f.edit();
                git_args(&m.path, &["add", "."]);
                git_args(
                    &m.path,
                    &[
                        "-c",
                        "user.name=Fixture",
                        "-c",
                        "user.email=f@invalid",
                        "commit",
                        "-qm",
                        "valuable commit",
                    ],
                );
            }
            "assume" => {
                git_args(
                    &m.path,
                    &["update-index", "--assume-unchanged", "tracked.txt"],
                );
                f.edit();
            }
            "sparse" => {
                git_args(&m.path, &["update-index", "--skip-worktree", "tracked.txt"]);
                f.edit();
            }
            "operation" => fs::write(f.admin().join("MERGE_HEAD"), &m.base_commit).unwrap(),
            "nested" => {
                fs::create_dir(m.path.join("nested")).unwrap();
                git_args(&m.path.join("nested"), &["init", "--quiet"]);
            }
            "conflict" => {
                let blob = git_args(&m.path, &["rev-parse", "HEAD:tracked.txt"]);
                let mut child = Command::new("git")
                    .arg("-C")
                    .arg(&m.path)
                    .args(["update-index", "--index-info"])
                    .stdin(std::process::Stdio::piped())
                    .spawn()
                    .unwrap();
                use std::io::Write;
                child.stdin.take().unwrap().write_all(format!("0 {}\ttracked.txt\n100644 {blob} 1\ttracked.txt\n100644 {blob} 2\ttracked.txt\n100644 {blob} 3\ttracked.txt\n", "0".repeat(40)).as_bytes()).unwrap();
                assert!(child.wait().unwrap().success());
            }
            "cached-edit" => {
                git_args(&m.path, &["config", "core.trustctime", "false"]);
                git_args(&m.path, &["config", "core.checkstat", "minimal"]);
                let tracked = m.path.join("tracked.txt");
                let file = fs::OpenOptions::new().write(true).open(&tracked).unwrap();
                let times = fs::FileTimes::new()
                    .set_modified(std::time::SystemTime::now() - Duration::from_secs(60));
                file.set_times(times).unwrap();
                git_args(&m.path, &["update-index", "--refresh"]);
                fs::write(&tracked, "next\n").unwrap();
                file.set_times(times).unwrap();
                assert_eq!(git_args(&m.path, &["status", "--porcelain"]), "");
            }
            _ => unreachable!(),
        }
        let before = fs::read(f.admin().join("index")).unwrap();
        let selected = plan(&f).await;
        assert_eq!(selected.eligible(), 0, "{case}");
        assert!(m.path.exists());
        assert_eq!(fs::read(f.admin().join("index")).unwrap(), before, "{case}");
        assert!(!m.path.parent().unwrap().join("cleanup.json").exists());
    }
}

#[cfg(unix)]
#[tokio::test]
async fn ordinary_cleanup_rejects_stale_preview_and_changed_ownership() {
    for case in [
        "modified",
        "ignored",
        "owner",
        "lock",
        "backlink",
        "symlink",
        "parent-link",
        "replacement",
        "source",
        "unregistered",
        "lifecycle",
    ] {
        let mut f = Fixture::new().await;
        resolved(&mut f).await;
        let selected = plan(&f).await;
        assert_eq!(selected.eligible(), 1);
        let m = f.metadata();
        match case {
            "modified" => f.edit(),
            "ignored" => {
                fs::write(m.common_dir.join("info/exclude"), "valuable\n").unwrap();
                fs::write(m.path.join("valuable"), "keep").unwrap();
            }
            "owner" => fs::write(m.path.parent().unwrap().join("owner.json"), "{}").unwrap(),
            "lock" => fs::write(f.admin().join("locked"), "foreign lock").unwrap(),
            "backlink" => {
                fs::write(f.admin().join("gitdir"), f.source().display().to_string()).unwrap()
            }
            "symlink" => {
                fs::rename(&m.path, m.path.with_extension("saved")).unwrap();
                std::os::unix::fs::symlink(f.source(), &m.path).unwrap();
            }
            "parent-link" => {
                let parent = m.path.parent().unwrap();
                let saved = parent.with_extension("saved");
                fs::rename(parent, &saved).unwrap();
                std::os::unix::fs::symlink(&saved, parent).unwrap();
            }
            "replacement" => {
                fs::rename(&m.path, m.path.with_extension("saved")).unwrap();
                fs::create_dir(&m.path).unwrap();
                for name in [".git", "tracked.txt"] {
                    fs::copy(m.path.with_extension("saved").join(name), m.path.join(name)).unwrap();
                }
            }
            "source" => {
                fs::rename(f.source(), f.source().with_extension("saved")).unwrap();
                fs::create_dir(f.source()).unwrap();
                git_args(&f.source(), &["init", "--quiet"]);
            }
            "unregistered" => f.state.repositories.clear(),
            "lifecycle" => {
                f.state.runs[0].jobs[0]
                    .worktree
                    .as_mut()
                    .unwrap()
                    .base_commit = "a".repeat(40)
            }
            _ => unreachable!(),
        }
        let summary = execute(&mut f, selected).await;
        assert_eq!(summary.count(Outcome::Removed), 0, "{case}");
        assert!(m.path.exists(), "{case}");
    }
}

#[cfg(unix)]
#[tokio::test]
async fn ordinary_cleanup_supports_old_custom_bases_and_does_not_discover_user_worktrees() {
    let mut f = Fixture::new().await;
    let original = f.metadata();
    let custom = f.root.join("old storage ü 日本語");
    let reserved = worktrees::reserve_at(
        &f.store.directory().join("worktrees"),
        Some(&custom),
        1,
        0,
        original.repository.clone(),
        original.common_dir.clone(),
        original.base_commit.clone(),
    )
    .unwrap();
    reserved.preparation.unwrap();
    let m = reserved.metadata;
    worktrees::create(&m, &Cancellation::default(), &AtomicBool::new(true))
        .await
        .unwrap();
    f.state.runs[0].jobs[0].worktree = Some(m.clone());
    f.state.worktree_base = Some(f.root.join("new base"));
    resolved(&mut f).await;
    let foreign = f.root.join("user tree");
    git_cmd(
        &f.source(),
        &[
            "worktree".as_ref(),
            "add".as_ref(),
            "--detach".as_ref(),
            &git::path_argument(&foreign),
            original.base_commit.as_ref(),
        ],
    );
    let inventory = worktrees::inventory::discover(
        &f.store.directory().join("worktrees"),
        vec![m.clone(), m.clone()],
        &Cancellation::default(),
    );
    assert_eq!(
        inventory.entries.iter().filter(|e| e.metadata == m).count(),
        1
    );
    assert!(!inventory.entries.iter().any(|e| e.metadata.path == foreign));
    let selected = plan(&f).await;
    assert_eq!(selected.eligible(), 1);
    assert_eq!(execute(&mut f, selected).await.count(Outcome::Removed), 1);
    assert!(original.path.exists() && foreign.exists());
}

#[cfg(unix)]
#[tokio::test]
async fn ordinary_cleanup_bulk_deduplicates_skips_protection_and_continues_after_failure() {
    let mut f = Fixture::new().await;
    resolved(&mut f).await;
    let first = f.metadata();
    let mut entries = vec![entry(&f, &first).await, entry(&f, &first).await];
    for id in [2, 3, 4] {
        let m = worktrees::reserve(
            &f.store.directory().join("worktrees"),
            id,
            0,
            first.repository.clone(),
            first.common_dir.clone(),
            first.base_commit.clone(),
        )
        .unwrap();
        worktrees::create(&m, &Cancellation::default(), &AtomicBool::new(true))
            .await
            .unwrap();
        let mut job = f.state.runs[0].jobs[0].clone();
        job.worktree = Some(m.clone());
        if id == 3 {
            job.resolution = ResultResolution::Unresolved;
        }
        f.state.runs.push(Run {
            id,
            jobs: vec![job],
            ..f.state.runs[0].clone()
        });
        entries.push(entry(&f, &m).await);
    }
    let selection = entries.iter().map(|e| identity(&e.metadata)).collect();
    let selected = Plan::new(&f.state, &entries, &selection);
    assert_eq!(selected.targets.len(), 4);
    assert_eq!(selected.eligible(), 3);
    // A late ignored file invalidates just one target, after confirmation.
    fs::write(first.common_dir.join("info/exclude"), "valuable\n").unwrap();
    fs::write(entries[2].path.join("valuable"), "keep").unwrap();
    let summary = execute(&mut f, selected).await;
    assert_eq!(
        (
            summary.count(Outcome::Removed),
            summary.count(Outcome::Skipped),
            summary.count(Outcome::Failed)
        ),
        (2, 1, 1)
    );
    assert!(entries[2].path.exists() && entries[3].path.exists());
    assert_eq!(f.state.runs.len(), 4);
}

#[cfg(unix)]
#[tokio::test]
async fn ordinary_cleanup_rechecks_active_process_lease_and_shutdown() {
    let mut f = Fixture::new().await;
    resolved(&mut f).await;
    let selected = plan(&f).await;
    f.peer().await;
    let summary = execute(&mut f, selected).await;
    assert_eq!(summary.count(Outcome::Failed), 1);
    assert!(summary.entries[0].detail.contains("in use"));
    assert_prewrite_rejection(&f);
    f.stop_peer().await;
    f.state = f.store.load().unwrap();
    assert_eq!(
        f.inspect(false).await.availability,
        ResultAvailability::Available
    );
    let retry = plan(&f).await;
    assert_eq!(retry.eligible(), 1);
    assert_eq!(execute(&mut f, retry).await.count(Outcome::Removed), 1);

    let mut f = Fixture::new().await;
    resolved(&mut f).await;
    let mut batch = Batch::new(plan(&f).await);
    batch.stop();
    assert!(batch.done());
    assert_eq!(batch.summary.count(Outcome::Skipped), 1);
    assert!(f.metadata().path.exists());
    let selected = plan(&f).await;
    f.manager.shutdown();
    let summary = execute(&mut f, selected).await;
    assert_eq!(summary.count(Outcome::Removed), 0);
    assert!(f.metadata().path.exists());
}

#[cfg(unix)]
#[tokio::test]
async fn ordinary_cleanup_blocks_live_validation_even_when_ui_record_is_stale() {
    let mut f = Fixture::new().await;
    resolved(&mut f).await;
    let selected = plan(&f).await;
    let mut peer = Job::queued(f.state.repositories[0].clone());
    peer.before = Some(git::status(&f.source()).await.unwrap().summary);
    peer.status = JobStatus::Succeeded;
    let control = f.root.join("validation-control");
    fs::create_dir(&control).unwrap();
    let mut service = validation::Service::default();
    service
        .start(
            &tokio::runtime::Handle::current(),
            f.manager.lifecycle(),
            (2, 0),
            peer,
            validation::Command {
                executable: env!("CARGO_BIN_EXE_codeconvoy-test-agent").into(),
                arguments: vec!["validation-gate".into(), control.display().to_string()],
            },
        )
        .unwrap();
    tokio::time::timeout(Duration::from_secs(10), async {
        while !control.join("parent.ready").exists() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    // No Running record in AppState: the manager lease must still reject cleanup.
    let summary = execute(&mut f, selected).await;
    assert_eq!(summary.count(Outcome::Failed), 1);
    assert!(summary.entries[0].detail.contains("in use"));
    assert_prewrite_rejection(&f);
    service.shutdown();
    tokio::time::timeout(Duration::from_secs(10), async {
        while !service.is_idle() {
            service.poll();
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(
        f.inspect(false).await.availability,
        ResultAvailability::Available
    );
    let retry = plan(&f).await;
    assert_eq!(retry.eligible(), 1);
    assert_eq!(execute(&mut f, retry).await.count(Outcome::Removed), 1);
}

#[cfg(unix)]
#[tokio::test]
async fn ordinary_cleanup_stale_preview_can_be_reviewed_and_retried_after_restart() {
    let mut f = Fixture::new().await;
    resolved(&mut f).await;
    let selected = plan(&f).await;
    let m = f.metadata();
    let before = serde_json::to_value(&f.state.runs[0].jobs[0]).unwrap();
    let index = fs::read(f.admin().join("index")).unwrap();
    fs::write(m.path.join("late-file"), "keep").unwrap();
    let summary = execute(&mut f, selected).await;
    assert_eq!(summary.count(Outcome::Failed), 1);
    assert!(
        summary.entries[0]
            .detail
            .contains("changed since confirmation")
    );
    assert_prewrite_rejection(&f);
    assert_eq!(fs::read(f.admin().join("index")).unwrap(), index);
    assert_eq!(plan(&f).await.eligible(), 0);
    for key in [
        "resolution",
        "worktree",
        "worktree_result",
        "validation",
        "completion_changes",
    ] {
        assert_eq!(
            serde_json::to_value(&f.state.runs[0].jobs[0]).unwrap()[key],
            before[key]
        );
    }

    f.state = f.store.load().unwrap();
    assert_eq!(
        f.state.runs[0].jobs[0].result_availability,
        ResultAvailability::Available
    );
    assert!(!f.state.runs[0].jobs[0].result_checked);
    assert_eq!(
        f.inspect(false).await.availability,
        ResultAvailability::Available
    );
    assert_eq!(plan(&f).await.eligible(), 0); // The late file still needs review.
    assert_eq!(
        fs::read_to_string(m.path.join("late-file")).unwrap(),
        "keep"
    );
    fs::remove_file(m.path.join("late-file")).unwrap();
    let retry = plan(&f).await;
    assert_eq!(retry.eligible(), 1);
    assert_eq!(execute(&mut f, retry).await.count(Outcome::Removed), 1);
}

#[cfg(unix)]
#[tokio::test]
async fn ordinary_cleanup_rejected_outcome_save_failure_retains_durable_pending_intent() {
    let mut f = Fixture::new().await;
    resolved(&mut f).await;
    let mut batch = Batch::new(plan(&f).await);
    let op = batch.begin_next(&f.store, &mut f.state).unwrap();
    fs::write(f.metadata().path.join("late-file"), "keep").unwrap();
    let completion = op.execute(&f.manager.lifecycle()).await;
    assert!(matches!(
        completion,
        codeconvoy::persistence::results::Completion::ManagedCleanupBlocked(_)
    ));
    // Fail the outcome save after the known pre-write rejection. Only durable
    // pending intent survives restart, so absence of a journal cannot allow retry.
    let history = f.store.history_path();
    let saved = history.with_extension("saved");
    fs::rename(&history, &saved).unwrap();
    fs::create_dir(&history).unwrap();
    batch.finish(&f.store, &mut f.state, &op, completion);
    assert_eq!(batch.summary.count(Outcome::Failed), 1);
    assert!(
        batch.summary.entries[0]
            .detail
            .contains("outcome could not be saved")
    );
    assert_eq!(
        f.state.runs[0].jobs[0].result_availability,
        ResultAvailability::CleanupPending
    );
    fs::remove_dir(&history).unwrap();
    fs::rename(&saved, &history).unwrap();
    f.state = f.store.load().unwrap();
    assert_eq!(
        f.inspect(false).await.availability,
        ResultAvailability::CleanupFailed
    );
    assert_eq!(plan(&f).await.eligible(), 0);
    assert!(f.metadata().path.join("late-file").exists());
}

#[cfg(unix)]
#[tokio::test]
async fn ordinary_cleanup_does_not_block_on_unrelated_jobs_in_the_same_convoy() {
    let mut f = Fixture::new().await;
    resolved(&mut f).await;
    let other = Fixture::new().await;
    let m = f.metadata();
    let inspected = entry(&f, &m).await;
    let mut peer = Job::queued(other.state.repositories[0].clone());
    peer.before = Some(git::status(&other.source()).await.unwrap().summary);
    f.state.runs[0].jobs.push(peer.clone());
    for status in [JobStatus::Queued, JobStatus::Preparing, JobStatus::Running] {
        peer.status = status;
        f.state.runs[0].jobs[1] = peer.clone();
        assert!(f.state.runs[0].active());
        assert_eq!(
            classify(&f.state, &m, inspected.inspection.as_ref()).0,
            Lifecycle::Candidate
        );
        // Same, nested and shared-common-directory work still blocks cleanup.
        for path in [
            &m.repository.path,
            &m.repository.path.join("nested"),
            m.repository.path.parent().unwrap(),
        ] {
            f.state.runs[0].jobs[1].repository.path = path.to_owned();
            assert_eq!(
                classify(&f.state, &m, inspected.inspection.as_ref()).0,
                Lifecycle::Active
            );
        }
        f.state.runs[0].jobs[1] = peer.clone();
        f.state.runs[0].jobs[1].before.as_mut().unwrap().common_dir = Some(m.common_dir.clone());
        assert_eq!(
            classify(&f.state, &m, inspected.inspection.as_ref()).0,
            Lifecycle::Active
        );
    }
    f.state.runs[0].jobs[1] = peer;
    let selected = plan(&f).await;
    assert_eq!(selected.eligible(), 1);
    assert_eq!(execute(&mut f, selected).await.count(Outcome::Removed), 1);
    assert_eq!(f.state.runs[0].jobs[1].status, JobStatus::Running);
    assert!(other.source().join("tracked.txt").exists());
}

#[cfg(unix)]
#[tokio::test]
async fn ordinary_cleanup_intent_save_failure_prevents_git_removal() {
    let mut f = Fixture::new().await;
    resolved(&mut f).await;
    let selected = plan(&f).await;
    fs::create_dir(f.store.history_path()).unwrap();
    let before = serde_json::to_value(&f.state).unwrap();
    let summary = execute(&mut f, selected).await;
    assert_eq!(summary.count(Outcome::Failed), 1);
    assert!(f.metadata().path.join("tracked.txt").exists());
    assert!(
        !f.metadata()
            .path
            .parent()
            .unwrap()
            .join("cleanup.json")
            .exists()
    );
    assert_eq!(serde_json::to_value(&f.state).unwrap(), before);
}

#[tokio::test]
async fn ordinary_cleanup_rejects_missing_records_repository_roots_and_unregistered_worktrees() {
    for case in [
        "missing-owner",
        "root",
        "child",
        "user-worktree",
        "out-of-scope",
        "registration",
    ] {
        let f = Fixture::new().await;
        let mut m = f.metadata();
        let original = m.path.clone();
        match case {
            "missing-owner" => fs::rename(
                m.path.parent().unwrap().join("owner.json"),
                m.path.parent().unwrap().join("owner.saved"),
            )
            .unwrap(),
            "root" => m.path = m.repository.path.clone(),
            "child" => m.path = m.path.join("nested"),
            "user-worktree" => {
                let foreign = f.root.join("user-created");
                git_cmd(
                    &f.source(),
                    &[
                        "worktree".as_ref(),
                        "add".as_ref(),
                        "--detach".as_ref(),
                        &git::path_argument(&foreign),
                        m.base_commit.as_ref(),
                    ],
                );
                m.path = foreign;
            }
            "out-of-scope" => {
                m.path = f.root.join("unowned");
                fs::create_dir(&m.path).unwrap();
            }
            "registration" => {
                fs::rename(f.admin(), f.admin().with_extension("saved")).unwrap();
            }
            _ => unreachable!(),
        }
        let e = entry(&f, &m).await;
        assert!(e.inspection.unwrap().proof.is_none(), "{case}");
        assert!(original.join("tracked.txt").exists());
        assert!(f.source().join("tracked.txt").exists());
    }
}
