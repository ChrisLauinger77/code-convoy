#![allow(clippy::unwrap_used)]
use codeconvoy::{
    agents,
    domain::{AgentId, JobStatus, LOG_LIMIT, LogBuffer},
    process::Stream,
};

fn success() -> std::process::ExitStatus {
    #[cfg(unix)]
    use std::os::unix::process::ExitStatusExt;
    #[cfg(windows)]
    use std::os::windows::process::ExitStatusExt;
    std::process::ExitStatus::from_raw(0)
}

#[test]
fn representative_activity_fixtures_preserve_progress_results_and_backend_semantics() {
    for (agent, fixture, expected) in [
        (
            AgentId::Codex,
            include_str!("fixtures/codex-activity.jsonl"),
            &[
                "turn started",
                "fixture / lookup",
                "cargo fmt --check",
                "File change (update): README.md",
                "Reviewed README. Grüße",
                "Usage:",
            ][..],
        ),
        (
            AgentId::OpenCode,
            include_str!("fixtures/opencode-activity.jsonl"),
            &[
                "step started",
                "read · README.md",
                "bash · cargo fmt --check",
                "Reviewed README. Grüße",
                "Usage:",
            ][..],
        ),
        (
            AgentId::Claude,
            include_str!("fixtures/claude-activity.jsonl"),
            &[
                "system: init",
                "Tool: Read · README.md",
                "Tool: Bash · cargo fmt --check",
                "Reviewed README. Grüße",
                "Review complete",
                "Warning: 1 permission denial",
                "Usage:",
            ][..],
        ),
        (
            AgentId::Copilot,
            include_str!("fixtures/copilot-activity.txt"),
            &[
                "Inspecting README.md",
                "Reviewed README. Grüße",
                "Warning: requested command was denied",
                "Review complete",
            ][..],
        ),
    ] {
        let mut decoder = agents::backend(agent).unwrap().output();
        let mut activity = String::new();
        for chunk in fixture.as_bytes().chunks(3) {
            let fallback = decoder.push(Stream::Stdout, chunk);
            activity.push_str(&decoder.take_activity().unwrap_or(fallback));
        }
        let fallback = decoder.finish();
        activity.push_str(&decoder.take_activity().unwrap_or(fallback));
        for text in expected {
            assert!(
                activity.contains(text),
                "{agent:?}: missing {text:?} in {activity:?}"
            );
        }
        assert!(!activity.contains("raw-only"));
        assert_eq!(
            decoder.interpret(success()).0,
            JobStatus::Succeeded,
            "{agent:?}"
        );
        if agent == AgentId::Copilot {
            assert_eq!(activity, fixture);
        }
    }
}

#[test]
fn warnings_errors_and_malformed_records_remain_visible_without_inventing_events() {
    for agent in AgentId::ALL {
        let mut decoder = agents::backend(agent).unwrap().output();
        let fallback = decoder.push(Stream::Stderr, b"fixture warning\n");
        let mut activity = decoder.take_activity().unwrap_or(fallback);
        let error = match agent {
            AgentId::Codex => {
                "{\"type\":\"turn.failed\",\"error\":{\"message\":\"fixture failure\"}}\n"
            }
            AgentId::OpenCode | AgentId::Claude => {
                "{\"type\":\"error\",\"error\":\"fixture failure\"}\n"
            }
            AgentId::Copilot => "Error: fixture failure\n",
        };
        let fallback = decoder.push(Stream::Stdout, error.as_bytes());
        activity.push_str(&decoder.take_activity().unwrap_or(fallback));
        let fallback = decoder.push(Stream::Stdout, b"{broken-json\n");
        activity.push_str(&decoder.take_activity().unwrap_or(fallback));
        let fallback = decoder.finish();
        activity.push_str(&decoder.take_activity().unwrap_or(fallback));
        assert!(
            activity.contains("fixture warning") && activity.contains("fixture failure"),
            "{agent:?}"
        );
        assert!(
            activity.contains("Invalid") || activity.contains("{broken-json"),
            "{agent:?}"
        );
        // Copilot's plain output cannot override its process-exit completion contract.
        assert_eq!(
            decoder.interpret(success()).0,
            if agent == AgentId::Copilot {
                JobStatus::Succeeded
            } else {
                JobStatus::Failed
            }
        );
    }
}

#[test]
fn oversized_backend_output_is_bounded_and_the_following_message_survives() {
    for agent in AgentId::ALL {
        let mut decoder = agents::backend(agent).unwrap().output();
        let mut activity = LogBuffer::default();
        // A long plain record exercises each decoder's existing framing limit.
        for _ in 0..1024 {
            let fallback = decoder.push(Stream::Stdout, &vec![b'x'; 2048]);
            activity.append(&decoder.take_activity().unwrap_or(fallback));
            assert!(activity.text.len() <= LOG_LIMIT, "{agent:?}");
        }
        let record = match agent {
            AgentId::Codex => {
                "\n{\"type\":\"item.completed\",\"item\":{\"type\":\"agent_message\",\"text\":\"Final message\"}}\n"
            }
            AgentId::OpenCode => "\n{\"type\":\"text\",\"part\":{\"text\":\"Final message\"}}\n",
            AgentId::Claude => {
                "\n{\"type\":\"assistant\",\"message\":{\"content\":[{\"type\":\"text\",\"text\":\"Final message\"}]}}\n"
            }
            AgentId::Copilot => "\nFinal message\n",
        };
        let fallback = decoder.push(Stream::Stdout, record.as_bytes());
        activity.append(&decoder.take_activity().unwrap_or(fallback));
        assert!(activity.text.contains("Final message"), "{agent:?}");
        assert!(activity.text.len() <= LOG_LIMIT);
        if matches!(agent, AgentId::OpenCode | AgentId::Claude) {
            assert!(activity.text.contains("exceeded"));
        } else {
            assert!(activity.truncated);
        }
    }
}

#[test]
fn structured_activity_uses_actual_tool_events_and_points_to_raw_details() {
    let cases = [
        (
            AgentId::Codex,
            serde_json::json!({"type":"item.completed", "item":{"type":"command_execution", "command":"cargo fmt --check", "status":"completed", "aggregated_output":"large tool result only for raw details"}}),
            "cargo fmt --check",
        ),
        (
            AgentId::OpenCode,
            serde_json::json!({"type":"tool_use", "part":{"tool":"read", "state":{"status":"completed", "input":{"filePath":"README.md"}, "output":"large tool result only for raw details"}}}),
            "README.md",
        ),
        (
            AgentId::Claude,
            serde_json::json!({"type":"assistant", "message":{"content":[{"type":"tool_use", "name":"Read", "input":{"file_path":"README.md", "extra":"large tool result only for raw details"}}]}}),
            "README.md",
        ),
    ];
    for (agent, event, actual_detail) in cases {
        let mut decoder = agents::backend(agent).unwrap().output();
        let record = format!("{event}\n");
        let mut activity = String::new();
        for chunk in record.as_bytes().chunks(7) {
            decoder.push(Stream::Stdout, chunk);
            activity.push_str(&decoder.take_activity().unwrap());
        }
        assert!(activity.contains(actual_detail), "{agent:?}");
        assert!(!activity.contains("large tool result"));
        let unknown = b"{\"type\":\"future_event\",\"detail\":\"original unknown event data\"}\n";
        assert!(
            decoder
                .push(Stream::Stdout, unknown)
                .contains("original unknown event data")
        );
        let activity = decoder.take_activity().unwrap();
        assert!(activity.contains("future_event") && activity.contains("Raw output"));
        assert!(!activity.contains("original unknown event data"));
        assert_eq!(decoder.take_activity().unwrap(), "");
    }
}

#[test]
fn concurrent_codex_decoders_keep_thread_identifiers_associated_with_each_job() {
    let mut first = agents::backend(AgentId::Codex).unwrap().output();
    let mut second = agents::backend(AgentId::Codex).unwrap().output();
    first.push(
        Stream::Stdout,
        b"{\"type\":\"thread.started\",\"thread_id\":\"first-repository-thread\"}\n",
    );
    second.push(
        Stream::Stdout,
        b"{\"type\":\"thread.started\",\"thread_id\":\"second-repository-thread\"}\n",
    );
    let first = first.take_activity().unwrap();
    let second = second.take_activity().unwrap();
    assert!(
        first.contains("first-repository-thread") && !first.contains("second-repository-thread")
    );
    assert!(
        second.contains("second-repository-thread") && !second.contains("first-repository-thread")
    );
}
