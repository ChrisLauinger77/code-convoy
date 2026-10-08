#![cfg(feature = "test-support")]
#![allow(clippy::unwrap_used)]
use codeconvoy::{
    agents,
    domain::*,
    git,
    persistence::Store,
    runner::{self, Event, PreparedRun, RunManager},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
    process::Command,
    time::Duration,
};
use tokio::sync::mpsc;

struct Harness {
    directory: tempfile::TempDir,
    control: PathBuf,
    manager: RunManager,
    rx: mpsc::Receiver<Event>,
    events: Vec<Event>,
}
impl Harness {
    fn new(limit: usize) -> Self {
        let directory = tempfile::tempdir().unwrap();
        let control = directory.path().join("control");
        std::fs::create_dir(&control).unwrap();
        let (tx, rx) = mpsc::channel(256);
        Self {
            directory,
            control,
            manager: RunManager::new(limit, tx),
            rx,
            events: Vec::new(),
        }
    }
    async fn repo(&self, name: &str) -> Repository {
        let path = self.directory.path().join(name);
        if !path.exists() {
            assert!(
                Command::new("git")
                    .args(["init", "--quiet"])
                    .arg(&path)
                    .status()
                    .unwrap()
                    .success()
            );
        }
        git::register(&path).await.unwrap()
    }
    fn task(&self, ticket: u64, agent: AgentId, limit: usize, fail: bool) -> TaskConfig {
        TaskConfig {
            execution_mode: ExecutionMode::Direct,
            attachments: Vec::new(),
            prompt: format!(
                "codeconvoy-fixture-gate\n{}",
                serde_json::json!({"control":self.control,"ticket":ticket.to_string(),"fail":fail})
            ),
            agent,
            concurrency: limit,
            options: [(
                "executable".into(),
                env!("CARGO_BIN_EXE_codeconvoy-test-agent").into(),
            )]
            .into(),
        }
    }
    async fn prepared(
        &self,
        ticket: u64,
        agent: AgentId,
        limit: usize,
        fail: bool,
        repos: &[Repository],
    ) -> PreparedRun {
        runner::prepare(self.task(ticket, agent, limit, fail), repos.to_vec())
            .await
            .unwrap()
    }
    fn start(&mut self, id: u64, prepared: PreparedRun) {
        let backend = agents::backend(prepared.task.agent).unwrap();
        self.manager.start(id, prepared, backend).unwrap();
    }
    fn release(&self, id: u64, repo: &Repository) {
        std::fs::write(
            self.control.join(format!("{id}-{}.release", repo.name)),
            "release",
        )
        .unwrap();
    }
    fn ready(events: &[Event], run: u64, job: usize) -> bool {
        events
            .iter()
            .filter_map(|e| match e {
                Event::Output {
                    run: r,
                    job: j,
                    text,
                    ..
                } if *r == run && *j == job => Some(text.as_str()),
                _ => None,
            })
            .collect::<String>()
            .contains("fixture gate ready")
    }
    fn finished(events: &[Event], run: u64, job: usize, status: JobStatus) -> bool {
        events.iter().any(|e| matches!(e, Event::Finished {run:r,job:j,status:s,..} if *r == run && *j == job && *s == status))
    }
    async fn until(&mut self, predicate: impl Fn(&[Event]) -> bool) {
        tokio::time::timeout(Duration::from_secs(10), async {
            while !predicate(&self.events) {
                self.events.push(self.rx.recv().await.unwrap());
            }
        })
        .await
        .unwrap_or_else(|_| panic!("Timed out waiting for lifecycle events: {:?}", self.events));
    }
    async fn finish_all(&mut self, count: usize) {
        self.until(|events| {
            events
                .iter()
                .filter(|e| matches!(e, Event::Finished { .. }))
                .count()
                == count
        })
        .await;
    }
}

#[tokio::test]
async fn native_image_preflight_rejects_missing_cli_interfaces_without_starting_work() {
    let h = Harness::new(1);
    let repo = h.repo("image-preflight").await;
    let path = h.directory.path().join("screenshot.png");
    std::fs::write(&path, include_bytes!("../assets/codeconvoy-256.png")).unwrap();
    let attachment = codeconvoy::attachments::Attachment::inspect(&path).unwrap();
    for agent in AgentId::ALL {
        let mut task = h.task(1, agent, 1, false);
        task.attachments.push(attachment.clone());
        let result = runner::prepare(task, vec![repo.clone()]).await;
        if agent == AgentId::Codex {
            // The fixture intentionally has no Codex exec image interface.
            let error = result.err().unwrap().to_string();
            assert!(error.contains("screenshot.png") && error.contains("--image"));
        } else {
            assert!(result.is_ok(), "{agent:?}");
        }
    }
    assert!(h.manager.is_idle());
    assert!(!h.control.join("1-image-preflight.input").exists());
}

#[tokio::test]
async fn queued_attachment_changes_fail_before_spawn_and_release_repository_capacity() {
    for missing in [false, true] {
        let mut h = Harness::new(1);
        let occupying = h.repo("occupying").await;
        let queued = h.repo("queued").await;
        let path = h.directory.path().join("requirements.md");
        std::fs::write(&path, "before").unwrap();
        let attachment = codeconvoy::attachments::Attachment::inspect(&path).unwrap();
        let first = h
            .prepared(
                1,
                AgentId::Codex,
                1,
                false,
                std::slice::from_ref(&occupying),
            )
            .await;
        let mut task = h.task(2, AgentId::Codex, 1, false);
        task.attachments.push(attachment);
        let second = runner::prepare(task, vec![queued.clone()]).await.unwrap();
        h.start(1, first);
        h.until(|events| Harness::ready(events, 1, 0)).await;
        h.start(2, second);
        h.until(|events| {
            events
                .iter()
                .any(|event| matches!(event, Event::Queued { run: 2, .. }))
        })
        .await;
        if missing {
            std::fs::remove_file(&path).unwrap();
        } else {
            std::fs::write(&path, "after!").unwrap();
        }
        h.release(1, &occupying);
        h.finish_all(2).await;
        assert!(Harness::finished(&h.events, 1, 0, JobStatus::Succeeded));
        assert!(Harness::finished(&h.events, 2, 0, JobStatus::Failed));
        assert!(
            !h.events
                .iter()
                .any(|event| matches!(event, Event::Started { run: 2, .. }))
        );
        assert!(!h.control.join("2-queued.input").exists());
        let detail = h
            .events
            .iter()
            .find_map(|event| match event {
                Event::Finished { run: 2, detail, .. } => Some(detail),
                _ => None,
            })
            .unwrap();
        assert!(detail.contains("requirements.md"));
        assert!(detail.contains(if missing { "missing" } else { "changed" }));
        let third = h
            .prepared(3, AgentId::Copilot, 1, false, std::slice::from_ref(&queued))
            .await;
        h.start(3, third);
        h.until(|events| Harness::ready(events, 3, 0)).await;
        h.release(3, &queued);
        h.finish_all(3).await;
        assert!(Harness::finished(&h.events, 3, 0, JobStatus::Succeeded));
    }
}

#[tokio::test]
#[cfg(unix)]
async fn attachment_that_becomes_unreadable_while_queued_never_reaches_the_agent() {
    use std::{fs, os::unix::fs::PermissionsExt};
    let mut h = Harness::new(1);
    let repo = h.repo("queued-unreadable").await;
    let path = h.directory.path().join("private.md");
    fs::write(&path, "private context").unwrap();
    let attachment = codeconvoy::attachments::Attachment::inspect(&path).unwrap();
    let permissions = fs::metadata(&path).unwrap().permissions();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o000)).unwrap();
    let mode_bits_enforced = fs::File::open(&path).is_err();
    fs::set_permissions(&path, permissions.clone()).unwrap();
    if !mode_bits_enforced {
        eprintln!("Skipping queued unreadable-file assertions: this account can bypass mode bits");
        return;
    }
    let first = h
        .prepared(1, AgentId::Codex, 1, false, std::slice::from_ref(&repo))
        .await;
    let mut task = h.task(2, AgentId::Codex, 1, false);
    task.attachments.push(attachment);
    let second = runner::prepare(task, vec![repo.clone()]).await.unwrap();
    h.start(1, first);
    h.until(|events| Harness::ready(events, 1, 0)).await;
    h.start(2, second);
    h.until(|events| {
        events
            .iter()
            .any(|event| matches!(event, Event::Queued { run: 2, .. }))
    })
    .await;
    fs::set_permissions(&path, fs::Permissions::from_mode(0o000)).unwrap();
    h.release(1, &repo);
    h.finish_all(2).await;
    fs::set_permissions(&path, permissions).unwrap();
    assert!(Harness::finished(&h.events, 2, 0, JobStatus::Failed));
    assert!(!h.control.join("2-queued-unreadable.input").exists());
    assert!(
        !h.events
            .iter()
            .any(|event| matches!(event, Event::Started { run: 2, .. }))
    );
    assert!(h.events.iter().any(|event| matches!(event, Event::Finished {run: 2, detail, ..} if detail.contains("Cannot read attachment") && detail.contains("private.md"))));
    let third = h
        .prepared(3, AgentId::Copilot, 1, false, std::slice::from_ref(&repo))
        .await;
    h.start(3, third);
    h.until(|events| Harness::ready(events, 3, 0)).await;
    h.release(3, &repo);
    h.finish_all(3).await;
    assert!(Harness::finished(&h.events, 3, 0, JobStatus::Succeeded));
}

#[tokio::test]
async fn every_backend_delivers_attachment_context_and_keeps_separate_raw_output() {
    let mut h = Harness::new(4);
    let path = h.directory.path().join("requirements.md");
    std::fs::write(&path, "attached context Grüße").unwrap();
    let attachment = codeconvoy::attachments::Attachment::inspect(&path).unwrap();
    let mut repos = Vec::new();
    for (index, agent) in AgentId::ALL.into_iter().enumerate() {
        let id = index as u64 + 1;
        let repo = h.repo(&format!("repo{id}")).await;
        let mut task = h.task(id, agent, 1, false);
        task.attachments.push(attachment.clone());
        let prepared = runner::prepare(task, vec![repo.clone()]).await.unwrap();
        h.start(id, prepared);
        repos.push((id, repo));
    }
    h.until(|events| (1..=4).all(|id| Harness::ready(events, id, 0)))
        .await;
    for (id, repo) in &repos {
        let input =
            std::fs::read_to_string(h.control.join(format!("{id}-{}.input", repo.name))).unwrap();
        let context: serde_json::Value =
            serde_json::from_str(input.lines().last().unwrap()).unwrap();
        assert_eq!(context["content"], "attached context Grüße");
        h.release(*id, repo);
    }
    h.finish_all(4).await;
    for (index, agent) in AgentId::ALL.into_iter().enumerate() {
        let id = index as u64 + 1;
        assert!(Harness::finished(&h.events, id, 0, JobStatus::Succeeded));
        let activity: String = h
            .events
            .iter()
            .filter_map(|event| match event {
                Event::Output { run, text, .. } if *run == id => Some(text.as_str()),
                _ => None,
            })
            .collect();
        let raw: String = h
            .events
            .iter()
            .filter_map(|event| match event {
                Event::Output { run, raw, .. } if *run == id => Some(raw.as_str()),
                _ => None,
            })
            .collect();
        assert!(activity.contains("fixture gate ready"));
        assert!(raw.contains("fixture gate ready"));
        assert!(!activity.contains("attached context"));
        assert!(!raw.contains("attached context"));
        if agent == AgentId::Copilot {
            assert_eq!(raw, activity);
        } else {
            assert!(
                raw.lines()
                    .all(|line| serde_json::from_str::<serde_json::Value>(line).is_ok())
            );
            assert_ne!(raw, activity);
        }
    }
}

#[tokio::test]
async fn drafts_are_snapshotted_and_codex_and_copilot_run_independently() {
    let mut h = Harness::new(2);
    let first = h.repo("codex").await;
    let second = h.repo("copilot").await;
    let mut state = AppState {
        draft: h.task(1, AgentId::Codex, 1, false),
        ..Default::default()
    };
    state
        .draft
        .options
        .insert("model_reasoning_effort".into(), "high".into());
    state
        .draft
        .options
        .insert("model".into(), "original-model".into());
    let mut selection = vec![first.clone()];
    let prepared = runner::prepare(state.draft.clone(), selection.clone())
        .await
        .unwrap();
    let original_prompt = prepared.task.prompt.clone();
    state.runs.push(prepared.snapshot(1));
    h.start(1, prepared);
    state.select_agent(AgentId::Copilot);
    state.draft.prompt = h.task(2, AgentId::Copilot, 2, false).prompt;
    state.draft.options.insert(
        "executable".into(),
        env!("CARGO_BIN_EXE_codeconvoy-test-agent").into(),
    );
    state.draft.concurrency = 2;
    selection.clear();
    selection.push(second.clone());
    let prepared = runner::prepare(state.draft.clone(), selection)
        .await
        .unwrap();
    state.runs.push(prepared.snapshot(2));
    h.start(2, prepared);
    h.until(|events| Harness::ready(events, 1, 0) && Harness::ready(events, 2, 0))
        .await;
    let snapshot = &state.runs[0];
    assert_eq!(snapshot.task.prompt, original_prompt);
    assert_eq!(snapshot.task.agent, AgentId::Codex);
    assert_eq!(snapshot.task.options["model_reasoning_effort"], "high");
    assert_eq!(snapshot.task.concurrency, 1);
    assert_eq!(snapshot.jobs[0].repository, first);
    assert_eq!(state.runs[1].task.agent, AgentId::Copilot);
    assert_eq!(
        std::fs::read_to_string(h.control.join("1-codex.input")).unwrap(),
        original_prompt
    );
    let args: Vec<String> =
        serde_json::from_slice(&std::fs::read(h.control.join("1-codex.args")).unwrap()).unwrap();
    assert_eq!(
        &args[1..],
        [
            "--no-daemon",
            "--ask-for-approval",
            "never",
            "exec",
            "--json",
            "--color",
            "never",
            "--ephemeral",
            "--sandbox",
            "read-only",
            "--model",
            "original-model",
            "--config",
            "model_reasoning_effort=\"high\"",
            "-"
        ]
    );
    let args: Vec<String> =
        serde_json::from_slice(&std::fs::read(h.control.join("2-copilot.args")).unwrap()).unwrap();
    assert!(args.iter().any(|s| s == "--no-ask-user"));
    assert!(args.iter().any(|s| s == "--allow-tool=write"));
    assert!(!args.iter().any(|s| s == "--json" || s == "--sandbox"));
    h.release(1, &first);
    h.release(2, &second);
    h.finish_all(2).await;
    assert!(Harness::finished(&h.events, 1, 0, JobStatus::Succeeded));
    assert!(Harness::finished(&h.events, 2, 0, JobStatus::Succeeded));
}

#[tokio::test]
async fn four_convoys_share_global_slots_and_keep_their_own_limits() {
    let mut h = Harness::new(4);
    let a = vec![h.repo("a1").await, h.repo("a2").await];
    let b = vec![h.repo("b1").await, h.repo("b2").await, h.repo("b3").await];
    let c = vec![h.repo("c1").await, h.repo("c2").await];
    let d = vec![h.repo("d1").await, h.repo("d2").await];
    let pd = h.prepared(4, AgentId::Claude, 2, false, &d).await;
    let pa = h.prepared(1, AgentId::Codex, 1, false, &a).await;
    let pb = h.prepared(2, AgentId::Copilot, 2, false, &b).await;
    let pc = h.prepared(3, AgentId::OpenCode, 2, false, &c).await;
    h.start(1, pa);
    h.start(2, pb);
    h.start(3, pc);
    h.start(4, pd);
    h.until(|e| (1..=4).all(|run| Harness::ready(e, run, 0)))
        .await;
    h.release(1, &a[0]);
    h.until(|e| {
        e.iter()
            .any(|event| matches!(event, Event::Started { job: 1, .. }))
    })
    .await;
    for (id, repos) in [(1, &a), (2, &b), (3, &c), (4, &d)] {
        for repo in repos {
            h.release(id, repo);
        }
    }
    h.finish_all(9).await;
    let mut active = BTreeSet::new();
    let mut maxima = BTreeMap::new();
    let mut global_max = 0;
    for event in &h.events {
        match event {
            Event::Started { run, job, .. } => {
                assert!(active.insert((*run, *job)));
                global_max = global_max.max(active.len());
                let count = active.iter().filter(|(r, _)| r == run).count();
                maxima
                    .entry(*run)
                    .and_modify(|v: &mut usize| *v = (*v).max(count))
                    .or_insert(count);
                assert!(count <= if *run == 1 { 1 } else { 2 });
                assert!(active.len() <= 4);
            }
            Event::Finished {
                run, job, status, ..
            } => {
                assert_eq!(*status, JobStatus::Succeeded);
                assert!(active.remove(&(*run, *job)));
            }
            _ => {}
        }
    }
    assert_eq!(global_max, 4);
    assert!(active.is_empty());
    assert_eq!(maxima.len(), 4);
}

#[tokio::test]
async fn cancelling_a_convoy_cancels_its_queue_and_leaves_another_running() {
    let mut h = Harness::new(2);
    let a = vec![h.repo("running").await, h.repo("queued").await];
    let b = h.repo("independent").await;
    let first = h.prepared(1, AgentId::Codex, 1, false, &a).await;
    let second = h
        .prepared(2, AgentId::Copilot, 1, false, std::slice::from_ref(&b))
        .await;
    h.start(1, first);
    h.start(2, second);
    h.until(|e| Harness::ready(e, 1, 0) && Harness::ready(e, 2, 0))
        .await;
    h.manager.cancel_run(1);
    h.until(|e| {
        Harness::finished(e, 1, 0, JobStatus::Cancelled)
            && Harness::finished(e, 1, 1, JobStatus::Cancelled)
    })
    .await;
    assert!(h.manager.is_active(2));
    assert!(!h.control.join("1-queued.input").exists());
    assert!(
        !h.events
            .iter()
            .any(|e| matches!(e, Event::Finished { run: 2, .. }))
    );
    h.release(2, &b);
    h.finish_all(3).await;
    assert!(Harness::finished(&h.events, 2, 0, JobStatus::Succeeded));
}

#[tokio::test]
async fn same_repository_is_serialized_and_released_after_success_failure_and_cancel() {
    for first_agent in [AgentId::Codex, AgentId::OpenCode, AgentId::Claude] {
        for first_status in [
            JobStatus::Succeeded,
            JobStatus::Failed,
            JobStatus::Cancelled,
        ] {
            let mut h = Harness::new(2);
            let shared = h.repo("shared").await;
            let other = h.repo("other").await;
            #[cfg(unix)]
            let mut alias = {
                let path = h.directory.path().join("shared-alias");
                std::os::unix::fs::symlink(&shared.path, &path).unwrap();
                git::register(&path).await.unwrap()
            };
            #[cfg(not(unix))]
            let mut alias = shared.clone();
            assert_eq!(alias.path, shared.path);
            alias.name = "A different display name".into();
            let first = h
                .prepared(
                    1,
                    first_agent,
                    1,
                    first_status == JobStatus::Failed,
                    std::slice::from_ref(&shared),
                )
                .await;
            let second = h
                .prepared(2, AgentId::Copilot, 2, false, &[alias, other.clone()])
                .await;
            h.start(1, first);
            h.until(|e| Harness::ready(e, 1, 0)).await;
            h.start(2, second);
            h.until(|e| {
                Harness::ready(e, 2, 1)
                    && e.iter().any(|event| {
                        matches!(
                            event,
                            Event::Queued {
                                run: 2,
                                job: 0,
                                reason: QueueReason::Repository(1)
                            }
                        )
                    })
            })
            .await;
            assert!(
                !h.events
                    .iter()
                    .any(|e| matches!(e, Event::Started { run: 2, job: 0, .. }))
            );
            if first_status == JobStatus::Cancelled {
                h.manager.cancel_run(1);
            } else {
                h.release(1, &shared);
            }
            h.until(|e| Harness::finished(e, 1, 0, first_status) && Harness::ready(e, 2, 0))
                .await;
            assert!(
                !h.events
                    .iter()
                    .any(|e| matches!(e, Event::Finished { run: 2, .. }))
            );
            h.release(2, &shared);
            h.release(2, &other);
            h.finish_all(3).await;
            assert!(Harness::finished(&h.events, 2, 0, JobStatus::Succeeded));
            assert!(Harness::finished(&h.events, 2, 1, JobStatus::Succeeded));
        }
    }
}

#[tokio::test]
async fn global_limit_changes_wake_waiters_without_cancelling_running_jobs() {
    let mut h = Harness::new(1);
    let repos = [
        h.repo("one").await,
        h.repo("two").await,
        h.repo("three").await,
    ];
    let first = h.prepared(1, AgentId::Codex, 1, false, &repos[..1]).await;
    let second = h
        .prepared(2, AgentId::Copilot, 1, false, &repos[1..2])
        .await;
    let third = h.prepared(3, AgentId::Codex, 1, false, &repos[2..]).await;
    h.start(1, first);
    h.until(|e| Harness::ready(e, 1, 0)).await;
    h.start(2, second);
    h.until(|e| {
        e.iter().any(|e| {
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
    h.manager.set_global_limit(2).unwrap();
    h.until(|e| Harness::ready(e, 2, 0)).await;
    h.manager.set_global_limit(1).unwrap();
    h.start(3, third);
    h.until(|e| {
        e.iter().any(|e| {
            matches!(
                e,
                Event::Queued {
                    run: 3,
                    reason: QueueReason::GlobalLimit,
                    ..
                }
            )
        })
    })
    .await;
    h.release(1, &repos[0]);
    h.until(|e| Harness::finished(e, 1, 0, JobStatus::Succeeded))
        .await;
    h.release(2, &repos[1]);
    h.until(|e| Harness::ready(e, 3, 0)).await;
    let third_start = h
        .events
        .iter()
        .position(|e| matches!(e, Event::Started { run: 3, .. }))
        .unwrap();
    assert!(Harness::finished(
        &h.events[..third_start],
        2,
        0,
        JobStatus::Succeeded
    ));
    h.release(3, &repos[2]);
    h.finish_all(3).await;
}

#[tokio::test]
async fn queued_shared_repository_is_rechecked_after_the_previous_convoy_edits_it() {
    let mut h = Harness::new(2);
    let repo = h.repo("shared").await;
    let mut task = h.task(1, AgentId::Codex, 1, false);
    task.prompt = format!(
        "codeconvoy-fixture-gate\n{}",
        serde_json::json!({"control":h.control,"ticket":"1","edit":true})
    );
    let first = runner::prepare(task, vec![repo.clone()]).await.unwrap();
    let second = h
        .prepared(2, AgentId::Copilot, 1, false, std::slice::from_ref(&repo))
        .await;
    h.start(1, first);
    h.until(|e| Harness::ready(e, 1, 0)).await;
    h.start(2, second);
    h.release(1, &repo);
    h.finish_all(2).await;
    assert!(h.events.iter().any(|e| matches!(e,Event::Finished{run:2,status:JobStatus::Failed,detail,..} if detail.contains("changed after preflight"))));
    assert!(!h.control.join("2-shared.input").exists());
    // A freshly reviewed snapshot can still use the repository after that failure.
    let third = h
        .prepared(3, AgentId::Copilot, 1, false, std::slice::from_ref(&repo))
        .await;
    h.start(3, third);
    h.until(|e| Harness::ready(e, 3, 0)).await;
    h.release(3, &repo);
    h.finish_all(3).await;
}

#[tokio::test]
async fn shutdown_cancels_all_convoys_and_drains_the_manager() {
    let mut h = Harness::new(2);
    let a = vec![h.repo("one").await, h.repo("queued").await];
    let b = vec![h.repo("two").await];
    let first = h.prepared(1, AgentId::Codex, 1, false, &a).await;
    let second = h.prepared(2, AgentId::Copilot, 1, false, &b).await;
    h.start(1, first);
    h.start(2, second);
    h.until(|e| Harness::ready(e, 1, 0) && Harness::ready(e, 2, 0))
        .await;
    h.manager.shutdown();
    h.manager.shutdown(); // Repeated close requests are idempotent.
    let rejected = h.prepared(3, AgentId::Codex, 1, false, &b).await;
    assert!(
        h.manager
            .start(3, rejected, agents::backend(AgentId::Codex).unwrap())
            .is_err()
    );
    h.finish_all(3).await;
    tokio::time::timeout(Duration::from_secs(5), &mut h.manager.join)
        .await
        .unwrap()
        .unwrap();
    assert!(h.manager.is_idle());
    for (run, job) in [(1, 0), (1, 1), (2, 0)] {
        assert!(Harness::finished(&h.events, run, job, JobStatus::Cancelled));
    }
    assert!(!h.control.join("1-queued.input").exists());
    assert!(!h.events.iter().any(|e| matches!(
        e,
        Event::Started { run: 1, job: 1, .. } | Event::Started { run: 3, .. }
    )));
}

#[tokio::test]
async fn shutdown_during_event_backpressure_never_starts_waiting_jobs() {
    let h = Harness::new(1);
    let repos = vec![h.repo("one").await, h.repo("two").await];
    let prepared = h.prepared(1, AgentId::Codex, 2, false, &repos).await;
    let (tx, mut rx) = mpsc::channel(1);
    // Fill the event queue before admission. The manager will suspend while
    // publishing CheckingRepository, so shutdown cannot rely on command polling.
    tx.send(Event::Queued {
        run: 99,
        job: 0,
        reason: QueueReason::GlobalLimit,
    })
    .await
    .unwrap();
    let mut manager = RunManager::new(2, tx);
    manager
        .start(1, prepared, agents::backend(AgentId::Codex).unwrap())
        .unwrap();
    tokio::task::yield_now().await;
    manager.shutdown();
    let finished = tokio::time::timeout(Duration::from_secs(5), async {
        let mut finished = 0;
        while let Some(event) = rx.recv().await {
            assert!(!matches!(event, Event::Started { .. }));
            if let Event::Finished { status, .. } = event {
                assert_eq!(status, JobStatus::Cancelled);
                finished += 1;
            }
        }
        finished
    })
    .await
    .unwrap();
    (&mut manager.join).await.unwrap();
    assert_eq!(finished, 2);
    assert!(!h.control.join("1-one.input").exists());
    assert!(!h.control.join("1-two.input").exists());
}

#[test]
fn persistence_recovers_multiple_runs_retains_snapshots_and_defaults_old_preferences() {
    let temp = tempfile::tempdir().unwrap();
    let store = Store::open(temp.path()).unwrap();
    let mut state = AppState {
        global_concurrency: 7,
        ..Default::default()
    };
    for (id, status) in [
        (1, JobStatus::Running),
        (2, JobStatus::Queued),
        (3, JobStatus::Succeeded),
        (4, JobStatus::Failed),
    ] {
        let mut job = Job::queued(Repository {
            name: format!("repo{id}"),
            path: Path::new("/repo").join(id.to_string()),
        });
        job.status = status;
        job.log.append("session only");
        state.runs.push(Run {
            provenance: None,
            id,
            created_at: now(),
            task: TaskConfig {
                prompt: format!("snapshot{id}"),
                ..Default::default()
            },
            jobs: vec![job],
        });
    }
    store.save(&state).unwrap();
    let loaded = store.load().unwrap();
    assert_eq!(loaded.global_concurrency, 7);
    for run in &loaded.runs[..2] {
        assert_eq!(run.status(), JobStatus::Cancelled);
        assert!(run.jobs[0].interrupted);
    }
    assert_eq!(loaded.runs[2].status(), JobStatus::Succeeded);
    assert_eq!(loaded.runs[3].status(), JobStatus::Failed);
    assert_eq!(loaded.runs[0].task.prompt, "snapshot1");
    assert!(loaded.runs.iter().all(|r| r.jobs[0].log.text.is_empty()));
    assert_eq!(
        serde_json::from_str::<AppState>("{\"version\":1}")
            .unwrap()
            .global_concurrency,
        DEFAULT_GLOBAL_CONCURRENCY
    );
    state.global_concurrency = 0;
    store.save(&state).unwrap();
    assert_eq!(store.load().unwrap().global_concurrency, 1);
    state.global_concurrency = 99;
    store.save(&state).unwrap();
    assert_eq!(store.load().unwrap().global_concurrency, MAX_CONCURRENCY);
}

#[test]
fn history_never_evicts_active_convoys_and_mixed_results_are_truthful() {
    let mut state = AppState::default();
    for id in 0..(MAX_HISTORY + 5) as u64 {
        let mut job = Job::queued(Repository {
            name: "repo".into(),
            path: "/repo".into(),
        });
        if id < MAX_HISTORY as u64 + 3 {
            job.finish(JobStatus::Succeeded, Some(0), String::new());
        }
        state.runs.push(Run {
            provenance: None,
            id,
            created_at: now(),
            task: TaskConfig::default(),
            jobs: vec![job],
        });
    }
    state.trim_history();
    assert_eq!(state.runs.len(), MAX_HISTORY + 2);
    assert_eq!(state.runs.iter().filter(|r| r.active()).count(), 2);
    let mut run = state.runs[0].clone();
    let mut failed = run.jobs[0].clone();
    failed.status = JobStatus::Failed;
    run.jobs.push(failed);
    assert_eq!(run.status(), JobStatus::Failed);
    run.jobs[1].status = JobStatus::Cancelled;
    assert_eq!(run.status(), JobStatus::Cancelled);
}

#[tokio::test]
async fn independent_opencode_convoys_snapshot_options_and_release_global_capacity() {
    let mut h = Harness::new(1);
    let a = h.repo("opencode-a").await;
    let b = h.repo("opencode-b").await;
    let mut draft = h.task(1, AgentId::OpenCode, 2, false);
    draft.options.extend([
        ("model".into(), "provider/model".into()),
        ("agent".into(), "build".into()),
        ("variant".into(), "high".into()),
        ("permissions".into(), "auto".into()),
    ]);
    let first = runner::prepare(draft.clone(), vec![a.clone()])
        .await
        .unwrap();
    let snapshot = first.snapshot(1);
    h.start(1, first);
    draft.prompt = "changed draft".into();
    draft.options.insert("variant".into(), "low".into());
    let second = h
        .prepared(2, AgentId::OpenCode, 1, false, std::slice::from_ref(&b))
        .await;
    h.start(2, second);
    h.until(|e| {
        Harness::ready(e, 1, 0)
            && e.iter().any(|e| {
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
    assert!(!h.control.join("2-opencode-b.input").exists());
    h.release(1, &a);
    h.until(|e| Harness::ready(e, 2, 0)).await;
    assert!(Harness::finished(&h.events, 1, 0, JobStatus::Succeeded));
    h.release(2, &b);
    h.finish_all(2).await;
    assert!(Harness::finished(&h.events, 2, 0, JobStatus::Succeeded));
    assert_eq!(snapshot.task.options["variant"], "high");
    assert_eq!(snapshot.task.concurrency, 2);
    assert_ne!(snapshot.task.prompt, draft.prompt);
    assert_eq!(
        std::fs::read_to_string(h.control.join("1-opencode-a.input")).unwrap(),
        snapshot.task.prompt
    );
    let args: Vec<String> =
        serde_json::from_slice(&std::fs::read(h.control.join("1-opencode-a.args")).unwrap())
            .unwrap();
    assert_eq!(&args[1..5], ["run", "--format", "json", "--dir"]);
    assert_eq!(args[5], a.path.to_str().unwrap());
    assert_eq!(
        &args[6..],
        [
            "--model=provider/model",
            "--agent=build",
            "--variant=high",
            "--auto"
        ]
    );
}
#[tokio::test]
async fn independent_claude_convoys_snapshot_options_and_release_global_capacity() {
    let mut h = Harness::new(1);
    let a = h.repo("claude-a").await;
    let b = h.repo("claude-b").await;
    let mut draft = h.task(1, AgentId::Claude, 2, false);
    draft.options.extend([
        ("model".into(), "sonnet".into()),
        ("max_turns".into(), "10".into()),
        ("effort".into(), "high".into()),
        ("permission_mode".into(), "acceptEdits".into()),
    ]);
    let first = runner::prepare(draft.clone(), vec![a.clone()])
        .await
        .unwrap();
    let snapshot = first.snapshot(1);
    h.start(1, first);
    draft.prompt = "changed draft".into();
    draft.options.insert("effort".into(), "low".into());
    let second = h
        .prepared(2, AgentId::Claude, 1, false, std::slice::from_ref(&b))
        .await;
    h.start(2, second);
    h.until(|e| {
        Harness::ready(e, 1, 0)
            && e.iter().any(|e| {
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
    assert!(!h.control.join("2-claude-b.input").exists());
    h.release(1, &a);
    h.until(|e| Harness::ready(e, 2, 0)).await;
    assert!(Harness::finished(&h.events, 1, 0, JobStatus::Succeeded));
    h.release(2, &b);
    h.finish_all(2).await;
    assert!(Harness::finished(&h.events, 2, 0, JobStatus::Succeeded));
    assert_eq!(snapshot.task.options["effort"], "high");
    assert_eq!(snapshot.task.concurrency, 2);
    assert_ne!(snapshot.task.prompt, draft.prompt);
    assert_eq!(
        std::fs::read_to_string(h.control.join("1-claude-a.input")).unwrap(),
        snapshot.task.prompt
    );
    let args: Vec<String> =
        serde_json::from_slice(&std::fs::read(h.control.join("1-claude-a.args")).unwrap()).unwrap();
    assert_eq!(
        &args[1..],
        [
            "--print",
            "--input-format",
            "text",
            "--output-format",
            "stream-json",
            "--verbose",
            "--no-session-persistence",
            "--permission-mode",
            "acceptEdits",
            "--model=sonnet",
            "--effort=high",
            "--max-turns=10"
        ]
    );
}

#[tokio::test]
async fn claude_build_spawn_and_zero_exit_protocol_failures_release_slots_and_repository_leases() {
    for failure in ["build", "spawn", "result_error", "malformed", "incomplete"] {
        let mut h = Harness::new(1);
        let repo = h.repo("shared").await;
        let mut task = h.task(1, AgentId::Claude, 1, false);
        task.prompt = format!(
            "codeconvoy-fixture-gate\n{}",
            serde_json::json!({"control":h.control,"ticket":"1","claude_outcome":failure})
        );
        let mut first = runner::prepare(task, vec![repo.clone()]).await.unwrap();
        // Exercise worker failures after admission, beyond preflight validation.
        if failure == "build" {
            first.task.options.insert("max_turns".into(), "0".into());
        } else if failure == "spawn" {
            first.task.options.insert(
                "executable".into(),
                h.directory
                    .path()
                    .join("removed-executable")
                    .to_str()
                    .unwrap()
                    .into(),
            );
        }
        let second = h
            .prepared(2, AgentId::Claude, 1, false, std::slice::from_ref(&repo))
            .await;
        h.start(1, first);
        h.start(2, second);
        h.release(1, &repo);
        h.until(|e| Harness::finished(e, 1, 0, JobStatus::Failed) && Harness::ready(e, 2, 0))
            .await;
        h.release(2, &repo);
        h.finish_all(2).await;
        assert!(Harness::finished(&h.events, 2, 0, JobStatus::Succeeded));
    }
}
