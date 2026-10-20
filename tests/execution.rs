#![cfg(feature = "test-support")]
#![allow(clippy::unwrap_used)]
#[path = "support/executable.rs"]
mod executable;

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
    let mut handle = runner::RunManager::new(16, tx);
    handle.start(42, prepared, Arc::new(Fixture)).unwrap();
    let events = finish(&mut rx, 4).await;
    let (mut active, mut maximum, mut successes, mut failures) = (0, 0, 0, 0);
    let mut output = String::new();
    for event in events {
        match event {
            Event::Preparing { .. } | Event::Result { .. } => {
                panic!("direct job emitted isolated lifecycle")
            }
            Event::Queued { .. } | Event::Changes { .. } => {}
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
    while !handle.is_idle() {
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
    let mut handle = runner::RunManager::new(16, tx);
    handle.start(1, prepared, Arc::new(Fixture)).unwrap();
    handle.cancel(1, 1);
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
    let mut handle = runner::RunManager::new(16, tx);
    handle.start(1, prepared, Arc::new(Fixture)).unwrap();
    tokio::time::timeout(Duration::from_secs(5), async {
        while !directory.path().join("tree/descendant-ready").exists() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    handle.cancel(1, 0);
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
    let mut handle = runner::RunManager::new(16, tx);
    handle.start(100, prepared, Arc::new(Copilot)).unwrap();
    let events = finish(&mut rx, 4).await;
    let (mut active, mut maximum, mut successes, mut failures) = (0, 0, 0, 0);
    let mut finished = std::collections::HashSet::new();
    let mut streamed = std::collections::HashSet::new();
    let mut logs = String::new();
    for event in events {
        match event {
            Event::Preparing { .. } | Event::Result { .. } => {
                panic!("direct job emitted isolated lifecycle")
            }
            Event::Queued { .. } | Event::Changes { .. } => {}
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
    while !handle.is_idle() {
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
    let mut handle = runner::RunManager::new(16, tx);
    handle.start(100, prepared, Arc::new(Copilot)).unwrap();
    tokio::time::timeout(Duration::from_secs(5), async {
        while !directory.path().join("tree/descendant-ready").exists() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    handle.cancel(100, 0);
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
    let mut handle = runner::RunManager::new(16, tx);
    handle.start(100, prepared, Arc::new(Copilot)).unwrap();
    handle.cancel(100, 1);
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

async fn prepare_opencode(root: &Path, names: &[&str], concurrency: usize) -> PreparedRun {
    let mut prepared = prepare(root, names, concurrency).await;
    prepared.task.agent = AgentId::OpenCode;
    prepared.task.options.insert(
        "executable".into(),
        env!("CARGO_BIN_EXE_codeconvoy-test-agent").into(),
    );
    runner::prepare(
        prepared.task,
        prepared
            .repositories
            .into_iter()
            .map(|r| r.repository)
            .collect(),
    )
    .await
    .unwrap()
}

/// The fixture agent linked as `opencode2` emulates the OpenCode 2 interface.
fn opencode2_fixture() -> (tempfile::TempDir, String) {
    let directory = executable::directory();
    let link = directory
        .path()
        .join(format!("opencode2{}", std::env::consts::EXE_SUFFIX));
    executable::install(&link);
    let path = link.to_str().unwrap().to_owned();
    (directory, path)
}

async fn prepare_opencode2(
    root: &Path,
    names: &[&str],
    concurrency: usize,
    executable: &str,
) -> PreparedRun {
    let mut prepared = prepare(root, names, concurrency).await;
    prepared.task.agent = AgentId::OpenCode;
    prepared.task.options = [
        ("executable".into(), executable.into()),
        ("version".into(), "2".into()),
    ]
    .into();
    runner::prepare(
        prepared.task,
        prepared
            .repositories
            .into_iter()
            .map(|r| r.repository)
            .collect(),
    )
    .await
    .unwrap()
}

fn outcomes(events: &[Event]) -> std::collections::BTreeMap<usize, JobStatus> {
    events
        .iter()
        .filter_map(|e| match e {
            Event::Finished { job, status, .. } => Some((*job, *status)),
            _ => None,
        })
        .collect()
}

#[tokio::test]
async fn opencode_2_jobs_run_standalone_in_each_repository_and_isolate_failure() {
    use codeconvoy::agents::opencode::OpenCode;
    let (_fixture, executable) = opencode2_fixture();
    let dir = tempfile::tempdir().unwrap();
    let names = ["Grüße repo 日本 with spaces", "fail", "changed"];
    let prepared = prepare_opencode2(dir.path(), &names, 1, &executable).await;
    assert_eq!(prepared.task.options["version"], "2");
    let prompt = prepared.task.prompt.clone();
    std::fs::write(dir.path().join("changed/unreviewed"), "new change").unwrap();
    let (tx, mut rx) = mpsc::channel(256);
    let mut manager = runner::RunManager::new(1, tx);
    manager.start(300, prepared, Arc::new(OpenCode)).unwrap();
    let events = finish(&mut rx, 3).await;
    let outcomes = outcomes(&events);
    // The fixture rejects --dir and requires --standalone, PWD removal and
    // `run --format json`; it writes agent-input in its working directory.
    assert_eq!(outcomes[&0], JobStatus::Succeeded);
    assert_eq!(outcomes[&1], JobStatus::Failed);
    assert_eq!(outcomes[&2], JobStatus::Failed);
    for name in &names[..2] {
        assert_eq!(
            std::fs::read_to_string(dir.path().join(name).join("agent-input")).unwrap(),
            prompt
        );
    }
    assert!(!dir.path().join("changed/agent-input").exists());
    assert!(events.iter().any(|e| matches!(e, Event::Finished {job:2,detail,..} if detail.contains("changed after preflight"))));
    assert!(events.iter().any(|e| matches!(e, Event::Finished {job:1,detail,..} if detail.contains("completion was not confirmed"))));
}

#[tokio::test]
async fn opencode_2_cancellation_kills_descendants_and_leaves_other_jobs_running() {
    use codeconvoy::agents::opencode::OpenCode;
    let (_fixture, executable) = opencode2_fixture();
    let dir = tempfile::tempdir().unwrap();
    let mut prepared = prepare_opencode2(dir.path(), &["tree", "other"], 2, &executable).await;
    prepared.task.prompt = "cancel-test".into();
    let (tx, mut rx) = mpsc::channel(256);
    let mut manager = runner::RunManager::new(2, tx);
    manager.start(300, prepared, Arc::new(OpenCode)).unwrap();
    tokio::time::timeout(Duration::from_secs(5), async {
        while !dir.path().join("tree/descendant-ready").exists() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    manager.cancel(300, 0);
    let outcomes = outcomes(&finish(&mut rx, 2).await);
    assert_eq!(outcomes[&0], JobStatus::Cancelled);
    assert_eq!(outcomes[&1], JobStatus::Succeeded);
    // The standalone server ran in its own process group; release waited for it.
    #[cfg(unix)]
    for name in ["tree", "other"] {
        assert!(!alive(standalone_server(&dir.path().join(name))), "{name}");
    }
    tokio::time::sleep(Duration::from_millis(1400)).await;
    assert!(!dir.path().join("tree/orphan-survived").exists());
}

#[cfg(unix)]
fn standalone_server(repository: &Path) -> libc::pid_t {
    std::fs::read_to_string(repository.join("opencode2-server.pid"))
        .unwrap()
        .parse()
        .unwrap()
}

#[cfg(unix)]
fn alive(pid: libc::pid_t) -> bool {
    // SAFETY: signal zero only probes.
    let probed = unsafe { libc::kill(pid, 0) };
    probed == 0 || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
}

#[tokio::test]
#[cfg(any(target_os = "macos", target_os = "linux"))]
async fn an_opencode_2_server_that_outlives_run_keeps_the_repository_reserved() {
    use codeconvoy::agents::opencode::OpenCode;
    let (_fixture, executable) = opencode2_fixture();
    let dir = tempfile::tempdir().unwrap();
    // In a directory named orphan, the stand-in leaves a server that ignores
    // stops running, as if run had been killed before its finalizer.
    let prepared = prepare_opencode2(dir.path(), &["orphan"], 1, &executable).await;
    let (tx, mut rx) = mpsc::channel(256);
    let mut manager = runner::RunManager::new(1, tx);
    manager.start(300, prepared, Arc::new(OpenCode)).unwrap();
    let events = finish(&mut rx, 1).await;
    assert_eq!(outcomes(&events)[&0], JobStatus::Failed);
    assert!(
        events.iter().any(
            |e| matches!(e, Event::Finished {detail,..} if detail.contains("remain reserved"))
        )
    );
    let server = standalone_server(&dir.path().join("orphan"));
    assert!(alive(server));
    // SAFETY: the stubborn stand-in ignores SIGTERM for 30 s, so the PID still
    // belongs to this test's own stray server.
    unsafe { libc::kill(server, libc::SIGKILL) };
}

#[tokio::test]
async fn an_opencode_binary_replaced_after_check_cli_fails_before_reading_the_task() {
    use codeconvoy::agents::opencode::OpenCode;
    let (_fixture, opencode2) = opencode2_fixture();
    let opencode1 = env!("CARGO_BIN_EXE_codeconvoy-test-agent");
    // Settings for one major version, executed by the other: each CLI rejects
    // the other's distinguishing argument, and nothing is retried.
    for (version, executable, rejection) in [
        ("1", opencode2.as_str(), "Unrecognized flag: --dir"),
        ("2", opencode1, "Unknown argument: standalone"),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let mut prepared = prepare(dir.path(), &["repo"], 1).await;
        prepared.task.agent = AgentId::OpenCode;
        prepared.task.options = [
            ("executable".into(), executable.into()),
            ("version".into(), version.into()),
        ]
        .into();
        let (tx, mut rx) = mpsc::channel(256);
        let mut manager = runner::RunManager::new(1, tx);
        manager.start(300, prepared, Arc::new(OpenCode)).unwrap();
        let events = finish(&mut rx, 1).await;
        assert_eq!(outcomes(&events)[&0], JobStatus::Failed);
        assert!(events.iter().any(|e| match e {
            Event::Output { text, .. } => text.contains(rejection),
            Event::Finished { detail, .. } => detail.contains(rejection),
            _ => false,
        }));
        assert!(!dir.path().join("repo/agent-input").exists());
    }
}

#[tokio::test]
async fn opencode_never_starts_outside_a_missing_or_invalid_execution_directory() {
    use codeconvoy::agents::opencode::OpenCode;
    let (_fixture, opencode2) = opencode2_fixture();
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("not a directory ü");
    std::fs::write(&file, "file").unwrap();
    for (version, executable) in [
        ("1", env!("CARGO_BIN_EXE_codeconvoy-test-agent")),
        ("2", opencode2.as_str()),
    ] {
        for path in [dir.path().join("missing repository"), file.clone()] {
            let task = TaskConfig {
                agent: AgentId::OpenCode,
                prompt: "must not run".into(),
                options: [
                    ("executable".into(), executable.into()),
                    ("version".into(), version.into()),
                ]
                .into(),
                ..Default::default()
            };
            let spec = OpenCode
                .build(
                    &task,
                    &Repository {
                        name: "invalid".into(),
                        path: path.clone(),
                    },
                )
                .unwrap();
            assert_eq!(spec.directory, path);
            let error = process::spawn(&spec).err().unwrap().to_string();
            assert!(error.contains("Cannot start"), "{error}");
        }
    }
    // Neither the test's nor CodeConvoy's own directory received the task.
    assert!(!Path::new("agent-input").exists());
    assert!(!dir.path().join("agent-input").exists());
    // A repository removed after preflight fails its job; nothing falls back.
    let mut prepared = prepare_opencode(dir.path(), &["removed"], 1).await;
    prepared.task.prompt = "must not run".into();
    std::fs::remove_dir_all(dir.path().join("removed")).unwrap();
    let (tx, mut rx) = mpsc::channel(256);
    let mut manager = runner::RunManager::new(1, tx);
    manager.start(300, prepared, Arc::new(OpenCode)).unwrap();
    assert_eq!(outcomes(&finish(&mut rx, 1).await)[&0], JobStatus::Failed);
    assert!(!Path::new("agent-input").exists());
}

#[tokio::test]
async fn opencode_jobs_pass_stdin_and_cwd_and_isolate_failure_and_baseline_changes() {
    use codeconvoy::agents::opencode::OpenCode;
    let dir = tempfile::tempdir().unwrap();
    let prepared = prepare_opencode(dir.path(), &["one", "fail", "changed", "cancelled"], 1).await;
    assert_eq!(prepared.task.options["permissions"], "existing");
    assert_eq!(prepared.task.options["model"], "");
    assert_eq!(prepared.task.options["agent"], "");
    assert_eq!(prepared.task.options["variant"], "");
    let prompt = prepared.task.prompt.clone();
    std::fs::write(dir.path().join("changed/unreviewed"), "new change").unwrap();
    let (tx, mut rx) = mpsc::channel(256);
    let mut manager = runner::RunManager::new(1, tx);
    manager.start(300, prepared, Arc::new(OpenCode)).unwrap();
    manager.cancel(300, 3);
    let events = finish(&mut rx, 4).await;
    let outcomes: std::collections::BTreeMap<_, _> = events
        .iter()
        .filter_map(|e| match e {
            Event::Finished { job, status, .. } => Some((*job, *status)),
            _ => None,
        })
        .collect();
    assert_eq!(outcomes[&0], JobStatus::Succeeded);
    assert_eq!(outcomes[&1], JobStatus::Failed);
    assert_eq!(outcomes[&2], JobStatus::Failed);
    assert_eq!(outcomes[&3], JobStatus::Cancelled);
    for name in ["one", "fail"] {
        assert_eq!(
            std::fs::read_to_string(dir.path().join(name).join("agent-input")).unwrap(),
            prompt
        );
    }
    for name in ["changed", "cancelled"] {
        assert!(!dir.path().join(name).join("agent-input").exists());
    }
    assert!(events.iter().any(|e| matches!(e, Event::Finished {job:2,detail,..} if detail.contains("changed after preflight"))));
    let log: String = events
        .iter()
        .filter_map(|e| match e {
            Event::Output { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect();
    assert!(log.contains("[stderr] fixture diagnostic"));
    assert!(
        log.contains("OpenCode step started")
            && log.contains("done")
            && log.contains("fixture failure")
    );
    let raw: String = events
        .iter()
        .filter_map(|event| match event {
            Event::Output { raw, .. } => Some(raw.as_str()),
            _ => None,
        })
        .collect();
    assert!(raw.contains("step_start") && raw.contains("fixture failure"));
}

#[tokio::test]
async fn opencode_cancellation_kills_descendants_and_leaves_other_jobs_running() {
    use codeconvoy::agents::opencode::OpenCode;
    let dir = tempfile::tempdir().unwrap();
    let mut prepared = prepare_opencode(dir.path(), &["tree", "other"], 2).await;
    prepared.task.prompt = "cancel-test".into();
    let (tx, mut rx) = mpsc::channel(256);
    let mut manager = runner::RunManager::new(2, tx);
    manager.start(300, prepared, Arc::new(OpenCode)).unwrap();
    tokio::time::timeout(Duration::from_secs(5), async {
        while !dir.path().join("tree/descendant-ready").exists() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    manager.cancel(300, 0);
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
    assert!(!dir.path().join("tree/orphan-survived").exists());
}

async fn prepare_claude(root: &Path, names: &[&str], concurrency: usize) -> PreparedRun {
    let mut prepared = prepare(root, names, concurrency).await;
    prepared.task.agent = AgentId::Claude;
    prepared.task.options.insert(
        "executable".into(),
        env!("CARGO_BIN_EXE_codeconvoy-test-agent").into(),
    );
    runner::prepare(
        prepared.task,
        prepared
            .repositories
            .into_iter()
            .map(|r| r.repository)
            .collect(),
    )
    .await
    .unwrap()
}

#[tokio::test]
async fn claude_jobs_pass_stdin_and_cwd_and_isolate_failure_and_baseline_changes() {
    use codeconvoy::agents::claude::Claude;
    let dir = tempfile::tempdir().unwrap();
    let prepared = prepare_claude(dir.path(), &["one", "fail", "changed", "cancelled"], 1).await;
    assert_eq!(prepared.task.options["permission_mode"], "dontAsk");
    assert_eq!(prepared.task.options["model"], "");
    assert_eq!(prepared.task.options["max_turns"], "");
    assert_eq!(prepared.task.options["effort"], "");
    let prompt = prepared.task.prompt.clone();
    std::fs::write(dir.path().join("changed/unreviewed"), "new change").unwrap();
    let (tx, mut rx) = mpsc::channel(256);
    let mut manager = runner::RunManager::new(1, tx);
    manager.start(300, prepared, Arc::new(Claude)).unwrap();
    manager.cancel(300, 3);
    let events = finish(&mut rx, 4).await;
    let outcomes: std::collections::BTreeMap<_, _> = events
        .iter()
        .filter_map(|e| match e {
            Event::Finished { job, status, .. } => Some((*job, *status)),
            _ => None,
        })
        .collect();
    assert_eq!(outcomes[&0], JobStatus::Succeeded);
    assert_eq!(outcomes[&1], JobStatus::Failed);
    assert_eq!(outcomes[&2], JobStatus::Failed);
    assert_eq!(outcomes[&3], JobStatus::Cancelled);
    for name in ["one", "fail"] {
        assert_eq!(
            std::fs::read_to_string(dir.path().join(name).join("agent-input")).unwrap(),
            prompt
        );
    }
    for name in ["changed", "cancelled"] {
        assert!(!dir.path().join(name).join("agent-input").exists());
    }
    assert!(events.iter().any(|e| matches!(e, Event::Finished {job:2,detail,..} if detail.contains("changed after preflight"))));
    let log: String = events
        .iter()
        .filter_map(|e| match e {
            Event::Output { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect();
    assert!(log.contains("[stderr] fixture diagnostic"));
    assert!(log.contains("init") && log.contains("done") && log.contains("fixture failure"));
}

#[tokio::test]
async fn claude_cancellation_kills_descendants_and_leaves_other_jobs_running() {
    use codeconvoy::agents::claude::Claude;
    let dir = tempfile::tempdir().unwrap();
    let mut prepared = prepare_claude(dir.path(), &["tree", "other"], 2).await;
    prepared.task.prompt = "cancel-test".into();
    let (tx, mut rx) = mpsc::channel(256);
    let mut manager = runner::RunManager::new(2, tx);
    manager.start(300, prepared, Arc::new(Claude)).unwrap();
    tokio::time::timeout(Duration::from_secs(5), async {
        while !dir.path().join("tree/descendant-ready").exists() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    manager.cancel(300, 0);
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
    assert!(!dir.path().join("tree/orphan-survived").exists());
}

#[tokio::test]
async fn direct_completion_observation_arrives_before_finish_for_success_and_failure() {
    let directory = tempfile::tempdir().unwrap();
    let prepared = prepare(directory.path(), &["success", "fail"], 2).await;
    let (tx, mut rx) = mpsc::channel(256);
    let mut manager = runner::RunManager::new(2, tx);
    manager.start(90, prepared, Arc::new(Fixture)).unwrap();
    let events = finish(&mut rx, 2).await;
    for job in 0..2 {
        let observed = events.iter().position(|e| matches!(e, Event::Changes {job:j,changes,..} if *j == job && changes.changed == Some(true) && changes.files.is_some_and(|n| n > 0))).unwrap();
        let finished = events
            .iter()
            .position(|e| matches!(e, Event::Finished {job:j,..} if *j == job))
            .unwrap();
        assert!(observed < finished);
    }
    while !manager.is_idle() {
        tokio::task::yield_now().await;
    }
}
