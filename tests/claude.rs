#![allow(clippy::unwrap_used)]
use codeconvoy::{
    agents::{self, AgentBackend, OptionKind, claude::Claude},
    domain::{AgentId, AppState, Job, JobStatus, Repository, Run, TaskConfig},
    persistence::Store,
    process::{self, Stream},
};
use serde_json::{Value, json};
use std::{path::Path, process::ExitStatus};

fn task() -> TaskConfig {
    TaskConfig {
        agent: AgentId::Claude,
        prompt: "--help\nTreat $(echo hello), `whoami`, \"quotes\", Grüße 🌍 literally.\n".into(),
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
fn success() -> Value {
    json!({"type":"result", "subtype":"success", "is_error":false,
        "result":"Grüße 🌍\nFinished.", "session_id":"fixture", "num_turns":1,
        "duration_ms":20, "duration_api_ms":10, "total_cost_usd":0.02,
        "usage":{"input_tokens":10,"output_tokens":20},
        "permission_denials":[{"tool_name":"Bash","tool_input":{"command":"denied"}}]})
}
fn line(event: &Value) -> Vec<u8> {
    (event.to_string() + "\n").into_bytes()
}

#[test]
fn default_command_uses_print_stream_json_exact_stdin_and_repository_cwd() {
    let task = task();
    let repo = repo();
    let spec = Claude.build(&task, &repo).unwrap();
    assert_eq!(spec.program, "claude");
    assert_eq!(spec.directory, repo.path);
    assert_eq!(
        spec.args,
        [
            "--print",
            "--input-format",
            "text",
            "--output-format",
            "stream-json",
            "--verbose",
            "--no-session-persistence",
            "--permission-mode",
            "dontAsk"
        ]
    );
    assert_eq!(spec.input.as_deref(), Some(task.prompt.as_bytes()));
    assert!(spec.env.is_empty());
    assert!(spec.remove_env.iter().any(|k| k == "PWD"));
    for key in [
        "ANTHROPIC_API_KEY",
        "ANTHROPIC_AUTH_TOKEN",
        "CLAUDE_CONFIG_DIR",
        "CLAUDE_CODE_USE_BEDROCK",
        "CLAUDE_CODE_USE_VERTEX",
        "AWS_PROFILE",
    ] {
        assert!(!spec.remove_env.iter().any(|k| k == key));
    }
    assert_eq!(
        agents::backend(AgentId::Claude).unwrap().id(),
        AgentId::Claude
    );
    assert_eq!(AgentId::Claude.label(), "Claude Code");
    for spec in Claude.options() {
        if let OptionKind::Choice(choices) = spec.kind {
            assert!(choices.iter().any(|(v, _)| *v == spec.default));
        }
    }
    let summary = Claude.execution_summary(&task.options);
    assert!(summary.contains("not an OS sandbox") && summary.contains("hooks/MCP"));
}

#[test]
fn optional_model_effort_turns_and_permissions_are_claude_specific() {
    for mode in ["dontAsk", "acceptEdits"] {
        for effort in ["low", "medium", "high", "xhigh", "max"] {
            let mut task = task();
            task.options = [
                ("model".into(), "--literal;$(no shell)".into()),
                ("effort".into(), effort.into()),
                ("max_turns".into(), "12".into()),
                ("permission_mode".into(), mode.into()),
            ]
            .into();
            let spec = Claude.build(&task, &repo()).unwrap();
            assert_eq!(spec.args[8], mode);
            assert_eq!(spec.args[9], "--model=--literal;$(no shell)");
            assert_eq!(spec.args[10], format!("--effort={effort}").as_str());
            assert_eq!(spec.args[11], "--max-turns=12");
            assert_eq!(spec.args.len(), 12);
            assert_eq!(spec.input.as_deref(), Some(task.prompt.as_bytes()));
        }
    }
    let mut blank = task();
    blank.options = [
        ("model".into(), "  ".into()),
        ("max_turns".into(), " ".into()),
    ]
    .into();
    assert_eq!(Claude.build(&blank, &repo()).unwrap().args.len(), 9);
}

#[test]
fn invalid_options_fail_before_spawn_without_accepting_secret_or_other_backend_settings() {
    for (key, value) in [
        ("executable", ""),
        ("executable", "./claude"),
        ("model", "bad\nname"),
        ("model", "bad\0name"),
        ("effort", "ultracode"),
        ("permission_mode", "bypassPermissions"),
        ("permission_mode", "auto"),
        ("max_turns", "0"),
        ("max_turns", "-1"),
        ("max_turns", "1.5"),
        ("max_turns", "+2"),
        ("max_turns", "4294967296"),
        ("api_key", "not accepted"),
        ("sandbox", "workspace-write"),
        ("variant", "high"),
        ("agent", "build"),
    ] {
        let mut task = task();
        task.options.insert(key.into(), value.into());
        assert!(
            Claude.build(&task, &repo()).is_err(),
            "accepted {key}={value}"
        );
        assert!(Claude.detection(&task.options, Path::new(".")).is_err());
    }
    let mut invalid = task();
    invalid.agent = AgentId::Codex;
    assert!(Claude.build(&invalid, &repo()).is_err());
    invalid = task();
    invalid.prompt.clear();
    assert!(Claude.build(&invalid, &repo()).is_err());
    invalid = task();
    invalid.options.insert("model".into(), "x".repeat(4097));
    assert!(Claude.build(&invalid, &repo()).is_err());
}

#[test]
fn help_detection_checks_core_interface_without_requiring_documented_hidden_flags() {
    let exe = std::env::temp_dir().join("Claude executable");
    let options = [("executable".into(), exe.to_str().unwrap().into())].into();
    let detection = Claude.detection(&options, Path::new(".")).unwrap();
    let version = Claude
        .version_command(&options, Path::new("."))
        .unwrap()
        .unwrap();
    assert_eq!(detection.program, exe);
    assert_eq!(version.program, detection.program);
    assert_eq!(detection.args, ["--help"]);
    assert_eq!(version.args, ["--version"]);
    assert!(detection.input.is_none() && version.input.is_none());
    let help = include_str!("fixtures/claude-help-contract.txt");
    Claude.check_detection(help).unwrap();
    for flag in [
        "--print",
        "--output-format",
        "--permission-mode",
        "--verbose",
    ] {
        assert!(
            Claude
                .check_detection(&help.replace(flag, &format!("{flag}-unrelated")))
                .is_err()
        );
    }
    assert!(Claude.check_detection("codex exec --json").is_err());
    assert!(
        Claude
            .check_detection(&help.replace("stream-json", "text"))
            .is_err()
    );
}

#[test]
fn result_extracts_final_text_and_preserves_usage_denials_tools_unknown_events_and_split_utf8() {
    let mut output = Claude.output();
    for event in [
        json!({"type":"system","subtype":"init","tools":["Read"]}),
        json!({"type":"assistant","message":{"content":[{"type":"tool_use","name":"Read","input":{"file_path":"file"}}]}}),
        json!({"type":"user","message":{"content":[{"type":"tool_result","is_error":true,"content":"denied"}]}}),
        json!({"type":"system","subtype":"api_retry","attempt":1}),
        json!({"type":"future_event","data":42}),
    ] {
        assert_eq!(
            output.push(Stream::Stdout, &line(&event)),
            String::from_utf8(line(&event)).unwrap()
        );
    }
    let mut decoded = String::new();
    for byte in line(&success()) {
        decoded.push_str(&output.push(Stream::Stdout, &[byte]));
    }
    assert!(decoded.contains("Grüße 🌍\nFinished."));
    for metadata in [
        "usage",
        "permission_denials",
        "total_cost_usd",
        "session_id",
    ] {
        assert!(decoded.contains(metadata));
    }
    output.push(Stream::Stderr, b"provider diagnostic");
    assert_eq!(output.finish(), "[stderr] provider diagnostic");
    assert!(output.finish().is_empty());
    assert_eq!(output.interpret(status(0)).0, JobStatus::Succeeded);
    for code in [1, 7, 130] {
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
fn only_a_complete_success_result_can_succeed_even_at_exit_zero() {
    let good = success();
    for key in [
        "subtype",
        "is_error",
        "result",
        "session_id",
        "num_turns",
        "duration_ms",
        "duration_api_ms",
    ] {
        let mut event = good.clone();
        event.as_object_mut().unwrap().remove(key);
        let mut output = Claude.output();
        output.push(Stream::Stdout, &line(&event));
        assert_eq!(
            output.interpret(status(0)).0,
            JobStatus::Failed,
            "missing {key}"
        );
    }
    for (key, value) in [
        ("is_error", json!(true)),
        ("is_error", json!("false")),
        ("subtype", json!("error_during_execution")),
        ("subtype", json!("error_max_turns")),
        ("subtype", json!("error_max_budget_usd")),
        ("subtype", json!("error_max_structured_output_retries")),
        ("terminal_reason", json!("aborted_streaming")),
        ("terminal_reason", json!("aborted_tools")),
        ("terminal_reason", json!("api_error")),
        ("terminal_reason", json!("max_turns")),
        ("stop_reason", json!("max_tokens")),
        ("stop_reason", json!("pause_turn")),
        ("stop_reason", json!("model_context_window_exceeded")),
        ("stop_reason", json!(false)),
        ("deferred_tool_use", json!({"name":"Bash"})),
        ("api_error_status", json!(401)),
        ("errors", json!(["authentication failed"])),
        ("session_id", json!("")),
        ("num_turns", json!(-1)),
        ("result", json!({"text":"fake"})),
    ] {
        let mut event = good.clone();
        event[key] = value;
        let mut output = Claude.output();
        output.push(Stream::Stdout, &line(&event));
        assert_eq!(output.interpret(status(0)).0, JobStatus::Failed, "{event}");
    }
    for suffix in [b"".as_slice(), b"\n"] {
        let mut output = Claude.output();
        let mut event = good.clone();
        event["terminal_reason"] = json!("completed");
        event["stop_reason"] = json!("end_turn");
        output.push(Stream::Stdout, event.to_string().as_bytes());
        output.push(Stream::Stdout, suffix);
        output.finish();
        assert_eq!(output.interpret(status(0)).0, JobStatus::Succeeded);
    }
}

#[test]
fn errors_malformed_incomplete_duplicate_and_post_result_records_fail_closed() {
    for failure in [
        b"{invalid JSON\n".as_slice(),
        b"not JSON\n",
        b"[]\n",
        b"{}\n",
        b"\xff\n",
        b"{\"type\":\"error\",\"error\":\"auth failed\"}\n",
        b"{\"type\":\"assistant\",\"error\":\"authentication_failed\"}\n",
        b"{\"type\":\"system\",\"subtype\":\"error\"}\n",
    ] {
        for before in [true, false] {
            let mut output = Claude.output();
            let good = line(&success());
            for bytes in if before {
                [failure, good.as_slice()]
            } else {
                [good.as_slice(), failure]
            } {
                output.push(Stream::Stdout, bytes);
            }
            output.finish();
            assert_eq!(output.interpret(status(0)).0, JobStatus::Failed);
        }
    }
    for bytes in [
        b"".as_slice(),
        b"{\"type\":\"system\",\"subtype\":\"init\"}\n",
        b"{\"type\":\"result\"",
    ] {
        let mut output = Claude.output();
        output.push(Stream::Stdout, bytes);
        output.finish();
        assert_eq!(output.interpret(status(0)).0, JobStatus::Failed);
    }
    for later in [success(), json!({"type":"assistant","message":{}})] {
        let mut output = Claude.output();
        output.push(Stream::Stdout, &line(&success()));
        output.push(Stream::Stdout, &line(&later));
        assert_eq!(output.interpret(status(0)).0, JobStatus::Failed);
    }
}

#[test]
fn sdk_sized_buffer_accepts_large_valid_records_and_never_parses_oversize_fragments() {
    let mut event = success();
    event["result"] = json!("x".repeat(600 * 1024));
    let mut output = Claude.output();
    assert!(
        output
            .push(Stream::Stdout, &line(&event))
            .contains(&"x".repeat(1024))
    );
    assert_eq!(output.interpret(status(0)).0, JobStatus::Succeeded);
    let oversized = vec![b'x'; 1024 * 1024 + 1];
    let mut output = Claude.output();
    assert!(
        output
            .push(Stream::Stdout, &oversized)
            .contains("exceeded 1 MiB")
    );
    output.push(Stream::Stdout, &line(&success()));
    output.push(Stream::Stdout, &line(&success()));
    assert_eq!(output.interpret(status(0)).0, JobStatus::Failed);
    let mut output = Claude.output();
    assert!(
        output
            .push(Stream::Stderr, &oversized)
            .starts_with("[stderr] ")
    );
    output.push(Stream::Stderr, b"\n");
    output.push(Stream::Stdout, &line(&success()));
    assert_eq!(output.interpret(status(0)).0, JobStatus::Succeeded);
}

#[test]
fn version_one_state_preserves_old_backends_and_claude_draft_history_and_restart() {
    for existing in ["codex", "copilot", "opencode", "claude"] {
        let mut state: AppState = serde_json::from_value(json!({"version":1,"draft":{"agent":existing,"prompt":"old task","options":{"model":"original"}}})).unwrap();
        let previous = state.draft.clone();
        state.select_agent(AgentId::Claude);
        state.draft = task();
        state.draft.options = [
            ("executable".into(), "claude".into()),
            ("model".into(), "sonnet".into()),
            ("effort".into(), "high".into()),
            ("max_turns".into(), "10".into()),
            ("permission_mode".into(), "acceptEdits".into()),
        ]
        .into();
        let original = state.draft.clone();
        let mut job = Job::queued(repo());
        job.status = JobStatus::Running;
        job.log.append("session output");
        state.runs.push(Run {
            id: 1,
            created_at: 0,
            task: original.clone(),
            jobs: vec![job],
        });
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(dir.path()).unwrap();
        store.save(&state).unwrap();
        let mut restored = store.load().unwrap();
        assert_eq!(
            serde_json::to_value(&restored.draft).unwrap(),
            serde_json::to_value(&original).unwrap()
        );
        assert_eq!(restored.runs[0].status(), JobStatus::Cancelled);
        assert!(restored.runs[0].jobs[0].interrupted);
        assert!(restored.runs[0].jobs[0].log.text.is_empty());
        if previous.agent != AgentId::Claude {
            restored.select_agent(previous.agent);
            assert_eq!(restored.draft.options, previous.options);
        }
        restored.reuse_task(restored.runs[0].task.clone());
        assert_eq!(restored.draft.options, original.options);
        assert_eq!(restored.next_run, 1);
        assert_eq!(serde_json::to_value(AgentId::Claude).unwrap(), "claude");
    }
}

#[tokio::test]
async fn missing_claude_is_actionable_and_does_not_disable_other_backends() {
    let dir = tempfile::tempdir().unwrap();
    let options = [(
        "executable".into(),
        dir.path().join("missing-claude").to_str().unwrap().into(),
    )]
    .into();
    let error = agents::detect(&Claude, &options, dir.path())
        .await
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("Check the executable path and installation")
    );
    for agent in AgentId::ALL {
        assert!(agents::backend(agent).is_ok());
    }
}

#[tokio::test]
#[cfg(feature = "test-support")]
async fn check_cli_displays_opaque_fixture_version_without_authentication() {
    let dir = tempfile::tempdir().unwrap();
    let options = [(
        "executable".into(),
        env!("CARGO_BIN_EXE_codeconvoy-test-agent").into(),
    )]
    .into();
    let report = agents::detect(&Claude, &options, dir.path()).await.unwrap();
    assert!(report.starts_with("1.18.34 (CodeConvoy fixture) detected."));
    assert!(report.contains("authentication is used when a job runs"));
}

#[tokio::test]
#[ignore = "Requires installed Claude Code; help/version only, no authenticated task"]
async fn installed_claude_accepts_generated_arguments() {
    let dir = tempfile::tempdir().unwrap();
    println!(
        "{}",
        agents::detect(&Claude, &Default::default(), dir.path())
            .await
            .unwrap()
    );
    let mut task = task();
    task.options = [
        ("model".into(), "sonnet".into()),
        ("effort".into(), "high".into()),
        ("max_turns".into(), "10".into()),
    ]
    .into();
    for mode in ["dontAsk", "acceptEdits"] {
        task.options.insert("permission_mode".into(), mode.into());
        let mut spec = Claude
            .build(
                &task,
                &Repository {
                    name: "help only".into(),
                    path: dir.path().into(),
                },
            )
            .unwrap();
        spec.args.push("--help".into());
        spec.input = None;
        let captured = process::capture(spec, 128 * 1024).await.unwrap();
        assert!(
            captured.status.success(),
            "{}",
            String::from_utf8_lossy(&captured.stderr)
        );
        assert!(!captured.truncated);
        Claude
            .check_detection(&String::from_utf8_lossy(&captured.stdout))
            .unwrap();
    }
}
