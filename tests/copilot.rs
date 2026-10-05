#![allow(clippy::unwrap_used)]
use codeconvoy::{
    agents::{self, AgentBackend, OptionKind, codex::Codex, copilot::Copilot},
    domain::{AgentId, AppState, JobStatus, Repository, TaskConfig},
    persistence::Store,
    process::{self, Stream},
};
use std::{path::Path, process::ExitStatus};

fn task() -> TaskConfig {
    TaskConfig {
        agent: AgentId::Copilot,
        prompt: "Review this repository.\nTreat $(echo hello) and --help literally. Grüße!".into(),
        ..Default::default()
    }
}
fn repository() -> Repository {
    Repository {
        name: "repo".into(),
        path: std::env::temp_dir().join("repo with spaces"),
    }
}
fn status(code: i32) -> ExitStatus {
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        ExitStatus::from_raw(code << 8)
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::ExitStatusExt;
        ExitStatus::from_raw(code as u32)
    }
}
#[test]
fn default_command_uses_stdin_repository_and_explicit_copilot_permissions() {
    let task = task();
    let repo = repository();
    let spec = Copilot.build(&task, &repo).unwrap();
    assert_eq!(spec.program, "copilot");
    assert_eq!(spec.directory, repo.path);
    assert_eq!(spec.input.as_deref(), Some(task.prompt.as_bytes()));
    let args: Vec<_> = spec.args.iter().map(|v| v.to_str().unwrap()).collect();
    assert_eq!(
        args,
        [
            "--no-auto-update",
            "--no-ask-user",
            "--no-color",
            "--plain-diff",
            "--output-format",
            "text",
            "--stream",
            "on",
            "--no-remote",
            "--no-remote-export",
            "--allow-tool=write",
            "--deny-tool=shell",
        ]
    );
    for key in [
        "GH_TOKEN",
        "GITHUB_TOKEN",
        "COPILOT_GITHUB_TOKEN",
        "COPILOT_HOME",
    ] {
        assert!(!spec.remove_env.iter().any(|k| k == key));
    }
    assert!(spec.remove_env.iter().any(|k| k == "COPILOT_ALLOW_ALL"));
    assert!(spec.env.is_empty());
}
#[test]
fn each_approval_mode_and_optional_capability_maps_without_codex_translation() {
    let repo = repository();
    for mode in ["existing", "file-edits", "all-tools"] {
        let mut task = task();
        task.options = [
            ("tool_approvals".into(), mode.into()),
            ("model".into(), "model with spaces;literal".into()),
            ("reasoning_effort".into(), "max".into()),
            ("temp_access".into(), "disallow".into()),
        ]
        .into();
        let args = Copilot.build(&task, &repo).unwrap().args;
        assert!(
            args.windows(2)
                .any(|a| a == ["--model", "model with spaces;literal"])
        );
        assert!(args.windows(2).any(|a| a == ["--reasoning-effort", "max"]));
        assert!(args.iter().any(|a| a == "--disallow-temp-dir"));
        assert_eq!(
            args.iter().any(|a| a == "--allow-all-tools"),
            mode == "all-tools"
        );
        assert_eq!(
            args.iter().any(|a| a == "--deny-tool=shell"),
            mode == "file-edits"
        );
        assert_eq!(
            args.iter().any(|a| a == "--allow-tool=write"),
            mode == "file-edits"
        );
        for forbidden in [
            "--allow-all",
            "--allow-all-paths",
            "--allow-all-urls",
            "--yolo",
            "--sandbox",
            "--config",
            "--resume",
            "--continue",
            "--share",
            "--fleet",
        ] {
            assert!(!args.iter().any(|a| a == forbidden));
        }
        assert!(
            !Copilot
                .execution_summary(&task.options)
                .contains("Sandbox:")
        );
    }
}
#[test]
fn unsupported_options_and_invalid_execution_settings_are_rejected() {
    let repo = repository();
    for (key, value) in [
        ("sandbox", "workspace-write"),
        ("model_reasoning_effort", "high"),
        ("token", "secret"),
        ("reasoning_effort", "minimal"),
        ("tool_approvals", "yolo"),
        ("temp_access", "anywhere"),
        ("model", "line\nbreak"),
        ("model", "nul\0byte"),
        ("executable", ""),
        ("executable", "./copilot"),
    ] {
        let mut task = task();
        task.options.insert(key.into(), value.into());
        assert!(Copilot.build(&task, &repo).is_err(), "accepted {key}");
    }
    let mut invalid = task();
    invalid.prompt.clear();
    assert!(Copilot.build(&invalid, &repo).is_err());
    invalid = task();
    invalid.agent = AgentId::Codex;
    assert!(Copilot.build(&invalid, &repo).is_err());
}
#[test]
fn capability_choices_come_from_the_inspected_cli_and_registry_is_independent() {
    let copilot = agents::backend(AgentId::Copilot).unwrap();
    let codex = agents::backend(AgentId::Codex).unwrap();
    assert_eq!(copilot.id(), AgentId::Copilot);
    assert!(copilot.options().iter().all(|s| s.key != "sandbox"));
    assert!(codex.options().iter().any(|s| s.key == "sandbox"));
    let effort = copilot
        .options()
        .iter()
        .find(|s| s.key == "reasoning_effort")
        .unwrap();
    let OptionKind::Choice(choices) = effort.kind else {
        panic!("expected choices")
    };
    assert_eq!(
        choices.iter().map(|(v, _)| *v).collect::<Vec<_>>(),
        ["", "none", "low", "medium", "high", "xhigh", "max"]
    );
    assert!(agents::backend(AgentId::Claude).is_err());
}
#[test]
fn detection_uses_same_executable_and_version_policy_and_rejects_incompatible_help() {
    let exe = std::env::temp_dir().join("copilot with spaces");
    let options = [("executable".into(), exe.to_str().unwrap().to_owned())].into();
    let help = Copilot.detection(&options, Path::new(".")).unwrap();
    let version = Copilot
        .version_command(&options, Path::new("."))
        .unwrap()
        .unwrap();
    assert_eq!(help.program, exe);
    assert_eq!(help.program, version.program);
    assert_eq!(help.args, ["--no-auto-update", "--help"]);
    assert_eq!(version.args, ["--no-auto-update", "--version"]);
    assert!(help.input.is_none());
    let actual_help = include_str!("fixtures/copilot-1.0.65-help.txt");
    assert!(Copilot.check_detection(actual_help).is_ok());
    // A longer flag must not satisfy a missing shorter one.
    let missing_remote = actual_help.replace("--no-remote ", "--unsupported ");
    assert!(missing_remote.contains("--no-remote-export"));
    assert!(Copilot.check_detection(&missing_remote).is_err());
    assert!(
        Copilot
            .check_detection(&actual_help.replace("--no-ask-user", "--unsupported"))
            .is_err()
    );
    assert!(
        Copilot
            .check_detection("gh copilot suggest --help")
            .is_err()
    );
}
#[test]
fn copilot_streams_raw_partial_lines_and_split_unicode_without_codex_events() {
    let mut output = Copilot.output();
    assert_eq!(output.push(Stream::Stdout, b"live partial"), "live partial");
    let mut decoded = String::new();
    for byte in "Grüße 🌍".as_bytes() {
        decoded.push_str(&output.push(Stream::Stdout, &[*byte]));
    }
    assert_eq!(decoded, "Grüße 🌍");
    assert_eq!(
        output.push(Stream::Stderr, b"diagnostic"),
        "[stderr] diagnostic"
    );
    let raw = b"{\"type\":\"turn.failed\"}\n";
    assert_eq!(
        output.push(Stream::Stdout, raw),
        String::from_utf8_lossy(raw)
    );
    assert!(output.finish().is_empty());
    assert_eq!(output.interpret(status(0)).0, JobStatus::Succeeded);
    assert_eq!(Codex.output().interpret(status(0)).0, JobStatus::Failed);
}
#[test]
fn invalid_utf8_and_exit_failures_are_handled_without_success_markers() {
    let mut output = Copilot.output();
    assert_eq!(output.push(Stream::Stdout, b"a\xffb\xf0"), "a�b");
    assert_eq!(output.finish(), "�");
    assert!(output.finish().is_empty());
    for code in [1, 2, 7, 130] {
        assert_eq!(output.interpret(status(code)).0, JobStatus::Failed);
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        assert_eq!(
            output.interpret(ExitStatus::from_raw(9)).0,
            JobStatus::Failed
        );
    }
}
#[test]
fn old_state_and_per_agent_preferences_survive_switching_reuse_and_restart() {
    let mut state: AppState = serde_json::from_str(r#"{"version":1,"draft":{"prompt":"existing task","agent":"codex","options":{"sandbox":"workspace-write","model":"original"}},"runs":[]}"#).unwrap();
    assert!(state.agent_options.is_empty());
    state.select_agent(AgentId::Copilot);
    assert!(state.draft.options.is_empty());
    state
        .draft
        .options
        .insert("tool_approvals".into(), "all-tools".into());
    state
        .draft
        .options
        .insert("reasoning_effort".into(), "max".into());
    state.select_agent(AgentId::Codex);
    assert_eq!(state.draft.options["sandbox"], "workspace-write");
    assert_eq!(state.draft.prompt, "existing task");
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path()).unwrap();
    store.save(&state).unwrap();
    let mut state = store.load().unwrap();
    state.select_agent(AgentId::Copilot);
    assert_eq!(state.draft.options["reasoning_effort"], "max");
    assert!(!state.draft.options.contains_key("sandbox"));
    state.reuse_task(TaskConfig {
        prompt: "old Codex run".into(),
        ..Default::default()
    });
    state.select_agent(AgentId::Copilot);
    assert_eq!(state.draft.options["tool_approvals"], "all-tools");
}
#[tokio::test]
async fn missing_executable_produces_an_actionable_error_without_disabling_codex() {
    let dir = tempfile::tempdir().unwrap();
    let options = [(
        "executable".into(),
        dir.path()
            .join("not-installed")
            .to_str()
            .unwrap()
            .to_owned(),
    )]
    .into();
    let error = agents::detect(&Copilot, &options, dir.path())
        .await
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("Check the executable path and installation")
    );
    assert!(agents::backend(AgentId::Codex).is_ok());
}
#[tokio::test]
#[ignore = "Requires locally installed Copilot; executes only --help/--version, never an agent task"]
async fn installed_copilot_accepts_the_exact_command_flags() {
    let dir = tempfile::tempdir().unwrap();
    let detection = agents::detect(&Copilot, &Default::default(), dir.path())
        .await
        .unwrap();
    assert!(detection.contains("GitHub Copilot CLI"));
    println!("{detection}");
    for mode in ["file-edits", "existing", "all-tools"] {
        let mut task = task();
        task.options = [
            ("tool_approvals".into(), mode.into()),
            ("temp_access".into(), "disallow".into()),
            ("model".into(), "auto".into()),
            ("reasoning_effort".into(), "max".into()),
        ]
        .into();
        let repo = Repository {
            name: "help only".into(),
            path: dir.path().to_owned(),
        };
        let mut command = Copilot.build(&task, &repo).unwrap();
        command.args.push("--help".into());
        command.input = None; // Help exits before authentication or prompt execution.
        let result = process::capture(command, 128 * 1024).await.unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        Copilot
            .check_detection(&String::from_utf8_lossy(&result.stdout))
            .unwrap();
    }
}
