#![allow(clippy::unwrap_used)]
use codeconvoy::{
    agents::{self, AgentBackend, OptionKind, opencode::OpenCode},
    domain::{AgentId, AppState, JobStatus, Repository, TaskConfig},
    persistence::Store,
    process::{self, Stream},
};
use std::{path::Path, process::ExitStatus};

fn task() -> TaskConfig {
    TaskConfig {
        agent: AgentId::OpenCode,
        prompt: "--help\nTreat $(echo hello), `whoami`, quotes and Grüße 🌍 literally.".into(),
        ..Default::default()
    }
}
fn repo() -> Repository {
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
const STOP: &[u8] = b"{\"type\":\"step_finish\",\"sessionID\":\"ses_fixture\",\"part\":{\"type\":\"step-finish\",\"reason\":\"stop\",\"cost\":0,\"tokens\":{}}}\n";

#[test]
fn default_command_uses_exact_stdin_native_repository_path_and_existing_rules() {
    let task = task();
    let repo = repo();
    let spec = OpenCode.build(&task, &repo).unwrap();
    assert_eq!(spec.program, "opencode");
    assert_eq!(spec.directory, repo.path);
    assert_eq!(spec.args[..4], ["run", "--format", "json", "--dir"]);
    assert_eq!(spec.args[4], repo.path.as_os_str());
    assert_eq!(spec.args.len(), 5);
    assert_eq!(spec.input.as_deref(), Some(task.prompt.as_bytes()));
    assert!(spec.remove_env.iter().any(|key| key == "PWD"));
    for key in [
        "OPENCODE_CONFIG",
        "OPENCODE_CONFIG_CONTENT",
        "OPENCODE_CONFIG_DIR",
        "OPENCODE_PERMISSION",
        "OPENAI_API_KEY",
        "ANTHROPIC_API_KEY",
    ] {
        assert!(!spec.remove_env.iter().any(|k| k == key));
    }
    assert!(spec.env.is_empty());
    assert!(
        OpenCode
            .execution_summary(&task.options)
            .contains("not an OS sandbox")
    );
    let backend = agents::backend(AgentId::OpenCode).unwrap();
    assert_eq!(backend.id(), AgentId::OpenCode);
    for spec in backend.options() {
        if let OptionKind::Choice(choices) = spec.kind {
            assert!(choices.iter().any(|(value, _)| *value == spec.default));
        }
    }
    assert!(agents::backend(AgentId::Claude).is_ok());
}

#[test]
fn optional_model_agent_variant_and_permission_modes_are_opencode_specific() {
    for mode in ["existing", "auto"] {
        let mut task = task();
        task.options = [
            ("model".into(), "provider/model/submodel".into()),
            ("agent".into(), "--literal agent;$(no shell)".into()),
            ("variant".into(), "custom-high".into()),
            ("permissions".into(), mode.into()),
        ]
        .into();
        let spec = OpenCode.build(&task, &repo()).unwrap();
        assert_eq!(
            spec.args[5..8],
            [
                "--model=provider/model/submodel",
                "--agent=--literal agent;$(no shell)",
                "--variant=custom-high"
            ]
        );
        assert_eq!(spec.args.iter().any(|a| a == "--auto"), mode == "auto");
        assert_eq!(spec.args.len(), if mode == "auto" { 9 } else { 8 });
        assert_eq!(spec.input.as_deref(), Some(task.prompt.as_bytes()));
        assert!(spec.env.is_empty()); // No permission/provider configuration is rewritten.
    }
    let mut blank = task();
    for key in ["model", "agent", "variant"] {
        blank.options.insert(key.into(), "  ".into());
    }
    assert_eq!(OpenCode.build(&blank, &repo()).unwrap().args.len(), 5);
}

#[test]
fn rejects_invalid_configuration_without_a_provider_catalog_or_credentials() {
    for (key, value) in [
        ("model", "unqualified"),
        ("model", "/model"),
        ("model", "provider/"),
        ("model", "provider/model with spaces"),
        ("agent", "bad\nname"),
        ("variant", "bad\0name"),
        ("permissions", "allow-all"),
        ("sandbox", "workspace-write"),
        ("reasoning_effort", "high"),
        ("api_key", "never-accepted"),
        ("executable", ""),
        ("executable", "./opencode"),
    ] {
        let mut task = task();
        task.options.insert(key.into(), value.into());
        assert!(
            OpenCode.build(&task, &repo()).is_err(),
            "accepted {key}={value}"
        );
        assert!(OpenCode.detection(&task.options, Path::new(".")).is_err());
    }
    let mut invalid = task();
    invalid.prompt.clear();
    assert!(OpenCode.build(&invalid, &repo()).is_err());
    invalid = task();
    invalid.agent = AgentId::Copilot;
    assert!(OpenCode.build(&invalid, &repo()).is_err());
    invalid = task();
    invalid.options.insert("variant".into(), "x".repeat(4097));
    assert!(OpenCode.build(&invalid, &repo()).is_err());
}

#[test]
fn help_detection_checks_complete_flags_and_uses_the_selected_executable_for_version() {
    let exe = std::env::temp_dir().join("OpenCode executable");
    let options = [("executable".into(), exe.to_str().unwrap().into())].into();
    let detection = OpenCode.detection(&options, Path::new(".")).unwrap();
    let version = OpenCode
        .version_command(&options, Path::new("."))
        .unwrap()
        .unwrap();
    assert_eq!(detection.program, exe);
    assert_eq!(detection.program, version.program);
    assert_eq!(detection.args, ["run", "--help"]);
    assert_eq!(version.args, ["--version"]);
    assert!(detection.input.is_none() && version.input.is_none());
    let help = include_str!("fixtures/opencode-run-help.txt");
    OpenCode.check_detection(help).unwrap();
    for flag in [
        "--format",
        "--dir",
        "--model",
        "--agent",
        "--variant",
        "--auto",
    ] {
        let error = OpenCode
            .check_detection(&help.replace(flag, &format!("{flag}-unrelated")))
            .unwrap_err();
        assert!(error.to_string().contains(flag));
    }
    assert!(OpenCode.check_detection("codex exec --json").is_err());
    assert!(
        OpenCode
            .check_detection(&help.replace("json", "text"))
            .is_err()
    );
}

#[test]
fn json_events_handle_split_utf8_and_preserve_tools_unknown_events_and_diagnostics() {
    let mut output = OpenCode.output();
    let text = serde_json::json!({"type":"text", "part":{"text":"Grüße 🌍"}}).to_string() + "\n";
    let mut decoded = String::new();
    for byte in text.bytes() {
        decoded.push_str(&output.push(Stream::Stdout, &[byte]));
    }
    assert_eq!(decoded, "Grüße 🌍\n");
    let tool = "{\"type\":\"tool_use\",\"part\":{\"tool\":\"bash\",\"state\":{\"status\":\"error\",\"error\":\"denied\"}}}\n";
    assert_eq!(output.push(Stream::Stdout, tool.as_bytes()), tool);
    let unknown = b"{\"type\":\"future_opencode_event\",\"details\":42}\n";
    assert_eq!(
        output.push(Stream::Stdout, unknown),
        String::from_utf8_lossy(unknown)
    );
    assert_eq!(
        output.push(Stream::Stdout, b"permission requested; auto-rejecting\n"),
        "permission requested; auto-rejecting\n"
    );
    output.push(Stream::Stderr, b"provider diagnostic");
    output.push(Stream::Stdout, STOP);
    assert_eq!(output.finish(), "[stderr] provider diagnostic");
    assert!(output.finish().is_empty());
    // A tool error can be recovered by the model. Session errors cannot.
    assert_eq!(output.interpret(status(0)).0, JobStatus::Succeeded);
    for code in [1, 7, 130] {
        assert_eq!(output.interpret(status(code)).0, JobStatus::Failed);
    }
}

#[test]
fn success_requires_a_final_stop_and_no_session_errors_even_when_exit_is_zero() {
    for data in [
        "",
        "{\"type\":\"turn.completed\"}\n",
        "{\"type\":\"text\",\"part\":{\"text\":\"done\"}}\n",
        "{\"type\":\"step_finish\",\"part\":{\"reason\":\"tool-calls\"}}\n",
        "{\"type\":\"step_finish\",\"part\":{\"reason\":\"length\"}}\n",
        "{\"type\":\"step_finish\",\"part\":{\"reason\":\"content-filter\"}}\n",
    ] {
        let mut output = OpenCode.output();
        output.push(Stream::Stdout, data.as_bytes());
        output.finish();
        assert_eq!(output.interpret(status(0)).0, JobStatus::Failed, "{data}");
    }
    for failure in [
        b"{\"type\":\"error\",\"error\":{\"name\":\"ProviderAuthError\"}}\n".as_slice(),
        b"{invalid JSON\n",
    ] {
        for failure_first in [true, false] {
            let mut output = OpenCode.output();
            for data in if failure_first {
                [failure, STOP]
            } else {
                [STOP, failure]
            } {
                output.push(Stream::Stdout, data);
            }
            output.finish();
            assert_eq!(output.interpret(status(0)).0, JobStatus::Failed);
        }
    }
    let mut output = OpenCode.output();
    output.push(Stream::Stdout, STOP);
    output.push(Stream::Stdout, b"{\"type\":\"step_start\"}\n");
    assert_eq!(output.interpret(status(0)).0, JobStatus::Failed);
    output.push(Stream::Stdout, &STOP[..STOP.len() - 1]);
    output.finish();
    assert_eq!(output.interpret(status(0)).0, JobStatus::Succeeded);
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
fn oversized_records_do_not_reassemble_a_fake_success_or_hide_output() {
    let mut output = OpenCode.output();
    let prefix = vec![b'x'; 256 * 1024];
    assert!(
        output
            .push(Stream::Stdout, &prefix)
            .contains("exceeded 256 KiB")
    );
    // This looks valid only if an overlong record is split into new JSON records.
    output.push(Stream::Stdout, STOP);
    output.push(Stream::Stdout, STOP);
    assert_eq!(output.interpret(status(0)).0, JobStatus::Failed);
    let mut stderr = OpenCode.output();
    assert!(
        stderr
            .push(Stream::Stderr, &prefix)
            .starts_with("[stderr] ")
    );
    stderr.push(Stream::Stderr, b"\n");
    stderr.push(Stream::Stdout, STOP);
    assert_eq!(stderr.interpret(status(0)).0, JobStatus::Succeeded);
}

#[test]
fn version_one_state_roundtrips_opencode_drafts_preferences_and_history() {
    for existing in ["codex", "copilot"] {
        let mut state: AppState = serde_json::from_value(serde_json::json!({"version":1, "draft":{"agent":existing,"prompt":"old task","options":{"model":"original"}}})).unwrap();
        let previous = state.draft.clone();
        state.select_agent(AgentId::OpenCode);
        assert!(state.draft.options.is_empty());
        state.draft = task();
        state.draft.options = [
            ("model".into(), "provider/model".into()),
            ("agent".into(), "build".into()),
            ("variant".into(), "high".into()),
            ("permissions".into(), "auto".into()),
        ]
        .into();
        let original = state.draft.clone();
        state.runs.push(codeconvoy::domain::Run {
            id: 1,
            created_at: 0,
            task: original.clone(),
            jobs: vec![],
        });
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(dir.path()).unwrap();
        store.save(&state).unwrap();
        let mut state = store.load().unwrap();
        assert_eq!(
            serde_json::to_value(&state.draft).unwrap(),
            serde_json::to_value(&original).unwrap()
        );
        assert_eq!(state.runs[0].task.agent, AgentId::OpenCode);
        state.select_agent(previous.agent);
        assert_eq!(state.draft.options, previous.options);
        state.reuse_task(state.runs[0].task.clone());
        assert_eq!(state.draft.options, original.options);
        assert_eq!(state.next_run, 1);
        assert_eq!(serde_json::to_value(AgentId::OpenCode).unwrap(), "opencode");
        let persisted = std::fs::read_to_string(dir.path().join("state.json")).unwrap();
        assert!(!persisted.contains("API_KEY"));
    }
}

#[tokio::test]
async fn missing_executable_is_actionable_and_other_backends_still_exist() {
    let dir = tempfile::tempdir().unwrap();
    let options = [(
        "executable".into(),
        dir.path().join("missing-opencode").to_str().unwrap().into(),
    )]
    .into();
    let error = agents::detect(&OpenCode, &options, dir.path())
        .await
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("Check the executable path and installation")
    );
    for agent in [AgentId::Codex, AgentId::Copilot] {
        assert!(agents::backend(agent).is_ok());
    }
}

#[tokio::test]
#[cfg(feature = "test-support")]
async fn detects_fixture_help_and_displays_version_without_parsing_a_release_number() {
    let dir = tempfile::tempdir().unwrap();
    let options = [(
        "executable".into(),
        env!("CARGO_BIN_EXE_codeconvoy-test-agent").into(),
    )]
    .into();
    let report = agents::detect(&OpenCode, &options, dir.path())
        .await
        .unwrap();
    assert!(report.starts_with("1.18.34 (CodeConvoy fixture) detected."));
    assert!(report.contains("authentication is used when a job runs"));
}

#[tokio::test]
#[ignore = "Requires installed OpenCode; help/version only, no authenticated task"]
async fn installed_opencode_accepts_generated_arguments() {
    let dir = tempfile::tempdir().unwrap();
    println!(
        "{}",
        agents::detect(&OpenCode, &Default::default(), dir.path())
            .await
            .unwrap()
    );
    for mode in ["existing", "auto"] {
        let mut task = task();
        task.options = [
            ("model".into(), "provider/model".into()),
            ("agent".into(), "build".into()),
            ("variant".into(), "high".into()),
            ("permissions".into(), mode.into()),
        ]
        .into();
        let mut command = OpenCode
            .build(
                &task,
                &Repository {
                    name: "help only".into(),
                    path: dir.path().into(),
                },
            )
            .unwrap();
        command.args.push("--help".into());
        command.input = None;
        let captured = process::capture(command, 128 * 1024).await.unwrap();
        assert!(
            captured.status.success(),
            "{}",
            String::from_utf8_lossy(&captured.stderr)
        );
        assert!(!captured.truncated);
        OpenCode
            .check_detection(&String::from_utf8_lossy(&captured.stdout))
            .unwrap();
    }
}
