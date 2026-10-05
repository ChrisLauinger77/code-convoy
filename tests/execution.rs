#![cfg(feature = "test-support")]
#![allow(clippy::unwrap_used)]
use codeconvoy::{
    agents::{AgentBackend, AgentOutput, OptionSpec, codex::Codex},
    domain::{AgentId, AgentOptions, JobStatus, Repository, TaskConfig},
    git,
    process::{self, Cancellation, CommandSpec},
    runner::{self, Event, PreparedRepository, PreparedRun},
};
use std::{path::Path, process::Command, sync::Arc, time::Duration};
use tokio::sync::mpsc;

struct Fixture;
impl AgentBackend for Fixture {
    fn id(&self) -> AgentId {
        AgentId::Codex
    }
    fn options(&self) -> &'static [OptionSpec] {
        &[]
    }
    fn detection(&self, _: &AgentOptions, directory: &Path) -> anyhow::Result<CommandSpec> {
        Ok(CommandSpec::new(
            env!("CARGO_BIN_EXE_codeconvoy-test-agent"),
            directory,
        ))
    }
    fn check_detection(&self, _: &str) -> anyhow::Result<()> {
        Ok(())
    }
    fn build(&self, task: &TaskConfig, repo: &Repository) -> anyhow::Result<CommandSpec> {
        let mut spec = self.detection(&task.options, &repo.path)?;
        spec.input = Some(task.prompt.as_bytes().to_vec());
        Ok(spec)
    }
    fn output(&self) -> Box<dyn AgentOutput> {
        Codex.output()
    }
    fn execution_summary(&self, _: &AgentOptions) -> String {
        "Test fixture".into()
    }
}
async fn prepare(root: &Path, names: &[&str], concurrency: usize) -> PreparedRun {
    let mut repositories = Vec::new();
    for name in names {
        let path = root.join(name);
        std::fs::create_dir_all(&path).unwrap();
        assert!(
            Command::new("git")
                .args(["init", "--quiet"])
                .arg(&path)
                .status()
                .unwrap()
                .success()
        );
        let repository = git::register(&path).await.unwrap();
        let state = git::status(&repository.path).await.unwrap();
        repositories.push(PreparedRepository { repository, state });
    }
    PreparedRun {
        task: TaskConfig {
            prompt: "literal prompt with $(shell)\nsecond line".into(),
            concurrency,
            ..Default::default()
        },
        repositories,
    }
}
async fn finish(rx: &mut mpsc::Receiver<Event>, count: usize) -> Vec<Event> {
    tokio::time::timeout(Duration::from_secs(15), async {
        let mut events = Vec::new();
        let mut finished = 0;
        while finished < count {
            let event = rx.recv().await.unwrap();
            if matches!(event, Event::Finished { .. }) {
                finished += 1;
            }
            events.push(event);
        }
        events
    })
    .await
    .unwrap()
}
#[tokio::test]
async fn concurrency_limit_is_respected_and_failure_does_not_abort_other_jobs() {
    let directory = tempfile::tempdir().unwrap();
    let prepared = prepare(directory.path(), &["one", "fail", "three", "four"], 2).await;
    let (tx, mut rx) = mpsc::channel(256);
    let handle = runner::start(42, prepared, Arc::new(Fixture), tx);
    let events = finish(&mut rx, 4).await;
    let (mut active, mut maximum, mut successes, mut failures) = (0, 0, 0, 0);
    let mut output = String::new();
    for event in events {
        match event {
            Event::Started { .. } => {
                active += 1;
                maximum = maximum.max(active);
            }
            Event::Finished { status, .. } => {
                active -= 1;
                if status == JobStatus::Succeeded {
                    successes += 1;
                } else if status == JobStatus::Failed {
                    failures += 1;
                }
            }
            Event::Output { text, .. } => output.push_str(&text),
        }
    }
    assert_eq!(maximum, 2);
    assert_eq!(active, 0);
    assert_eq!(successes, 3);
    assert_eq!(failures, 1);
    assert!(output.contains("fixture diagnostic"));
    assert!(output.contains("done"));
    assert_eq!(
        std::fs::read_to_string(directory.path().join("one/agent-input")).unwrap(),
        "literal prompt with $(shell)\nsecond line"
    );
    while !handle.join.is_finished() {
        tokio::task::yield_now().await;
    }
}
#[tokio::test]
async fn queued_cancellation_never_spawns_and_dirty_change_after_preflight_fails() {
    let directory = tempfile::tempdir().unwrap();
    let prepared = prepare(directory.path(), &["first", "cancelled", "changed"], 1).await;
    std::fs::write(
        directory.path().join("changed/new-file"),
        "changed after preflight",
    )
    .unwrap();
    let (tx, mut rx) = mpsc::channel(256);
    let handle = runner::start(1, prepared, Arc::new(Fixture), tx);
    handle.cancel(1);
    let events = finish(&mut rx, 3).await;
    assert!(!directory.path().join("cancelled/agent-input").exists());
    assert!(!directory.path().join("changed/agent-input").exists());
    assert!(events.iter().any(|e| matches!(
        e,
        Event::Finished {
            job: 1,
            status: JobStatus::Cancelled,
            ..
        }
    )));
    assert!(events.iter().any(|e| matches!(e, Event::Finished { job: 2, status: JobStatus::Failed, detail, .. } if detail.contains("changed after preflight"))));
}
#[tokio::test]
async fn cancelling_a_running_job_terminates_its_descendants() {
    let directory = tempfile::tempdir().unwrap();
    let mut prepared = prepare(directory.path(), &["tree"], 1).await;
    prepared.task.prompt = "spawn-child".into();
    let (tx, mut rx) = mpsc::channel(256);
    let handle = runner::start(1, prepared, Arc::new(Fixture), tx);
    tokio::time::timeout(Duration::from_secs(5), async {
        while !directory.path().join("tree/descendant-ready").exists() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    handle.cancel(0);
    let events = finish(&mut rx, 1).await;
    assert!(events.iter().any(|e| matches!(
        e,
        Event::Finished {
            status: JobStatus::Cancelled,
            ..
        }
    )));
    tokio::time::sleep(Duration::from_millis(1400)).await;
    assert!(
        !directory.path().join("tree/orphan-survived").exists(),
        "descendant survived cancellation"
    );
}
#[tokio::test]
async fn capture_bounds_output_without_deadlocking_and_spawn_errors_are_actionable() {
    let directory = tempfile::tempdir().unwrap();
    let spec = CommandSpec::new(
        env!("CARGO_BIN_EXE_codeconvoy-test-agent"),
        directory.path(),
    )
    .args(&["flood"]);
    let output = process::capture(spec, 4096).await.unwrap();
    assert!(output.status.success());
    assert_eq!(output.stdout.len(), 4096);
    assert!(output.truncated);
    let missing = CommandSpec::new(
        directory.path().join("missing-executable"),
        directory.path(),
    );
    let error = match process::spawn(&missing) {
        Ok(_) => panic!("missing executable spawned"),
        Err(error) => error,
    };
    assert!(error.to_string().contains("Check the executable"));
    let cancellation = Cancellation::default();
    cancellation.cancel();
    tokio::time::timeout(Duration::from_millis(50), cancellation.cancelled())
        .await
        .unwrap();
}

async fn prepare_copilot(root: &Path, names: &[&str], concurrency: usize) -> PreparedRun {
    let mut prepared = prepare(root, names, concurrency).await;
    prepared.task.agent = AgentId::Copilot;
    prepared.task.options.insert(
        "executable".into(),
        env!("CARGO_BIN_EXE_codeconvoy-test-agent").into(),
    );
    let repositories = prepared
        .repositories
        .into_iter()
        .map(|r| r.repository)
        .collect();
    // Exercise the real backend registry, help/version detection, defaults, and preflight.
    runner::prepare(prepared.task, repositories).await.unwrap()
}
#[tokio::test]
async fn copilot_jobs_stream_before_completion_and_isolate_failures_with_a_concurrency_limit() {
    use codeconvoy::agents::copilot::Copilot;
    let directory = tempfile::tempdir().unwrap();
    let prepared = prepare_copilot(directory.path(), &["one", "fail", "three", "four"], 2).await;
    assert_eq!(prepared.task.options["tool_approvals"], "file-edits");
    let prompt = prepared.task.prompt.clone();
    let (tx, mut rx) = mpsc::channel(256);
    let handle = runner::start(100, prepared, Arc::new(Copilot), tx);
    let events = finish(&mut rx, 4).await;
    let (mut active, mut maximum, mut successes, mut failures) = (0, 0, 0, 0);
    let mut finished = std::collections::HashSet::new();
    let mut streamed = std::collections::HashSet::new();
    let mut logs = String::new();
    for event in events {
        match event {
            Event::Started { .. } => {
                active += 1;
                maximum = maximum.max(active);
            }
            Event::Output { job, text, .. } => {
                if text.contains("live fragment without newline") {
                    assert!(
                        !finished.contains(&job),
                        "live output arrived after completion"
                    );
                    streamed.insert(job);
                }
                logs.push_str(&text);
            }
            Event::Finished {
                job,
                status,
                exit_code,
                ..
            } => {
                active -= 1;
                finished.insert(job);
                if status == JobStatus::Succeeded {
                    successes += 1;
                    assert_eq!(exit_code, Some(0));
                } else {
                    failures += 1;
                    assert_eq!(status, JobStatus::Failed);
                    assert_eq!(exit_code, Some(7));
                }
            }
        }
    }
    assert_eq!((maximum, active, successes, failures), (2, 0, 3, 1));
    assert_eq!(streamed.len(), 4);
    assert!(logs.contains("[stderr] fixture diagnostic"));
    assert!(logs.contains("done"));
    for name in ["one", "fail", "three", "four"] {
        assert_eq!(
            std::fs::read_to_string(directory.path().join(name).join("agent-input")).unwrap(),
            prompt
        );
    }
    while !handle.join.is_finished() {
        tokio::task::yield_now().await;
    }
}
#[tokio::test]
async fn copilot_running_cancellation_kills_descendants_without_cancelling_another_repository() {
    use codeconvoy::agents::copilot::Copilot;
    let directory = tempfile::tempdir().unwrap();
    let mut prepared = prepare_copilot(directory.path(), &["tree", "other"], 2).await;
    prepared.task.prompt = "cancel-test".into();
    let (tx, mut rx) = mpsc::channel(256);
    let handle = runner::start(100, prepared, Arc::new(Copilot), tx);
    tokio::time::timeout(Duration::from_secs(5), async {
        while !directory.path().join("tree/descendant-ready").exists() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    handle.cancel(0);
    let events = finish(&mut rx, 2).await;
    assert!(events.iter().any(|e| matches!(
        e,
        Event::Finished {
            job: 0,
            status: JobStatus::Cancelled,
            ..
        }
    )));
    assert!(events.iter().any(|e| matches!(
        e,
        Event::Finished {
            job: 1,
            status: JobStatus::Succeeded,
            ..
        }
    )));
    tokio::time::sleep(Duration::from_millis(1400)).await;
    assert!(!directory.path().join("tree/orphan-survived").exists());
}
#[tokio::test]
async fn copilot_queued_cancel_and_preflight_changes_never_spawn_an_agent() {
    use codeconvoy::agents::copilot::Copilot;
    let directory = tempfile::tempdir().unwrap();
    let prepared = prepare_copilot(directory.path(), &["first", "cancelled", "changed"], 1).await;
    std::fs::write(
        directory.path().join("changed/new-file"),
        "changed after preflight",
    )
    .unwrap();
    let (tx, mut rx) = mpsc::channel(256);
    let handle = runner::start(100, prepared, Arc::new(Copilot), tx);
    handle.cancel(1);
    let events = finish(&mut rx, 3).await;
    assert!(!directory.path().join("cancelled/agent-input").exists());
    assert!(!directory.path().join("changed/agent-input").exists());
    assert!(events.iter().any(|e| matches!(
        e,
        Event::Finished {
            job: 1,
            status: JobStatus::Cancelled,
            ..
        }
    )));
    assert!(events.iter().any(|e| matches!(e, Event::Finished { job: 2, status: JobStatus::Failed, detail, .. } if detail.contains("changed after preflight"))));
}
