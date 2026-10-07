#![cfg(feature = "test-support")]
#![allow(clippy::unwrap_used)]
use codeconvoy::{
    agents,
    attachments::Attachment,
    domain::*,
    git,
    persistence::Store,
    process::Cancellation,
    runner::{self, Event, PreparedRun, RunManager},
    worktrees,
};
use std::{
    collections::BTreeMap, fs, path::Path, process::Command, sync::atomic::AtomicBool,
    time::Duration,
};
use tokio::sync::mpsc;

fn command(path: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .args(["-c", "commit.gpgsign=false"])
        .arg("-C")
        .arg(path)
        .args(args)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{:?}: {}",
        args,
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).unwrap().trim().to_owned()
}
async fn repository(root: &Path, name: &str) -> Repository {
    let path = root.join(name);
    fs::create_dir(&path).unwrap();
    command(&path, &["init", "--quiet"]);
    command(&path, &["config", "core.autocrlf", "false"]);
    fs::write(path.join("tracked.txt"), "committed base\n").unwrap();
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
struct Harness {
    root: tempfile::TempDir,
    manager: RunManager,
    rx: mpsc::Receiver<Event>,
    events: Vec<Event>,
}
impl Harness {
    fn new(limit: usize) -> Self {
        let root = tempfile::Builder::new()
            .prefix("convoy ü worktrees ")
            .tempdir()
            .unwrap();
        fs::create_dir(root.path().join("control")).unwrap();
        let (tx, rx) = mpsc::channel(256);
        let manager = RunManager::with_worktree_directory(
            limit,
            tx,
            root.path().join("retained worktrees ü"),
        );
        Self {
            root,
            manager,
            rx,
            events: vec![],
        }
    }
    fn task(&self, run: u64, agent: AgentId, edit: bool, fail: bool) -> TaskConfig {
        TaskConfig {
            prompt: format!(
                "codeconvoy-fixture-gate\n{}",
                serde_json::json!({"control":self.root.path().join("control"), "ticket":run.to_string(), "edit_before":edit, "fail":fail})
            ),
            agent,
            execution_mode: ExecutionMode::IsolatedWorktree,
            options: [(
                "executable".into(),
                env!("CARGO_BIN_EXE_codeconvoy-test-agent").into(),
            )]
            .into(),
            ..Default::default()
        }
    }
    async fn start(&mut self, run: u64, task: TaskConfig, repos: Vec<Repository>) -> Run {
        let prepared = runner::prepare(task, repos).await.unwrap();
        let snapshot = prepared.snapshot(run);
        self.start_prepared(run, prepared);
        snapshot
    }
    fn start_prepared(&mut self, run: u64, prepared: PreparedRun) {
        let backend = agents::backend(prepared.task.agent).unwrap();
        self.manager.start(run, prepared, backend).unwrap();
    }
    async fn until(&mut self, predicate: impl Fn(&[Event]) -> bool) {
        tokio::time::timeout(Duration::from_secs(30), async {
            while !predicate(&self.events) {
                self.events.push(self.rx.recv().await.unwrap());
            }
        })
        .await
        .unwrap();
    }
    fn ready(events: &[Event], run: u64) -> bool {
        events.iter().any(|e| matches!(e, Event::Output {run:r,text,..} if *r == run && text.contains("fixture gate ready")))
    }
    fn finished(events: &[Event], run: u64, status: JobStatus) -> bool {
        events
            .iter()
            .any(|e| matches!(e, Event::Finished {run:r,status:s,..} if *r == run && *s == status))
    }
    fn metadata(&self, run: u64) -> WorktreeMetadata {
        self.events
            .iter()
            .find_map(|e| match e {
                Event::Preparing {
                    run: r,
                    worktree: Some(w),
                    ..
                } if *r == run => Some(w.clone()),
                _ => None,
            })
            .unwrap()
    }
    fn release(&self, run: u64, name: &str) {
        fs::write(
            self.root
                .path()
                .join("control")
                .join(format!("{run}-{name}.release")),
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

#[test]
fn old_state_defaults_direct_and_reuse_only_copies_mode() {
    let state: AppState = serde_json::from_str(r#"{"version":1,"draft":{"prompt":"v0.2"},"runs":[{"id":1,"created_at":0,"task":{"prompt":"old"},"jobs":[{"repository":{"name":"x","path":"/x"},"status":"succeeded","started_at":null,"finished_at":null,"exit_code":0,"before":null,"interrupted":false}]}]}"#).unwrap();
    assert_eq!(state.draft.execution_mode, ExecutionMode::Direct);
    assert_eq!(state.runs[0].task.execution_mode, ExecutionMode::Direct);
    assert_eq!(state.runs[0].jobs[0].execution_mode, ExecutionMode::Direct);
    assert!(state.runs[0].jobs[0].worktree.is_none());
    let mut state = state;
    state.reuse_task(TaskConfig {
        execution_mode: ExecutionMode::IsolatedWorktree,
        ..Default::default()
    });
    assert_eq!(state.draft.execution_mode, ExecutionMode::IsolatedWorktree);
}

#[tokio::test]
async fn dirty_source_concurrent_same_repository_jobs_keep_independent_retained_results_and_stable_diff()
 {
    let mut h = Harness::new(2);
    let repo = repository(h.root.path(), "source ü with spaces").await;
    let base = command(&repo.path, &["rev-parse", "HEAD"]);
    fs::write(repo.path.join("tracked.txt"), "local dirty change\n").unwrap();
    fs::write(repo.path.join("untracked.txt"), "local untracked\n").unwrap();
    let before = git::diff(&repo.path).await.unwrap();
    let first = h
        .start(
            1,
            h.task(1, AgentId::Codex, false, false),
            vec![repo.clone()],
        )
        .await;
    let second = h
        .start(
            2,
            h.task(2, AgentId::Copilot, false, false),
            vec![repo.clone()],
        )
        .await;
    assert_eq!(first.task.execution_mode, ExecutionMode::IsolatedWorktree);
    assert_eq!(
        second.jobs[0].execution_mode,
        ExecutionMode::IsolatedWorktree
    );
    h.until(|e| Harness::ready(e, 1) && Harness::ready(e, 2))
        .await;
    let a = h.metadata(1);
    let b = h.metadata(2);
    assert_ne!(a.path, b.path);
    assert_eq!(a.base_commit, base);
    assert_eq!(b.base_commit, base);
    assert_eq!(a.common_dir, git::common_dir(&repo.path).await.unwrap());
    assert_eq!(git::common_dir(&a.path).await.unwrap(), a.common_dir);
    assert_eq!(a.repository, repo);
    assert_eq!((a.run, a.job), (1, 0));
    let recorded: WorktreeMetadata =
        serde_json::from_slice(&fs::read(a.path.parent().unwrap().join("owner.json")).unwrap())
            .unwrap();
    assert_eq!(recorded, a);
    for w in [&a, &b] {
        assert_eq!(
            fs::read_to_string(w.path.join("tracked.txt")).unwrap(),
            "committed base\n"
        );
        assert!(!w.path.join("untracked.txt").exists());
        assert!(command(&w.path, &["status", "--porcelain"]).is_empty());
    }
    fs::write(a.path.join("tracked.txt"), "isolated one\n").unwrap();
    let diff = worktrees::diff(&a).await.unwrap();
    assert!(diff.contains("-committed base") && diff.contains("+isolated one"));
    assert_eq!(git::diff(&repo.path).await.unwrap(), before);
    // Advance the source HEAD and dirty state after the isolated snapshot.
    command(&repo.path, &["add", "tracked.txt"]);
    command(
        &repo.path,
        &[
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=fixture@example.invalid",
            "commit",
            "-qm",
            "independent source edit",
        ],
    );
    fs::write(repo.path.join("tracked.txt"), "new dirty state\n").unwrap();
    assert_eq!(worktrees::diff(&a).await.unwrap(), diff);
    h.release(1, "tree");
    h.release(2, "tree");
    h.until(|e| {
        Harness::finished(e, 1, JobStatus::Succeeded)
            && Harness::finished(e, 2, JobStatus::Succeeded)
    })
    .await;
    assert!(worktrees::inspect(&a).await.unwrap().changed.unwrap());
    assert_eq!(worktrees::inspect(&b).await.unwrap().changed, Some(false));
    assert!(a.path.is_dir() && b.path.is_dir());
    assert_eq!(
        fs::read_to_string(repo.path.join("tracked.txt")).unwrap(),
        "new dirty state\n"
    );
    // Reuse snapshots the setting and creates an entirely fresh attempt.
    let mut task = first.task;
    task.prompt = h.task(3, AgentId::Codex, false, false).prompt;
    h.start(3, task, vec![repo]).await;
    h.until(|e| Harness::ready(e, 3)).await;
    assert_ne!(h.metadata(3).path, a.path);
    h.release(3, "tree");
    h.until(|e| Harness::finished(e, 3, JobStatus::Succeeded))
        .await;
    h.close().await;
}

#[tokio::test]
async fn all_backends_preserve_invocations_attachments_and_failed_cancelled_results() {
    for (index, agent) in AgentId::ALL.into_iter().enumerate() {
        let mut h = Harness::new(1);
        let repo = repository(h.root.path(), "source").await;
        let before = git::diff(&repo.path).await.unwrap();
        let attachment_path = h.root.path().join("context ü.txt");
        fs::write(&attachment_path, "attachment unchanged sentinel").unwrap();
        let mut task = h.task(1, agent, true, index == 1);
        task.attachments
            .push(Attachment::inspect(&attachment_path).unwrap());
        let original_spec = agents::backend(agent).unwrap().build(&task, &repo).unwrap();
        h.start(1, task.clone(), vec![repo.clone()]).await;
        h.until(|e| Harness::ready(e, 1)).await;
        let metadata = h.metadata(1);
        let execution = Repository {
            path: metadata.path.clone(),
            name: repo.name.clone(),
        };
        let isolated_spec = agents::backend(agent)
            .unwrap()
            .build(&task, &execution)
            .unwrap();
        assert_eq!(isolated_spec.directory, metadata.path);
        assert_eq!(isolated_spec.input, original_spec.input);
        let args: Vec<String> =
            serde_json::from_slice(&fs::read(h.root.path().join("control/1-tree.args")).unwrap())
                .unwrap();
        assert_eq!(
            &args[1..],
            isolated_spec
                .args
                .iter()
                .map(|s| s.to_str().unwrap())
                .collect::<Vec<_>>()
        );
        assert_eq!(
            fs::read_to_string(h.root.path().join("control/1-tree.cwd")).unwrap(),
            metadata.path.to_str().unwrap()
        );
        assert!(
            fs::read_to_string(h.root.path().join("control/1-tree.input"))
                .unwrap()
                .contains("attachment unchanged sentinel")
        );
        assert!(!metadata.path.join("context ü.txt").exists());
        if index >= 2 {
            h.manager.cancel_run(1);
        } else {
            h.release(1, "tree");
        }
        let expected = if index >= 2 {
            JobStatus::Cancelled
        } else if index == 1 {
            JobStatus::Failed
        } else {
            JobStatus::Succeeded
        };
        h.until(|e| Harness::finished(e, 1, expected)).await;
        assert_eq!(
            fs::read_to_string(metadata.path.join("tracked.txt")).unwrap(),
            "1"
        );
        assert_eq!(
            worktrees::inspect(&metadata).await.unwrap().changed,
            Some(true)
        );
        assert!(h.events.iter().any(|e| matches!(
            e,
            Event::Result {
                result: Some(WorktreeResult {
                    exists: true,
                    changed: Some(true),
                    ..
                }),
                ..
            }
        )));
        assert_eq!(git::diff(&repo.path).await.unwrap(), before);
        h.close().await;
    }
}

#[tokio::test]
async fn preparation_failure_has_no_direct_fallback_and_other_jobs_continue() {
    let mut h = Harness::new(2);
    let bad = repository(h.root.path(), "bad").await;
    let good = repository(h.root.path(), "good").await;
    let mut prepared = runner::prepare(h.task(1, AgentId::Codex, false, false), vec![bad.clone()])
        .await
        .unwrap();
    prepared.repositories[0].state.summary.head = Some("a".repeat(40));
    h.start_prepared(1, prepared);
    h.start(2, h.task(2, AgentId::Codex, false, false), vec![good])
        .await;
    h.until(|e| Harness::finished(e, 1, JobStatus::Failed) && Harness::ready(e, 2))
        .await;
    assert!(
        !h.events
            .iter()
            .any(|e| matches!(e, Event::Started { run: 1, .. }))
    );
    assert!(h.events.iter().any(|e| matches!(e, Event::Finished{run:1,detail,..} if detail.contains("Base commit is unavailable"))));
    assert!(git::status(&bad.path).await.unwrap().entries.is_empty());
    assert!(
        h.metadata(1)
            .path
            .parent()
            .unwrap()
            .join("owner.json")
            .is_file()
    );
    h.release(2, "tree");
    h.until(|e| Harness::finished(e, 2, JobStatus::Succeeded))
        .await;
    h.start(3, h.task(3, AgentId::Codex, false, false), vec![bad])
        .await;
    h.until(|e| Harness::ready(e, 3)).await;
    h.release(3, "tree");
    h.until(|e| Harness::finished(e, 3, JobStatus::Succeeded))
        .await;
    h.close().await;
}

#[tokio::test]
async fn global_and_convoy_limits_cover_preparation_and_execution_and_revalidate_attachments() {
    let mut h = Harness::new(2);
    let a = repository(h.root.path(), "a").await;
    let b = repository(h.root.path(), "b").await;
    let c = repository(h.root.path(), "c").await;
    let mut task = h.task(1, AgentId::Codex, false, false);
    task.concurrency = 1;
    h.start(1, task, vec![a.clone(), b]).await;
    h.start(2, h.task(2, AgentId::Copilot, false, false), vec![a])
        .await;
    let mut queued = h.task(3, AgentId::Codex, false, false);
    let context = h.root.path().join("queued.txt");
    fs::write(&context, "before").unwrap();
    queued
        .attachments
        .push(Attachment::inspect(&context).unwrap());
    h.start(3, queued, vec![c]).await;
    h.until(|e| {
        Harness::ready(e, 1)
            && Harness::ready(e, 2)
            && e.iter().any(|e| matches!(e, Event::Queued { run: 3, .. }))
    })
    .await;
    assert!(!h.events.iter().any(|e| matches!(
        e,
        Event::Preparing { run: 3, .. } | Event::Preparing { run: 1, job: 1, .. }
    )));
    fs::write(context, "changed").unwrap();
    h.release(1, "tree");
    h.release(2, "tree");
    h.until(|e| {
        Harness::finished(e, 3, JobStatus::Failed)
            && e.iter()
                .filter(|e| matches!(e, Event::Finished { .. }))
                .count()
                == 4
    })
    .await;
    assert!(
        !h.events
            .iter()
            .any(|e| matches!(e, Event::Started { run: 3, .. }))
    );
    let mut active = BTreeMap::<(u64, usize), bool>::new();
    for event in &h.events {
        match event {
            Event::Preparing { run, job, .. } => {
                active.insert((*run, *job), true);
            }
            Event::Finished { run, job, .. } => {
                active.remove(&(*run, *job));
            }
            _ => {}
        }
        assert!(active.len() <= 2);
        assert!(active.keys().filter(|(run, _)| *run == 1).count() <= 1);
    }
    h.close().await;
}

#[tokio::test]
async fn ownership_collision_cancellation_and_missing_repository_are_explicit() {
    let temp = tempfile::tempdir().unwrap();
    let repo = repository(temp.path(), "repo").await;
    let summary = git::status(&repo.path).await.unwrap().summary;
    let root = temp.path().join("storage");
    let reserve = || {
        worktrees::reserve(
            &root,
            1,
            0,
            repo.clone(),
            summary.common_dir.clone().unwrap(),
            summary.head.clone().unwrap(),
        )
        .unwrap()
    };
    let a = reserve();
    let b = reserve();
    assert_ne!(a.path, b.path);
    fs::create_dir(&a.path).unwrap();
    fs::write(a.path.join("user-data"), "keep").unwrap();
    let safe = AtomicBool::new(true);
    assert!(
        worktrees::create(&a, &Cancellation::default(), &safe)
            .await
            .unwrap_err()
            .to_string()
            .contains("destination")
    );
    assert_eq!(
        fs::read_to_string(a.path.join("user-data")).unwrap(),
        "keep"
    );
    let cancel = Cancellation::default();
    cancel.cancel();
    assert!(worktrees::create(&b, &cancel, &safe).await.is_err());
    assert!(b.path.parent().unwrap().join("owner.json").exists());
    assert!(!b.path.exists());
    let c = reserve();
    fs::write(c.path.parent().unwrap().join("owner.json"), "{}").unwrap();
    assert!(
        worktrees::create(&c, &Cancellation::default(), &safe)
            .await
            .is_err()
    );
    let d = reserve();
    fs::rename(&repo.path, temp.path().join("moved")).unwrap();
    assert!(
        worktrees::create(&d, &Cancellation::default(), &safe)
            .await
            .is_err()
    );
    let blocked_root = temp.path().join("file");
    fs::write(&blocked_root, "keep").unwrap();
    assert!(
        worktrees::reserve(
            &blocked_root,
            2,
            0,
            repo,
            summary.common_dir.unwrap(),
            summary.head.unwrap()
        )
        .is_err()
    );
}

#[tokio::test]
async fn reservation_rejects_linked_storage_without_creating_an_attempt() {
    let temp = tempfile::tempdir().unwrap();
    let repo = repository(temp.path(), "repo").await;
    let summary = git::status(&repo.path).await.unwrap().summary;
    let root = temp.path().join("storage link");
    let target = temp.path().join("target ü");
    fs::create_dir(&target).unwrap();
    for dangling in [false, true] {
        #[cfg(unix)]
        std::os::unix::fs::symlink(&target, &root).unwrap();
        #[cfg(windows)]
        std::os::windows::fs::symlink_dir(&target, &root).unwrap();
        if dangling {
            fs::remove_dir(&target).unwrap();
        }
        let error = worktrees::reserve(
            &root,
            1,
            0,
            repo.clone(),
            summary.common_dir.clone().unwrap(),
            summary.head.clone().unwrap(),
        )
        .unwrap_err();
        assert!(error.to_string().contains("Cannot use worktree storage"));
        assert!(
            fs::symlink_metadata(&root)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        if dangling {
            assert!(!target.exists());
        } else {
            assert_eq!(fs::read_dir(&target).unwrap().count(), 0);
        }
        #[cfg(unix)]
        fs::remove_file(&root).unwrap();
        #[cfg(windows)]
        fs::remove_dir(&root).unwrap();
    }
    // Refusal must not poison later reservation at an ordinary storage root.
    let m = worktrees::reserve(
        &root,
        1,
        0,
        repo,
        summary.common_dir.unwrap(),
        summary.head.unwrap(),
    )
    .unwrap();
    assert!(m.path.parent().unwrap().join("owner.json").is_file());
    assert!(!m.path.exists());
}

#[tokio::test]
async fn retained_metadata_roundtrips_and_preparing_recovers_as_interrupted_without_deleting_results()
 {
    let temp = tempfile::tempdir().unwrap();
    let repo = repository(temp.path(), "repo").await;
    let status = git::status(&repo.path).await.unwrap();
    let w = worktrees::reserve(
        &temp.path().join("storage"),
        1,
        0,
        repo.clone(),
        status.summary.common_dir.unwrap(),
        status.summary.head.unwrap(),
    )
    .unwrap();
    worktrees::create(&w, &Cancellation::default(), &AtomicBool::new(true))
        .await
        .unwrap();
    let mut job = Job::queued(repo);
    job.status = JobStatus::Preparing;
    job.execution_mode = ExecutionMode::IsolatedWorktree;
    job.worktree = Some(w.clone());
    let state = AppState {
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
    let store = Store::open(&temp.path().join("state")).unwrap();
    store.save(&state).unwrap();
    let mut loaded = store.load().unwrap();
    assert_eq!(loaded.runs[0].jobs[0].worktree, Some(w.clone()));
    assert!(loaded.runs[0].jobs[0].interrupted);
    assert_eq!(loaded.runs[0].status(), JobStatus::Cancelled);
    loaded.clear_history();
    assert!(w.path.is_dir());
}

#[cfg(unix)]
#[tokio::test]
async fn cancellation_during_git_checkout_terminates_its_descendants_and_retains_ownership() {
    use fs2::FileExt;
    let mut h = Harness::new(1);
    let repo = repository(h.root.path(), "source").await;
    fs::write(
        repo.path.join(".gitattributes"),
        "tracked.txt filter=convoygate\n",
    )
    .unwrap();
    command(&repo.path, &["add", ".gitattributes"]);
    command(
        &repo.path,
        &[
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=fixture@example.invalid",
            "commit",
            "-qm",
            "filter fixture",
        ],
    );
    let quote = |s: &str| format!("'{}'", s.replace('\'', "'\\''"));
    let filter = format!(
        "{} smudge-gate {}",
        quote(env!("CARGO_BIN_EXE_codeconvoy-test-agent")),
        quote(h.root.path().join("control").to_str().unwrap())
    );
    command(&repo.path, &["config", "filter.convoygate.smudge", &filter]);
    command(
        &repo.path,
        &["config", "filter.convoygate.required", "true"],
    );
    command(&repo.path, &["config", "filter.convoygate.clean", "cat"]);
    let original = git::diff(&repo.path).await.unwrap();
    h.start(
        1,
        h.task(1, AgentId::Codex, false, false),
        vec![repo.clone()],
    )
    .await;
    h.until(|e| {
        e.iter().any(|e| {
            matches!(
                e,
                Event::Preparing {
                    worktree: Some(_),
                    ..
                }
            )
        })
    })
    .await;
    tokio::time::timeout(Duration::from_secs(15), async {
        while !h.root.path().join("control/filter.ready").exists() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    assert!(!h.events.iter().any(|e| matches!(e, Event::Started { .. })));
    h.manager.cancel_run(1);
    h.until(|e| Harness::finished(e, 1, JobStatus::Cancelled))
        .await;
    let lock = fs::OpenOptions::new()
        .write(true)
        .open(h.root.path().join("control/filter.lock"))
        .unwrap();
    lock.try_lock_exclusive()
        .expect("Git filter descendant must be dead before cancellation finishes");
    let metadata = h.metadata(1);
    assert!(metadata.path.parent().unwrap().join("owner.json").is_file());
    assert!(!h.root.path().join("control/filter.survived").exists());
    assert_eq!(git::diff(&repo.path).await.unwrap(), original);
    command(
        &repo.path,
        &["config", "--unset", "filter.convoygate.smudge"],
    );
    command(
        &repo.path,
        &["config", "--unset", "filter.convoygate.required"],
    );
    // Capacity and administration are released after confirmed cancellation.
    h.start(2, h.task(2, AgentId::Codex, false, false), vec![repo])
        .await;
    h.until(|e| Harness::ready(e, 2)).await;
    h.release(2, "tree");
    h.until(|e| Harness::finished(e, 2, JobStatus::Succeeded))
        .await;
    h.close().await;
}

#[tokio::test]
async fn direct_jobs_on_linked_worktrees_share_repository_identity_and_wait_for_isolated_jobs() {
    let mut h = Harness::new(2);
    let repo = repository(h.root.path(), "source").await;
    let state = git::status(&repo.path).await.unwrap();
    let retained = worktrees::reserve(
        &h.root.path().join("old-result"),
        99,
        0,
        repo.clone(),
        state.summary.common_dir.unwrap(),
        state.summary.head.unwrap(),
    )
    .unwrap();
    worktrees::create(&retained, &Cancellation::default(), &AtomicBool::new(true))
        .await
        .unwrap();
    h.start(1, h.task(1, AgentId::Codex, false, false), vec![repo])
        .await;
    h.until(|e| Harness::ready(e, 1)).await;
    let linked = git::register(&retained.path).await.unwrap();
    let mut direct = h.task(2, AgentId::Codex, false, false);
    direct.execution_mode = ExecutionMode::Direct;
    h.start(2, direct, vec![linked]).await;
    h.until(|e| {
        e.iter().any(|e| {
            matches!(
                e,
                Event::Queued {
                    run: 2,
                    reason: QueueReason::Repository(1),
                    ..
                }
            )
        })
    })
    .await;
    assert!(
        !h.events
            .iter()
            .any(|e| matches!(e, Event::Started { run: 2, .. }))
    );
    h.release(1, "tree");
    h.until(|e| Harness::ready(e, 2)).await;
    h.release(2, "tree");
    h.until(|e| Harness::finished(e, 2, JobStatus::Succeeded))
        .await;
    h.close().await;
}

#[cfg(unix)]
#[tokio::test]
async fn preparation_does_not_execute_source_post_checkout_hooks_or_create_branches() {
    use std::os::unix::fs::PermissionsExt;
    let temp = tempfile::tempdir().unwrap();
    let repo = repository(temp.path(), "source").await;
    let hook = repo.path.join(".git/hooks/post-checkout");
    fs::write(&hook, "#!/bin/sh\nexit 73\n").unwrap();
    fs::set_permissions(&hook, fs::Permissions::from_mode(0o755)).unwrap();
    let refs = command(&repo.path, &["show-ref"]);
    let state = git::status(&repo.path).await.unwrap();
    let w = worktrees::reserve(
        &temp.path().join("storage"),
        1,
        0,
        repo.clone(),
        state.summary.common_dir.unwrap(),
        state.summary.head.unwrap(),
    )
    .unwrap();
    worktrees::create(&w, &Cancellation::default(), &AtomicBool::new(true))
        .await
        .unwrap();
    assert_eq!(command(&repo.path, &["show-ref"]), refs);
    assert_eq!(
        command(&w.path, &["rev-parse", "--abbrev-ref", "HEAD"]),
        "HEAD"
    );
    // Even an agent commit is compared against the original base, not new HEAD.
    fs::write(w.path.join("tracked.txt"), "agent commit\n").unwrap();
    command(&w.path, &["add", "tracked.txt"]);
    command(
        &w.path,
        &[
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=fixture@example.invalid",
            "commit",
            "-qm",
            "fixture agent commit",
        ],
    );
    assert_eq!(worktrees::inspect(&w).await.unwrap().changed, Some(true));
    assert!(worktrees::diff(&w).await.unwrap().contains("+agent commit"));
    fs::write(w.path.join("tracked.txt"), "committed base\n").unwrap();
    assert_eq!(worktrees::inspect(&w).await.unwrap().changed, Some(true));
    command(&w.path, &["add", "tracked.txt"]);
    assert_eq!(worktrees::inspect(&w).await.unwrap().changed, Some(false));
}

#[cfg(unix)]
fn checkout_gate(repo: &Repository, control: &Path) {
    fs::write(
        repo.path.join(".gitattributes"),
        "tracked.txt filter=convoygate\n",
    )
    .unwrap();
    command(&repo.path, &["add", ".gitattributes"]);
    command(
        &repo.path,
        &[
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=fixture@example.invalid",
            "commit",
            "-qm",
            "filter fixture",
        ],
    );
    // Git filter configuration requires shell quoting. Production Git arguments
    // never use a shell; this fixture deliberately holds checkout in progress.
    let quote = |s: &str| format!("'{}'", s.replace('\'', "'\\''"));
    let binary = quote(env!("CARGO_BIN_EXE_codeconvoy-test-agent"));
    command(
        &repo.path,
        &[
            "config",
            "filter.convoygate.smudge",
            &format!("{binary} smudge-gate {}", quote(control.to_str().unwrap())),
        ],
    );
    command(
        &repo.path,
        &["config", "filter.convoygate.required", "true"],
    );
    command(
        &repo.path,
        &[
            "config",
            "filter.convoygate.clean",
            &format!("{binary} passthrough"),
        ],
    );
}
#[cfg(unix)]
async fn wait_file(path: &Path) {
    tokio::time::timeout(Duration::from_secs(15), async {
        while !path.exists() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
}

#[cfg(unix)]
#[tokio::test]
async fn stop_scopes_and_shutdown_cancel_checkout_without_starting_queued_agents() {
    use fs2::FileExt;
    for scope in ["job", "convoy", "all", "shutdown"] {
        let mut h = Harness::new(1);
        let repo = repository(h.root.path(), "source").await;
        let control = h.root.path().join("control");
        checkout_gate(&repo, &control);
        let before = git::diff(&repo.path).await.unwrap();
        h.start(
            1,
            h.task(1, AgentId::Codex, false, false),
            vec![repo.clone()],
        )
        .await;
        wait_file(&control.join("filter.ready")).await;
        h.start(
            2,
            h.task(2, AgentId::Codex, false, false),
            vec![repo.clone()],
        )
        .await;
        h.until(|events| {
            events.iter().any(|e| {
                matches!(
                    e,
                    Event::Queued {
                        run: 2,
                        reason: QueueReason::GlobalLimit,
                        ..
                    }
                )
            })
        })
        .await;
        match scope {
            "job" => h.manager.cancel(1, 0),
            "convoy" => h.manager.cancel_run(1),
            "all" => h.manager.cancel_all(),
            "shutdown" => h.manager.shutdown(),
            _ => unreachable!(),
        }
        h.until(|e| Harness::finished(e, 1, JobStatus::Cancelled))
            .await;
        let metadata = h.metadata(1);
        assert!(metadata.path.parent().unwrap().join("owner.json").is_file());
        assert!(
            !h.events
                .iter()
                .any(|e| matches!(e, Event::Started { run: 1, .. }))
        );
        let lock = fs::OpenOptions::new()
            .write(true)
            .open(control.join("filter.lock"))
            .unwrap();
        // The peer may now enter checkout for a single-job/convoy stop. In those
        // cases release its filter and prove forward progress instead.
        if matches!(scope, "all" | "shutdown") {
            h.until(|e| Harness::finished(e, 2, JobStatus::Cancelled))
                .await;
            lock.try_lock_exclusive().unwrap();
            assert!(!h.events.iter().any(|e| matches!(
                e,
                Event::Preparing { run: 2, .. } | Event::Started { run: 2, .. }
            )));
            assert!(h.manager.is_idle());
        }
        drop(lock);
        fs::write(control.join("filter.release"), "go").unwrap();
        if matches!(scope, "job" | "convoy") {
            h.until(|e| Harness::ready(e, 2)).await;
            h.release(2, "tree");
            h.until(|e| Harness::finished(e, 2, JobStatus::Succeeded))
                .await;
        }
        if scope == "all" {
            // Stop All releases capacity and does not close future admission.
            h.start(
                3,
                h.task(3, AgentId::Codex, false, false),
                vec![repo.clone()],
            )
            .await;
            h.until(|e| Harness::ready(e, 3)).await;
            h.release(3, "tree");
            h.until(|e| Harness::finished(e, 3, JobStatus::Succeeded))
                .await;
        }
        assert_eq!(git::diff(&repo.path).await.unwrap(), before);
        assert!(h.events.iter().any(|e| matches!(e, Event::Output {run:1,text,raw,..} if text.trim() == "[CodeConvoy] Preparation cancelled" && raw.is_empty())));
        if scope == "shutdown" {
            let prepared = runner::prepare(h.task(3, AgentId::Codex, false, false), vec![repo])
                .await
                .unwrap();
            assert!(
                h.manager
                    .start(3, prepared, agents::backend(AgentId::Codex).unwrap())
                    .is_err()
            );
        }
        h.close().await;
    }
}

#[cfg(unix)]
#[tokio::test]
async fn preparation_collision_cancelled_waiter_releases_slots_without_affecting_owner() {
    let mut h = Harness::new(2);
    let repo = repository(h.root.path(), "source").await;
    let control = h.root.path().join("control");
    checkout_gate(&repo, &control);
    let before = git::diff(&repo.path).await.unwrap();
    h.start(
        1,
        h.task(1, AgentId::Codex, false, false),
        vec![repo.clone()],
    )
    .await;
    wait_file(&control.join("filter.ready")).await;
    h.start(
        2,
        h.task(2, AgentId::Codex, false, false),
        vec![repo.clone()],
    )
    .await;
    h.until(|events| {
        events.iter().any(|e| {
            matches!(
                e,
                Event::Preparing {
                    run: 2,
                    worktree: None,
                    ..
                }
            )
        })
    })
    .await;
    h.manager.cancel(2, 0);
    h.until(|e| Harness::finished(e, 2, JobStatus::Cancelled))
        .await;
    assert!(!h.events.iter().any(|e| matches!(
        e,
        Event::Preparing {
            run: 2,
            worktree: Some(_),
            ..
        } | Event::Started { run: 2, .. }
    )));
    assert!(h.manager.is_active(1));
    let mut direct = h.task(3, AgentId::Codex, false, false);
    direct.execution_mode = ExecutionMode::Direct;
    h.start(3, direct, vec![repo.clone()]).await;
    h.until(|events| {
        events.iter().any(|e| {
            matches!(
                e,
                Event::Queued {
                    run: 3,
                    reason: QueueReason::Repository(1),
                    ..
                }
            )
        })
    })
    .await;
    fs::write(control.join("filter.release"), "go").unwrap();
    h.until(|e| Harness::ready(e, 1)).await;
    h.release(1, "tree");
    h.until(|e| Harness::finished(e, 1, JobStatus::Succeeded) && Harness::ready(e, 3))
        .await;
    h.release(3, "source");
    h.until(|e| Harness::finished(e, 3, JobStatus::Succeeded))
        .await;
    assert_eq!(git::diff(&repo.path).await.unwrap(), before);
    h.close().await;
}

#[tokio::test]
async fn cancelled_changed_agent_keeps_result_and_same_repository_peer_continues() {
    let mut h = Harness::new(2);
    let repo = repository(h.root.path(), "source").await;
    let original = git::diff(&repo.path).await.unwrap();
    for run in [1, 2] {
        h.start(
            run,
            h.task(run, AgentId::Codex, true, false),
            vec![repo.clone()],
        )
        .await;
    }
    h.until(|e| Harness::ready(e, 1) && Harness::ready(e, 2))
        .await;
    h.manager.cancel(1, 0);
    h.until(|e| Harness::finished(e, 1, JobStatus::Cancelled))
        .await;
    assert!(h.manager.is_active(2));
    assert!(
        !h.events
            .iter()
            .any(|e| matches!(e, Event::Finished { run: 2, .. }))
    );
    assert!(h.events.iter().any(|e| matches!(
        e,
        Event::Result {
            run: 1,
            result: Some(WorktreeResult {
                changed: Some(true),
                observed_this_session: true,
                ..
            }),
            ..
        }
    )));
    h.release(2, "tree");
    h.until(|e| Harness::finished(e, 2, JobStatus::Succeeded))
        .await;
    for run in [1, 2] {
        assert_eq!(
            fs::read_to_string(h.metadata(run).path.join("tracked.txt")).unwrap(),
            run.to_string()
        );
    }
    assert_eq!(git::diff(&repo.path).await.unwrap(), original);
    h.close().await;
}

#[tokio::test]
async fn git_change_detection_includes_index_untracked_binary_and_commits_but_excludes_ignored_files()
 {
    let h = Harness::new(1);
    let repo = repository(h.root.path(), "source").await;
    fs::write(repo.path.join(".gitignore"), "ignored.txt\n").unwrap();
    command(&repo.path, &["add", ".gitignore"]);
    command(
        &repo.path,
        &[
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=fixture@example.invalid",
            "commit",
            "-qm",
            "ignore fixture",
        ],
    );
    let state = git::status(&repo.path).await.unwrap();
    let w = worktrees::reserve(
        &h.root.path().join("results"),
        1,
        0,
        repo,
        state.summary.common_dir.unwrap(),
        state.summary.head.unwrap(),
    )
    .unwrap();
    worktrees::create(&w, &Cancellation::default(), &AtomicBool::new(true))
        .await
        .unwrap();
    fs::write(w.path.join("ignored.txt"), "useful but ignored").unwrap();
    assert_eq!(worktrees::inspect(&w).await.unwrap().changed, Some(false));
    fs::write(w.path.join("new ü binary.bin"), [0, 128, 255]).unwrap();
    assert_eq!(worktrees::inspect(&w).await.unwrap().changed, Some(true));
    assert!(worktrees::diff(&w).await.unwrap().contains("binary.bin"));
    fs::remove_file(w.path.join("new ü binary.bin")).unwrap();
    fs::write(w.path.join("tracked.txt"), "staged contents").unwrap();
    command(&w.path, &["add", "tracked.txt"]);
    fs::write(w.path.join("tracked.txt"), "committed base\n").unwrap();
    // Working tree equals base, but the staged content is still useful data.
    assert_eq!(worktrees::inspect(&w).await.unwrap().changed, Some(true));
    assert!(
        worktrees::diff(&w)
            .await
            .unwrap()
            .contains("+staged contents")
    );
    command(&w.path, &["add", "tracked.txt"]);
    assert_eq!(worktrees::inspect(&w).await.unwrap().changed, Some(false));
    fs::write(w.path.join("tracked.txt"), [0, 255, 128]).unwrap();
    command(&w.path, &["add", "tracked.txt"]);
    command(
        &w.path,
        &[
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=fixture@example.invalid",
            "commit",
            "-qm",
            "agent fixture commit",
        ],
    );
    assert_eq!(worktrees::inspect(&w).await.unwrap().changed, Some(true));
    assert!(worktrees::diff(&w).await.unwrap().contains("Binary files"));
    fs::remove_file(w.path.parent().unwrap().join("owner.json")).unwrap();
    assert!(worktrees::inspect(&w).await.is_err()); // Never unknown -> unchanged.
    h.close().await;
}

#[tokio::test]
async fn result_outcomes_and_lifecycle_are_independent_and_raw_output_is_backend_only() {
    for status in [
        JobStatus::Succeeded,
        JobStatus::Failed,
        JobStatus::Cancelled,
    ] {
        for changed in [false, true] {
            let mut h = Harness::new(1);
            let repo = repository(h.root.path(), "source").await;
            h.start(
                1,
                h.task(1, AgentId::Codex, changed, status == JobStatus::Failed),
                vec![repo],
            )
            .await;
            h.until(|e| Harness::ready(e, 1)).await;
            if status == JobStatus::Cancelled {
                h.manager.cancel(1, 0);
            } else {
                h.release(1, "tree");
            }
            h.until(|e| Harness::finished(e, 1, status)).await;
            assert!(h.events.iter().any(|e| matches!(e, Event::Result {result:Some(result),..} if result.changed == Some(changed) && result.exists && result.observed_this_session)));
            assert!(h.metadata(1).path.exists());
            let activity: String = h
                .events
                .iter()
                .filter_map(|e| {
                    if let Event::Output { text, .. } = e {
                        Some(text.as_str())
                    } else {
                        None
                    }
                })
                .collect();
            let raw: String = h
                .events
                .iter()
                .filter_map(|e| {
                    if let Event::Output { raw, .. } = e {
                        Some(raw.as_str())
                    } else {
                        None
                    }
                })
                .collect();
            assert!(activity.contains("[CodeConvoy] Preparing isolated worktree"));
            assert!(activity.contains("[CodeConvoy] Worktree ready"));
            assert!(activity.contains("[CodeConvoy] Running agent in isolated worktree"));
            assert!(activity.contains(if changed {
                "[CodeConvoy] Isolated changes retained"
            } else {
                "[CodeConvoy] No repository changes"
            }));
            assert!(!raw.contains("[CodeConvoy]"));
            assert!(raw.contains("fixture gate ready"));
            h.close().await;
        }
    }
}

#[tokio::test]
async fn isolated_image_and_text_transport_remains_backend_owned_for_every_backend() {
    use sha2::{Digest, Sha256};
    for agent in AgentId::ALL {
        let mut h = Harness::new(1);
        let repo = repository(h.root.path(), "source ü").await;
        fs::write(repo.path.join(".git/codeconvoy-workflow.json"), "{}").unwrap();
        let text = h.root.path().join("outside context ü.md");
        let image = h.root.path().join("outside image 日本語.png");
        fs::write(&text, "context sentinel ü").unwrap();
        fs::write(&image, include_bytes!("../assets/screenshot.png")).unwrap();
        let mut task = h.task(1, agent, false, false);
        task.prompt = "Review the attached context".into();
        task.attachments = [text, image]
            .iter()
            .map(|p| Attachment::inspect(p).unwrap())
            .collect();
        let before = git::diff(&repo.path).await.unwrap();
        h.start(1, task, vec![repo.clone()]).await;
        h.until(|e| Harness::finished(e, 1, JobStatus::Succeeded))
            .await;
        let w = h.metadata(1);
        let private = command(&w.path, &["rev-parse", "--absolute-git-dir"]);
        let receipt: serde_json::Value = serde_json::from_slice(
            &fs::read(Path::new(&private).join("codeconvoy-received.json")).unwrap(),
        )
        .unwrap();
        assert!(
            receipt["text"]
                .as_str()
                .unwrap()
                .contains("context sentinel ü")
        );
        assert_eq!(
            receipt["image_sha256"],
            serde_json::json!([Sha256::digest(include_bytes!("../assets/screenshot.png"))
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<String>()])
        );
        assert!(!w.path.join("outside context ü.md").exists());
        assert!(!w.path.join("outside image 日本語.png").exists());
        assert_eq!(worktrees::inspect(&w).await.unwrap().changed, Some(false));
        assert_eq!(git::diff(&repo.path).await.unwrap(), before);
        h.close().await;
    }
}

#[tokio::test]
async fn queued_missing_attachment_and_agent_spawn_failure_release_capacity() {
    for missing_attachment in [false, true] {
        let mut h = Harness::new(1);
        let repo = repository(h.root.path(), "source").await;
        h.start(
            1,
            h.task(1, AgentId::Codex, false, false),
            vec![repo.clone()],
        )
        .await;
        h.until(|e| Harness::ready(e, 1)).await;
        let context = h.root.path().join("context ü.md");
        fs::write(&context, "context").unwrap();
        let mut task = h.task(2, AgentId::Codex, false, false);
        task.attachments
            .push(Attachment::inspect(&context).unwrap());
        let mut prepared = runner::prepare(task, vec![repo.clone()]).await.unwrap();
        if !missing_attachment {
            // CLI was available at review and absent when the job is admitted.
            prepared.task.options.insert(
                "executable".into(),
                h.root.path().join("missing-agent").to_str().unwrap().into(),
            );
        }
        h.start_prepared(2, prepared);
        h.until(|e| e.iter().any(|e| matches!(e, Event::Queued { run: 2, .. })))
            .await;
        if missing_attachment {
            fs::remove_file(context).unwrap();
        }
        h.start(
            3,
            h.task(3, AgentId::Codex, false, false),
            vec![repo.clone()],
        )
        .await;
        h.release(1, "tree");
        h.until(|e| Harness::finished(e, 2, JobStatus::Failed) && Harness::ready(e, 3))
            .await;
        assert!(
            !h.events
                .iter()
                .any(|e| matches!(e, Event::Started { run: 2, .. }))
        );
        assert!(h.events.iter().any(|e| matches!(
            e,
            Event::Result {
                run: 2,
                result: Some(WorktreeResult {
                    changed: Some(false),
                    ..
                }),
                ..
            }
        )));
        h.release(3, "tree");
        h.until(|e| Harness::finished(e, 3, JobStatus::Succeeded))
            .await;
        assert!(git::status(&repo.path).await.unwrap().entries.is_empty());
        h.close().await;
    }
}

#[tokio::test]
async fn isolated_attachment_capability_mismatch_is_rejected_before_worktree_creation() {
    let h = Harness::new(1);
    let repo = repository(h.root.path(), "source").await;
    // This fixture advertises Codex --image only with the workflow marker.
    let path = h.root.path().join("image ü.png");
    fs::write(&path, include_bytes!("../assets/screenshot.png")).unwrap();
    let mut task = h.task(1, AgentId::Codex, false, false);
    task.attachments.push(Attachment::inspect(&path).unwrap());
    assert!(
        format!(
            "{:#}",
            runner::prepare(task, vec![repo]).await.err().unwrap()
        )
        .contains("cannot supply the requested image attachments")
    );
    assert!(!h.root.path().join("retained worktrees ü").exists());
    h.close().await;
}

#[tokio::test]
async fn saved_result_observations_are_compatible_but_never_recovery_validation() {
    let mut h = Harness::new(1);
    let repo = repository(h.root.path(), "source").await;
    let mut run = h
        .start(1, h.task(1, AgentId::Codex, false, false), vec![repo])
        .await;
    h.until(|e| Harness::ready(e, 1)).await;
    h.release(1, "tree");
    h.until(|e| Harness::finished(e, 1, JobStatus::Succeeded))
        .await;
    let w = h.metadata(1);
    run.jobs[0].worktree = Some(w.clone());
    run.jobs[0].worktree_result = Some(worktrees::inspect(&w).await.unwrap());
    run.jobs[0].finish(JobStatus::Succeeded, Some(0), String::new());
    let state = AppState {
        runs: vec![run],
        ..Default::default()
    };
    let store = Store::open(&h.root.path().join("state")).unwrap();
    store.save(&state).unwrap();
    // Missing checkout on reload must not cause a fabricated validation result.
    fs::rename(&w.path, w.path.with_file_name("moved-by-fixture")).unwrap();
    let loaded = store.load().unwrap();
    let observation = loaded.runs[0].jobs[0].worktree_result.as_ref().unwrap();
    assert!(observation.exists);
    assert_eq!(observation.changed, Some(false));
    assert!(!observation.observed_this_session);
    assert_eq!(
        serde_json::to_value(&loaded.runs).unwrap(),
        serde_json::to_value(&state.runs).unwrap()
    );
    let defaults: WorktreeResult = serde_json::from_str("{}").unwrap();
    assert!(!defaults.exists && defaults.changed.is_none() && !defaults.observed_this_session);
    h.close().await;
}

#[cfg(unix)]
#[tokio::test]
async fn git_common_directory_preserves_os_bytes_and_trailing_carriage_return() {
    let temp = tempfile::tempdir().unwrap();
    let repo = repository(temp.path(), "source ü").await;
    #[cfg(target_os = "linux")]
    let name = {
        use std::os::unix::ffi::OsStringExt;
        std::ffi::OsString::from_vec(b"git administration \xff\r".to_vec())
    };
    #[cfg(not(target_os = "linux"))]
    let name = std::ffi::OsString::from("git administration ü\r");
    let admin = temp.path().join(name);
    fs::rename(repo.path.join(".git"), &admin).unwrap();
    let mut link = b"gitdir: ".to_vec();
    link.extend(admin.as_os_str().as_encoded_bytes());
    // Git's gitfile reader trims CRLF; a trailing /. keeps the actual name.
    link.extend(b"/.\n");
    fs::write(repo.path.join(".git"), link).unwrap();
    assert_eq!(
        git::common_dir(&repo.path).await.unwrap(),
        admin.canonicalize().unwrap()
    );
    assert!(git::status(&repo.path).await.unwrap().entries.is_empty());
}

#[cfg(unix)]
#[tokio::test]
async fn stop_convoy_during_preparation_cancels_its_queue_and_leaves_other_convoy_running() {
    let mut h = Harness::new(2);
    let repo = repository(h.root.path(), "source").await;
    let queued = repository(h.root.path(), "queued").await;
    let peer = repository(h.root.path(), "peer").await;
    let control = h.root.path().join("control");
    checkout_gate(&repo, &control);
    let mut task = h.task(1, AgentId::Codex, false, false);
    task.concurrency = 1;
    h.start(1, task, vec![repo.clone(), queued]).await;
    wait_file(&control.join("filter.ready")).await;
    h.start(2, h.task(2, AgentId::Codex, false, false), vec![peer])
        .await;
    h.until(|e| Harness::ready(e, 2)).await;
    h.manager.cancel_run(1);
    h.until(|events| {
        events
            .iter()
            .filter(|e| {
                matches!(
                    e,
                    Event::Finished {
                        run: 1,
                        status: JobStatus::Cancelled,
                        ..
                    }
                )
            })
            .count()
            == 2
    })
    .await;
    assert!(!h.events.iter().any(|e| matches!(
        e,
        Event::Started { run: 1, .. } | Event::Preparing { run: 1, job: 1, .. }
    )));
    assert!(h.manager.is_active(2));
    assert!(git::status(&repo.path).await.unwrap().entries.is_empty());
    h.release(2, "tree");
    h.until(|e| Harness::finished(e, 2, JobStatus::Succeeded))
        .await;
    h.close().await;
}

#[cfg(unix)]
#[tokio::test]
async fn bounded_result_inspection_preserves_unknown_state_and_confirms_process_cleanup() {
    use fs2::FileExt;
    let mut h = Harness::new(1);
    let repo = repository(h.root.path(), "source").await;
    let control = h.root.path().join("control");
    checkout_gate(&repo, &control);
    fs::write(control.join("filter.release"), "go").unwrap();
    h.start(
        1,
        h.task(1, AgentId::Codex, true, false),
        vec![repo.clone()],
    )
    .await;
    h.until(|e| Harness::ready(e, 1)).await;
    fs::remove_file(control.join("filter.release")).unwrap();
    fs::remove_file(control.join("filter.ready")).unwrap();
    let smudge = command(&repo.path, &["config", "filter.convoygate.smudge"]);
    command(&repo.path, &["config", "filter.convoygate.clean", &smudge]);
    h.release(1, "tree");
    wait_file(&control.join("filter.ready")).await;
    h.until(|e| Harness::finished(e, 1, JobStatus::Succeeded))
        .await;
    assert!(h.events.iter().any(
        |e| matches!(e, Event::Result {run:1,result:None,detail,..} if detail.contains("timed out"))
    ));
    let lock = fs::OpenOptions::new()
        .write(true)
        .open(control.join("filter.lock"))
        .unwrap();
    lock.try_lock_exclusive().unwrap();
    assert!(h.manager.is_idle());
    assert_eq!(
        fs::read_to_string(h.metadata(1).path.join("tracked.txt")).unwrap(),
        "1"
    );
    h.close().await;
}
