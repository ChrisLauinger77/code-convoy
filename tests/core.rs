#![allow(clippy::unwrap_used)]
use codeconvoy::{
    agents::{self, AgentBackend, codex::Codex},
    domain::*,
    git,
    persistence::Store,
    process::Stream,
};
use std::{fs, path::Path, process::Command};

fn repo(path: &Path) {
    fs::create_dir_all(path).unwrap();
    assert!(
        Command::new("git")
            .args(["init", "--quiet"])
            .arg(path)
            .status()
            .unwrap()
            .success()
    );
}
fn git_cmd(path: &Path, args: &[&str]) {
    let result = Command::new("git")
        .args(["-c", "commit.gpgsign=false", "-c", "core.hooksPath="])
        .arg("-C")
        .arg(path)
        .args(args)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
}

#[tokio::test]
async fn direct_diff_observation_covers_clean_unborn_staged_and_untracked_files() {
    let temp = tempfile::tempdir().unwrap();
    repo(temp.path());
    // Inspection takes registered canonical paths, including Windows extended
    // paths and macOS temporary directories reached through symlink aliases.
    let registered = git::register(temp.path()).await.unwrap();
    let path = registered.path.as_path();
    let empty = git::inspect_diff(path).await.unwrap();
    assert_eq!(empty.changed, Some(false));
    assert!(empty.text.contains("--- STAGED CHANGES ---"));
    fs::write(path.join("tracked.txt"), "base\n").unwrap();
    git_cmd(path, &["add", "tracked.txt"]);
    assert_eq!(git::inspect_diff(path).await.unwrap().changed, Some(true));
    git_cmd(
        path,
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
    assert_eq!(git::inspect_diff(path).await.unwrap().changed, Some(false));
    fs::write(path.join("new.txt"), "").unwrap();
    let untracked = git::inspect_diff(path).await.unwrap();
    assert_eq!(untracked.changed, Some(true));
    assert!(untracked.text.contains("?? new.txt"));
    fs::remove_file(path.join("new.txt")).unwrap();
    fs::write(path.join("tracked.txt"), "staged alternative\n").unwrap();
    assert_eq!(git::inspect_diff(path).await.unwrap().changed, Some(true));
    git_cmd(path, &["add", "tracked.txt"]);
    fs::write(path.join("tracked.txt"), "base\n").unwrap();
    let index = fs::read(path.join(".git/index")).unwrap();
    let alternative = git::inspect_diff(path).await.unwrap();
    assert_eq!(alternative.changed, Some(true));
    assert!(alternative.text.contains("+staged alternative"));
    assert_eq!(fs::read(path.join(".git/index")).unwrap(), index);
}

#[test]
fn codex_arguments_are_separate_and_options_are_backend_owned() {
    let task = TaskConfig {
        prompt: "-rf $(touch /tmp/nope)\nnot a shell".into(),
        options: [
            ("model".into(), "model with spaces;echo injected".into()),
            ("model_reasoning_effort".into(), "high".into()),
            ("sandbox".into(), "workspace-write".into()),
        ]
        .into(),
        ..Default::default()
    };
    let repository = Repository {
        name: "repo".into(),
        path: "/tmp/repo with spaces".into(),
    };
    let command = Codex.build(&task, &repository).unwrap();
    assert_eq!(command.input.unwrap(), task.prompt.as_bytes());
    assert_eq!(command.directory, repository.path);
    let args: Vec<_> = command.args.iter().map(|a| a.to_string_lossy()).collect();
    assert_eq!(args[0], "--no-daemon");
    assert!(args.contains(&"model with spaces;echo injected".into()));
    assert!(args.contains(&"model_reasoning_effort=\"high\"".into()));
    assert!(!args.iter().any(|a| a.contains("dangerously")));
    assert!(
        Codex
            .validate(&[("thinking".into(), "high".into())].into())
            .is_err()
    );
    assert!(
        Codex
            .validate(&[("sandbox".into(), "danger-full-access".into())].into())
            .is_err()
    );
    assert!(agents::backend(AgentId::Claude).is_ok());
    assert!(agents::backend(AgentId::Copilot).is_ok());
}
#[test]
fn codex_event_decoder_handles_split_utf8_errors_and_large_lines() {
    let mut decoder = Codex.output();
    let line =
        "{\"type\":\"item.completed\",\"item\":{\"type\":\"agent_message\",\"text\":\"Grüße\"}}\n";
    let mut output = String::new();
    for byte in line.as_bytes() {
        output.push_str(&decoder.push(Stream::Stdout, &[*byte]));
    }
    assert!(output.contains("Grüße"));
    decoder.push(Stream::Stdout, b"{\"type\":\"turn.completed\"}\n");
    assert_eq!(decoder.interpret(exit_status(0)).0, JobStatus::Succeeded);
    decoder.push(
        Stream::Stdout,
        b"{\"type\":\"turn.failed\",\"error\":\"failed\"}\n",
    );
    assert_eq!(decoder.interpret(exit_status(0)).0, JobStatus::Failed);
    assert!(
        decoder
            .push(Stream::Stderr, b"diagnostic\n")
            .contains("[stderr]")
    );
    let big = vec![b'a'; 1024 * 1024];
    assert!(!decoder.push(Stream::Stdout, &big).is_empty());
    assert!(decoder.finish().is_empty());
}
#[test]
fn logs_are_bounded_at_utf8_boundaries() {
    let mut log = LogBuffer::default();
    log.append(&"ü".repeat(LOG_LIMIT));
    assert!(log.text.len() <= LOG_LIMIT);
    assert!(log.truncated);
    log.append("tail");
    assert!(log.text.ends_with("tail"));
}
#[test]
fn task_validation_and_terminal_status_are_guarded() {
    let repo = Repository {
        name: "a".into(),
        path: "/tmp/a".into(),
    };
    let mut task = TaskConfig::default();
    assert!(task.validate(std::slice::from_ref(&repo)).is_err());
    task.prompt = "Fix tests".into();
    assert!(task.validate(&[]).is_err());
    assert!(task.validate(&[repo.clone(), repo.clone()]).is_err());
    task.concurrency = 0;
    assert!(task.validate(std::slice::from_ref(&repo)).is_err());
    let mut job = Job::queued(repo);
    job.finish(JobStatus::Failed, Some(2), "failed".into());
    job.finish(JobStatus::Succeeded, Some(0), "late update".into());
    assert_eq!(job.status, JobStatus::Failed);
}
#[test]
fn persistence_is_atomic_excludes_output_and_recovers_interruption() {
    let directory = tempfile::tempdir().unwrap();
    let store = Store::open(directory.path()).unwrap();
    assert!(Store::open(directory.path()).is_err());
    let mut state = AppState::default();
    let mut job = Job::queued(Repository {
        name: "demo".into(),
        path: "/demo".into(),
    });
    job.status = JobStatus::Running;
    job.log.append("sensitive CLI output");
    job.detail = "sensitive diagnostic".into();
    state.runs.push(Run {
        provenance: None,
        id: 1,
        created_at: now(),
        task: TaskConfig::default(),
        jobs: vec![job],
    });
    store.save(&state).unwrap();
    state.draft.prompt = "second save".into();
    store.save(&state).unwrap();
    let json = fs::read_to_string(directory.path().join("state.json")).unwrap();
    assert!(!json.contains("sensitive"));
    let loaded = store.load().unwrap();
    assert_eq!(loaded.draft.prompt, "second save");
    assert_eq!(loaded.runs[0].jobs[0].status, JobStatus::Cancelled);
    assert!(loaded.runs[0].jobs[0].interrupted);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(directory.path().join("state.json"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
    }
    fs::write(directory.path().join("state.json"), b"bad json").unwrap();
    assert!(store.load().is_err());
    assert_eq!(
        fs::read(directory.path().join("state.json")).unwrap(),
        b"bad json"
    );
    fs::write(directory.path().join("state.json"), b"{\"version\":9}").unwrap();
    assert!(store.load().is_err());
}
#[tokio::test]
async fn git_registration_status_and_diff_include_staged_untracked_and_unborn() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("repo with spaces");
    repo(&path);
    let registered = git::register(&path).await.unwrap();
    assert_eq!(registered.name, "repo with spaces");
    assert_eq!(registered.path, path.canonicalize().unwrap());
    assert_eq!(
        git::status(&registered.path).await.unwrap().summary.changed,
        0
    );
    assert!(
        git::status(&registered.path)
            .await
            .unwrap()
            .summary
            .head
            .is_none()
    );
    fs::write(path.join("file.txt"), "original\n").unwrap();
    git_cmd(&path, &["add", "file.txt"]);
    fs::write(path.join("file.txt"), "modified\n").unwrap();
    fs::write(path.join("untracked file.txt"), "new\n").unwrap();
    let state = git::status(&registered.path).await.unwrap();
    assert_eq!(state.summary.changed, 2);
    let diff = git::diff(&registered.path).await.unwrap();
    assert!(diff.contains("+original"));
    assert!(diff.contains("+modified"));
    assert!(diff.contains("untracked file.txt"));
    fs::create_dir(path.join("subdir")).unwrap();
    assert!(git::register(&path.join("subdir")).await.is_err());
    assert!(git::register(directory.path()).await.is_err());
    git_cmd(
        &path,
        &[
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.invalid",
            "commit",
            "--quiet",
            "-m",
            "initial",
        ],
    );
    let tree = git::status(&registered.path).await.unwrap();
    assert!(tree.summary.head.is_some());
    git_cmd(&path, &["checkout", "--detach", "--quiet"]);
    assert_eq!(
        git::status(&registered.path).await.unwrap().summary.branch,
        "(detached HEAD)"
    );
}
#[test]
fn git_status_counts_renames_as_one_entry_and_escapes_filenames() {
    let entries = git::parse_status(b"R  destination\0source\0?? file\nname\0").unwrap();
    assert_eq!(entries.len(), 2);
    assert!(entries[0].contains("from source"));
    assert!(entries[1].contains("\\n"));
    assert!(git::parse_status(b"R  incomplete\0").is_err());
    assert!(git::parse_status(b"bad\0").is_err());
}

#[test]
#[cfg(windows)]
fn git_arguments_preserve_drive_unc_unicode_and_spaces_without_changing_identity() {
    for (native, expected) in [
        (r"\\?\C:\work tree\日本語", r"C:\work tree\日本語"),
        (r"\\?\C:\", r"C:\"),
        (
            r"\\?\UNC\server\share\work tree\ü",
            r"\\server\share\work tree\ü",
        ),
        (r"\\?\UNC\server\share\", r"\\server\share\"),
        (r"C:\ordinary path\ü", r"C:\ordinary path\ü"),
        (
            r"\\server\share\ordinary path",
            r"\\server\share\ordinary path",
        ),
    ] {
        let path = std::path::PathBuf::from(native);
        assert_eq!(
            git::path_argument(&path),
            std::ffi::OsString::from(expected)
        );
        assert_eq!(path.as_os_str(), native);
    }
}

#[test]
fn overlapping_repositories_and_relative_executables_are_rejected() {
    let task = TaskConfig {
        prompt: "Review".into(),
        ..Default::default()
    };
    let parent = Repository {
        name: "parent".into(),
        path: "/parent".into(),
    };
    let child = Repository {
        name: "child".into(),
        path: "/parent/child".into(),
    };
    assert!(
        task.validate(&[parent, child])
            .unwrap_err()
            .to_string()
            .contains("overlap")
    );
    assert!(
        Codex
            .detection(
                &[("executable".into(), "./codex".into())].into(),
                Path::new(".")
            )
            .is_err()
    );
}

#[test]
fn session_log_budget_evicts_oldest_logs() {
    let mut state = AppState::default();
    let mut job = Job::queued(Repository {
        name: "repo".into(),
        path: "/repo".into(),
    });
    job.log.append(&"x".repeat(LOG_LIMIT));
    state.runs.push(Run {
        provenance: None,
        id: 2,
        created_at: now(),
        task: TaskConfig::default(),
        jobs: vec![job.clone()],
    });
    state.runs.push(Run {
        provenance: None,
        id: 1,
        created_at: now(),
        task: TaskConfig::default(),
        jobs: vec![job; TOTAL_LOG_LIMIT / LOG_LIMIT],
    });
    state.trim_logs();
    let size: usize = state
        .runs
        .iter()
        .flat_map(|r| &r.jobs)
        .map(|j| j.log.text.len())
        .sum();
    assert!(size <= TOTAL_LOG_LIMIT);
    assert!(!state.runs[0].jobs[0].log.text.is_empty());
    assert!(state.runs[1].jobs[0].log.text.is_empty());
    assert_eq!(state.runs[1].jobs[0].log.text.capacity(), 0);
    assert!(state.runs[1].jobs[0].log.truncated);
}

fn exit_status(code: i32) -> std::process::ExitStatus {
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        std::process::ExitStatus::from_raw(code << 8)
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::ExitStatusExt;
        std::process::ExitStatus::from_raw(code as u32)
    }
}
