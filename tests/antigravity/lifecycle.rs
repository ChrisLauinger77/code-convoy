//! Antigravity through the real scheduler, leases, worktrees and process
//! supervision. Shared runner edge cases (event backpressure, delayed cleanup)
//! are backend-independent and covered by the Cursor and core suites.
use super::*;
use codeconvoy::{
    continuation::Retry,
    domain::{AppState, ExecutionMode, JobStatus, QueueReason, WorktreeMetadata},
    git,
    runner::{self, Event, RunManager},
    worktrees,
};
use serde_json::json;
use std::{fs, process::Command, time::Duration};
use tokio::sync::mpsc;

struct Harness {
    root: tempfile::TempDir,
    manager: RunManager,
    rx: mpsc::Receiver<Event>,
    events: Vec<Event>,
}
impl Harness {
    fn new(limit: usize) -> Self {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir(root.path().join("control")).unwrap();
        let (tx, rx) = mpsc::channel(512);
        let manager = RunManager::with_worktree_directory(limit, tx, root.path().join("retained"));
        Self {
            root,
            manager,
            rx,
            events: Vec::new(),
        }
    }
    async fn repository(&self, name: &str) -> Repository {
        let path = self.root.path().join(name);
        fs::create_dir_all(&path).unwrap();
        command(&path, &["init", "--quiet"]);
        command(&path, &["config", "core.autocrlf", "false"]);
        fs::write(path.join("tracked.txt"), "base\n").unwrap();
        command(&path, &["add", "tracked.txt"]);
        command(
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
        git::register(&path).await.unwrap()
    }
    fn task(&self, ticket: u64, mode: ExecutionMode, extra: Value) -> TaskConfig {
        let mut control =
            json!({"control": self.root.path().join("control"), "ticket": ticket.to_string()});
        control
            .as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        TaskConfig {
            agent: AgentId::Antigravity,
            execution_mode: mode,
            prompt: format!("codeconvoy-fixture-gate\n{control}"),
            options: [(
                "executable".into(),
                env!("CARGO_BIN_EXE_codeconvoy-test-agent").into(),
            )]
            .into(),
            ..Default::default()
        }
    }
    async fn start(&mut self, ticket: u64, task: TaskConfig, repo: &Repository) {
        let prepared = runner::prepare(task, vec![repo.clone()]).await.unwrap();
        self.manager
            .start(
                ticket,
                prepared,
                agents::backend(AgentId::Antigravity).unwrap(),
            )
            .unwrap();
    }
    async fn until(&mut self, condition: impl Fn(&[Event]) -> bool) {
        tokio::time::timeout(Duration::from_secs(30), async {
            while !condition(&self.events) {
                self.events.push(self.rx.recv().await.unwrap());
            }
        })
        .await
        .unwrap_or_else(|_| panic!("Antigravity lifecycle timed out: {:?}", self.events));
    }
    fn ready(events: &[Event], run: u64) -> bool {
        events.iter().any(|event| matches!(event, Event::Output {run: r, text, ..} if *r == run && text.contains("fixture gate ready")))
    }
    fn finished(events: &[Event], run: u64, expected: JobStatus) -> bool {
        events.iter().any(|event| matches!(event, Event::Finished {run: r, status, ..} if *r == run && *status == expected))
    }
    fn detail(&self, run: u64) -> &str {
        self.events
            .iter()
            .find_map(|event| match event {
                Event::Finished { run: r, detail, .. } if *r == run => Some(detail.as_str()),
                _ => None,
            })
            .unwrap()
    }
    fn metadata(&self, run: u64) -> WorktreeMetadata {
        self.events
            .iter()
            .find_map(|event| match event {
                Event::Preparing {
                    run: r,
                    worktree: Some(metadata),
                    ..
                } if *r == run => Some(metadata.clone()),
                _ => None,
            })
            .unwrap()
    }
    fn release(&self, ticket: u64, name: &str) {
        fs::write(
            self.root
                .path()
                .join(format!("control/{ticket}-{name}.release")),
            "go",
        )
        .unwrap();
    }
    async fn close(mut self) {
        // Let any test-only descendant exit even if an assertion path skipped it.
        let _ = fs::write(self.root.path().join("control/release"), "finish");
        self.manager.shutdown();
        tokio::time::timeout(Duration::from_secs(10), &mut self.manager.join)
            .await
            .unwrap()
            .unwrap();
    }
}
fn command(path: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .arg("-C")
        .arg(path)
        .args(["-c", "commit.gpgsign=false"])
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().into()
}
async fn file_ready(path: &Path) {
    tokio::time::timeout(Duration::from_secs(10), async {
        while !path.exists() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
}
fn child_lock(root: &Path) -> fs::File {
    fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(root.join("control/child.lock"))
        .unwrap()
}

#[tokio::test]
async fn terminal_outcomes_keep_results_and_release_slots_in_both_execution_modes() {
    for mode in [ExecutionMode::Direct, ExecutionMode::IsolatedWorktree] {
        for (outcome, expected, diagnostic) in [
            ("success", JobStatus::Succeeded, "SUCCESS result"),
            (
                "failure",
                JobStatus::Failed,
                "Antigravity error — controlled failure",
            ),
            ("missing_auth", JobStatus::Failed, "sign-in is required"),
            ("malformed", JobStatus::Failed, "Invalid Antigravity JSON"),
            ("incomplete", JobStatus::Failed, "incomplete"),
            ("interrupted", JobStatus::Failed, "INTERRUPTED"),
            ("active_step", JobStatus::Failed, "step was still active"),
            ("agy_error", JobStatus::Failed, "AGY_ERROR"),
        ] {
            let mut h = Harness::new(1);
            let repo = h.repository("source").await;
            let before = command(&repo.path, &["rev-parse", "HEAD"]);
            let task = h.task(
                1,
                mode,
                json!({"edit_before": true, "antigravity_outcome": outcome, "fail": outcome == "failure"}),
            );
            let options = task.options.clone();
            h.start(1, task, &repo).await;
            h.until(|events| Harness::ready(events, 1)).await;
            let name = if mode == ExecutionMode::IsolatedWorktree {
                "tree"
            } else {
                "source"
            };
            h.release(1, name);
            h.until(|events| Harness::finished(events, 1, expected))
                .await;
            assert!(
                h.detail(1).contains(diagnostic),
                "{mode:?} {outcome}: {}",
                h.detail(1)
            );
            assert_eq!(
                h.events
                    .iter()
                    .filter(|event| matches!(event, Event::Started { run: 1, .. }))
                    .count(),
                1
            );
            // The configured defaults never add permission-widening flags.
            let args: Vec<String> = serde_json::from_slice(
                &fs::read(h.root.path().join(format!("control/1-{name}.args"))).unwrap(),
            )
            .unwrap();
            assert!(!args.iter().any(|arg| {
                arg == "--dangerously-skip-permissions"
                    || arg == "--sandbox"
                    || arg.starts_with("--print-timeout")
            }));
            if mode == ExecutionMode::IsolatedWorktree {
                let retained = h.metadata(1);
                assert!(
                    retained.path.is_dir()
                        && retained.path.parent().unwrap().join("owner.json").is_file()
                );
                assert_eq!(
                    worktrees::inspect(&retained).await.unwrap().changed,
                    Some(true)
                );
                assert_eq!(retained.base_commit, before);
                assert_eq!(
                    fs::read_to_string(repo.path.join("tracked.txt")).unwrap(),
                    "base\n"
                );
            } else {
                assert_eq!(
                    fs::read_to_string(repo.path.join("tracked.txt")).unwrap(),
                    "1"
                );
            }
            // Capacity and the repository lease are released on every outcome.
            let mut next = h.task(2, ExecutionMode::Direct, json!({}));
            next.options = options;
            h.start(2, next, &repo).await;
            h.until(|events| Harness::ready(events, 2)).await;
            h.release(2, "source");
            h.until(|events| Harness::finished(events, 2, JobStatus::Succeeded))
                .await;
            h.close().await;
        }
    }
}

#[tokio::test]
async fn cancellation_stops_descendants_before_nested_repository_admission_and_preserves_peer() {
    let mut h = Harness::new(3);
    let source = h.repository("source").await;
    let nested = h.repository("source/nested").await;
    let peer = h.repository("peer").await;
    h.start(
        1,
        h.task(
            1,
            ExecutionMode::Direct,
            json!({"spawn_child": true, "edit_before": true}),
        ),
        &source,
    )
    .await;
    h.until(|events| Harness::ready(events, 1)).await;
    file_ready(&h.root.path().join("control/child.ready")).await;
    let lock = child_lock(h.root.path());
    assert!(fs2::FileExt::try_lock_exclusive(&lock).is_err());
    h.start(2, h.task(2, ExecutionMode::Direct, json!({})), &nested)
        .await;
    h.until(|events| {
        events.iter().any(|event| {
            matches!(
                event,
                Event::Queued {
                    run: 2,
                    reason: QueueReason::Repository(1),
                    ..
                }
            )
        })
    })
    .await;
    h.start(3, h.task(3, ExecutionMode::Direct, json!({})), &peer)
        .await;
    h.until(|events| Harness::ready(events, 3)).await;
    h.manager.cancel_run(1);
    h.until(|events| {
        Harness::finished(events, 1, JobStatus::Cancelled) && Harness::ready(events, 2)
    })
    .await;
    // The agy process tree, including its tool process, ended before the
    // nested repository was admitted; the unrelated peer kept running.
    assert!(fs2::FileExt::try_lock_exclusive(&lock).is_ok());
    let finished = h
        .events
        .iter()
        .position(|event| matches!(event, Event::Finished { run: 1, .. }))
        .unwrap();
    let admitted = h
        .events
        .iter()
        .position(|event| matches!(event, Event::Started { run: 2, .. }))
        .unwrap();
    assert!(finished < admitted);
    assert!(
        !h.events
            .iter()
            .any(|event| matches!(event, Event::Finished { run: 3, .. }))
    );
    assert_eq!(
        fs::read_to_string(source.path.join("tracked.txt")).unwrap(),
        "1"
    );
    h.release(2, "nested");
    h.release(3, "peer");
    h.until(|events| {
        Harness::finished(events, 2, JobStatus::Succeeded)
            && Harness::finished(events, 3, JobStatus::Succeeded)
    })
    .await;
    h.close().await;
}

#[tokio::test]
async fn concurrent_isolated_results_are_independent_and_direct_access_waits_for_both() {
    let mut h = Harness::new(3);
    let repo = h.repository("source").await;
    for run in [1, 2] {
        h.start(
            run,
            h.task(
                run,
                ExecutionMode::IsolatedWorktree,
                json!({"edit_before": true}),
            ),
            &repo,
        )
        .await;
    }
    h.until(|events| Harness::ready(events, 1) && Harness::ready(events, 2))
        .await;
    let first = h.metadata(1);
    let second = h.metadata(2);
    assert_ne!(first.path, second.path);
    // Each agy process ran in, and reported, its own checkout.
    for (run, metadata) in [(1, &first), (2, &second)] {
        assert_eq!(
            Path::new(
                &fs::read_to_string(h.root.path().join(format!("control/{run}-tree.cwd"))).unwrap()
            )
            .canonicalize()
            .unwrap(),
            metadata.path
        );
    }
    h.start(3, h.task(3, ExecutionMode::Direct, json!({})), &repo)
        .await;
    h.until(|events| {
        events.iter().any(|event| {
            matches!(
                event,
                Event::Queued {
                    run: 3,
                    reason: QueueReason::Repository(_),
                    ..
                }
            )
        })
    })
    .await;
    h.manager.cancel_run(1);
    h.until(|events| Harness::finished(events, 1, JobStatus::Cancelled))
        .await;
    assert!(
        !h.events
            .iter()
            .any(|event| matches!(event, Event::Started { run: 3, .. }))
    );
    h.release(2, "tree");
    h.until(|events| {
        Harness::finished(events, 2, JobStatus::Succeeded) && Harness::ready(events, 3)
    })
    .await;
    h.release(3, "source");
    h.until(|events| Harness::finished(events, 3, JobStatus::Succeeded))
        .await;
    assert_eq!(
        fs::read_to_string(first.path.join("tracked.txt")).unwrap(),
        "1"
    );
    assert_eq!(
        fs::read_to_string(second.path.join("tracked.txt")).unwrap(),
        "2"
    );
    assert_eq!(
        fs::read_to_string(repo.path.join("tracked.txt")).unwrap(),
        "base\n"
    );
    h.close().await;
}

#[tokio::test]
async fn shutdown_cancels_process_trees_and_retains_isolated_evidence() {
    let mut h = Harness::new(2);
    let repo = h.repository("source").await;
    let peer = h.repository("peer").await;
    h.start(
        1,
        h.task(
            1,
            ExecutionMode::IsolatedWorktree,
            json!({"spawn_child": true, "edit_before": true}),
        ),
        &repo,
    )
    .await;
    h.start(2, h.task(2, ExecutionMode::Direct, json!({})), &peer)
        .await;
    h.until(|events| Harness::ready(events, 1) && Harness::ready(events, 2))
        .await;
    file_ready(&h.root.path().join("control/child.ready")).await;
    let lock = child_lock(h.root.path());
    h.manager.shutdown();
    h.until(|events| {
        Harness::finished(events, 1, JobStatus::Cancelled)
            && Harness::finished(events, 2, JobStatus::Cancelled)
    })
    .await;
    assert!(fs2::FileExt::try_lock_exclusive(&lock).is_ok());
    let retained = h.metadata(1);
    assert_eq!(
        worktrees::inspect(&retained).await.unwrap().changed,
        Some(true)
    );
    assert_eq!(
        fs::read_to_string(repo.path.join("tracked.txt")).unwrap(),
        "base\n"
    );
    h.close().await;
}

#[tokio::test]
async fn time_limit_stops_descendants_and_releases_repository_after_confirmed_cleanup() {
    let mut h = Harness::new(2);
    let repo = h.repository("source").await;
    let mut task = h.task(
        1,
        ExecutionMode::IsolatedWorktree,
        json!({"spawn_child": true, "edit_before": true}),
    );
    task.options.insert("timeout_seconds".into(), "3".into());
    h.start(1, task, &repo).await;
    h.until(|events| Harness::ready(events, 1)).await;
    file_ready(&h.root.path().join("control/child.ready")).await;
    let lock = child_lock(h.root.path());
    assert!(fs2::FileExt::try_lock_exclusive(&lock).is_err());
    h.start(2, h.task(2, ExecutionMode::Direct, json!({})), &repo)
        .await;
    h.until(|events| Harness::finished(events, 1, JobStatus::Failed) && Harness::ready(events, 2))
        .await;
    assert!(
        h.detail(1).contains("timed out after 3 seconds")
            && h.detail(1).contains("cleanup was confirmed"),
        "{}",
        h.detail(1)
    );
    assert!(fs2::FileExt::try_lock_exclusive(&lock).is_ok());
    assert_eq!(
        worktrees::inspect(&h.metadata(1)).await.unwrap().changed,
        Some(true)
    );
    h.release(2, "source");
    h.until(|events| Harness::finished(events, 2, JobStatus::Succeeded))
        .await;
    h.close().await;
}

#[tokio::test]
async fn background_process_left_by_a_successful_run_is_stopped_before_release() {
    for mode in [ExecutionMode::Direct, ExecutionMode::IsolatedWorktree] {
        let mut h = Harness::new(2);
        let repo = h.repository("source").await;
        h.start(1, h.task(1, mode, json!({"daemon": true})), &repo)
            .await;
        h.until(|events| Harness::ready(events, 1)).await;
        file_ready(&h.root.path().join("control/child.ready")).await;
        let lock = child_lock(h.root.path());
        assert!(fs2::FileExt::try_lock_exclusive(&lock).is_err());
        h.release(
            1,
            if mode == ExecutionMode::Direct {
                "source"
            } else {
                "tree"
            },
        );
        h.until(|events| Harness::finished(events, 1, JobStatus::Succeeded))
            .await;
        // agy exited without waiting for it; the owned tree was still stopped
        // before the job finished and the repository became available again.
        assert!(
            fs2::FileExt::try_lock_exclusive(&lock).is_ok(),
            "{mode:?}: background descendant survived"
        );
        h.start(2, h.task(2, ExecutionMode::Direct, json!({})), &repo)
            .await;
        h.until(|events| Harness::ready(events, 2)).await;
        h.release(2, "source");
        h.until(|events| Harness::finished(events, 2, JobStatus::Succeeded))
            .await;
        h.close().await;
    }
}

#[tokio::test]
async fn retry_reuses_the_original_permissions_without_broadening_them() {
    let mut h = Harness::new(1);
    let repo = h.repository("source").await;
    let task = h.task(
        1,
        ExecutionMode::Direct,
        json!({"antigravity_outcome": "missing_auth"}),
    );
    let prepared = runner::prepare(task, vec![repo.clone()]).await.unwrap();
    let mut run = prepared.snapshot(1);
    h.manager
        .start(1, prepared, agents::backend(AgentId::Antigravity).unwrap())
        .unwrap();
    h.until(|events| Harness::ready(events, 1)).await;
    h.release(1, "source");
    h.until(|events| Harness::finished(events, 1, JobStatus::Failed))
        .await;
    assert!(h.detail(1).contains("sign-in is required"));
    run.jobs[0].status = JobStatus::Failed;
    let state = AppState {
        repositories: vec![repo.clone()],
        runs: vec![run.clone()],
        ..Default::default()
    };
    let retry = Retry::from_state(&state, 1, 0).unwrap();
    assert_eq!(retry.task.options, run.task.options);
    assert_eq!(retry.task.options["permissions"], "configured");
    assert_eq!(retry.task.options["sandbox"], "configured");
    let (retried, _) = retry.prepare().await.unwrap();
    let spec = Antigravity.build(&retried.task, &repo).unwrap();
    assert!(
        !spec
            .args
            .iter()
            .any(|arg| { arg == "--dangerously-skip-permissions" || arg == "--sandbox" })
    );
    h.close().await;
}
