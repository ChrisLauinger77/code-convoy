#![cfg(feature = "test-support")]
#![allow(clippy::unwrap_used)]
use codeconvoy::{
    agents,
    domain::*,
    git,
    persistence::Store,
    process::Cancellation,
    runner::{self, Event, RunManager},
    worktrees,
};
use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::AtomicBool,
    time::Duration,
};
use tokio::sync::mpsc;

fn git_cmd(path: &Path, args: &[&std::ffi::OsStr]) -> String {
    let out = Command::new("git")
        .arg("-C")
        .arg(path)
        .args(["-c", "commit.gpgsign=false"])
        .args(args)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).unwrap().trim().into()
}
fn git_args(path: &Path, args: &[&str]) -> String {
    git_cmd(
        path,
        &args.iter().map(std::ffi::OsStr::new).collect::<Vec<_>>(),
    )
}
struct Fixture {
    _temp: tempfile::TempDir,
    root: PathBuf,
    store: Store,
    state: AppState,
    manager: RunManager,
    rx: mpsc::Receiver<Event>,
}
impl Fixture {
    async fn new() -> Self {
        let temp = tempfile::Builder::new()
            .prefix("convoy recovery ü ")
            .tempdir()
            .unwrap();
        let root = temp.path().canonicalize().unwrap();
        let source = root.join("source 日本語 with spaces");
        fs::create_dir(&source).unwrap();
        git_args(&source, &["init", "--quiet"]);
        git_args(&source, &["config", "core.autocrlf", "false"]);
        fs::write(source.join("tracked.txt"), "base\n").unwrap();
        git_args(&source, &["add", "tracked.txt"]);
        git_args(
            &source,
            &[
                "-c",
                "user.name=Fixture",
                "-c",
                "user.email=fixture@example.invalid",
                "commit",
                "-qm",
                "base",
            ],
        );
        let repository = git::register(&source).await.unwrap();
        let summary = git::status(&source).await.unwrap().summary;
        let store = Store::open(&root.join("state")).unwrap();
        let root_store = store.directory().join("worktrees");
        let m = worktrees::reserve(
            &root_store,
            1,
            0,
            repository.clone(),
            summary.common_dir.unwrap(),
            summary.head.unwrap(),
        )
        .unwrap();
        worktrees::create(&m, &Cancellation::default(), &AtomicBool::new(true))
            .await
            .unwrap();
        let mut job = Job::queued(repository.clone());
        job.execution_mode = ExecutionMode::IsolatedWorktree;
        job.worktree = Some(m);
        job.finish(JobStatus::Succeeded, Some(0), String::new());
        let state = AppState {
            repositories: vec![repository],
            next_run: 2,
            runs: vec![Run {
                id: 1,
                created_at: 0,
                task: TaskConfig {
                    execution_mode: ExecutionMode::IsolatedWorktree,
                    ..Default::default()
                },
                jobs: vec![job],
            }],
            ..Default::default()
        };
        let (tx, rx) = mpsc::channel(256);
        let manager = RunManager::with_worktree_directory(2, tx, root_store);
        Self {
            _temp: temp,
            root,
            store,
            state,
            manager,
            rx,
        }
    }
    fn metadata(&self) -> WorktreeMetadata {
        self.state.runs[0].jobs[0].worktree.clone().unwrap()
    }
    fn source(&self) -> PathBuf {
        self.state.repositories[0].path.clone()
    }
    fn edit(&self) {
        fs::write(self.metadata().path.join("tracked.txt"), "retained work\n").unwrap();
    }
    async fn inspect(&mut self, diff: bool) -> worktrees::recovery::Report {
        let report = self
            .manager
            .lifecycle()
            .inspect(1, 0, &self.state.runs[0].jobs[0], diff)
            .await;
        report.apply(&mut self.state.runs[0].jobs[0]);
        report
    }
    fn reload(&mut self) {
        self.store.save(&self.state).unwrap();
        self.state = self.store.load().unwrap();
    }
    async fn cleanup(&mut self) -> anyhow::Result<()> {
        self.store
            .cleanup_result(&mut self.state, &self.manager, 1, 0)
            .await
    }
    fn admin(&self) -> PathBuf {
        let bytes = fs::read_to_string(self.metadata().path.join(".git")).unwrap();
        PathBuf::from(bytes.trim().strip_prefix("gitdir: ").unwrap())
            .canonicalize()
            .unwrap()
    }
    async fn peer(&mut self) {
        let control = self.root.join("control");
        fs::create_dir(&control).unwrap();
        let task = TaskConfig {
            execution_mode: ExecutionMode::IsolatedWorktree,
            prompt: format!(
                "codeconvoy-fixture-gate\n{}",
                serde_json::json!({"control":control,"ticket":"peer","edit_before":true,"fail":false})
            ),
            options: [(
                "executable".into(),
                env!("CARGO_BIN_EXE_codeconvoy-test-agent").into(),
            )]
            .into(),
            ..Default::default()
        };
        let prepared = runner::prepare(task, vec![self.state.repositories[0].clone()])
            .await
            .unwrap();
        self.manager
            .start(2, prepared, agents::backend(AgentId::Codex).unwrap())
            .unwrap();
        tokio::time::timeout(Duration::from_secs(20), async {
            loop { if matches!(self.rx.recv().await.unwrap(), Event::Output { text, .. } if text.contains("fixture gate ready")) { break; } }
        }).await.unwrap();
    }
    async fn stop_peer(&mut self) {
        self.manager.cancel_run(2);
        tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                if matches!(
                    self.rx.recv().await.unwrap(),
                    Event::Finished { run: 2, .. }
                ) {
                    break;
                }
            }
        })
        .await
        .unwrap();
    }
}

#[tokio::test]
async fn changed_result_roundtrip_and_restart_recovers_available() {
    let mut f = Fixture::new().await;
    f.edit();
    assert_eq!(
        f.inspect(false).await.availability,
        ResultAvailability::Available
    );
    f.reload();
    assert!(!f.state.runs[0].jobs[0].result_checked);
    assert_eq!(
        f.state.runs[0].jobs[0]
            .worktree_result
            .as_ref()
            .unwrap()
            .changed,
        Some(true)
    );
    let report = f.inspect(true).await;
    assert_eq!(
        report.availability,
        ResultAvailability::Available,
        "{}",
        report.detail
    );
    assert!(report.diff.unwrap().contains("+retained work"));
    assert_eq!(
        fs::read_to_string(f.source().join("tracked.txt")).unwrap(),
        "base\n"
    );
}
#[tokio::test]
async fn recovered_diff_uses_original_base_after_source_commit_and_external_edits() {
    let mut f = Fixture::new().await;
    f.edit();
    f.reload();
    fs::write(f.source().join("tracked.txt"), "later source\n").unwrap();
    git_args(&f.source(), &["add", "tracked.txt"]);
    git_args(
        &f.source(),
        &[
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=fixture@example.invalid",
            "commit",
            "-qm",
            "later",
        ],
    );
    let diff = f.inspect(true).await.diff.unwrap();
    assert!(
        diff.contains("-base") && diff.contains("+retained work") && !diff.contains("later source")
    );
    fs::write(
        f.metadata().path.join("tracked.txt"),
        "external retained edit\n",
    )
    .unwrap();
    assert!(
        f.inspect(true)
            .await
            .diff
            .unwrap()
            .contains("+external retained edit")
    );
}
#[tokio::test]
async fn missing_directory_is_unavailable_and_never_recreated() {
    let mut f = Fixture::new().await;
    let m = f.metadata();
    fs::remove_dir_all(&m.path).unwrap();
    f.reload();
    let report = f.inspect(true).await;
    assert_eq!(
        report.availability,
        ResultAvailability::Missing,
        "{}",
        report.detail
    );
    assert!(report.diff.is_none() && !m.path.exists());
    assert_eq!(f.state.runs[0].jobs[0].status, JobStatus::Succeeded);
}
#[tokio::test]
async fn externally_removed_worktree_is_missing() {
    let mut f = Fixture::new().await;
    let m = f.metadata();
    git_cmd(
        &f.source(),
        &[
            "worktree".as_ref(),
            "remove".as_ref(),
            "--force".as_ref(),
            "--force".as_ref(),
            &git::path_argument(&m.path),
        ],
    );
    assert_eq!(
        f.inspect(false).await.availability,
        ResultAvailability::Missing
    );
}
#[tokio::test]
async fn removed_git_registration_with_files_remaining_is_stale() {
    let mut f = Fixture::new().await;
    // Equivalent resulting state to external metadata pruning while files survive.
    fs::remove_dir_all(f.admin()).unwrap();
    assert_eq!(
        f.inspect(false).await.availability,
        ResultAvailability::Stale
    );
    assert!(f.cleanup().await.is_err());
    assert!(f.metadata().path.exists());
}
#[tokio::test]
async fn external_prune_of_missing_directory_is_detected_without_broad_app_pruning() {
    let mut f = Fixture::new().await;
    let m = f.metadata();
    git_cmd(
        &f.source(),
        &[
            "worktree".as_ref(),
            "unlock".as_ref(),
            &git::path_argument(&m.path),
        ],
    );
    fs::remove_dir_all(&m.path).unwrap();
    git_args(&f.source(), &["worktree", "prune", "--expire", "now"]);
    assert_eq!(
        f.inspect(false).await.availability,
        ResultAvailability::Missing
    );
}
#[tokio::test]
async fn unavailable_source_preserves_result_and_blocks_cleanup() {
    let mut f = Fixture::new().await;
    f.edit();
    fs::rename(f.source(), f.root.join("moved source")).unwrap();
    assert_eq!(
        f.inspect(false).await.availability,
        ResultAvailability::Stale
    );
    assert!(f.cleanup().await.is_err());
    assert!(f.metadata().path.is_dir());
}
#[tokio::test]
async fn manifest_mismatch_blocks_cleanup() {
    let mut f = Fixture::new().await;
    let m = f.metadata();
    let mut other = m.clone();
    other.run = 999;
    fs::write(
        m.path.parent().unwrap().join("owner.json"),
        serde_json::to_vec(&other).unwrap(),
    )
    .unwrap();
    assert_eq!(
        f.inspect(false).await.availability,
        ResultAvailability::Invalid
    );
    assert!(f.cleanup().await.is_err());
    assert!(m.path.is_dir());
}
#[tokio::test]
async fn tampered_persisted_path_cannot_delete_arbitrary_directory() {
    let mut f = Fixture::new().await;
    let arbitrary = f.root.join("valuable");
    fs::create_dir(&arbitrary).unwrap();
    fs::write(arbitrary.join("keep"), "valuable").unwrap();
    f.state.runs[0].jobs[0].worktree.as_mut().unwrap().path = arbitrary.clone();
    assert!(f.cleanup().await.is_err());
    assert_eq!(
        fs::read_to_string(arbitrary.join("keep")).unwrap(),
        "valuable"
    );
}
#[tokio::test]
async fn source_can_never_be_cleanup_target() {
    let mut f = Fixture::new().await;
    f.state.runs[0].jobs[0].worktree.as_mut().unwrap().path = f.source();
    assert!(f.cleanup().await.is_err());
    assert!(f.source().join("tracked.txt").is_file());
}
#[tokio::test]
async fn unrelated_worktree_cannot_be_cleanup_target() {
    let mut f = Fixture::new().await;
    let other = f.root.join("user worktree");
    git_cmd(
        &f.source(),
        &[
            "worktree".as_ref(),
            "add".as_ref(),
            "--detach".as_ref(),
            &git::path_argument(&other),
        ],
    );
    f.state.runs[0].jobs[0].worktree.as_mut().unwrap().path = other.clone();
    assert!(f.cleanup().await.is_err());
    assert!(other.join("tracked.txt").exists());
}
#[tokio::test]
async fn verified_cleanup_removes_only_owned_checkout_and_registration() {
    let mut f = Fixture::new().await;
    f.edit();
    let m = f.metadata();
    let other = f.root.join("unrelated");
    git_cmd(
        &f.source(),
        &[
            "worktree".as_ref(),
            "add".as_ref(),
            "--detach".as_ref(),
            &git::path_argument(&other),
        ],
    );
    f.cleanup().await.unwrap();
    assert!(!m.path.exists());
    assert!(m.path.parent().unwrap().join("owner.json").is_file());
    assert!(other.join("tracked.txt").exists());
    assert!(f.source().join("tracked.txt").exists());
    assert_eq!(
        f.store.load().unwrap().runs[0].jobs[0].result_availability,
        ResultAvailability::Cleaned
    );
    assert_eq!(
        f.inspect(false).await.availability,
        ResultAvailability::Cleaned
    );
    assert!(f.state.remove_from_history(1));
    assert!(
        worktrees::recovery::orphans(&f.store.directory().join("worktrees"), &HashSet::new())
            .unwrap()
            .is_empty()
    );
}
#[tokio::test]
async fn already_missing_owned_checkout_is_cleaned_via_exact_registration() {
    let mut f = Fixture::new().await;
    let m = f.metadata();
    fs::remove_dir_all(&m.path).unwrap();
    f.cleanup().await.unwrap();
    assert_eq!(
        f.state.runs[0].jobs[0].result_availability,
        ResultAvailability::Cleaned
    );
}
#[tokio::test]
async fn already_removed_owned_worktree_is_idempotently_resolved() {
    let mut f = Fixture::new().await;
    f.cleanup().await.unwrap();
    f.cleanup().await.unwrap();
    assert_eq!(
        f.inspect(false).await.availability,
        ResultAvailability::Cleaned
    );
}
#[tokio::test]
async fn cleanup_failure_survives_restart_with_metadata_and_diagnostics() {
    let mut f = Fixture::new().await;
    let m = f.metadata();
    fs::write(f.admin().join("locked"), "user changed ownership\n").unwrap();
    assert!(f.cleanup().await.is_err());
    f.state = f.store.load().unwrap();
    assert_eq!(
        f.state.runs[0].jobs[0].result_availability,
        ResultAvailability::CleanupFailed
    );
    assert_eq!(
        f.inspect(false).await.availability,
        ResultAvailability::CleanupFailed
    );
    assert!(m.path.is_dir());
    assert!(!f.state.runs[0].jobs[0].worktree_detail.is_empty());
}
#[tokio::test]
async fn active_job_and_peer_are_protected_and_cleanup_lock_releases_after_refusal() {
    let mut f = Fixture::new().await;
    f.edit();
    f.peer().await;
    assert!(
        f.cleanup()
            .await
            .unwrap_err()
            .to_string()
            .contains("Repository is in use")
    );
    assert!(f.manager.is_active(2));
    assert!(f.metadata().path.exists());
    f.stop_peer().await;
    f.reload();
    let report = f.inspect(true).await;
    assert_eq!(report.availability, ResultAvailability::CleanupFailed);
    assert!(report.diff.unwrap().contains("+retained work"));
    f.cleanup().await.unwrap();
    assert_eq!(git_args(&f.source(), &["status", "--porcelain"]), "");
}
#[tokio::test]
async fn active_status_itself_blocks_cleanup_even_without_a_manager_job() {
    let mut f = Fixture::new().await;
    for status in [JobStatus::Queued, JobStatus::Preparing, JobStatus::Running] {
        f.state.runs[0].jobs[0].status = status;
        assert!(f.cleanup().await.is_err());
    }
    assert!(f.metadata().path.exists());
}
async fn terminal_retention(status: JobStatus) {
    let mut f = Fixture::new().await;
    f.edit();
    f.state.runs[0].jobs[0].status = status;
    f.reload();
    let report = f.inspect(false).await;
    assert_eq!(
        report.availability,
        ResultAvailability::Available,
        "{}",
        report.detail
    );
    assert_eq!(report.result.unwrap().changed, Some(true));
    assert_eq!(
        f.state.runs[0].jobs[0].status,
        if status.is_terminal() {
            status
        } else {
            JobStatus::Cancelled
        }
    );
    assert_eq!(f.state.runs[0].jobs[0].interrupted, !status.is_terminal());
    assert!(f.metadata().path.exists());
}
#[tokio::test]
async fn failed_changes_survive_restart() {
    terminal_retention(JobStatus::Failed).await;
}
#[tokio::test]
async fn cancelled_changes_survive_restart() {
    terminal_retention(JobStatus::Cancelled).await;
}
#[tokio::test]
async fn interrupted_changes_survive_restart_without_execution() {
    terminal_retention(JobStatus::Running).await;
}
#[tokio::test]
async fn unchanged_policy_retains_and_protects_history() {
    let mut f = Fixture::new().await;
    f.reload();
    let report = f.inspect(false).await;
    assert_eq!(report.result.unwrap().changed, Some(false));
    assert!(f.metadata().path.exists());
    assert!(!f.state.remove_from_history(1));
    assert_eq!(f.state.clear_history(), 0);
}
#[tokio::test]
async fn history_cap_pins_results_and_preserves_direct_history_policy() {
    let mut f = Fixture::new().await;
    f.edit();
    for id in 2..42 {
        let mut job = Job::queued(f.state.repositories[0].clone());
        job.finish(JobStatus::Succeeded, Some(0), String::new());
        f.state.runs.insert(
            0,
            Run {
                id,
                created_at: 0,
                task: TaskConfig::default(),
                jobs: vec![job],
            },
        );
    }
    f.state.trim_history();
    assert_eq!(f.state.runs.len(), MAX_HISTORY + 1);
    assert!(!f.state.remove_from_history(1));
    assert!(f.state.remove_from_history(41));
    assert_eq!(f.state.clear_history(), MAX_HISTORY - 1);
    assert_eq!(f.state.runs[0].id, 1);
    assert!(f.metadata().path.exists());
}
#[tokio::test]
async fn reuse_after_recovery_has_fresh_baseline_and_no_previous_resource() {
    let mut f = Fixture::new().await;
    f.edit();
    f.reload();
    f.inspect(false).await;
    let task = TaskConfig {
        prompt: "new task".into(),
        options: [(
            "executable".into(),
            env!("CARGO_BIN_EXE_codeconvoy-test-agent").into(),
        )]
        .into(),
        ..f.state.runs[0].task.clone()
    };
    f.state.reuse_task(task);
    let prepared = runner::prepare(f.state.draft.clone(), f.state.repositories.clone())
        .await
        .unwrap();
    let fresh = prepared.snapshot(2);
    assert_eq!(fresh.task.execution_mode, ExecutionMode::IsolatedWorktree);
    assert!(fresh.jobs[0].worktree.is_none() && fresh.jobs[0].worktree_result.is_none());
    assert_eq!(fresh.jobs[0].status, JobStatus::Queued);
}
#[tokio::test]
async fn v02_defaults_preserve_direct_state_and_preferences() {
    let mut f = Fixture::new().await;
    f.state.runs[0].jobs[0] = Job::queued(f.state.repositories[0].clone());
    f.state.runs[0].jobs[0].finish(JobStatus::Succeeded, Some(0), String::new());
    f.state.draft.prompt = "saved task".into();
    f.state.global_concurrency = 7;
    f.state
        .save_template(
            None,
            TaskTemplate {
                name: "template".into(),
                prompt: "text".into(),
            },
        )
        .unwrap();
    let mut value = serde_json::to_value(&f.state).unwrap();
    value
        .get_mut("draft")
        .unwrap()
        .as_object_mut()
        .unwrap()
        .remove("execution_mode");
    value["runs"][0]["task"]
        .as_object_mut()
        .unwrap()
        .remove("execution_mode");
    let job = value["runs"][0]["jobs"][0].as_object_mut().unwrap();
    for field in [
        "execution_mode",
        "worktree",
        "worktree_result",
        "result_availability",
    ] {
        job.remove(field);
    }
    fs::write(
        f.store.directory().join("state.json"),
        serde_json::to_vec(&value).unwrap(),
    )
    .unwrap();
    let state = f.store.load().unwrap();
    assert_eq!(state.draft.execution_mode, ExecutionMode::Direct);
    assert_eq!(state.runs[0].jobs[0].execution_mode, ExecutionMode::Direct);
    assert!(state.runs[0].jobs[0].worktree.is_none());
    assert_eq!(state.global_concurrency, 7);
    assert_eq!(state.draft.prompt, "saved task");
    assert_eq!(state.templates.len(), 1);
    assert_eq!(state.repositories, f.state.repositories);
}
#[tokio::test]
async fn orphans_are_reported_but_never_removed() {
    let f = Fixture::new().await;
    let m = f.metadata();
    let root = f.store.directory().join("worktrees");
    assert_eq!(
        worktrees::recovery::orphans(&root, &HashSet::new()).unwrap(),
        vec![m.path.parent().unwrap().to_owned()]
    );
    assert!(
        worktrees::recovery::orphans(&root, &[m.path.clone()].into())
            .unwrap()
            .is_empty()
    );
    assert!(m.path.exists());
}
#[tokio::test]
async fn cleanup_completed_before_state_update_is_recovered_from_durable_intent() {
    let mut f = Fixture::new().await;
    f.edit();
    let stale = f.state.clone();
    f.cleanup().await.unwrap();
    // Simulate losing the final state replacement and even the journal completion.
    let path = f.metadata().path.parent().unwrap().join("cleanup.json");
    let mut value: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    value["completed"] = false.into();
    fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
    f.store.save(&stale).unwrap();
    f.state = f.store.load().unwrap();
    assert_eq!(
        f.inspect(false).await.availability,
        ResultAvailability::Cleaned
    );
    let journal: serde_json::Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    assert_eq!(journal["completed"], true);
}
#[tokio::test]
async fn saved_cleanup_intent_before_any_removal_preserves_files() {
    let mut f = Fixture::new().await;
    f.edit();
    f.state.runs[0].jobs[0].result_availability = ResultAvailability::CleanupPending;
    f.reload();
    assert_eq!(
        f.inspect(false).await.availability,
        ResultAvailability::CleanupFailed
    );
    assert!(f.metadata().path.exists());
}
#[tokio::test]
async fn saved_preparation_metadata_without_checkout_is_missing() {
    let mut f = Fixture::new().await;
    let m = f.metadata();
    f.cleanup().await.unwrap();
    let next = worktrees::reserve(
        &f.store.directory().join("worktrees"),
        1,
        0,
        m.repository,
        m.common_dir,
        m.base_commit,
    )
    .unwrap();
    f.state.runs[0].jobs[0].worktree = Some(next);
    f.state.runs[0].jobs[0].result_availability = ResultAvailability::Unchecked;
    f.state.runs[0].jobs[0].status = JobStatus::Preparing;
    f.reload();
    assert_eq!(
        f.inspect(false).await.availability,
        ResultAvailability::Missing
    );
    assert!(f.state.runs[0].jobs[0].interrupted);
}
#[tokio::test]
async fn unavailable_base_never_means_no_changes_or_permission_to_delete() {
    let mut f = Fixture::new().await;
    let mut m = f.metadata();
    m.base_commit = "a".repeat(40);
    fs::write(
        m.path.parent().unwrap().join("owner.json"),
        serde_json::to_vec(&m).unwrap(),
    )
    .unwrap();
    f.state.runs[0].jobs[0].worktree = Some(m.clone());
    assert_eq!(
        f.inspect(false).await.availability,
        ResultAvailability::Stale
    );
    assert!(f.cleanup().await.is_err());
    assert!(m.path.exists());
}
#[tokio::test]
async fn registering_a_retained_checkout_protects_it_from_internal_cleanup() {
    let mut f = Fixture::new().await;
    f.state
        .repositories
        .push(git::register(&f.metadata().path).await.unwrap());
    assert!(f.cleanup().await.is_err());
    assert!(f.metadata().path.exists());
}
#[tokio::test]
async fn job_identity_mismatch_is_invalid_and_blocks_cleanup() {
    let mut f = Fixture::new().await;
    f.state.runs[0].jobs[0].worktree.as_mut().unwrap().job = 9;
    assert_eq!(
        f.inspect(false).await.availability,
        ResultAvailability::Invalid
    );
    assert!(f.cleanup().await.is_err());
}
#[tokio::test]
async fn missing_storage_is_missing_without_recreation() {
    let mut f = Fixture::new().await;
    let root = f.store.directory().join("worktrees");
    fs::rename(&root, f.root.join("offline storage")).unwrap();
    assert_eq!(
        f.inspect(false).await.availability,
        ResultAvailability::Missing
    );
    assert!(f.cleanup().await.is_err());
    assert!(!root.exists());
}
#[cfg(unix)]
#[tokio::test]
async fn symlink_checkout_and_attempt_substitutions_fail_closed() {
    use std::os::unix::fs::symlink;
    let mut f = Fixture::new().await;
    let m = f.metadata();
    let relocated = f.root.join("keep files");
    fs::rename(&m.path, &relocated).unwrap();
    symlink(&relocated, &m.path).unwrap();
    assert_eq!(
        f.inspect(false).await.availability,
        ResultAvailability::Invalid
    );
    assert!(f.cleanup().await.is_err());
    assert!(relocated.join("tracked.txt").exists());
    fs::remove_file(&m.path).unwrap();
    fs::rename(relocated, &m.path).unwrap();
    let attempt = m.path.parent().unwrap();
    let elsewhere = f.root.join("keep attempt");
    fs::rename(attempt, &elsewhere).unwrap();
    symlink(&elsewhere, attempt).unwrap();
    assert!(f.cleanup().await.is_err());
    assert!(elsewhere.join("tree/tracked.txt").exists());
}
#[tokio::test]
async fn traversal_and_copied_manifest_cannot_expand_trusted_storage_boundary() {
    let mut f = Fixture::new().await;
    let m = f.metadata();
    let parent = m.path.parent().unwrap();
    // PathBuf::join normalizes `..` away on Windows verbatim paths. Build the
    // untrusted persisted spelling without normalization so every OS exercises
    // the actual traversal rejection, rather than submitting the original path.
    let mut traversal = parent.as_os_str().to_owned();
    traversal.push(std::path::MAIN_SEPARATOR_STR);
    traversal.push("..");
    traversal.push(std::path::MAIN_SEPARATOR_STR);
    traversal.push(parent.file_name().unwrap());
    traversal.push(std::path::MAIN_SEPARATOR_STR);
    traversal.push("tree");
    let traversal = PathBuf::from(traversal);
    assert!(
        traversal
            .components()
            .any(|c| c == std::path::Component::ParentDir)
    );
    f.state.runs[0].jobs[0].worktree.as_mut().unwrap().path = traversal;
    assert_eq!(
        f.inspect(false).await.availability,
        ResultAvailability::Invalid
    );
    assert!(f.cleanup().await.is_err());
    assert!(m.path.exists());
}

#[tokio::test]
async fn git_removal_failure_retains_result_without_a_filesystem_fallback() {
    let mut f = Fixture::new().await;
    let m = f.metadata();
    let protected = m.path.join("protected");
    fs::create_dir(&protected).unwrap();
    let keep = protected.join("keep");
    fs::write(&keep, "valuable").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&protected, fs::Permissions::from_mode(0o555)).unwrap();
        // Root can bypass these permissions; ordinary CI/desktop users cannot.
        if fs::write(protected.join("permission probe"), "probe").is_ok() {
            fs::set_permissions(&protected, fs::Permissions::from_mode(0o755)).unwrap();
            return;
        }
    }
    #[cfg(windows)]
    let handle = {
        use std::os::windows::fs::OpenOptionsExt;
        fs::OpenOptions::new()
            .read(true)
            .share_mode(0)
            .open(&keep)
            .unwrap()
    };
    let result = f.cleanup().await;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&protected, fs::Permissions::from_mode(0o755)).unwrap();
    }
    #[cfg(windows)]
    drop(handle);
    let error = result.unwrap_err();
    assert!(
        error
            .to_string()
            .contains("Could not remove isolated worktree"),
        "{error:#}"
    );
    assert_eq!(fs::read_to_string(keep).unwrap(), "valuable");
    assert_eq!(
        f.store.load().unwrap().runs[0].jobs[0].result_availability,
        ResultAvailability::CleanupFailed
    );
    assert!(f.source().join("tracked.txt").exists());
}

#[tokio::test]
async fn failed_intent_save_prevents_any_destructive_operation() {
    let mut f = Fixture::new().await;
    fs::create_dir(f.store.directory().join("state.json")).unwrap();
    assert!(
        f.cleanup()
            .await
            .unwrap_err()
            .to_string()
            .contains("no cleanup was started")
    );
    assert!(f.metadata().path.exists());
    assert!(
        !f.metadata()
            .path
            .parent()
            .unwrap()
            .join("cleanup.json")
            .exists()
    );
    assert_eq!(
        f.state.runs[0].jobs[0].result_availability,
        ResultAvailability::Unchecked
    );
}

#[tokio::test]
async fn cleaned_history_is_protected_until_the_saved_cleanup_is_revalidated() {
    let mut f = Fixture::new().await;
    f.cleanup().await.unwrap();
    f.reload();
    assert!(!f.state.remove_from_history(1));
    assert_eq!(
        f.inspect(false).await.availability,
        ResultAvailability::Cleaned
    );
    assert!(f.state.remove_from_history(1));
}

#[cfg(unix)]
#[tokio::test]
async fn symlinked_manifest_is_rejected_without_touching_its_target() {
    let mut f = Fixture::new().await;
    let owner = f.metadata().path.parent().unwrap().join("owner.json");
    let outside = f.root.join("external owner record");
    fs::rename(&owner, &outside).unwrap();
    std::os::unix::fs::symlink(&outside, owner).unwrap();
    assert_eq!(
        f.inspect(false).await.availability,
        ResultAvailability::Invalid
    );
    assert!(f.cleanup().await.is_err());
    assert!(outside.is_file());
}
