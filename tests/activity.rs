#![allow(clippy::unwrap_used)]
use codeconvoy::{agents, domain::AgentId, process::Stream};

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
