#![cfg(feature = "test-support")]
#![allow(clippy::unwrap_used)]
use codeconvoy::{
    agents,
    domain::*,
    git,
    persistence::Store,
    process::Cancellation,
    runner::{self, Event, RunManager},
    validation::{self, Command, Record, Service, Status},
    worktrees,
};
use std::{
    path::{Path, PathBuf},
    process::Command as Process,
    sync::atomic::AtomicBool,
    time::Duration,
};
use tokio::sync::mpsc;

fn git_run(path: &Path, args: &[&str]) {
    let out = Process::new("git")
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
}
fn command(mode: &str, arguments: &[&str]) -> Command {
    Command {
        executable: env!("CARGO_BIN_EXE_codeconvoy-test-agent").into(),
        arguments: std::iter::once(mode.to_owned())
            .chain(arguments.iter().map(|s| s.to_string()))
            .collect(),
    }
}
struct Fixture {
    _temp: tempfile::TempDir,
    root: PathBuf,
    store: Store,
    state: AppState,
    manager: RunManager,
    service: Service,
    events: mpsc::Receiver<Event>,
}
impl Fixture {
    async fn new(isolated: bool) -> Self {
        let temp = tempfile::Builder::new()
            .prefix("validation ü ")
            .tempdir()
            .unwrap();
        let root = temp.path().canonicalize().unwrap();
        let path = root.join("repository with spaces");
        std::fs::create_dir(&path).unwrap();
        git_run(&path, &["init", "--quiet"]);
        git_run(&path, &["config", "core.autocrlf", "false"]);
        std::fs::write(path.join("tracked.txt"), "baseline\n").unwrap();
        git_run(&path, &["add", "tracked.txt"]);
        git_run(
            &path,
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
        let repo = git::register(&path).await.unwrap();
        let before = git::status(&path).await.unwrap().summary;
        let store = Store::open(&root.join("state")).unwrap();
        let worktrees_path = store.directory().join("worktrees");
        let mut job = Job::queued(repo.clone());
        job.before = Some(before.clone());
        if isolated {
            let m = worktrees::reserve(
                &worktrees_path,
                1,
                0,
                repo.clone(),
                before.common_dir.clone().unwrap(),
                before.head.clone().unwrap(),
            )
            .unwrap();
            worktrees::create(&m, &Cancellation::default(), &AtomicBool::new(true))
                .await
                .unwrap();
            job.execution_mode = ExecutionMode::IsolatedWorktree;
            job.worktree = Some(m);
        }
        job.finish(JobStatus::Succeeded, Some(0), "agent succeeded".into());
        let state = AppState {
            repositories: vec![repo],
            runs: vec![Run {
                id: 1,
                provenance: None,
                created_at: now(),
                task: TaskConfig::default(),
                jobs: vec![job],
            }],
            next_run: 2,
            ..Default::default()
        };
        let (tx, events) = mpsc::channel(256);
        let manager = RunManager::with_worktree_directory(2, tx, worktrees_path);
        Self {
            _temp: temp,
            root,
            store,
            state,
            manager,
            service: Service::default(),
            events,
        }
    }
    fn job(&self) -> &Job {
        &self.state.runs[0].jobs[0]
    }
    fn start(&mut self, command: Command) {
        let record = self
            .service
            .start(
                &tokio::runtime::Handle::current(),
                self.manager.lifecycle(),
                (1, 0),
                self.job().clone(),
                command,
            )
            .unwrap();
        self.state.runs[0].jobs[0].validation = Some(record);
    }
    async fn finish(&mut self) -> Record {
        tokio::time::timeout(Duration::from_secs(15), async {
            loop {
                for (key, record) in self.service.poll() {
                    assert_eq!(key, (1, 0));
                    self.state.runs[0].jobs[0].validation = Some(record);
                }
                if self.service.is_idle() {
                    return self.job().validation.clone().unwrap();
                }
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .unwrap()
    }
    fn gate(&self) -> PathBuf {
        let path = self.root.join("control");
        std::fs::create_dir_all(&path).unwrap();
        path
    }
}
async fn ready(path: &Path) {
    tokio::time::timeout(Duration::from_secs(10), async {
        while !path.exists() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
}
async fn unlocked(path: &Path) {
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let file = std::fs::OpenOptions::new().write(true).open(path).unwrap();
            if fs2::FileExt::try_lock_exclusive(&file).is_ok() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn configuration_round_trips_edits_removals_and_legacy_defaults() {
    let mut f = Fixture::new(false).await;
    let path = f.job().repository.path.clone();
    assert!(f.state.repository_validation.is_empty());
    assert!(f.job().validation.is_none());
    let c = command(
        "validation-pass",
        &["", "a b", "'quoted'", "$(not-a-shell)", "日本語"],
    );
    f.state
        .save_validation(path.clone(), Some(c.clone()))
        .unwrap();
    f.store.save(&f.state).unwrap();
    assert_eq!(f.store.load().unwrap().repository_validation[&path], c);
    let changed = command("validation-fail", &[]);
    f.state
        .save_validation(path.clone(), Some(changed.clone()))
        .unwrap();
    f.store.save(&f.state).unwrap();
    assert_eq!(
        f.store.load().unwrap().repository_validation[&path],
        changed
    );
    f.state.save_validation(path.clone(), None).unwrap();
    f.store.save(&f.state).unwrap();
    assert!(f.store.load().unwrap().repository_validation.is_empty());
    assert!(
        f.state
            .save_validation(f.root.join("unregistered"), Some(c))
            .is_err()
    );
    let mut legacy = serde_json::to_value(&f.state).unwrap();
    legacy
        .as_object_mut()
        .unwrap()
        .remove("repository_validation");
    legacy["runs"][0]["jobs"][0]
        .as_object_mut()
        .unwrap()
        .remove("validation");
    let loaded: AppState = serde_json::from_value(legacy).unwrap();
    assert!(loaded.repository_validation.is_empty());
    assert!(loaded.runs[0].jobs[0].validation.is_none());
    assert_eq!(validation::label(&loaded.runs[0].jobs[0], true), "Not run");
}
#[tokio::test]
async fn pass_captures_literal_arguments_output_time_and_history_without_changing_agent_or_baseline()
 {
    let mut f = Fixture::new(false).await;
    let before = serde_json::to_value(f.job()).unwrap();
    let args = [
        "",
        "two words",
        "'quote'",
        "{braces}",
        "$(touch should-not-exist)",
        "日本語",
    ];
    let c = command("validation-pass", &args);
    f.start(c.clone());
    assert!(f.state.runs[0].history_protected());
    assert!(!f.state.remove_from_history(1));
    let record = f.finish().await;
    assert_eq!(record.status, Status::Passed);
    assert_eq!(record.exit_code, Some(0));
    assert!(
        record
            .output
            .contains(&serde_json::to_string(&args).unwrap())
    );
    assert!(record.output.contains("validation stdout Grüße 日本語"));
    assert!(record.output.contains("[stderr] validation stderr"));
    assert!(record.started_at > 0 && record.finished_at.unwrap() >= record.started_at);
    assert!(record.duration_ms > 0);
    assert!(!f.job().repository.path.join("should-not-exist").exists());
    let mut after = serde_json::to_value(f.job()).unwrap();
    after.as_object_mut().unwrap().remove("validation");
    assert_eq!(after, before);
    f.store.save(&f.state).unwrap();
    let saved = f
        .store
        .load()
        .unwrap()
        .runs
        .remove(0)
        .jobs
        .remove(0)
        .validation
        .unwrap();
    assert_eq!(saved.output, record.output);
    assert_eq!(saved.command, c);
    assert_eq!(saved.status, Status::Passed);
    assert_eq!(saved.directory, f.job().repository.path);
}
#[tokio::test]
async fn failure_is_separate_from_agent_outcome_and_latest_result_replaces_previous() {
    let mut f = Fixture::new(false).await;
    f.start(command("validation-pass", &[]));
    assert_eq!(f.finish().await.status, Status::Passed);
    f.start(command("validation-fail", &[]));
    let result = f.finish().await;
    assert_eq!(result.status, Status::Failed);
    assert_eq!(result.exit_code, Some(7));
    assert_eq!(f.job().status, JobStatus::Succeeded);
    assert_eq!(f.state.runs[0].status(), JobStatus::Succeeded);
    assert_eq!(f.job().exit_code, Some(0));
}
#[tokio::test]
async fn launch_errors_and_disappeared_directories_are_unavailable() {
    let mut f = Fixture::new(false).await;
    f.start(Command {
        executable: f.root.join("missing-executable").to_str().unwrap().into(),
        arguments: vec![],
    });
    assert_eq!(f.finish().await.status, Status::Unavailable);
    let path = f.job().repository.path.clone();
    std::fs::rename(&path, f.root.join("moved")).unwrap();
    f.start(command("validation-edit", &[]));
    let result = f.finish().await;
    assert_eq!(result.status, Status::Unavailable);
    assert!(result.exit_code.is_none());
}
#[tokio::test]
async fn isolated_execution_uses_original_worktree_and_never_falls_back_when_missing() {
    let mut f = Fixture::new(true).await;
    let path = f.job().worktree.as_ref().unwrap().path.clone();
    f.start(command("validation-edit", &[]));
    assert_eq!(f.finish().await.status, Status::Passed);
    assert_eq!(
        std::fs::read_to_string(path.join("tracked.txt")).unwrap(),
        "validation changed this file\n"
    );
    assert_eq!(
        std::fs::read_to_string(f.job().repository.path.join("tracked.txt")).unwrap(),
        "baseline\n"
    );
    std::fs::rename(&path, path.with_file_name("temporarily-moved")).unwrap();
    f.start(command("validation-edit", &[]));
    assert_eq!(f.finish().await.status, Status::Unavailable);
    assert_eq!(
        std::fs::read_to_string(f.job().repository.path.join("tracked.txt")).unwrap(),
        "baseline\n"
    );
}
#[tokio::test]
async fn missing_or_mismatched_historical_identity_cannot_launch() {
    let mut f = Fixture::new(true).await;
    f.state.runs[0].jobs[0].worktree.as_mut().unwrap().job = 19;
    f.start(command("validation-edit", &[]));
    assert_eq!(f.finish().await.status, Status::Unavailable);
    f.state.runs[0].jobs[0].worktree = None;
    f.start(command("validation-edit", &[]));
    assert_eq!(f.finish().await.status, Status::Unavailable);
    assert_eq!(
        std::fs::read_to_string(f.job().repository.path.join("tracked.txt")).unwrap(),
        "baseline\n"
    );
}
#[tokio::test]
async fn cancellation_reaps_parent_and_descendant_and_releases_repository() {
    let mut f = Fixture::new(false).await;
    let gate = f.gate();
    f.start(command("validation-gate", &[gate.to_str().unwrap()]));
    ready(&gate.join("child.ready")).await;
    ready(&gate.join("parent.ready")).await;
    assert!(
        f.service
            .start(
                &tokio::runtime::Handle::current(),
                f.manager.lifecycle(),
                (1, 0),
                f.job().clone(),
                command("validation-pass", &[])
            )
            .is_err()
    );
    f.service.cancel((1, 0));
    assert_eq!(f.finish().await.status, Status::Cancelled);
    unlocked(&gate.join("parent.lock")).await;
    unlocked(&gate.join("child.lock")).await;
    f.start(command("validation-pass", &[]));
    assert_eq!(f.finish().await.status, Status::Passed);
}
#[tokio::test]
async fn shutdown_cancels_validation_process_tree_and_interrupted_history_never_resumes() {
    let mut f = Fixture::new(false).await;
    let gate = f.gate();
    f.start(command("validation-gate", &[gate.to_str().unwrap()]));
    f.store.save(&f.state).unwrap();
    let interrupted = f.store.load().unwrap();
    assert_eq!(
        interrupted.runs[0].jobs[0]
            .validation
            .as_ref()
            .unwrap()
            .status,
        Status::Cancelled
    );
    assert_eq!(interrupted.runs[0].jobs[0].status, JobStatus::Succeeded);
    ready(&gate.join("child.ready")).await;
    f.manager.shutdown();
    assert_eq!(f.finish().await.status, Status::Cancelled);
    tokio::time::timeout(Duration::from_secs(10), &mut f.manager.join)
        .await
        .unwrap()
        .unwrap();
    unlocked(&gate.join("parent.lock")).await;
    unlocked(&gate.join("child.lock")).await;
}
#[tokio::test]
async fn competing_result_validation_and_cleanup_are_blocked_by_shared_lease() {
    let mut f = Fixture::new(true).await;
    let gate = f.gate();
    f.start(command("validation-gate", &[gate.to_str().unwrap()]));
    ready(&gate.join("child.ready")).await;
    let mut second = Service::default();
    second
        .start(
            &tokio::runtime::Handle::current(),
            f.manager.lifecycle(),
            (1, 0),
            f.job().clone(),
            command("validation-pass", &[]),
        )
        .unwrap();
    let mut result = None;
    tokio::time::timeout(Duration::from_secs(10), async {
        while !second.is_idle() {
            for (_, r) in second.poll() {
                result = Some(r);
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(result.unwrap().status, Status::Unavailable);
    assert!(
        f.store
            .cleanup_result(&mut f.state, &f.manager, 1, 0)
            .await
            .is_err()
    );
    assert!(f.job().worktree.as_ref().unwrap().path.exists());
    f.service.cancel((1, 0));
    assert_eq!(f.finish().await.status, Status::Cancelled);
}
#[tokio::test]
async fn active_agent_excludes_validation_but_unrelated_work_continues() {
    let mut f = Fixture::new(false).await;
    let gate = f.gate();
    let task = TaskConfig {
        prompt: format!(
            "codeconvoy-fixture-gate\n{}",
            serde_json::json!({"control":gate,"ticket":"agent","fail":false})
        ),
        options: [(
            "executable".into(),
            env!("CARGO_BIN_EXE_codeconvoy-test-agent").into(),
        )]
        .into(),
        ..Default::default()
    };
    let prepared = runner::prepare(task, vec![f.job().repository.clone()])
        .await
        .unwrap();
    f.manager
        .start(2, prepared, agents::backend(AgentId::Codex).unwrap())
        .unwrap();
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if matches!(f.events.recv().await.unwrap(), Event::Started { .. }) {
                break;
            }
        }
    })
    .await
    .unwrap();
    f.start(command("validation-edit", &[]));
    assert_eq!(f.finish().await.status, Status::Unavailable);
    assert!(f.manager.is_active(2));
    // An unrelated checkout can validate using the same manager while agent slots are active.
    let other = Fixture::new(false).await;
    let mut service = Service::default();
    service
        .start(
            &tokio::runtime::Handle::current(),
            f.manager.lifecycle(),
            (8, 0),
            other.job().clone(),
            command("validation-pass", &[]),
        )
        .unwrap();
    let mut result = None;
    tokio::time::timeout(Duration::from_secs(10), async {
        while !service.is_idle() {
            for (_, r) in service.poll() {
                result = Some(r);
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(result.unwrap().status, Status::Passed);
    f.manager.cancel_all();
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if matches!(f.events.recv().await.unwrap(), Event::Finished { .. }) {
                break;
            }
        }
    })
    .await
    .unwrap();
}
#[tokio::test]
async fn output_is_bounded_and_later_file_changes_do_not_rewrite_historical_result() {
    let mut f = Fixture::new(false).await;
    f.start(command("validation-flood", &[]));
    let record = f.finish().await;
    assert!(record.truncated && record.output.len() <= validation::OUTPUT_LIMIT);
    let saved = serde_json::to_value(&record).unwrap();
    std::fs::write(
        f.job().repository.path.join("tracked.txt"),
        "subsequent edit\n",
    )
    .unwrap();
    assert_eq!(
        serde_json::to_value(f.job().validation.as_ref().unwrap()).unwrap(),
        saved
    );
    assert!(
        git::diff_job(f.job())
            .await
            .unwrap()
            .contains("subsequent edit")
    );
}
