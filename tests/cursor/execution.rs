use super::*;
use codeconvoy::{
    domain::{ExecutionMode, JobStatus},
    git,
    process::{self, Cancellation},
    runner::{self, Event, RunManager},
    worktrees,
};
use std::{fs, process::Command, time::Duration};

fn git_command(path: &Path, args: &[&str]) -> String {
    let result = Command::new("git")
        .arg("-C")
        .arg(path)
        .args(["-c", "commit.gpgsign=false"])
        .args(args)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    String::from_utf8(result.stdout).unwrap().trim().into()
}

#[tokio::test]
async fn managed_isolated_cursor_edit_retains_fixed_base_and_preserves_dirty_source() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source ü with spaces");
    fs::create_dir(&source).unwrap();
    git_command(&source, &["init", "--quiet"]);
    git_command(&source, &["config", "core.autocrlf", "false"]);
    fs::write(source.join("tracked.txt"), "committed base\n").unwrap();
    fs::create_dir_all(source.join(".cursor/rules")).unwrap();
    fs::write(source.join("AGENTS.md"), "Committed agent instructions\n").unwrap();
    fs::write(
        source.join(".cursor/rules/project.mdc"),
        "---\nalwaysApply: true\n---\nCommitted Cursor instructions\n",
    )
    .unwrap();
    git_command(
        &source,
        &["add", "tracked.txt", "AGENTS.md", ".cursor/rules"],
    );
    git_command(
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
    let base = git_command(&source, &["rev-parse", "HEAD"]);
    fs::write(source.join("tracked.txt"), "source dirty sentinel\n").unwrap();
    fs::write(source.join("AGENTS.md"), "Uncommitted agent instructions\n").unwrap();
    fs::write(
        source.join(".cursor/rules/project.mdc"),
        "Uncommitted Cursor instructions\n",
    )
    .unwrap();
    fs::write(source.join("untracked.txt"), "source-only untracked\n").unwrap();
    let repository = git::register(&source).await.unwrap();
    let mut task = task();
    task.prompt = "cursor-fixture:edit".into();
    task.execution_mode = ExecutionMode::IsolatedWorktree;
    task.options = [
        (
            "executable".into(),
            env!("CARGO_BIN_EXE_codeconvoy-test-agent").into(),
        ),
        ("permissions".into(), "force".into()),
        ("workspace_trust".into(), "trust".into()),
    ]
    .into();
    let prepared = runner::prepare(task, vec![repository.clone()])
        .await
        .unwrap();
    assert!(prepared.dirty());
    let (tx, mut rx) = tokio::sync::mpsc::channel(256);
    let mut manager =
        RunManager::with_worktree_directory(1, tx, temp.path().join("owned results ü"));
    manager
        .start(1, prepared, agents::backend(AgentId::Cursor).unwrap())
        .unwrap();
    let events = tokio::time::timeout(Duration::from_secs(30), async {
        let mut events = Vec::new();
        loop {
            let event = rx.recv().await.unwrap();
            let finished = matches!(event, Event::Finished { .. });
            events.push(event);
            if finished {
                break;
            }
        }
        events
    })
    .await;
    manager.shutdown();
    tokio::time::timeout(Duration::from_secs(10), &mut manager.join)
        .await
        .unwrap()
        .unwrap();
    let events = events.unwrap();
    assert!(
        events.iter().any(|event| matches!(
            event,
            Event::Finished {
                status: JobStatus::Succeeded,
                exit_code: Some(0),
                ..
            }
        )),
        "{events:?}"
    );
    let metadata = events
        .iter()
        .find_map(|event| match event {
            Event::Preparing {
                worktree: Some(metadata),
                ..
            } => Some(metadata),
            _ => None,
        })
        .unwrap();
    assert_eq!(metadata.base_commit, base);
    assert_eq!(metadata.repository, repository);
    // Verify CodeConvoy's checkout/transport boundary, not Cursor's rule loading.
    assert_eq!(
        fs::read_to_string(metadata.path.join("AGENTS.md")).unwrap(),
        "Committed agent instructions\n"
    );
    assert_eq!(
        fs::read_to_string(metadata.path.join(".cursor/rules/project.mdc")).unwrap(),
        "---\nalwaysApply: true\n---\nCommitted Cursor instructions\n"
    );
    assert_eq!(git_command(&metadata.path, &["rev-parse", "HEAD"]), base);
    assert_eq!(
        git_command(&metadata.path, &["rev-parse", "--abbrev-ref", "HEAD"]),
        "HEAD"
    );
    assert_eq!(
        fs::read_to_string(metadata.path.join("tracked.txt")).unwrap(),
        "Cursor fixture edit\n"
    );
    assert!(!metadata.path.join("untracked.txt").exists());
    assert_eq!(
        fs::read_to_string(metadata.path.join("agent-input")).unwrap(),
        "cursor-fixture:edit"
    );
    assert_eq!(
        Path::new(
            fs::read_to_string(metadata.path.join("cursor-cwd"))
                .unwrap()
                .trim()
        )
        .canonicalize()
        .unwrap(),
        metadata.path
    );
    assert_eq!(
        worktrees::inspect(metadata).await.unwrap().changed,
        Some(true)
    );
    assert_eq!(
        fs::read_to_string(source.join("tracked.txt")).unwrap(),
        "source dirty sentinel\n"
    );
    assert_eq!(
        fs::read_to_string(source.join("untracked.txt")).unwrap(),
        "source-only untracked\n"
    );
    assert_eq!(git_command(&source, &["rev-parse", "HEAD"]), base);
    assert_eq!(
        fs::read_to_string(source.join("AGENTS.md")).unwrap(),
        "Uncommitted agent instructions\n"
    );
    assert_eq!(
        fs::read_to_string(source.join(".cursor/rules/project.mdc")).unwrap(),
        "Uncommitted Cursor instructions\n"
    );
    let raw: String = events
        .iter()
        .filter_map(|event| match event {
            Event::Output { raw, .. } => Some(raw.as_str()),
            _ => None,
        })
        .collect();
    assert!(raw.contains("\"type\":\"result\"") && raw.contains("Cursor fixture diagnostic"));
}

#[tokio::test]
async fn real_process_fixture_checks_prompt_transport_and_failed_or_incomplete_outcomes() {
    for (prompt, expected) in [
        ("--help", JobStatus::Succeeded),
        ("--version", JobStatus::Succeeded),
        ("logout", JobStatus::Succeeded),
        (
            "--help\n$(touch nope); `whoami` Grüße 🌍",
            JobStatus::Succeeded,
        ),
        ("cursor-fixture:missing-auth", JobStatus::Failed),
        ("cursor-fixture:failure", JobStatus::Failed),
        ("cursor-fixture:permission", JobStatus::Failed),
        ("cursor-fixture:interrupted", JobStatus::Failed),
        ("cursor-fixture:malformed", JobStatus::Failed),
        ("cursor-fixture:incomplete", JobStatus::Failed),
    ] {
        let temp = tempfile::tempdir().unwrap();
        fs::create_dir_all(temp.path().join(".cursor/rules")).unwrap();
        fs::write(temp.path().join("AGENTS.md"), "Direct agent instructions\n").unwrap();
        fs::write(
            temp.path().join(".cursor/rules/project.mdc"),
            "---\nalwaysApply: true\n---\nDirect Cursor instructions\n",
        )
        .unwrap();
        let mut task = task();
        task.prompt = prompt.into();
        task.options.insert(
            "executable".into(),
            env!("CARGO_BIN_EXE_codeconvoy-test-agent").into(),
        );
        let repository = Repository {
            name: "fixture".into(),
            path: temp.path().into(),
        };
        let spec = Cursor.build(&task, &repository).unwrap();
        let decoder = std::sync::Mutex::new(Cursor.output());
        let execution = process::execute(
            Cursor.spawn(&spec).unwrap(),
            spec.input,
            &Cancellation::default(),
            process::cancel,
            |stream, bytes| {
                decoder.lock().unwrap().push(stream, bytes);
            },
        )
        .await
        .unwrap();
        let mut decoder = decoder.into_inner().unwrap();
        decoder.finish();
        let (outcome, detail) = decoder.interpret(execution.status.unwrap());
        assert_eq!(outcome, expected, "{detail}");
        assert_eq!(
            fs::read_to_string(temp.path().join("agent-input")).unwrap(),
            prompt
        );
        assert_eq!(
            fs::read_to_string(temp.path().join("AGENTS.md")).unwrap(),
            "Direct agent instructions\n"
        );
        assert_eq!(
            fs::read_to_string(temp.path().join(".cursor/rules/project.mdc")).unwrap(),
            "---\nalwaysApply: true\n---\nDirect Cursor instructions\n"
        );
        assert!(!temp.path().join("nope").exists());
        if prompt.contains("missing-auth") {
            assert!(detail.contains("agent login"));
        }
    }
}

#[tokio::test]
async fn missing_cursor_executable_has_actionable_detection_failure() {
    let temp = tempfile::tempdir().unwrap();
    let options = [(
        "executable".into(),
        temp.path().join("missing-cursor").display().to_string(),
    )]
    .into();
    let error = agents::detect(&Cursor, &options, temp.path())
        .await
        .unwrap_err();
    let error = agents::availability::CliError::from_error(error);
    assert!(error.missing);
    assert!(
        error
            .detail
            .contains("Check the executable path and installation")
    );
}
