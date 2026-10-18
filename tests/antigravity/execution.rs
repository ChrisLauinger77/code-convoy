use super::*;
use codeconvoy::{
    domain::JobStatus,
    git,
    runner::{self, Event, RunManager},
};
use std::{fs, process::Command, time::Duration};

struct Outcome {
    status: JobStatus,
    exit_code: Option<i32>,
    detail: String,
    activity: String,
    raw: String,
}

async fn repository(root: &Path, name: &str) -> Repository {
    let path = root.join(name);
    fs::create_dir_all(&path).unwrap();
    for args in [
        &["init", "--quiet"][..],
        &["config", "core.autocrlf", "false"],
    ] {
        assert!(
            Command::new("git")
                .arg("-C")
                .arg(&path)
                .args(args)
                .status()
                .unwrap()
                .success()
        );
    }
    git::register(&path).await.unwrap()
}

async fn run(prompt: &str, options: &[(&str, &str)]) -> (tempfile::TempDir, Repository, Outcome) {
    let temp = tempfile::tempdir().unwrap();
    let repository = repository(temp.path(), "source ü with spaces").await;
    let mut task = task();
    task.prompt = prompt.into();
    task.options = [(
        "executable".to_owned(),
        env!("CARGO_BIN_EXE_codeconvoy-test-agent").to_owned(),
    )]
    .into();
    for (key, value) in options {
        task.options.insert((*key).into(), (*value).into());
    }
    let prepared = runner::prepare(task, vec![repository.clone()])
        .await
        .unwrap();
    let (tx, mut rx) = tokio::sync::mpsc::channel(256);
    let mut manager = RunManager::new(1, tx);
    manager
        .start(1, prepared, agents::backend(AgentId::Antigravity).unwrap())
        .unwrap();
    let outcome = tokio::time::timeout(Duration::from_secs(30), async {
        let (mut activity, mut raw) = (String::new(), String::new());
        loop {
            match rx.recv().await.unwrap() {
                Event::Output {
                    text, raw: bytes, ..
                } => {
                    activity.push_str(&text);
                    raw.push_str(&bytes);
                }
                Event::Finished {
                    status,
                    exit_code,
                    detail,
                    ..
                } => {
                    break Outcome {
                        status,
                        exit_code,
                        detail,
                        activity,
                        raw,
                    };
                }
                _ => {}
            }
        }
    })
    .await
    .unwrap();
    manager.shutdown();
    tokio::time::timeout(Duration::from_secs(10), &mut manager.join)
        .await
        .unwrap()
        .unwrap();
    (temp, repository, outcome)
}

#[tokio::test]
async fn successful_run_receives_the_task_on_stdin_in_the_assigned_checkout() {
    let prompt = "Review --help\n$(touch nope); `whoami` \"quotes\" Grüße 🌍";
    let (_temp, repository, outcome) = run(prompt, &[("model", "fixture-model")]).await;
    assert_eq!(outcome.status, JobStatus::Succeeded, "{}", outcome.detail);
    assert_eq!(outcome.exit_code, Some(0));
    assert!(outcome.detail.contains("SUCCESS result"));
    assert!(
        outcome
            .activity
            .contains("Antigravity conversation: antigravity-fixture")
    );
    assert!(outcome.activity.contains("Fixture task complete."));
    assert!(
        outcome
            .activity
            .contains("[stderr] Antigravity fixture diagnostic")
    );
    assert!(
        outcome
            .raw
            .lines()
            .any(|line| line.starts_with("{\"event\":\"result\""))
    );
    assert_eq!(
        fs::read_to_string(repository.path.join("agent-input")).unwrap(),
        prompt
    );
    let args: Vec<String> =
        serde_json::from_slice(&fs::read(repository.path.join("antigravity-args")).unwrap())
            .unwrap();
    let mut expected_task = task();
    expected_task.prompt = prompt.into();
    expected_task
        .options
        .insert("model".into(), "fixture-model".into());
    let expected = Antigravity.build(&expected_task, &repository).unwrap();
    assert_eq!(
        args,
        expected
            .args
            .iter()
            .map(|arg| arg.to_str().unwrap())
            .collect::<Vec<_>>()
    );
    assert!(!repository.path.join("nope").exists());
}

#[tokio::test]
async fn failed_interrupted_malformed_and_incomplete_runs_fail_closed() {
    for (scenario, expected) in [
        ("missing-auth", "sign-in is required"),
        (
            "error",
            "Antigravity reported ERROR — fixture quota exhausted",
        ),
        (
            "error-exit-zero",
            "Antigravity reported ERROR — fixture model failure",
        ),
        ("interrupted", "INTERRUPTED"),
        ("malformed", "Invalid Antigravity JSON record"),
        ("incomplete", "incomplete"),
        ("wrong-cwd", "different or unavailable working directory"),
    ] {
        let (_temp, _repository, outcome) =
            run(&format!("antigravity-fixture:{scenario}"), &[]).await;
        assert_eq!(
            outcome.status,
            JobStatus::Failed,
            "{scenario}: {}",
            outcome.detail
        );
        assert!(
            outcome.detail.contains(expected),
            "{scenario}: {}",
            outcome.detail
        );
    }
}

#[tokio::test]
async fn refused_actions_are_reported_and_edits_are_retained() {
    let (_temp, _repository, outcome) = run("antigravity-fixture:denied", &[]).await;
    assert_eq!(outcome.status, JobStatus::Succeeded, "{}", outcome.detail);
    assert!(outcome.detail.contains("refused 1 tool action(s)"));
    assert!(outcome.activity.contains("Warning: Antigravity refused 1"));
    assert!(
        outcome
            .activity
            .contains("[stderr] Antigravity fixture refused")
    );

    // Windows sandbox support depends on the host build; see the unit test.
    let sandbox = !cfg!(windows);
    let mut options = vec![("permissions", "skip")];
    if sandbox {
        options.push(("sandbox", "enabled"));
    }
    let (_temp, repository, outcome) = run("antigravity-fixture:edit", &options).await;
    assert_eq!(outcome.status, JobStatus::Succeeded, "{}", outcome.detail);
    assert_eq!(
        fs::read_to_string(repository.path.join("tracked.txt")).unwrap(),
        "Antigravity fixture edit\n"
    );
    let args = fs::read_to_string(repository.path.join("antigravity-args")).unwrap();
    assert!(args.contains("--dangerously-skip-permissions"));
    assert_eq!(args.contains("--sandbox"), sandbox);
    assert!(!args.contains("--print-timeout"));
}
