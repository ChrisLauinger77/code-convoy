#![allow(clippy::unwrap_used)]
use codeconvoy::{
    agents::{self, AgentBackend, cursor::Cursor},
    domain::{AgentId, AppState, Repository, TaskConfig},
    process::Stream,
};
use serde_json::{Value, json};
use std::{path::Path, process::ExitStatus};

fn task() -> TaskConfig {
    TaskConfig {
        agent: AgentId::Cursor,
        prompt: "--help\n$(touch nope); `whoami` \"quotes\" Grüße 🌍".into(),
        ..Default::default()
    }
}
fn repository() -> Repository {
    Repository {
        name: "source".into(),
        path: std::env::temp_dir().join("source ü with spaces"),
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
        "duration_ms":12, "duration_api_ms":10, "session_id":"fixture", "result":"done"})
}

#[test]
fn shell_free_command_keeps_prompt_and_model_as_data_and_uses_assigned_workspace() {
    let mut task = task();
    task.options
        .insert("model".into(), "--literal;$(no shell)".into());
    let repository = repository();
    let spec = Cursor.build(&task, &repository).unwrap();
    assert_eq!(spec.program, "agent");
    assert_eq!(spec.directory, repository.path);
    assert_eq!(
        spec.args[..4],
        ["agent", "--print", "--output-format", "stream-json"]
    );
    let mut workspace = std::ffi::OsString::from("--workspace=");
    workspace.push(&repository.path);
    assert_eq!(spec.args[4], workspace);
    assert_eq!(spec.args[5], "--model=--literal;$(no shell)");
    assert_eq!(spec.args[6], "--");
    assert_eq!(spec.args[7], task.prompt.as_str());
    assert!(spec.input.is_none() && spec.env.is_empty());
    assert!(spec.remove_env.iter().any(|key| key == "PWD"));
    assert!(!spec.remove_env.iter().any(|key| key.starts_with("CURSOR_")));
    for forbidden in [
        "--force",
        "--yolo",
        "--trust",
        "--approve-mcps",
        "--worktree",
        "--sandbox",
        "--resume",
        "--continue",
    ] {
        assert!(!spec.args.iter().any(|arg| arg == forbidden));
    }
    assert!(
        Cursor
            .execution_summary(&task.options)
            .contains("not an OS sandbox")
    );
    assert!(Cursor.attachment_summary().contains("process inspection"));
    task.options.insert("permissions".into(), "force".into());
    let forced = Cursor.build(&task, &repository).unwrap();
    assert_eq!(forced.args[6], "--force");
    assert!(
        Cursor
            .execution_summary(&task.options)
            .contains("Explicit --force")
    );
    task.options
        .insert("workspace_trust".into(), "trust".into());
    let trusted = Cursor.build(&task, &repository).unwrap();
    assert_eq!(trusted.args[7], "--trust");
    assert!(
        Cursor
            .execution_summary(&task.options)
            .contains("Explicit --trust")
    );
}

#[test]
fn backend_rejects_invalid_options_secrets_relative_launchers_and_unrepresentable_prompts() {
    for (key, value) in [
        ("api_key", "not accepted"),
        ("auth_token", "not accepted"),
        ("permissions", "yolo"),
        ("workspace_trust", "automatic"),
        ("sandbox", "disabled"),
        ("executable", ""),
        ("executable", "./agent"),
        ("model", "bad\nmodel"),
        ("model", "bad\0model"),
    ] {
        let mut task = task();
        task.options.insert(key.into(), value.into());
        assert!(Cursor.build(&task, &repository()).is_err(), "{key}");
    }
    for prompt in ["bad\0prompt".into(), "a".repeat(128 * 1024)] {
        let mut task = task();
        task.prompt = prompt;
        assert!(Cursor.build(&task, &repository()).is_err());
    }
    let mut wrong = task();
    wrong.agent = AgentId::Claude;
    assert!(Cursor.build(&wrong, &repository()).is_err());
    #[cfg(windows)]
    for executable in ["agent.cmd", "agent.bat", "agent.ps1"] {
        assert!(
            Cursor
                .detection(
                    &[("executable".into(), executable.into())].into(),
                    Path::new(".")
                )
                .is_err()
        );
    }
}

#[test]
fn availability_checks_cursor_identity_and_each_required_flag_without_authentication() {
    let help = include_str!("fixtures/cursor-help-contract.txt");
    Cursor.check_detection(help).unwrap();
    for flag in [
        "--print",
        "--output-format",
        "--workspace",
        "--model",
        "--force",
        "--trust",
    ] {
        assert!(
            Cursor
                .check_detection(&help.replace(flag, "--unrelated"))
                .is_err()
        );
    }
    assert!(
        Cursor
            .check_detection(&help.replace("Cursor", "Other"))
            .is_err()
    );
    assert!(
        Cursor
            .check_detection(include_str!("fixtures/claude-help-contract.txt"))
            .is_err()
    );
    let options = [("executable".into(), "cursor-agent".into())].into();
    let probe = Cursor.detection(&options, Path::new(".")).unwrap();
    assert_eq!(probe.program, "cursor-agent");
    assert_eq!(probe.args, ["--help"]);
    assert!(probe.input.is_none());
    assert_eq!(
        Cursor
            .version_command(&options, Path::new("."))
            .unwrap()
            .unwrap()
            .args,
        ["--version"]
    );
}

#[test]
fn cursor_fixture_decodes_at_every_byte_boundary_with_independent_stderr() {
    let fixture = include_bytes!("fixtures/cursor-success.jsonl");
    for split in 0..=fixture.len() {
        let mut decoder = Cursor.output();
        let mut output = decoder.push(Stream::Stdout, &fixture[..split]);
        output.push_str(&decoder.push(Stream::Stderr, b"diagnostic is not JSON\n"));
        output.push_str(&decoder.push(Stream::Stdout, &fixture[split..]));
        output.push_str(&decoder.finish());
        assert!(output.contains("Grüße 🌍"));
        assert!(output.contains("[stderr] diagnostic is not JSON"));
        assert_eq!(
            decoder.interpret(status(0)).0,
            codeconvoy::domain::JobStatus::Succeeded
        );
        assert_eq!(
            decoder.interpret(status(7)).0,
            codeconvoy::domain::JobStatus::Failed
        );
    }
}

#[test]
fn zero_exit_requires_complete_unambiguous_terminal_result() {
    use codeconvoy::domain::JobStatus;
    let valid = success().to_string();
    for text in [
        "".into(),
        "{broken-json\n".into(),
        "{\"type\":\"system\"}\n".into(),
        format!("{valid}\n{valid}\n"),
        format!("{valid}\n{{broken-json\n"),
        format!("{valid}\n{{\"type\":\"assistant\"}}\n"),
        format!("{{\"type\":\"error\",\"error\":\"failed\"}}\n{valid}\n"),
    ] {
        let mut decoder = Cursor.output();
        decoder.push(Stream::Stdout, text.as_bytes());
        decoder.finish();
        assert_eq!(decoder.interpret(status(0)).0, JobStatus::Failed, "{text}");
    }
    for key in [
        "subtype",
        "is_error",
        "result",
        "session_id",
        "duration_ms",
        "duration_api_ms",
    ] {
        let mut result = success();
        result.as_object_mut().unwrap().remove(key);
        let mut decoder = Cursor.output();
        decoder.push(Stream::Stdout, result.to_string().as_bytes());
        decoder.finish();
        assert_eq!(decoder.interpret(status(0)).0, JobStatus::Failed, "{key}");
    }
    for (key, value) in [
        ("is_error", json!(true)),
        ("subtype", json!("error")),
        ("session_id", json!("")),
        ("duration_ms", json!(-1)),
        ("result", json!(null)),
    ] {
        let mut result = success();
        result[key] = value;
        let mut decoder = Cursor.output();
        decoder.push(Stream::Stdout, result.to_string().as_bytes());
        decoder.finish();
        assert_eq!(decoder.interpret(status(0)).0, JobStatus::Failed, "{key}");
    }
    // A complete terminal JSON record need not end with a newline.
    let mut decoder = Cursor.output();
    decoder.push(Stream::Stdout, valid.as_bytes());
    decoder.finish();
    assert_eq!(decoder.interpret(status(0)).0, JobStatus::Succeeded);
}

#[test]
fn oversized_stdout_stays_failed_even_when_followed_by_success() {
    let mut decoder = Cursor.output();
    for _ in 0..1025 {
        decoder.push(Stream::Stdout, &[b'x'; 1024]);
    }
    decoder.push(Stream::Stdout, b"\n");
    decoder.push(Stream::Stdout, success().to_string().as_bytes());
    decoder.finish();
    assert_eq!(
        decoder.interpret(status(0)).0,
        codeconvoy::domain::JobStatus::Failed
    );
}

#[test]
fn registry_preferences_and_persistence_keep_cursor_independent() {
    assert!(AgentId::ALL.contains(&AgentId::Cursor));
    assert_eq!(AgentId::Cursor.label(), "Cursor CLI");
    assert_eq!(
        agents::backend(AgentId::Cursor).unwrap().id(),
        AgentId::Cursor
    );
    assert_eq!(serde_json::to_value(AgentId::Cursor).unwrap(), "cursor");
    let mut state = AppState::default();
    state
        .draft
        .options
        .insert("model".into(), "codex-only".into());
    state.select_agent(AgentId::Cursor);
    assert!(state.draft.options.is_empty());
    state
        .draft
        .options
        .insert("model".into(), "cursor-only".into());
    state.select_agent(AgentId::Codex);
    assert_eq!(state.draft.options["model"], "codex-only");
    state.select_agent(AgentId::Cursor);
    assert_eq!(state.draft.options["model"], "cursor-only");
    let temp = tempfile::tempdir().unwrap();
    let store = codeconvoy::persistence::Store::open(temp.path()).unwrap();
    store.save(&state).unwrap();
    let restored = store.load().unwrap();
    assert_eq!(restored.draft.agent, AgentId::Cursor);
    assert_eq!(restored.draft.options, state.draft.options);
    assert_eq!(restored.agent_options, state.agent_options);
}

#[cfg(feature = "test-support")]
#[path = "cursor/execution.rs"]
mod execution;
