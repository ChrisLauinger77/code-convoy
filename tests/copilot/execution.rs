//! Real managed processes with synthetic Copilot behavior; no provider access.
use super::*;
use codeconvoy::{
    domain::ExecutionMode,
    git,
    runner::{self, Event, RunManager},
};
use std::{fs, path::PathBuf, process::Command, time::Duration};

#[path = "../support/executable.rs"]
mod executable;

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

fn raw(events: &[Event]) -> String {
    events
        .iter()
        .filter_map(|event| match event {
            Event::Output { raw, .. } => Some(raw.as_str()),
            _ => None,
        })
        .collect()
}

struct Harness {
    _root: tempfile::TempDir,
    _fixture_root: tempfile::TempDir,
    source: Repository,
    executable: PathBuf,
    base: String,
    manager: RunManager,
    rx: tokio::sync::mpsc::Receiver<Event>,
    events: Vec<Event>,
}
/// Fixture CLI generations: 1.0.65, 1.0.99-1 (--trust-folder) and 1.0.103
/// (--sync-session instead of --no-remote/--no-remote-export).
const CLIS: [&str; 3] = ["old", "new", "sync"];

impl Harness {
    async fn new(cli: &str) -> Self {
        let root = tempfile::tempdir().unwrap();
        let fixture_root = executable::directory();
        let path = root.path().join("source ü with spaces");
        fs::create_dir(&path).unwrap();
        git_command(&path, &["init", "--quiet"]);
        git_command(&path, &["config", "core.autocrlf", "false"]);
        fs::write(path.join("tracked.txt"), "committed base\n").unwrap();
        git_command(&path, &["add", "tracked.txt"]);
        git_command(
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
        let base = git_command(&path, &["rev-parse", "HEAD"]);
        fs::write(path.join("tracked.txt"), "source dirty sentinel\n").unwrap();
        fs::write(path.join("untracked.txt"), "source only\n").unwrap();
        let source = git::register(&path).await.unwrap();
        let executable = fixture_root.path().join(format!(
            "copilot-trust-{cli}{}",
            std::env::consts::EXE_SUFFIX
        ));
        executable::install(&executable);
        let (tx, rx) = tokio::sync::mpsc::channel(256);
        let manager =
            RunManager::with_worktree_directory(1, tx, root.path().join("owned results ü"));
        Self {
            _root: root,
            _fixture_root: fixture_root,
            source,
            executable,
            base,
            manager,
            rx,
            events: Vec::new(),
        }
    }
    fn task(&self, mode: &str, timeout: &str) -> TaskConfig {
        TaskConfig {
            agent: AgentId::Copilot,
            execution_mode: ExecutionMode::IsolatedWorktree,
            prompt: format!(
                "copilot-trust-fixture:{mode}\n--help\n$(touch nope); `whoami` Grüße 🌍"
            ),
            options: [
                (
                    "executable".into(),
                    self.executable.to_str().unwrap().into(),
                ),
                ("timeout_seconds".into(), timeout.into()),
            ]
            .into(),
            ..Default::default()
        }
    }
    async fn start(&mut self, id: u64, mode: &str, timeout: &str) -> TaskConfig {
        let task = self.task(mode, timeout);
        let prepared = runner::prepare(task.clone(), vec![self.source.clone()])
            .await
            .unwrap();
        self.manager
            .start(id, prepared, agents::backend(AgentId::Copilot).unwrap())
            .unwrap();
        task
    }
    async fn until(&mut self, predicate: impl Fn(&[Event]) -> bool) {
        tokio::time::timeout(Duration::from_secs(30), async {
            while !predicate(&self.events) {
                self.events.push(self.rx.recv().await.unwrap());
            }
        })
        .await
        .unwrap_or_else(|_| panic!("Copilot fixture lifecycle timed out: {:?}", self.events));
    }
    async fn finished(&mut self, id: u64) {
        self.until(|events| {
            events
                .iter()
                .any(|event| matches!(event, Event::Finished { run, .. } if *run == id))
        })
        .await;
    }
    fn worktree(&self, id: u64) -> &codeconvoy::domain::WorktreeMetadata {
        self.events
            .iter()
            .find_map(|event| match event {
                Event::Preparing {
                    run,
                    worktree: Some(metadata),
                    ..
                } if *run == id => Some(metadata),
                _ => None,
            })
            .unwrap()
    }
    fn outcome(&self, id: u64) -> (JobStatus, &str) {
        self.events
            .iter()
            .find_map(|event| match event {
                Event::Finished {
                    run,
                    status,
                    detail,
                    ..
                } if *run == id => Some((*status, detail.as_str())),
                _ => None,
            })
            .unwrap()
    }
    fn assert_source_preserved(&self) {
        assert_eq!(
            fs::read_to_string(self.source.path.join("tracked.txt")).unwrap(),
            "source dirty sentinel\n"
        );
        assert_eq!(
            fs::read_to_string(self.source.path.join("untracked.txt")).unwrap(),
            "source only\n"
        );
        assert_eq!(
            git_command(&self.source.path, &["rev-parse", "HEAD"]),
            self.base
        );
    }
    async fn close(&mut self) {
        self.manager.shutdown();
        tokio::time::timeout(Duration::from_secs(10), &mut self.manager.join)
            .await
            .unwrap()
            .unwrap();
    }
}

#[tokio::test]
async fn fresh_owned_worktrees_receive_stdin_without_granting_trust_on_old_and_new_clis() {
    for cli in CLIS {
        let mut h = Harness::new(cli).await;
        let task = h.start(1, "complete", "").await;
        h.finished(1).await;
        h.close().await;
        assert_eq!(h.outcome(1).0, JobStatus::Succeeded, "{:?}", h.events);
        let metadata = h.worktree(1);
        assert_eq!(metadata.base_commit, h.base);
        assert_eq!(git_command(&metadata.path, &["rev-parse", "HEAD"]), h.base);
        assert_eq!(
            git_command(&metadata.path, &["rev-parse", "--abbrev-ref", "HEAD"]),
            "HEAD"
        );
        let receipt: serde_json::Value =
            serde_json::from_slice(&fs::read(metadata.path.join("copilot-received.json")).unwrap())
                .unwrap();
        assert_eq!(receipt["input"], task.prompt);
        assert_eq!(
            Path::new(receipt["directory"].as_str().unwrap())
                .canonicalize()
                .unwrap(),
            metadata.path
        );
        let args = receipt["arguments"].as_array().unwrap();
        for required in ["--no-ask-user", "--allow-tool=write", "--deny-tool=shell"] {
            assert!(args.iter().any(|arg| arg == required));
        }
        for forbidden in [
            "--trust-folder",
            "--allow-all",
            "--allow-all-paths",
            "--allow-all-urls",
            "--prompt",
            "-p",
            "--interactive",
        ] {
            assert!(!args.iter().any(|arg| arg == forbidden));
        }
        // Launch preflight's help check selects the remote flags this CLI offers.
        let legacy = cli != "sync";
        for (flag, expected) in [
            ("--no-remote", legacy),
            ("--no-remote-export", legacy),
            ("--sync-session=off", !legacy),
        ] {
            assert_eq!(
                args.iter().any(|arg| arg == flag),
                expected,
                "{cli}: {flag}"
            );
        }
        assert_eq!(
            fs::read_to_string(metadata.path.join("tracked.txt")).unwrap(),
            "Copilot fixture edit\n"
        );
        assert!(!metadata.path.join("untracked.txt").exists());
        assert!(!metadata.path.join("nope").exists());
        h.assert_source_preserved();
    }
}

#[tokio::test]
async fn trust_rejection_and_confirmation_eof_fail_with_raw_diagnostics_and_actionable_details() {
    for cli in CLIS {
        for mode in ["reject", "confirmation"] {
            let mut h = Harness::new(cli).await;
            h.start(1, mode, "60").await;
            h.finished(1).await;
            h.close().await;
            let (status, detail) = h.outcome(1);
            assert_eq!(status, JobStatus::Failed);
            assert!(detail.contains("workspace trust") && detail.contains("outside CodeConvoy"));
            assert!(!detail.contains("timed out"));
            let raw = raw(&h.events);
            if mode == "reject" {
                assert!(raw.contains("Error: Workspace trust required"));
            } else {
                assert!(raw.contains("Do you trust the files in this folder?"));
                assert!(raw.contains("stdin is closed"));
            }
            assert_eq!(
                fs::read_to_string(h.worktree(1).path.join("tracked.txt")).unwrap(),
                "committed base\n"
            );
            h.assert_source_preserved();
        }
    }
}

#[tokio::test]
async fn blocked_confirmation_is_bounded_or_cancelled_and_tree_cleanup_releases_capacity() {
    for cli in CLIS {
        for cancel in [false, true] {
            let mut h = Harness::new(cli).await;
            h.start(1, "block", if cancel { "60" } else { "2" }).await;
            h.until(|events| raw(events).contains("Do you trust the files in this folder?"))
                .await;
            let path = h.worktree(1).path.clone();
            let locks = ["blocked-parent.lock", "blocked-child.lock"].map(|name| {
                let lock = fs::OpenOptions::new()
                    .read(true)
                    .write(true)
                    .open(path.join(name))
                    .unwrap();
                assert!(fs2::FileExt::try_lock_exclusive(&lock).is_err());
                lock
            });
            if cancel {
                h.manager.cancel_run(1);
            }
            h.finished(1).await;
            let (status, detail) = h.outcome(1);
            if cancel {
                assert_eq!(status, JobStatus::Cancelled);
            } else {
                assert_eq!(status, JobStatus::Failed);
                assert!(detail.contains("execution timed out after 2 seconds"));
                assert!(detail.contains("cleanup was confirmed"));
            }
            for lock in &locks {
                fs2::FileExt::try_lock_exclusive(lock).unwrap();
            }
            // A second ordinary job proves both scheduler and repository leases
            // were released only after the blocked process tree stopped.
            h.start(2, "complete", "60").await;
            h.finished(2).await;
            assert_eq!(h.outcome(2).0, JobStatus::Succeeded);
            assert_ne!(h.worktree(2).path, path);
            assert!(path.exists());
            h.assert_source_preserved();
            h.close().await;
        }
    }
}

/// Launched from a terminal, the CLI shares its controlling terminal from a
/// background process group: a confirmation read there stops it, so neither
/// output nor stdin EOF ends the job. The deadline and Stop must still do so.
#[cfg(unix)]
#[tokio::test]
async fn confirmation_that_stops_the_cli_like_a_terminal_read_is_bounded_or_cancelled() {
    for cancel in [false, true] {
        let mut h = Harness::new("sync").await;
        h.start(1, "terminal", if cancel { "60" } else { "3" })
            .await;
        h.until(|events| raw(events).contains("Do you trust the files in this folder?"))
            .await;
        let path = h.worktree(1).path.clone();
        let pid = fs::read_to_string(path.join("stopped.pid")).unwrap();
        tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                let stat = Command::new("ps")
                    .args(["-o", "stat=", "-p", &pid])
                    .output()
                    .unwrap();
                if String::from_utf8_lossy(&stat.stdout)
                    .trim_start()
                    .starts_with('T')
                {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("fixture never reached the stopped state");
        let lock = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(path.join("stopped.lock"))
            .unwrap();
        assert!(fs2::FileExt::try_lock_exclusive(&lock).is_err());
        if cancel {
            h.manager.cancel_run(1);
        }
        h.finished(1).await;
        let (status, detail) = h.outcome(1);
        if cancel {
            assert_eq!(status, JobStatus::Cancelled);
        } else {
            assert_eq!(status, JobStatus::Failed);
            assert!(detail.contains("execution timed out after 3 seconds"));
            assert!(detail.contains("cleanup was confirmed"));
        }
        fs2::FileExt::try_lock_exclusive(&lock).unwrap();
        h.start(2, "complete", "60").await;
        h.finished(2).await;
        assert_eq!(h.outcome(2).0, JobStatus::Succeeded);
        assert_ne!(h.worktree(2).path, path);
        h.assert_source_preserved();
        h.close().await;
    }
}

#[tokio::test]
#[ignore = "Requires a real Copilot executable; tests only startup through the authentication gate in disposable directories"]
async fn real_copilot_stdin_in_fresh_worktree_reaches_authentication_without_a_trust_prompt() {
    let mut h = Harness::new("new").await;
    let path = h._root.path().join("real CLI worktree ü");
    git_command(
        &h.source.path,
        &[
            "worktree",
            "add",
            "--detach",
            path.to_str().unwrap(),
            "HEAD",
        ],
    );
    let executable =
        std::env::var("CODECONVOY_COPILOT_EXECUTABLE").unwrap_or_else(|_| "copilot".into());
    let task = TaskConfig {
        agent: AgentId::Copilot,
        prompt: "Reply exactly CODECONVOY_TRUST_AUDIT. Do not use tools or change files.".into(),
        options: [("executable".into(), executable)].into(),
        ..Default::default()
    };
    let config = h._root.path().join("empty copilot config");
    let gh_config = h._root.path().join("empty gh config");
    fs::create_dir_all(&config).unwrap();
    fs::create_dir_all(&gh_config).unwrap();
    let env: Vec<(String, String)> = vec![
        ("COPILOT_HOME".into(), config.to_str().unwrap().into()),
        ("GH_CONFIG_DIR".into(), gh_config.to_str().unwrap().into()),
        (
            "COPILOT_CACHE_HOME".into(),
            h._root.path().join("cli cache").to_str().unwrap().into(),
        ),
    ];
    let removed = ["GH_TOKEN", "GITHUB_TOKEN", "COPILOT_GITHUB_TOKEN"].map(str::to_owned);
    // As at launch preflight: the help check selects the remote-session flags.
    let mut help_command = Copilot.detection(&task.options, &path).unwrap();
    help_command.env = env.clone();
    help_command.remove_env.extend(removed.clone());
    let help = process::capture(help_command, 128 * 1024).await.unwrap();
    assert!(help.status.success());
    let help = Copilot.help_text(&help.stdout, &help.stderr);
    Copilot.check_detection_for(&task.options, &help).unwrap();
    for trust in [false, true] {
        let mut spec = Copilot
            .build(
                &task,
                &Repository {
                    name: "real CLI".into(),
                    path: path.clone(),
                },
            )
            .unwrap();
        spec.env = env.clone();
        spec.remove_env.extend(removed.clone());
        if trust && !help.contains("--trust-folder") {
            continue; // Older CLIs must not receive the optional trust flag.
        }
        if trust {
            spec.args.push("--trust-folder".into());
        }
        let decoder = std::sync::Mutex::new((Copilot.output(), String::new()));
        let result = process::execute_until(
            Copilot.spawn(&spec).unwrap(),
            spec.input,
            &process::Cancellation::default(),
            Some(tokio::time::Instant::now() + Duration::from_secs(30)),
            process::cancel,
            |stream, bytes| {
                let mut data = decoder.lock().unwrap();
                data.0.push(stream, bytes);
                data.1.push_str(&String::from_utf8_lossy(bytes));
            },
        )
        .await
        .unwrap();
        assert!(!result.cancelled && !result.timed_out);
        let mut data = decoder.into_inner().unwrap();
        data.0.finish();
        assert_eq!(
            data.0.interpret(result.status.unwrap()).0,
            JobStatus::Failed
        );
        assert!(
            data.1.contains("No authentication information found"),
            "Unexpected startup result: {}",
            data.1
        );
        assert!(!data.1.contains("Do you trust the files in this folder?"));
        assert_eq!(git_command(&path, &["status", "--porcelain"]), "");
        println!(
            "Real Copilot fresh-worktree stdin startup (trust flag={trust}): authentication rejection, no trust prompt; not authenticated E2E."
        );
    }
    h.assert_source_preserved();
    h.close().await;
}
