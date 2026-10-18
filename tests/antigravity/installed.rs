//! Opt-in end-to-end jobs with a real agy through the real runner. Ignored by
//! default and additionally gated by an explicit environment acknowledgement,
//! because a signed-in agy would make a model call.
use super::*;
use codeconvoy::{
    domain::{ExecutionMode, JobStatus},
    git,
    runner::{self, Event, RunManager},
};
use std::{fs, process::Command, time::Duration};

struct Outcome {
    status: JobStatus,
    exit_code: Option<i32>,
    detail: String,
    activity: String,
}

fn executable() -> String {
    std::env::var("CODECONVOY_AGY").unwrap_or_else(|_| "agy".into())
}

fn acknowledged(variable: &str) {
    assert_eq!(
        std::env::var(variable).as_deref(),
        Ok("1"),
        "Set {variable}=1 to confirm the sign-in state this test requires."
    );
}

async fn repository(root: &Path) -> Repository {
    let path = root.join("source ü");
    fs::create_dir_all(&path).unwrap();
    fs::write(path.join("tracked.txt"), "base\n").unwrap();
    for args in [
        &["init", "--quiet"][..],
        &["config", "core.autocrlf", "false"],
        &["add", "tracked.txt"],
        &[
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=fixture@example.invalid",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "-qm",
            "base",
        ],
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

async fn run(
    repository: &Repository,
    prompt: &str,
    mode: ExecutionMode,
    options: &[(&str, &str)],
) -> Outcome {
    let mut task = task();
    task.prompt = prompt.into();
    task.execution_mode = mode;
    task.options = [("executable".to_owned(), executable())].into();
    for (key, value) in options {
        task.options.insert((*key).into(), (*value).into());
    }
    // Preparation runs the real Check CLI (help, version, flag contract).
    let prepared = runner::prepare(task, vec![repository.clone()])
        .await
        .unwrap();
    let (tx, mut rx) = tokio::sync::mpsc::channel(512);
    let worktrees = tempfile::tempdir().unwrap();
    let mut manager = RunManager::with_worktree_directory(1, tx, worktrees.path().into());
    manager
        .start(1, prepared, agents::backend(AgentId::Antigravity).unwrap())
        .unwrap();
    let outcome = tokio::time::timeout(Duration::from_secs(600), async {
        let mut activity = String::new();
        loop {
            match rx.recv().await.unwrap() {
                Event::Output { text, .. } => activity.push_str(&text),
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
                    };
                }
                _ => {}
            }
        }
    })
    .await
    .unwrap();
    manager.shutdown();
    tokio::time::timeout(Duration::from_secs(30), &mut manager.join)
        .await
        .unwrap()
        .unwrap();
    println!(
        "{:?} {:?}: {}",
        outcome.status, outcome.exit_code, outcome.detail
    );
    outcome
}

#[tokio::test]
#[ignore = "Requires an installed agy that is NOT signed in (use an isolated HOME); starts real jobs that stop at the sign-in gate"]
async fn installed_agy_job_without_sign_in_fails_with_sign_in_guidance() {
    acknowledged("CODECONVOY_AGY_SIGNED_OUT");
    let temp = tempfile::tempdir().unwrap();
    let repository = repository(temp.path()).await;
    let mut variants = vec![
        vec![],
        vec![("model", "gemini-fixture-model"), ("effort", "high")],
        vec![("permissions", "skip")],
    ];
    if !cfg!(windows) {
        variants.push(vec![("sandbox", "enabled")]);
    }
    for mode in [ExecutionMode::Direct, ExecutionMode::IsolatedWorktree] {
        for options in &variants {
            let outcome = run(&repository, "Reply with exactly: ok", mode, options).await;
            // Every argv variant passes agy's flag parser and stops at sign-in.
            assert_eq!(outcome.status, JobStatus::Failed, "{options:?}");
            assert_eq!(outcome.exit_code, Some(1), "{options:?}");
            assert!(
                outcome.detail.contains("sign-in is required"),
                "{options:?}: {}",
                outcome.detail
            );
            assert!(outcome.activity.contains("Antigravity result: ERROR"));
        }
    }
    // agy wrote nothing into the registered checkout.
    let status = Command::new("git")
        .arg("-C")
        .arg(&repository.path)
        .args(["status", "--porcelain", "--untracked-files=all"])
        .output()
        .unwrap();
    assert_eq!(String::from_utf8_lossy(&status.stdout), "");
}

#[tokio::test]
#[ignore = "Requires a signed-in agy; makes one model call that creates a file in a disposable repository"]
async fn authenticated_agy_completes_a_disposable_task() {
    acknowledged("CODECONVOY_AGY_AUTHENTICATED");
    let temp = tempfile::tempdir().unwrap();
    let repository = repository(temp.path()).await;
    let outcome = run(
        &repository,
        "Create a file named codeconvoy-e2e.txt in the current directory containing exactly the text ok. Do not modify or create any other file and do not run commands.",
        ExecutionMode::Direct,
        &[],
    )
    .await;
    assert_eq!(outcome.status, JobStatus::Succeeded, "{}", outcome.detail);
    assert_eq!(outcome.exit_code, Some(0));
    assert!(outcome.detail.contains("SUCCESS result (conversation"));
    assert!(outcome.activity.contains("Antigravity conversation:"));
    assert_eq!(
        fs::read_to_string(repository.path.join("codeconvoy-e2e.txt"))
            .unwrap()
            .trim(),
        "ok"
    );
    assert_eq!(
        fs::read_to_string(repository.path.join("tracked.txt")).unwrap(),
        "base\n"
    );
}
