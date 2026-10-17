use super::*;
use codeconvoy::{agents::AgentOutput, domain::JobStatus};

fn message(text: &str) -> Value {
    json!({"type":"assistant", "session_id":"fixture", "message":{"role":"assistant", "content":[{"type":"text", "text":text}]}})
}
fn tool(state: &str, id: &str) -> Value {
    json!({"type":"tool_call", "subtype":state, "call_id":id, "session_id":"fixture", "tool_call":{"readToolCall":{"args":{"path":"README.md"}, "result":{"success":{"content":"raw-only contents"}}}}})
}
fn decode(events: &[Value]) -> (Box<dyn AgentOutput>, String) {
    let mut decoder = Cursor.output();
    let mut activity = String::new();
    for event in events {
        activity.push_str(&decoder.push(Stream::Stdout, format!("{event}\n").as_bytes()));
    }
    activity.push_str(&decoder.finish());
    (decoder, activity)
}

#[test]
fn captured_unauthenticated_help_matches_detection_but_proves_no_agent_protocol() {
    Cursor
        .check_detection(include_str!("../fixtures/cursor-2026.10.16-help.txt"))
        .unwrap();
    let help = include_str!("../fixtures/cursor-2026.10.16-help.txt")
        .replace("agent [prompt...]", "other [prompt...]");
    assert!(
        Cursor
            .check_detection(&help)
            .unwrap_err()
            .to_string()
            .contains("agent subcommand")
    );
    let evidence: Value =
        serde_json::from_str(include_str!("../fixtures/cursor-2026.10.16-syntax.json")).unwrap();
    assert_eq!(evidence["cli_version"], "2026.10.16-e3cb207");
    assert_eq!(evidence["authentication_attempted"], false);
    assert_eq!(evidence["paid_agent_task_executed"], false);
    assert_eq!(evidence["cleanup"]["temporary_installation_removed"], true);
}

#[test]
fn complete_cursor_events_show_progress_without_dumping_context_or_tool_payloads() {
    let fixture = include_str!("../fixtures/cursor-activity.jsonl");
    for size in [1, 3, 17, fixture.len()] {
        let mut decoder = Cursor.output();
        let mut activity = String::new();
        for bytes in fixture.as_bytes().chunks(size) {
            activity.push_str(&decoder.push(Stream::Stdout, bytes));
        }
        activity.push_str(&decoder.finish());
        for text in [
            "Cursor session: fixture",
            "reported permissions: default",
            "Inspecting README. Grüße 🌍",
            "tool started: readToolCall · README.md",
            "tool completed: writeToolCall · summary.txt",
            "Review complete",
        ] {
            assert!(activity.contains(text), "{text}: {activity}");
        }
        assert!(!activity.contains("raw-only"));
        assert_eq!(decoder.interpret(status(0)).0, JobStatus::Succeeded);
        assert!(decoder.interpret(status(0)).1.contains("session fixture"));
    }
}

#[test]
fn missing_duplicate_or_conflicting_session_evidence_fails_closed() {
    let mut mismatch = message("another session");
    mismatch["session_id"] = json!("other");
    let mut missing = message("missing session");
    missing.as_object_mut().unwrap().remove("session_id");
    let mut invalid = init();
    invalid["session_id"] = json!("\n");
    let mut missing_cwd = init();
    missing_cwd.as_object_mut().unwrap().remove("cwd");
    for events in [
        vec![success()],
        vec![message("before init"), init(), success()],
        vec![init(), init(), success()],
        vec![init(), mismatch, success()],
        vec![init(), missing, success()],
        vec![invalid, success()],
        vec![missing_cwd, success()],
        vec![
            init(),
            json!({"type":"future_event", "session_id":"fixture"}),
            success(),
        ],
    ] {
        let (decoder, _) = decode(&events);
        assert_eq!(
            decoder.interpret(status(0)).0,
            JobStatus::Failed,
            "{events:?}"
        );
    }
    let mut extended_init = init();
    extended_init["future_field"] = json!({"token":"raw-only future metadata"});
    let mut extended_result = success();
    extended_result["future_field"] = json!([1, 2, 3]);
    let (decoder, activity) = decode(&[extended_init, extended_result]);
    assert_eq!(decoder.interpret(status(0)).0, JobStatus::Succeeded);
    assert!(!activity.contains("raw-only"));
}

#[test]
fn tool_completion_requires_matching_started_identity_and_supported_result() {
    let mut wrong_tool = tool("completed", "one");
    wrong_tool["tool_call"] = json!({"writeToolCall":{"result":{"success":{}}}});
    let mut unsupported = tool("completed", "one");
    unsupported["tool_call"]["readToolCall"]["result"] = json!({"unknown":{}});
    for events in [
        vec![init(), tool("started", "one"), success()],
        vec![init(), tool("completed", "one"), success()],
        vec![
            init(),
            tool("started", "one"),
            tool("completed", "two"),
            success(),
        ],
        vec![
            init(),
            tool("started", "one"),
            tool("started", "one"),
            tool("completed", "one"),
            success(),
        ],
        vec![
            init(),
            tool("started", "one"),
            tool("completed", "one"),
            tool("completed", "one"),
            success(),
        ],
        vec![init(), tool("started", "one"), wrong_tool, success()],
        vec![init(), tool("started", "one"), unsupported, success()],
    ] {
        let (decoder, _) = decode(&events);
        assert_eq!(
            decoder.interpret(status(0)).0,
            JobStatus::Failed,
            "{events:?}"
        );
    }
    let (decoder, _) = decode(&[
        init(),
        tool("started", "one"),
        tool("started", "two"),
        tool("completed", "two"),
        tool("completed", "one"),
        success(),
    ]);
    assert_eq!(decoder.interpret(status(0)).0, JobStatus::Succeeded);
}

#[test]
fn function_names_are_correlated_without_displaying_arguments() {
    let mut started = tool("started", "function-one");
    started["tool_call"] =
        json!({"function":{"name":"shell", "arguments":"raw-only command and environment"}});
    let mut completed = started.clone();
    completed["subtype"] = json!("completed");
    completed["tool_call"]["function"]["result"] = json!({"success":{}});
    let (decoder, activity) = decode(&[init(), started.clone(), completed.clone(), success()]);
    assert_eq!(decoder.interpret(status(0)).0, JobStatus::Succeeded);
    assert!(activity.contains("tool started: shell"));
    assert!(!activity.contains("raw-only"));
    completed["tool_call"]["function"]["name"] = json!("another function");
    let (decoder, _) = decode(&[init(), started, completed, success()]);
    assert_eq!(decoder.interpret(status(0)).0, JobStatus::Failed);
    let mut invalid = tool("started", "function-one");
    invalid["tool_call"] = json!({"function":{"arguments":"raw-only"}});
    let (decoder, _) = decode(&[init(), invalid, success()]);
    assert_eq!(decoder.interpret(status(0)).0, JobStatus::Failed);
}

#[test]
fn malformed_payloads_and_tool_tracking_overflow_cannot_complete() {
    for event in [
        json!({"type":"assistant", "session_id":"fixture", "message":{"content":"not blocks"}}),
        json!({"type":"assistant", "session_id":"fixture", "message":{"content":[{"type":"text","text":false}]}}),
        json!({"type":"tool_call", "subtype":"started", "session_id":"fixture", "call_id":"one", "tool_call":{"readToolCall":false}}),
        json!({"type":"tool_call", "subtype":"started", "session_id":"fixture", "call_id":"one", "tool_call":{}}),
        json!({"type":"tool_call", "subtype":"other", "session_id":"fixture", "call_id":"one", "tool_call":{"readToolCall":{}}}),
    ] {
        let (decoder, _) = decode(&[init(), event.clone(), success()]);
        assert_eq!(decoder.interpret(status(0)).0, JobStatus::Failed, "{event}");
    }
    let mut events = vec![init()];
    for number in 0..129 {
        events.push(tool("started", &number.to_string()));
    }
    for number in 0..129 {
        events.push(tool("completed", &number.to_string()));
    }
    events.push(success());
    let (decoder, _) = decode(&events);
    assert_eq!(decoder.interpret(status(0)).0, JobStatus::Failed);
}

#[test]
fn reported_workspace_must_match_the_assigned_filesystem_identity() {
    let temp = tempfile::tempdir().unwrap();
    let assigned = temp.path().join("assigned ü");
    let other = temp.path().join("other");
    std::fs::create_dir(&assigned).unwrap();
    std::fs::create_dir(&other).unwrap();
    for (reported, expected) in [
        (assigned.clone(), JobStatus::Succeeded),
        (other, JobStatus::Failed),
        (temp.path().join("missing"), JobStatus::Failed),
        (".".into(), JobStatus::Failed),
    ] {
        let mut decoder = Cursor.output_for(&Default::default(), &assigned);
        let mut initialized = init();
        initialized["cwd"] = json!(reported);
        for event in [initialized, success()] {
            decoder.push(Stream::Stdout, format!("{event}\n").as_bytes());
        }
        decoder.finish();
        assert_eq!(decoder.interpret(status(0)).0, expected);
    }
}

#[test]
fn stderr_diagnostics_are_streamed_and_classified_without_parsing_assistant_prose() {
    let captured = include_str!("../fixtures/cursor-2026.10.16-authentication.stderr.txt");
    for (diagnostic, expected) in [
        (captured, "authentication"),
        ("Permission denied: approval required", "permission"),
        ("workspace is not trusted", "trust"),
    ] {
        let mut decoder = Cursor.output();
        let mut displayed = String::new();
        for byte in diagnostic.as_bytes() {
            displayed.push_str(&decoder.push(Stream::Stderr, &[*byte]));
        }
        assert_eq!(displayed.replace("[stderr] ", ""), diagnostic);
        decoder.push(
            Stream::Stdout,
            include_bytes!("../fixtures/cursor-success.jsonl"),
        );
        decoder.finish();
        let (outcome, detail) = decoder.interpret(status(0));
        assert_eq!(outcome, JobStatus::Failed);
        assert!(detail.to_lowercase().contains(expected), "{detail}");
    }
    let (mut decoder, _) = decode(&[
        init(),
        message("Example strings: Authentication required, Permission denied"),
        success(),
    ]);
    let mut text = String::new();
    for byte in "Grüße 🌍 benign warning".as_bytes() {
        text.push_str(&decoder.push(Stream::Stderr, &[*byte]));
    }
    assert_eq!(text.replace("[stderr] ", ""), "Grüße 🌍 benign warning");
    assert_eq!(decoder.interpret(status(0)).0, JobStatus::Succeeded);
    let mut decoder = Cursor.output();
    decoder.push(Stream::Stdout, format!("{}\n", init()).as_bytes());
    decoder.push(Stream::Stderr, success().to_string().as_bytes());
    decoder.finish();
    assert_eq!(decoder.interpret(status(0)).0, JobStatus::Failed);
}

#[test]
fn permission_and_tool_errors_cannot_be_hidden_by_a_successful_terminal_result() {
    for (error, expected) in [
        (
            json!({"code":"permission_denied", "message":"blocked write"}),
            "permission",
        ),
        (json!({"message":"ordinary tool failure"}), "tool"),
    ] {
        let mut completed = tool("completed", "one");
        completed["tool_call"]["readToolCall"]["result"] = json!({"error":error});
        let (decoder, _) = decode(&[init(), tool("started", "one"), completed, success()]);
        let (outcome, detail) = decoder.interpret(status(0));
        assert_eq!(outcome, JobStatus::Failed);
        assert!(detail.to_lowercase().contains(expected), "{detail}");
    }
    let (decoder, _) = decode(&[
        init(),
        json!({"type":"error", "error":{"code":"approval_required", "message":"blocked operation"}}),
        success(),
    ]);
    let (outcome, detail) = decoder.interpret(status(0));
    assert_eq!(outcome, JobStatus::Failed);
    assert!(detail.contains("permission denial or approval requirement"));
}

#[test]
fn application_timeout_is_validated_without_changing_verified_cursor_arguments() {
    let original = Cursor.build(&task(), &repository()).unwrap();
    for value in ["1", "60", "86400", "", " "] {
        let mut task = task();
        task.options.insert("timeout_seconds".into(), value.into());
        assert_eq!(
            Cursor.build(&task, &repository()).unwrap().args,
            original.args
        );
    }
    for value in ["0", "-1", "+1", "1.5", "86401", "4294967296", "forever"] {
        let mut task = task();
        task.options.insert("timeout_seconds".into(), value.into());
        assert!(Cursor.build(&task, &repository()).is_err(), "{value}");
    }
    for agent in AgentId::ALL {
        assert!(
            agents::backend(agent)
                .unwrap()
                .execution_timeout(&Default::default())
                .unwrap()
                .is_none()
        );
    }
}
