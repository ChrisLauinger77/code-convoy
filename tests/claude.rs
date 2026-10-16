#![allow(clippy::unwrap_used)]
use codeconvoy::{
    agents::{self, AgentBackend, AgentOutput, OptionKind, claude::Claude},
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
fn auto_output() -> Box<dyn AgentOutput> {
    Claude.output_with_options(&[("permission_mode".into(), "auto".into())].into())
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
    for mode in ["dontAsk", "acceptEdits", "auto"] {
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
            assert_eq!(spec.args[7], "--permission-mode");
            assert_eq!(spec.args[8], mode);
            assert_eq!(spec.args[9], "--model=--literal;$(no shell)");
            assert_eq!(spec.args[10], format!("--effort={effort}").as_str());
            assert_eq!(spec.args[11], "--max-turns=12");
            assert_eq!(spec.args.len(), 12);
            assert!(
                !spec
                    .args
                    .iter()
                    .any(|arg| arg.to_string_lossy().contains("bypass")
                        || arg.to_string_lossy().contains("skip-permissions"))
            );
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
        ("permission_mode", "Auto"),
        ("permission_mode", "unsupported-mode"),
        ("permission_mode", "default"),
        ("permission_mode", "plan"),
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
fn auto_is_selectable_with_explicit_availability_and_policy_limits() {
    let permission = Claude
        .options()
        .iter()
        .find(|o| o.key == "permission_mode")
        .unwrap();
    assert_eq!(permission.default, "dontAsk");
    let OptionKind::Choice(choices) = permission.kind else {
        panic!("permissions must be selectable")
    };
    assert_eq!(
        choices,
        &[
            ("dontAsk", "Existing approvals; deny asks"),
            ("acceptEdits", "Accept edits / file commands"),
            ("auto", "Auto"),
        ]
    );
    let summary = Claude.execution_summary(&[("permission_mode".into(), "auto".into())].into());
    for explanation in [
        "classifier",
        "supporting Claude CLI",
        "organization policy",
        "Manual",
        "never retries with broader permissions",
        "not an OS sandbox",
    ] {
        assert!(summary.contains(explanation), "missing {explanation}");
    }
    assert!(
        Claude
            .execution_summary(&Default::default())
            .starts_with("Existing approvals apply; calls needing new approval are denied.")
    );
    assert!(
        Claude
            .execution_summary(&[("permission_mode".into(), "acceptEdits".into())].into())
            .starts_with("File edits and common filesystem commands are automatically approved.")
    );
}

#[test]
fn reported_permissions_are_visible_without_changing_existing_completion_rules() {
    for mode in ["auto", "dontAsk", "acceptEdits", "future-mode"] {
        let mut output = Claude.output();
        let init = line(&json!({"type":"system", "subtype":"init", "permissionMode":mode}));
        assert_eq!(output.push(Stream::Stdout, &init).as_bytes(), init);
        let activity = output.take_activity().unwrap();
        assert!(activity.contains(&format!(
            "[CodeConvoy] Claude reported permission mode: {mode}."
        )));
        assert!(!activity.contains("Auto was not active"));
        output.push(Stream::Stdout, &line(&success()));
        assert_eq!(output.interpret(status(0)).0, JobStatus::Succeeded);
    }
    // Older CLIs may omit the field. Never infer that the requested mode is active.
    for reported in [Value::Null, json!(""), json!(false)] {
        let mut output = auto_output();
        output.push(
            Stream::Stdout,
            &line(&json!({"type":"system", "subtype":"init", "permissionMode":reported})),
        );
        assert!(
            output
                .take_activity()
                .unwrap()
                .contains("Auto availability cannot be confirmed")
        );
        output.push(Stream::Stdout, &line(&success()));
        assert_eq!(output.interpret(status(0)).0, JobStatus::Succeeded);
    }
}

#[test]
fn policy_or_availability_fallback_is_visible_even_when_claude_completes_successfully() {
    for mode in ["default", "manual"] {
        let mut output = auto_output();
        // A reported Manual mode must not be mistaken for active Auto, including
        // if it follows an earlier Auto report. The raw record is kept intact.
        output.push(
            Stream::Stdout,
            &line(&json!({"type":"system", "subtype":"init", "permissionMode":"auto"})),
        );
        output.take_activity();
        let reported = line(&json!({"type":"system", "subtype":"init", "permissionMode":mode}));
        assert_eq!(output.push(Stream::Stdout, &reported).as_bytes(), reported);
        let activity = output.take_activity().unwrap();
        assert!(activity.contains("Auto was not active"));
        assert!(activity.contains("organization policy"));
        assert!(activity.contains("does not retry with broader permissions"));
        output.push(Stream::Stdout, &line(&success()));
        output.finish();
        for (exit, expected) in [(0, JobStatus::Succeeded), (1, JobStatus::Failed)] {
            let (outcome, message) = output.interpret(status(exit));
            assert_eq!(outcome, expected);
            // Keep the warning in the terminal summary as well as bounded Activity.
            assert!(message.contains("Auto was not active"));
            assert!(message.contains("organization policy"));
        }
    }
}

#[test]
fn unsupported_auto_or_policy_rejection_preserves_cli_diagnostic_and_fails() {
    for diagnostic in [
        "error: option '--permission-mode <mode>' argument 'auto' is invalid",
        "Auto mode is unavailable for this model",
        "Auto mode disabled by organization policy",
    ] {
        let mut output = auto_output();
        // A CLI may reject before emitting any JSON; stderr remains actionable.
        output.push(Stream::Stderr, diagnostic.as_bytes());
        assert!(output.finish().contains(diagnostic));
        assert!(output.take_activity().unwrap().contains(diagnostic));
        let (outcome, message) = output.interpret(status(1));
        assert_eq!(outcome, JobStatus::Failed);
        assert!(
            message.contains("CLI support, account/model availability and organization policy")
        );
    }
}

#[test]
fn auto_diagnostics_are_scoped_to_the_requested_mode() {
    let backend = agents::backend(AgentId::Claude).unwrap();
    for requested in [None, Some("dontAsk"), Some("acceptEdits"), Some("auto")] {
        let options = requested
            .map(|mode| ("permission_mode".into(), mode.into()))
            .into_iter()
            .collect();
        for reported in [None, Some("default"), Some("manual")] {
            let mut output = backend.output_with_options(&options);
            let mut init = json!({"type":"system", "subtype":"init"});
            if let Some(mode) = reported {
                init["permissionMode"] = json!(mode);
            }
            let raw = line(&init);
            assert_eq!(output.push(Stream::Stdout, &raw).as_bytes(), raw);
            let activity = output.take_activity().unwrap();
            assert_eq!(
                activity.contains("Auto"),
                requested == Some("auto"),
                "requested={requested:?}, reported={reported:?}: {activity}"
            );
            output.push(Stream::Stdout, &line(&success()));
            let (outcome, detail) = output.interpret(status(0));
            assert_eq!(outcome, JobStatus::Succeeded);
            assert_eq!(
                detail.contains("Auto"),
                requested == Some("auto") && reported.is_some()
            );
            let (outcome, detail) = output.interpret(status(1));
            assert_eq!(outcome, JobStatus::Failed);
            assert_eq!(detail.contains("Auto"), requested == Some("auto"));
        }
    }
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
fn observed_auto_background_follow_up_finishes_with_a_second_success_result() {
    let fixture = include_str!("fixtures/claude-auto-background.jsonl").trim_end();
    for chunk_size in [1, 37, fixture.len()] {
        let mut output = auto_output();
        let mut decoded = String::new();
        for chunk in fixture.as_bytes().chunks(chunk_size) {
            decoded.push_str(&output.push(Stream::Stdout, chunk));
        }
        decoded.push_str(&output.finish());
        assert_eq!(output.interpret(status(0)).0, JobStatus::Succeeded);
        assert_eq!(output.interpret(status(1)).0, JobStatus::Failed);
        assert_eq!(decoded.matches("[Claude result]\n").count(), 2);
        assert!(decoded.contains("Grüße 🌍 — background fixture verified."));
        assert!(decoded.contains("\"runId\":\"background-run\""));
        assert!(
            output
                .take_activity()
                .unwrap()
                .contains("permission mode: auto")
        );
    }
}

fn background_events() -> Vec<Value> {
    include_str!("fixtures/claude-auto-background.jsonl")
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

fn background_outcome(events: &[Value]) -> JobStatus {
    let mut output = auto_output();
    for event in events {
        output.push(Stream::Stdout, &line(event));
    }
    output.finish();
    output.interpret(status(0)).0
}

fn queued_background_events() -> Vec<Value> {
    let original = background_events();
    let mut second_start = original[4].clone();
    second_start["task_id"] = json!("second-task");
    second_start["run_id"] = json!("second-run");
    let mut second_notification = original[10].clone();
    second_notification["task_id"] = json!("second-task");
    second_notification["run_id"] = json!("second-run");
    let mut first_result = original[20].clone();
    first_result["queued_turn_count"] = json!(1);
    let mut second_result = original[20].clone();
    second_result["origin"]["runId"] = json!("second-run");
    second_result["result_index"] = json!(2);

    let mut events = original[..8].to_vec();
    events.push(second_start);
    events.extend_from_slice(&original[8..14]);
    // The second task finishes while Claude is answering the first notification.
    events.push(second_notification);
    events.extend_from_slice(&original[14..20]);
    events.push(first_result);
    events.push(original[12].clone());
    events.push(original[19].clone());
    events.push(second_result);
    events
}

fn resumed_background_events() -> Vec<Value> {
    let mut events = queued_background_events();
    let start = events
        .iter()
        .position(|event| event["subtype"] == "task_started" && event["task_id"] == "second-task")
        .unwrap();
    let start = events.remove(start);
    let notification = events
        .iter()
        .position(|event| {
            event["subtype"] == "task_notification" && event["task_id"] == "second-task"
        })
        .unwrap();
    // Start the resumed run while answering the prior run's notification.
    events.insert(notification, start);
    for event in &mut events {
        if event["task_id"] == "second-task" {
            event["task_id"] = json!("background-task");
        }
    }
    events
}

#[test]
fn resumed_background_runs_keep_independent_notifications_and_results() {
    let events = resumed_background_events();
    assert_eq!(background_outcome(&events), JobStatus::Succeeded);
    for end in 0..events.len() {
        assert_eq!(
            background_outcome(&events[..end]),
            JobStatus::Failed,
            "prefix {end}"
        );
    }

    let mut later_notification = events.clone();
    let notification = later_notification
        .iter()
        .position(|event| {
            event["subtype"] == "task_notification" && event["run_id"] == "second-run"
        })
        .unwrap();
    let notification = later_notification.remove(notification);
    let first_result = later_notification
        .iter()
        .position(|event| event["result_index"] == 1)
        .unwrap();
    later_notification.insert(first_result + 1, notification);
    assert_eq!(
        background_outcome(&later_notification),
        JobStatus::Succeeded
    );
}

#[test]
fn resumed_background_runs_reject_duplicate_runs_and_cross_run_completion() {
    let original = resumed_background_events();
    for reuse_task_id in [false, true] {
        let mut duplicate = original.clone();
        for event in &mut duplicate {
            if event["run_id"] == "second-run" {
                event["run_id"] = json!("background-run");
                if !reuse_task_id {
                    event["task_id"] = json!("different-task");
                }
            }
        }
        duplicate.last_mut().unwrap()["origin"]["runId"] = json!("background-run");
        assert_eq!(
            background_outcome(&duplicate),
            JobStatus::Failed,
            "duplicate run with reused task ID: {reuse_task_id}"
        );
    }
    for (key, value) in [
        ("task_id", json!("unknown-task")),
        ("run_id", json!("unknown-run")),
        ("session_id", json!("unknown-session")),
        ("status", json!("failed")),
    ] {
        let mut events = original.clone();
        let notification = events
            .iter_mut()
            .find(|event| {
                event["subtype"] == "task_notification" && event["run_id"] == "second-run"
            })
            .unwrap();
        notification[key] = value;
        assert_eq!(background_outcome(&events), JobStatus::Failed, "{key}");
    }
    let missing_notification: Vec<_> = original
        .iter()
        .filter(|event| {
            !(event["subtype"] == "task_notification" && event["run_id"] == "second-run")
        })
        .cloned()
        .collect();
    assert_eq!(background_outcome(&missing_notification), JobStatus::Failed);

    // The old run's answer cannot also answer the resumed run, even though
    // their task IDs match, and attributed results must keep notification order.
    let mut old_result = original.clone();
    old_result.last_mut().unwrap()["origin"]["runId"] = json!("background-run");
    assert_eq!(background_outcome(&old_result), JobStatus::Failed);
    let mut out_of_order = original.clone();
    let first_result = out_of_order
        .iter_mut()
        .find(|event| event["result_index"] == 1)
        .unwrap();
    first_result["origin"]["runId"] = json!("second-run");
    out_of_order.last_mut().unwrap()["origin"]["runId"] = json!("background-run");
    assert_eq!(background_outcome(&out_of_order), JobStatus::Failed);
}

#[test]
fn queued_background_notifications_drain_across_correlated_results() {
    let events = queued_background_events();
    assert_eq!(background_outcome(&events), JobStatus::Succeeded);
    for end in 0..events.len() {
        assert_eq!(
            background_outcome(&events[..end]),
            JobStatus::Failed,
            "prefix {end}"
        );
    }
    let mut initially_queued = events.clone();
    let initial = initially_queued
        .iter_mut()
        .find(|event| event["result_index"] == 0)
        .unwrap();
    initial["queued_turn_count"] = json!(1);
    assert_eq!(background_outcome(&initially_queued), JobStatus::Succeeded);

    // A coalesced batch can have a queued, zero-turn intermediate placeholder;
    // the final attributed result must still contain the completed answer.
    let first_result = events
        .iter()
        .position(|event| event["result_index"] == 1)
        .unwrap();
    let mut coalesced = events[..=first_result].to_vec();
    coalesced[first_result]["num_turns"] = json!(0);
    coalesced[first_result]["result"] = json!("");
    coalesced.push(events.last().unwrap().clone());
    assert_eq!(background_outcome(&coalesced), JobStatus::Succeeded);
}

#[test]
fn queued_results_require_valid_counts_and_an_explicit_drain() {
    let original = queued_background_events();
    for result_index in 0..=2 {
        for count in [json!(-1), json!(null), json!("0"), json!(false), json!(0.5)] {
            let mut events = original.clone();
            let result = events
                .iter_mut()
                .find(|event| event["result_index"] == result_index)
                .unwrap();
            result["queued_turn_count"] = count.clone();
            assert_eq!(
                background_outcome(&events),
                JobStatus::Failed,
                "result {result_index} count {count}"
            );
        }
    }
    let mut missing_zero = original.clone();
    missing_zero
        .last_mut()
        .unwrap()
        .as_object_mut()
        .unwrap()
        .remove("queued_turn_count");
    assert_eq!(background_outcome(&missing_zero), JobStatus::Failed);
    let mut positive_final = original.clone();
    positive_final.last_mut().unwrap()["queued_turn_count"] = json!(1);
    assert_eq!(background_outcome(&positive_final), JobStatus::Failed);

    // Optional fields remain compatible, but cannot silently erase a count
    // reported by an earlier result.
    let initial = missing_zero
        .iter_mut()
        .find(|event| event["result_index"] == 0)
        .unwrap();
    initial["queued_turn_count"] = json!(1);
    let intermediate = missing_zero
        .iter_mut()
        .find(|event| event["result_index"] == 1)
        .unwrap();
    intermediate
        .as_object_mut()
        .unwrap()
        .remove("queued_turn_count");
    assert_eq!(background_outcome(&missing_zero), JobStatus::Failed);
    missing_zero.last_mut().unwrap()["queued_turn_count"] = json!(0);
    assert_eq!(background_outcome(&missing_zero), JobStatus::Succeeded);
}

#[test]
fn queued_counts_cannot_authorize_uncorrelated_turns_or_hide_failures() {
    let original = queued_background_events();
    for subtype in ["task_started", "task_notification"] {
        let events: Vec<_> = original
            .iter()
            .filter(|event| !(event["task_id"] == "second-task" && event["subtype"] == subtype))
            .cloned()
            .collect();
        assert_eq!(background_outcome(&events), JobStatus::Failed, "{subtype}");
    }
    for result_index in 0..=2 {
        for (pointer, value) in [
            ("/is_error", json!(true)),
            ("/stop_reason", json!("max_tokens")),
            ("/session_id", json!("unrelated")),
            ("/result_index", json!(9)),
        ] {
            let mut events = original.clone();
            let result = events
                .iter_mut()
                .find(|event| event["result_index"] == result_index)
                .unwrap();
            *result.pointer_mut(pointer).unwrap() = value;
            assert_eq!(
                background_outcome(&events),
                JobStatus::Failed,
                "result {result_index} {pointer}"
            );
        }
    }
    let mut unrelated = original.clone();
    unrelated.last_mut().unwrap()["origin"]["runId"] = json!("unrelated");
    assert_eq!(background_outcome(&unrelated), JobStatus::Failed);
}

#[test]
fn untracked_task_notifications_are_activity_until_a_result_requires_correlation() {
    let notification = json!({"type":"system", "subtype":"task_notification",
        "task_id":"untracked-task", "run_id":"untracked-run",
        "session_id":"fixture", "status":"completed"});
    let mut output = Claude.output();
    output.push(Stream::Stdout, &line(&notification));
    output.push(Stream::Stdout, &line(&success()));
    assert_eq!(output.interpret(status(0)).0, JobStatus::Succeeded);
    output.push(Stream::Stdout, &line(&notification));
    output.finish();
    assert_eq!(output.interpret(status(0)).0, JobStatus::Failed);

    let original = background_events();
    // An unrelated notification is also ordinary activity during the answer,
    // but cannot authorize another turn while waiting after a result.
    for (index, expected) in [
        (8, JobStatus::Succeeded),
        (9, JobStatus::Failed),
        (12, JobStatus::Failed),
        (13, JobStatus::Succeeded),
        (21, JobStatus::Failed),
    ] {
        let mut events = original.clone();
        events.insert(index, notification.clone());
        assert_eq!(background_outcome(&events), expected, "event {index}");
    }
}

#[test]
fn failed_tracked_notifications_cannot_be_repaired_by_successful_follow_up_results() {
    let original = background_events();
    for status in ["completed", "failed", "killed", "stopped"] {
        for before_result in [true, false] {
            let mut notification = original[10].clone();
            notification["status"] = json!(status);
            let mut events = original[..8].to_vec();
            events.extend(if before_result {
                [notification, original[8].clone()]
            } else {
                [original[8].clone(), notification]
            });
            events.extend_from_slice(&original[12..]);
            assert_eq!(
                background_outcome(&events),
                if status == "completed" {
                    JobStatus::Succeeded
                } else {
                    JobStatus::Failed
                },
                "{status} notification before initial result: {before_result}"
            );
        }
    }
}

#[test]
fn single_turn_results_require_no_queued_turns() {
    assert_eq!(background_outcome(&[success()]), JobStatus::Succeeded);
    for (count, expected) in [
        (json!(0), JobStatus::Succeeded),
        (json!(1), JobStatus::Failed),
        (json!(-1), JobStatus::Failed),
        (json!(null), JobStatus::Failed),
        (json!("0"), JobStatus::Failed),
        (json!(false), JobStatus::Failed),
    ] {
        let mut result = success();
        result["queued_turn_count"] = count.clone();
        assert_eq!(background_outcome(&[result]), expected, "count {count}");
    }
}

#[test]
fn missing_initial_result_index_allows_a_single_turn_but_fails_follow_up_with_a_hint() {
    let mut events = background_events();
    events[8].as_object_mut().unwrap().remove("result_index");
    assert_eq!(
        background_outcome(&[events[8].clone()]),
        JobStatus::Succeeded
    );
    let mut output = auto_output();
    for event in events {
        output.push(Stream::Stdout, &line(&event));
    }
    output.finish();
    let (outcome, message) = output.interpret(status(0));
    assert_eq!(outcome, JobStatus::Failed);
    assert!(message.contains("uncorrelated background follow-up"));
    assert!(message.contains("consecutive result_index"));
}

#[test]
fn background_follow_up_requires_complete_correlated_turns() {
    let events = background_events();
    assert_eq!(background_outcome(&events), JobStatus::Succeeded);
    // Includes exit after the first success, notification, re-init, tools and
    // final assistant text. None can stand in for the follow-up result.
    for end in 0..events.len() {
        assert_eq!(
            background_outcome(&events[..end]),
            JobStatus::Failed,
            "prefix {end}"
        );
    }
    for missing in [4, 8, 10, 12, 20] {
        let mut incomplete = events.clone();
        incomplete.remove(missing);
        assert_eq!(
            background_outcome(&incomplete),
            JobStatus::Failed,
            "missing {missing}"
        );
    }
    // The task update and UI task-list snapshots are informational, not the
    // evidence that authorizes a follow-up turn or completes it.
    let without_snapshots: Vec<_> = events
        .iter()
        .filter(|event| {
            !matches!(
                event["subtype"].as_str(),
                Some("background_tasks_changed" | "task_updated")
            )
        })
        .cloned()
        .collect();
    assert_eq!(background_outcome(&without_snapshots), JobStatus::Succeeded);
    for (index, pointer, value) in [
        (4, "/is_backgrounded", json!(false)),
        (4, "/task_id", json!(null)),
        (4, "/run_id", json!("")),
        (4, "/session_id", json!("different")),
        (9, "/task_id", json!("different")),
        (9, "/run_id", json!("different")),
        (9, "/patch/status", json!("failed")),
        (10, "/status", json!("failed")),
        (10, "/status", json!("stopped")),
        (10, "/task_id", json!("different")),
        (10, "/run_id", json!("different")),
        (10, "/session_id", json!("different")),
        (
            11,
            "/tasks",
            json!([{"task_id":"unknown","run_id":"unknown"}]),
        ),
        (11, "/tasks", json!(null)),
        (12, "/session_id", json!("different")),
        (13, "/session_id", json!("different")),
        (20, "/origin/kind", json!("human")),
        (20, "/origin/producer", json!("unknown")),
        (20, "/origin/runId", json!("different")),
        (20, "/origin", json!(null)),
        (20, "/session_id", json!("different")),
        (20, "/result_index", json!(0)),
        (20, "/result_index", json!(2)),
        (20, "/result_index", json!(null)),
        (8, "/result_index", json!(null)),
        (8, "/result_index", json!(u64::MAX)),
    ] {
        let mut changed = events.clone();
        *changed[index].pointer_mut(pointer).unwrap() = value;
        assert_eq!(
            background_outcome(&changed),
            JobStatus::Failed,
            "event {index} {pointer}"
        );
    }
}

#[test]
fn background_results_cannot_erase_failure_or_authorize_unrelated_output() {
    let events = background_events();
    for index in [9, 10, 11, 12] {
        for (key, value) in [
            ("error", json!("failed")),
            ("errors", json!(["failed"])),
            ("is_error", json!(true)),
        ] {
            let mut changed = events.clone();
            changed[index][key] = value;
            assert_eq!(background_outcome(&changed), JobStatus::Failed);
        }
    }
    for index in [8, 20] {
        for (key, value) in [
            ("subtype", json!("error_during_execution")),
            ("is_error", json!(true)),
            ("is_error", json!("false")),
            ("result", json!(null)),
            ("duration_ms", json!(-1)),
            ("duration_api_ms", json!(null)),
            ("num_turns", json!(null)),
            ("terminal_reason", json!("aborted_tools")),
            ("terminal_reason", json!("aborted_streaming")),
            ("stop_reason", json!("max_tokens")),
            ("stop_reason", json!("pause_turn")),
            ("api_error_status", json!(401)),
            ("errors", json!(["failed"])),
            ("deferred_tool_use", json!({"name":"Bash"})),
            ("queued_turn_count", json!(-1)),
            ("queued_turn_count", json!("0")),
        ] {
            let mut changed = events.clone();
            changed[index][key] = value;
            assert_eq!(
                background_outcome(&changed),
                JobStatus::Failed,
                "result {index} {key}"
            );
        }
    }
    for index in [9, 13, 20, 21] {
        for extra in [
            json!({"type":"error","error":"failed"}),
            json!({"type":"system","subtype":"error"}),
            json!({"type":"assistant","error":"failed"}),
            events[8].clone(),
            events[10].clone(),
            events[20].clone(),
        ] {
            let mut changed = events.clone();
            changed.insert(index, extra);
            assert_eq!(
                background_outcome(&changed),
                JobStatus::Failed,
                "extra event at {index}"
            );
        }
        let mut output = auto_output();
        for event in &events[..index] {
            output.push(Stream::Stdout, &line(event));
        }
        output.push(Stream::Stdout, b"{malformed JSON}\n");
        for event in &events[index..] {
            output.push(Stream::Stdout, &line(event));
        }
        output.finish();
        assert_eq!(output.interpret(status(0)).0, JobStatus::Failed);
    }
    for extra in [
        json!({"type":"assistant","message":{}}),
        json!({"type":"system","subtype":"unknown"}),
        events[11].clone(),
    ] {
        let mut changed = events.clone();
        changed.push(extra);
        assert_eq!(background_outcome(&changed), JobStatus::Failed);
    }
}

#[test]
fn background_notifications_require_all_results_in_notification_order() {
    let original = background_events();
    let mut second_start = original[4].clone();
    second_start["task_id"] = json!("second-task");
    second_start["run_id"] = json!("second-run");
    let mut second_notification = original[10].clone();
    second_notification["task_id"] = json!("second-task");
    second_notification["run_id"] = json!("second-run");
    let mut second_result = original[20].clone();
    second_result["origin"]["runId"] = json!("second-run");

    // Launch order is deliberately the opposite of notification/result order.
    let mut events = original[..4].to_vec();
    events.push(second_start.clone());
    events.extend_from_slice(&original[4..11]);
    events.push(second_notification.clone());
    events.extend_from_slice(&original[11..20]);
    let mut placeholder = original[20].clone();
    placeholder["num_turns"] = json!(0);
    placeholder["result"] = json!("");
    events.push(placeholder);
    assert_eq!(background_outcome(&events), JobStatus::Failed);
    second_result["result_index"] = json!(2);
    events.push(second_result.clone());
    assert_eq!(background_outcome(&events), JobStatus::Succeeded);
    let last = events.len() - 1;
    events.swap(last - 1, last);
    assert_eq!(background_outcome(&events), JobStatus::Failed);

    let mut empty_answer = original.clone();
    empty_answer[20]["num_turns"] = json!(0);
    empty_answer[20]["result"] = json!("");
    assert_eq!(background_outcome(&empty_answer), JobStatus::Failed);

    // A second independent follow-up needs a fresh init and matching result.
    let mut sequential = original[..8].to_vec();
    sequential.push(second_start);
    sequential.extend_from_slice(&original[8..]);
    sequential.push(second_notification);
    sequential.push(original[12].clone());
    sequential.push(second_result);
    assert_eq!(background_outcome(&sequential), JobStatus::Succeeded);
}

#[test]
fn early_background_notification_still_needs_an_attributed_answer() {
    let original = background_events();
    let mut events = original[..8].to_vec();
    events.extend_from_slice(&original[9..12]);
    events.push(original[8].clone());
    assert_eq!(background_outcome(&events), JobStatus::Failed);
    events.extend_from_slice(&original[12..]);
    assert_eq!(background_outcome(&events), JobStatus::Succeeded);
}

#[test]
fn background_correlation_is_bounded_and_cannot_be_cleared_by_later_output() {
    let original = background_events();
    for length in [256, 257] {
        let run = "x".repeat(length);
        let mut events = original.clone();
        for index in [4, 9, 10] {
            events[index]["run_id"] = json!(run);
        }
        events[20]["origin"]["runId"] = json!(run);
        assert_eq!(
            background_outcome(&events),
            if length == 256 {
                JobStatus::Succeeded
            } else {
                JobStatus::Failed
            }
        );
    }
    for (count, reuse_task_id) in [(128, false), (129, false), (128, true), (129, true)] {
        let task_id = |n| {
            if reuse_task_id {
                "shared-task".to_owned()
            } else {
                format!("task-{n}")
            }
        };
        let mut events = original[..4].to_vec();
        for n in 0..count {
            let mut start = original[4].clone();
            start["task_id"] = json!(task_id(n));
            start["run_id"] = json!(format!("run-{n}"));
            events.push(start);
        }
        events.extend_from_slice(&original[5..9]);
        for n in 0..count {
            let mut notification = original[10].clone();
            notification["task_id"] = json!(task_id(n));
            notification["run_id"] = json!(format!("run-{n}"));
            events.push(notification);
            events.push(original[12].clone());
            let mut result = original[20].clone();
            result["origin"]["runId"] = json!(format!("run-{n}"));
            result["result_index"] = json!(n + 1);
            events.push(result);
        }
        assert_eq!(
            background_outcome(&events),
            if count == 128 {
                JobStatus::Succeeded
            } else {
                JobStatus::Failed
            },
            "pending runs {count}, reused task ID: {reuse_task_id}"
        );
    }
}

#[test]
fn observed_ui_render_notification_can_follow_result_but_cannot_hide_errors_or_execution() {
    let notification = json!({"type":"system", "subtype":"ui_invalidate", "event":"ui.render", "prompts_screened":true});
    for with_ids in [false, true] {
        let mut event = notification.clone();
        if with_ids {
            event["session_id"] = json!("fixture");
            event["uuid"] = json!("notification");
        }
        let mut output = Claude.output();
        output.push(Stream::Stdout, &line(&event));
        assert_eq!(output.interpret(status(0)).0, JobStatus::Failed);
        output.push(Stream::Stdout, &line(&success()));
        assert_eq!(
            output.push(Stream::Stdout, &line(&event)).as_bytes(),
            line(&event)
        );
        assert_eq!(output.interpret(status(0)).0, JobStatus::Succeeded);
        assert_eq!(output.interpret(status(1)).0, JobStatus::Failed);
        output.push(Stream::Stdout, &line(&success()));
        assert_eq!(output.interpret(status(0)).0, JobStatus::Failed);
    }
    let mut missing = notification.clone();
    missing.as_object_mut().unwrap().remove("prompts_screened");
    let mut invalid = vec![missing];
    for (key, value) in [
        ("type", json!("assistant")),
        ("subtype", json!("error")),
        ("event", json!("execution")),
        ("prompts_screened", json!(false)),
        ("prompts_screened", json!("true")),
        ("error", json!("execution failed")),
        ("message", json!({"content":"later activity"})),
        ("unknown", json!(true)),
        ("uuid", json!(null)),
    ] {
        let mut event = notification.clone();
        event[key] = value;
        invalid.push(event);
    }
    for event in invalid {
        let mut output = Claude.output();
        output.push(Stream::Stdout, &line(&success()));
        output.push(Stream::Stdout, &line(&event));
        output.push(Stream::Stdout, &line(&notification));
        assert_eq!(output.interpret(status(0)).0, JobStatus::Failed, "{event}");
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
            provenance: None,
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
        // A lagging legacy counter must not reuse the retained convoy's ID.
        assert_eq!(restored.next_run, 2);
        assert_eq!(serde_json::to_value(AgentId::Claude).unwrap(), "claude");
    }
}

#[test]
fn saved_auto_and_legacy_permissions_preserve_preferences_history_and_templates() {
    for mode in [None, Some("dontAsk"), Some("acceptEdits"), Some("auto")] {
        let mut state: AppState = serde_json::from_value(json!({
            "version": 1,
            "draft": {"agent":"claude", "prompt":"old task", "options":{}},
            "templates": [{"name":"Review", "prompt":"Review the README"}]
        }))
        .unwrap();
        if let Some(mode) = mode {
            state
                .draft
                .options
                .insert("permission_mode".into(), mode.into());
        }
        let original = state.draft.clone();
        state.runs.push(Run {
            provenance: None,
            id: 1,
            created_at: 0,
            task: original.clone(),
            jobs: vec![Job::queued(repo())],
        });
        // Backend switching saves Claude's preferences without modifying templates.
        state.select_agent(AgentId::Codex);
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(dir.path()).unwrap();
        store.save(&state).unwrap();
        let mut restored = store.load().unwrap();
        restored.select_agent(AgentId::Claude);
        assert_eq!(restored.draft.options, original.options);
        assert_eq!(restored.runs[0].task.options, original.options);
        restored.reuse_task(restored.runs[0].task.clone());
        assert_eq!(restored.draft.options, original.options);
        restored.draft.prompt = restored.templates[0].prompt.clone();
        assert_eq!(
            serde_json::to_value(&restored.templates).unwrap(),
            json!([{"name":"Review", "prompt":"Review the README"}])
        );
        let spec = Claude.build(&restored.draft, &repo()).unwrap();
        assert_eq!(spec.args[8], mode.unwrap_or("dontAsk"));
        assert_eq!(spec.args.len(), 9);
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
    for mode in ["dontAsk", "acceptEdits", "auto"] {
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

#[tokio::test]
#[ignore = "Uses authenticated Claude and billable model calls; requires available Auto mode"]
async fn authenticated_claude_auto_and_disabled_auto_in_disposable_repositories() {
    for disabled in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        assert!(
            std::process::Command::new("git")
                .args(["init", "--quiet"])
                .current_dir(dir.path())
                .status()
                .unwrap()
                .success()
        );
        if disabled {
            std::fs::create_dir(dir.path().join(".claude")).unwrap();
            std::fs::write(
                dir.path().join(".claude/settings.json"),
                r#"{"permissions":{"disableAutoMode":"disable"}}"#,
            )
            .unwrap();
        } else {
            std::fs::write(
                dir.path().join("fixture.txt"),
                "CodeConvoy Auto mode fixture\n",
            )
            .unwrap();
        }
        let mut task = task();
        task.options = [
            ("permission_mode".into(), "auto".into()),
            ("max_turns".into(), "4".into()),
        ]
        .into();
        task.prompt = if disabled {
            "Reply with exactly AUTO_DISABLED_FIXTURE. Do not use any tools or access any files."
        } else {
            "This is an authorized disposable CodeConvoy compatibility fixture. Read fixture.txt and create auto-mode-check.txt in this directory containing exactly AUTO_MODE_OK followed by a newline. Do not access files outside this repository, use network tools, or run Git commands. Report whether the file was written."
        }.into();
        let spec = Claude
            .build(
                &task,
                &Repository {
                    name: "Auto fixture".into(),
                    path: dir.path().into(),
                },
            )
            .unwrap();
        let captured = tokio::time::timeout(
            std::time::Duration::from_secs(120),
            process::capture(spec, 1024 * 1024),
        )
        .await
        .unwrap()
        .unwrap();
        assert!(!captured.truncated);
        let events: Vec<Value> = String::from_utf8(captured.stdout.clone())
            .unwrap()
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect();
        let init = events
            .iter()
            .find(|e| e["type"] == "system" && e["subtype"] == "init")
            .unwrap();
        assert_eq!(
            init["permissionMode"],
            if disabled { "default" } else { "auto" }
        );
        let mut output = Claude.output_with_options(&task.options);
        output.push(Stream::Stdout, &captured.stdout);
        output.push(Stream::Stderr, &captured.stderr);
        output.finish();
        let (outcome, message) = output.interpret(captured.status);
        if outcome != JobStatus::Succeeded {
            for event in &events {
                if event["type"] == "system" && event["subtype"] == "ui_invalidate" {
                    let mut notification = event.clone();
                    let fields = notification.as_object_mut().unwrap();
                    fields.remove("session_id");
                    fields.remove("uuid");
                    println!("Claude fixture trailing notification: {notification}");
                }
                let diagnostics: serde_json::Map<String, Value> = [
                    "type",
                    "subtype",
                    "error",
                    "is_error",
                    "duration_ms",
                    "duration_api_ms",
                    "num_turns",
                    "terminal_reason",
                    "stop_reason",
                    "deferred_tool_use",
                    "api_error_status",
                    "errors",
                ]
                .into_iter()
                .filter_map(|key| event.get(key).map(|v| (key.to_owned(), v.clone())))
                .collect();
                println!(
                    "Claude fixture protocol diagnostic: {}",
                    Value::Object(diagnostics)
                );
            }
        }
        assert_eq!(outcome, JobStatus::Succeeded, "{message}");
        assert_eq!(message.contains("Auto was not active"), disabled);
        if !disabled {
            assert_eq!(
                std::fs::read_to_string(dir.path().join("auto-mode-check.txt")).unwrap(),
                "AUTO_MODE_OK\n"
            );
        }
        println!(
            "Authenticated Claude fixture: disabled={disabled}, reported mode={}, outcome={outcome:?}",
            init["permissionMode"]
        );
    }
}
