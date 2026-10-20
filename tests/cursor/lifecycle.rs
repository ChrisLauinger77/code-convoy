use super::*;
use codeconvoy::{
    agents::{AgentBackend, AgentOutput, OptionSpec},
    domain::{AgentOptions, ExecutionMode, JobStatus, QueueReason, WorktreeMetadata},
    git,
    process::{CommandSpec, ManagedChild},
    runner::{self, Event, PreparedRun, RunManager},
    worktrees,
};
use std::{fs, path::PathBuf, process::Command, sync::Arc, time::Duration};
use tokio::sync::mpsc;

#[path = "../support/executable.rs"]
mod executable;

struct Harness {
    root: tempfile::TempDir,
    manager: RunManager,
    rx: mpsc::Receiver<Event>,
    events: Vec<Event>,
}
impl Harness {
    fn new(limit: usize, capacity: usize) -> Self {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir(root.path().join("control")).unwrap();
        let (tx, rx) = mpsc::channel(capacity);
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
            json!({"control": self.root.path().join("control"), "ticket":ticket.to_string()});
        control
            .as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        TaskConfig {
            agent: AgentId::Cursor,
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
            .start(ticket, prepared, agents::backend(AgentId::Cursor).unwrap())
            .unwrap();
    }
    async fn until(&mut self, condition: impl Fn(&[Event]) -> bool) {
        tokio::time::timeout(Duration::from_secs(30), async {
            while !condition(&self.events) {
                self.events.push(self.rx.recv().await.unwrap());
            }
        })
        .await
        .unwrap_or_else(|_| panic!("Cursor lifecycle timed out: {:?}", self.events));
    }
    fn ready(events: &[Event], run: u64) -> bool {
        events.iter().any(|event| matches!(event, Event::Output {run:r, text, ..} if *r == run && text.contains("fixture gate ready")))
    }
    fn finished(events: &[Event], run: u64, expected: JobStatus) -> bool {
        events.iter().any(|event| matches!(event, Event::Finished {run:r, status, ..} if *r == run && *status == expected))
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
async fn lock_released(lock: &fs::File) {
    tokio::time::timeout(Duration::from_secs(10), async {
        while fs2::FileExt::try_lock_exclusive(lock).is_err() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("Cursor descendant retained its OS lock");
}

#[tokio::test]
async fn terminal_failures_keep_results_and_release_slots_in_both_execution_modes() {
    for mode in [ExecutionMode::Direct, ExecutionMode::IsolatedWorktree] {
        for (outcome, expected, diagnostic) in [
            ("success", JobStatus::Succeeded, "successful terminal"),
            ("failure", JobStatus::Failed, "process failed"),
            ("missing_auth", JobStatus::Failed, "authentication"),
            ("malformed", JobStatus::Failed, "Invalid Cursor JSON"),
            ("incomplete", JobStatus::Failed, "incomplete"),
            ("interrupted", JobStatus::Failed, "process failed"),
            ("permission", JobStatus::Failed, "permission"),
            ("recursive_permission", JobStatus::Failed, "permission"),
            ("ambiguous_permission", JobStatus::Failed, "permission"),
            ("permission_exit", JobStatus::Failed, "permission"),
            (
                "wrong_session",
                JobStatus::Failed,
                "session identifier changed",
            ),
        ] {
            let mut h = Harness::new(1, 512);
            let repo = h.repository("source").await;
            let before = command(&repo.path, &["rev-parse", "HEAD"]);
            let task = h.task(
                1,
                mode,
                json!({"edit_before":true, "cursor_outcome":outcome, "fail":outcome == "failure"}),
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
            assert!(h.events.iter().any(|event| matches!(event, Event::Finished {run:1, detail, ..} if detail.contains(diagnostic))), "{outcome}: {:?}", h.events);
            assert_eq!(
                h.events
                    .iter()
                    .filter(|event| matches!(event, Event::Started { run: 1, .. }))
                    .count(),
                1
            );
            let args: Vec<String> = serde_json::from_slice(
                &fs::read(h.root.path().join(format!("control/1-{name}.args"))).unwrap(),
            )
            .unwrap();
            assert!(!args.iter().any(|arg| arg == "--force" || arg == "--trust"));
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
    let mut h = Harness::new(3, 512);
    let source = h.repository("source").await;
    let nested = h.repository("source/nested").await;
    let peer = h.repository("peer").await;
    h.start(
        1,
        h.task(
            1,
            ExecutionMode::Direct,
            json!({"spawn_child":true, "edit_before":true}),
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
    assert!(fs2::FileExt::try_lock_exclusive(&lock).is_ok());
    assert!(
        !h.events
            .iter()
            .any(|event| matches!(event, Event::Finished { run: 3, .. }))
    );
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
async fn concurrent_isolated_cursor_results_are_independent_and_direct_access_waits_for_both() {
    let mut h = Harness::new(3, 512);
    let repo = h.repository("source").await;
    for run in [1, 2] {
        h.start(
            run,
            h.task(
                run,
                ExecutionMode::IsolatedWorktree,
                json!({"edit_before":true}),
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
async fn shutdown_cancels_cursor_process_trees_and_retains_isolated_evidence() {
    let mut h = Harness::new(2, 512);
    let repo = h.repository("source").await;
    let peer = h.repository("peer").await;
    h.start(
        1,
        h.task(
            1,
            ExecutionMode::IsolatedWorktree,
            json!({"spawn_child":true, "edit_before":true}),
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
async fn cursor_timeout_stops_descendants_and_releases_repository_after_confirmed_cleanup() {
    let mut h = Harness::new(2, 512);
    let repo = h.repository("source").await;
    let mut task = h.task(
        1,
        ExecutionMode::IsolatedWorktree,
        json!({"spawn_child":true, "edit_before":true}),
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
    assert!(h.events.iter().any(|event| matches!(event, Event::Finished {run:1, detail,..} if detail.contains("timed out after 3 seconds") && detail.contains("cleanup was confirmed"))));
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
async fn pending_delete_confirmation_times_out_or_cancels_before_releasing_owned_processes() {
    for mode in [ExecutionMode::Direct, ExecutionMode::IsolatedWorktree] {
        for cancel in [false, true] {
            let mut h = Harness::new(1, 512);
            let repo = h.repository("source").await;
            let mut task = h.task(
                1,
                mode,
                json!({
                    "spawn_child":true, "edit_before":true,
                    "cursor_pending_command":"rm -rf -- \"$UNSET_TARGET\"/*"
                }),
            );
            task.options.insert(
                "timeout_seconds".into(),
                if cancel { "60" } else { "3" }.into(),
            );
            h.start(1, task, &repo).await;
            h.until(|events| {
                Harness::ready(events, 1)
                    && events
                        .iter()
                        .filter_map(|event| match event {
                            Event::Output { run: 1, raw, .. } => Some(raw.as_str()),
                            _ => None,
                        })
                        .collect::<String>()
                        .contains("pending confirmation for risky delete")
            })
            .await;
            file_ready(&h.root.path().join("control/child.ready")).await;
            let lock = child_lock(h.root.path());
            assert!(fs2::FileExt::try_lock_exclusive(&lock).is_err());

            // A direct successor needs both capacity and the source lease,
            // including while the first attempt runs in an isolated checkout.
            h.start(2, h.task(2, ExecutionMode::Direct, json!({})), &repo)
                .await;
            assert!(
                !h.events
                    .iter()
                    .any(|event| matches!(event, Event::Started { run: 2, .. }))
            );
            if cancel {
                h.manager.cancel_run(1);
            }
            let expected = if cancel {
                JobStatus::Cancelled
            } else {
                JobStatus::Failed
            };
            h.until(|events| Harness::finished(events, 1, expected) && Harness::ready(events, 2))
                .await;
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
            if !cancel {
                assert!(h.events.iter().any(|event| matches!(event, Event::Finished {run:1, detail,..}
                    if detail.contains("timed out after 3 seconds") && detail.contains("cleanup was confirmed"))));
            }
            let raw: String = h
                .events
                .iter()
                .filter_map(|event| match event {
                    Event::Output { run: 1, raw, .. } => Some(raw.as_str()),
                    _ => None,
                })
                .collect();
            assert!(raw.contains("pending confirmation for risky delete"));
            assert!(raw.contains("UNSET_TARGET"));
            assert!(!raw.contains("\"type\":\"result\""));

            if mode == ExecutionMode::IsolatedWorktree {
                let retained = h.metadata(1);
                assert_eq!(
                    fs::read_to_string(retained.path.join("tracked.txt")).unwrap(),
                    "1"
                );
                assert_eq!(
                    worktrees::inspect(&retained).await.unwrap().changed,
                    Some(true)
                );
                assert!(retained.path.parent().unwrap().join("owner.json").is_file());
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
            h.release(2, "source");
            h.until(|events| Harness::finished(events, 2, JobStatus::Succeeded))
                .await;
            assert_eq!(
                h.events
                    .iter()
                    .filter(|event| matches!(event, Event::Finished { run: 1, .. }))
                    .count(),
                1
            );
            h.close().await;
        }
    }
}

/// Only the test's event-queue setup and cancellation response differ. Cursor's
/// actual command/output/deadline contracts still run through the real manager.
struct SupervisedCursor {
    fill: Option<mpsc::Sender<Event>>,
    refuse_cancel: bool,
    delayed_cancel: Option<Arc<std::sync::atomic::AtomicBool>>,
}
impl AgentBackend for SupervisedCursor {
    fn id(&self) -> AgentId {
        AgentId::Cursor
    }
    fn options(&self) -> &'static [OptionSpec] {
        Cursor.options()
    }
    fn detection(&self, options: &AgentOptions, directory: &Path) -> anyhow::Result<CommandSpec> {
        Cursor.detection(options, directory)
    }
    fn check_detection(&self, help: &str) -> anyhow::Result<()> {
        Cursor.check_detection(help)
    }
    fn build(&self, task: &TaskConfig, repo: &Repository) -> anyhow::Result<CommandSpec> {
        Cursor.build(task, repo)
    }
    fn output(&self) -> Box<dyn AgentOutput> {
        Cursor.output()
    }
    fn output_for(&self, options: &AgentOptions, directory: &Path) -> Box<dyn AgentOutput> {
        Cursor.output_for(options, directory)
    }
    fn execution_timeout(&self, options: &AgentOptions) -> anyhow::Result<Option<Duration>> {
        Cursor.execution_timeout(options)
    }
    fn execution_summary(&self, options: &AgentOptions) -> String {
        Cursor.execution_summary(options)
    }
    fn spawn(&self, spec: &CommandSpec) -> anyhow::Result<ManagedChild> {
        let child = Cursor.spawn(spec)?;
        if let Some(tx) = &self.fill {
            let _ = tx.try_send(Event::Output {
                run: 1,
                job: 0,
                text: "fixture filled event queue".into(),
                raw: String::new(),
            });
        }
        Ok(child)
    }
    fn cancel(&self, child: &mut ManagedChild) -> std::io::Result<()> {
        if let Some(started) = &self.delayed_cancel {
            // Model an accepted termination request whose owned process has
            // not exited yet. The test controls its eventual graceful exit.
            started.store(true, std::sync::atomic::Ordering::Release);
            Ok(())
        } else if self.refuse_cancel {
            Err(std::io::Error::other("fixture cancellation refusal"))
        } else {
            Cursor.cancel(child)
        }
    }
}

#[tokio::test]
async fn timeout_remains_failed_when_stop_or_shutdown_arrives_during_cleanup() {
    use std::sync::atomic::{AtomicBool, Ordering};

    for mode in [ExecutionMode::Direct, ExecutionMode::IsolatedWorktree] {
        for shutdown in [false, true] {
            let mut h = Harness::new(1, 512);
            let repo = h.repository("source").await;
            let mut task = h.task(1, mode, json!({"spawn_child":true, "edit_before":true}));
            task.options.insert("timeout_seconds".into(), "3".into());
            let prepared = runner::prepare(task, vec![repo.clone()]).await.unwrap();
            let cleanup_started = Arc::new(AtomicBool::new(false));
            h.manager
                .start(
                    1,
                    prepared,
                    Arc::new(SupervisedCursor {
                        fill: None,
                        refuse_cancel: false,
                        delayed_cancel: Some(cleanup_started.clone()),
                    }),
                )
                .unwrap();
            h.until(|events| Harness::ready(events, 1)).await;
            file_ready(&h.root.path().join("control/child.ready")).await;
            let lock = child_lock(h.root.path());
            tokio::time::timeout(Duration::from_secs(10), async {
                while !cleanup_started.load(Ordering::Acquire) {
                    tokio::time::sleep(Duration::from_millis(5)).await;
                }
            })
            .await
            .unwrap();
            assert!(fs2::FileExt::try_lock_exclusive(&lock).is_err());
            // The timeout already won; cleanup is now waiting for live native
            // processes. Stop/shutdown must not rewrite that terminal cause.
            if shutdown {
                h.manager.shutdown();
            } else {
                h.manager.cancel_run(1);
            }
            fs::write(h.root.path().join("control/release"), "release descendant").unwrap();
            h.release(
                1,
                if mode == ExecutionMode::Direct {
                    "source"
                } else {
                    "tree"
                },
            );
            h.until(|events| {
                events
                    .iter()
                    .any(|event| matches!(event, Event::Finished { run: 1, .. }))
            })
            .await;
            assert!(h.events.iter().any(|event| matches!(event, Event::Finished {run:1,status:JobStatus::Failed,detail,..} if detail.contains("timed out after 3 seconds") && detail.contains("cleanup was confirmed"))), "{mode:?}, shutdown={shutdown}: {:?}", h.events);
            lock_released(&lock).await;
            if mode == ExecutionMode::IsolatedWorktree {
                assert_eq!(
                    worktrees::inspect(&h.metadata(1)).await.unwrap().changed,
                    Some(true)
                );
                assert_eq!(
                    fs::read_to_string(repo.path.join("tracked.txt")).unwrap(),
                    "base\n"
                );
            }
            assert!(!h.manager.history_protected(1));
            h.close().await;
        }
    }
}

#[tokio::test]
async fn completed_process_is_not_reclassified_when_started_delivery_outlasts_deadline() {
    for (failed, expected, code) in [
        (false, JobStatus::Succeeded, 0),
        (true, JobStatus::Failed, 7),
    ] {
        let mut h = Harness::new(1, 1);
        let repo = h.repository("source").await;
        let mut task = h.task(
            1,
            ExecutionMode::Direct,
            json!({"observe_exit":true, "fail":failed}),
        );
        task.options.insert("timeout_seconds".into(), "3".into());
        let prepared = runner::prepare(task, vec![repo]).await.unwrap();
        h.manager.shutdown();
        (&mut h.manager.join).await.unwrap();
        let (tx, rx) = mpsc::channel(1);
        h.manager = RunManager::new(1, tx.clone());
        h.rx = rx;
        h.manager
            .start(
                1,
                prepared,
                Arc::new(SupervisedCursor {
                    fill: Some(tx),
                    refuse_cancel: false,
                    delayed_cancel: None,
                }),
            )
            .unwrap();
        h.until(|events| {
            events
                .iter()
                .any(|event| matches!(event, Event::Queued { run: 1, .. }))
        })
        .await;
        file_ready(&h.root.path().join("control/parent.ready")).await;
        let lock = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(h.root.path().join("control/parent.lock"))
            .unwrap();
        assert!(fs2::FileExt::try_lock_exclusive(&lock).is_err());
        h.release(1, "source");
        lock_released(&lock).await;
        // The child has exited. Keep the UI queue full beyond the configured
        // deadline; delivery latency must not replace its actual exit/result.
        tokio::time::sleep(Duration::from_secs(4)).await;
        h.until(|events| {
            events
                .iter()
                .any(|event| matches!(event, Event::Finished { run: 1, .. }))
        })
        .await;
        assert!(
            h.events.iter().any(
                |event| matches!(event, Event::Finished {run:1,status,exit_code,detail,..}
            if *status == expected && *exit_code == Some(code) && !detail.contains("timed out"))
            ),
            "{:?}",
            h.events
        );
        h.close().await;
    }
}

#[tokio::test]
async fn deadline_cleans_up_even_when_started_event_is_backpressured() {
    let mut h = Harness::new(1, 1);
    let repo = h.repository("source").await;
    let mut task = h.task(1, ExecutionMode::Direct, json!({"spawn_child":true}));
    task.options.insert("timeout_seconds".into(), "3".into());
    let prepared = runner::prepare(task, vec![repo]).await.unwrap();
    // Replace only this harness's manager to retain the channel's sending end.
    h.manager.shutdown();
    (&mut h.manager.join).await.unwrap();
    let (tx, rx) = mpsc::channel(1);
    h.manager = RunManager::new(1, tx.clone());
    h.rx = rx;
    h.manager
        .start(
            1,
            prepared,
            Arc::new(SupervisedCursor {
                fill: Some(tx),
                refuse_cancel: false,
                delayed_cancel: None,
            }),
        )
        .unwrap();
    h.until(|events| {
        events
            .iter()
            .any(|event| matches!(event, Event::Queued { run: 1, .. }))
    })
    .await;
    file_ready(&h.root.path().join("control/child.ready")).await;
    let lock = child_lock(h.root.path());
    lock_released(&lock).await; // No UI event is drained until OS cleanup is observed.
    h.until(|events| Harness::finished(events, 1, JobStatus::Failed))
        .await;
    assert!(
        h.events.iter().any(
            |event| matches!(event, Event::Finished {detail,..} if detail.contains("timed out"))
        )
    );
    assert_eq!(
        h.events
            .iter()
            .filter(|event| matches!(event, Event::Started { run: 1, .. }))
            .count(),
        1
    );
    h.close().await;
}

#[tokio::test]
async fn closed_started_receiver_awaits_owned_cleanup_without_quarantining_the_repository() {
    let mut h = Harness::new(1, 1);
    let repo = h.repository("source").await;
    let task = h.task(1, ExecutionMode::Direct, json!({"spawn_child":true}));
    let prepared = runner::prepare(task, vec![repo]).await.unwrap();
    h.manager.shutdown();
    (&mut h.manager.join).await.unwrap();
    let (tx, rx) = mpsc::channel(1);
    h.manager = RunManager::new(1, tx.clone());
    h.rx = rx;
    h.manager
        .start(
            1,
            prepared,
            Arc::new(SupervisedCursor {
                fill: Some(tx),
                refuse_cancel: false,
                delayed_cancel: None,
            }),
        )
        .unwrap();
    h.until(|events| {
        events
            .iter()
            .any(|event| matches!(event, Event::Queued { run: 1, .. }))
    })
    .await;
    file_ready(&h.root.path().join("control/child.ready")).await;
    let lock = child_lock(h.root.path());
    assert!(h.manager.history_protected(1));
    h.rx.close(); // No Stop or timeout: publication failure must request owned cleanup.
    lock_released(&lock).await;
    let released = tokio::time::timeout(Duration::from_secs(5), async {
        while h.manager.history_protected(1) {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await;
    h.close().await;
    assert!(
        released.is_ok(),
        "closed UI dropped supervision instead of awaiting owned cleanup"
    );
}

#[tokio::test]
async fn cancellation_interrupts_blocked_started_publication_and_awaits_descendants() {
    let mut h = Harness::new(1, 1);
    let repo = h.repository("source").await;
    let task = h.task(1, ExecutionMode::Direct, json!({"spawn_child":true}));
    let prepared = runner::prepare(task, vec![repo]).await.unwrap();
    h.manager.shutdown();
    (&mut h.manager.join).await.unwrap();
    let (tx, rx) = mpsc::channel(1);
    h.manager = RunManager::new(1, tx.clone());
    h.rx = rx;
    h.manager
        .start(
            1,
            prepared,
            Arc::new(SupervisedCursor {
                fill: Some(tx),
                refuse_cancel: false,
                delayed_cancel: None,
            }),
        )
        .unwrap();
    h.until(|events| {
        events
            .iter()
            .any(|event| matches!(event, Event::Queued { run: 1, .. }))
    })
    .await;
    file_ready(&h.root.path().join("control/child.ready")).await;
    let lock = child_lock(h.root.path());
    h.manager.cancel_run(1);
    lock_released(&lock).await; // Keep the queue blocked until cleanup is observed.
    h.until(|events| Harness::finished(events, 1, JobStatus::Cancelled))
        .await;
    assert!(
        !h.events
            .iter()
            .any(|event| matches!(event, Event::Started { run: 1, .. }))
    );
    h.close().await;
}

#[tokio::test]
async fn unconfirmed_timeout_cleanup_keeps_lease_and_history_protection_even_after_drop_kills_child()
 {
    let mut h = Harness::new(2, 512);
    let repo = h.repository("source").await;
    let mut task = h.task(
        1,
        ExecutionMode::IsolatedWorktree,
        json!({"spawn_child":true}),
    );
    task.options.insert("timeout_seconds".into(), "3".into());
    let prepared = runner::prepare(task, vec![repo.clone()]).await.unwrap();
    h.manager
        .start(
            1,
            prepared,
            Arc::new(SupervisedCursor {
                fill: None,
                refuse_cancel: true,
                delayed_cancel: None,
            }),
        )
        .unwrap();
    h.until(|events| Harness::ready(events, 1)).await;
    file_ready(&h.root.path().join("control/child.ready")).await;
    let lock = child_lock(h.root.path());
    h.start(2, h.task(2, ExecutionMode::Direct, json!({})), &repo)
        .await;
    h.until(|events| {
        Harness::finished(events, 1, JobStatus::Failed)
            && events.iter().any(|event| {
                matches!(
                    event,
                    Event::Queued {
                        run: 2,
                        reason: QueueReason::CleanupFailed(1),
                        ..
                    }
                )
            })
    })
    .await;
    lock_released(&lock).await;
    assert!(h.manager.history_protected(1));
    assert!(
        !h.events
            .iter()
            .any(|event| matches!(event, Event::Started { run: 2, .. }))
    );
    assert!(h.events.iter().any(|event| matches!(event, Event::Finished {run:1,detail,..} if detail.contains("fixture cancellation refusal") && detail.contains("reserved"))));
    assert!(h.metadata(1).path.exists());
    h.manager.cancel_run(2);
    h.until(|events| Harness::finished(events, 2, JobStatus::Cancelled))
        .await;
    h.close().await;
}

#[tokio::test]
async fn executable_removed_after_review_fails_before_launch_without_leaking_capacity() {
    let tools = executable::directory();
    let path: PathBuf = tools
        .path()
        .join("cursor-fixture")
        .with_extension(std::env::consts::EXE_EXTENSION);
    executable::install(&path);
    let mut h = Harness::new(1, 512);
    let repo = h.repository("source").await;
    let mut task = h.task(1, ExecutionMode::Direct, json!({}));
    task.options
        .insert("executable".into(), path.display().to_string());
    let prepared: PreparedRun = runner::prepare(task, vec![repo.clone()]).await.unwrap();
    fs::remove_file(path).unwrap();
    h.manager
        .start(1, prepared, agents::backend(AgentId::Cursor).unwrap())
        .unwrap();
    h.until(|events| Harness::finished(events, 1, JobStatus::Failed))
        .await;
    assert!(
        !h.events
            .iter()
            .any(|event| matches!(event, Event::Started { run: 1, .. }))
    );
    assert!(h.events.iter().any(|event| matches!(event, Event::Finished {run:1,detail,..} if detail.contains("Cannot start"))));
    h.start(2, h.task(2, ExecutionMode::Direct, json!({})), &repo)
        .await;
    h.until(|events| Harness::ready(events, 2)).await;
    h.release(2, "source");
    h.until(|events| Harness::finished(events, 2, JobStatus::Succeeded))
        .await;
    h.close().await;
}
