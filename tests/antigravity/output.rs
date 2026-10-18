use super::*;
use codeconvoy::{agents::AgentOutput, domain::JobStatus, process::Stream};
use serde_json::json;

const SUCCESS: &str = include_str!("../fixtures/antigravity-success.jsonl");

fn decode(output: &mut dyn AgentOutput, stdout: &str, stderr: &str) -> String {
    let mut text = output.push(Stream::Stderr, stderr.as_bytes());
    text.push_str(&output.push(Stream::Stdout, stdout.as_bytes()));
    text.push_str(&output.finish());
    text
}
fn outcome(stdout: &str, stderr: &str, code: i32) -> (JobStatus, String, String) {
    let mut output = Antigravity.output();
    let text = decode(output.as_mut(), stdout, stderr);
    let (status, detail) = output.interpret(status(code));
    (status, detail, text)
}
fn lines(fixture: &str) -> Vec<Value> {
    fixture
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}
fn join(records: &[Value]) -> String {
    records.iter().map(|record| format!("{record}\n")).collect()
}

#[test]
fn documented_success_decodes_at_every_byte_boundary_with_independent_stderr() {
    let fixture = SUCCESS.as_bytes();
    for split in 0..=fixture.len() {
        let mut decoder = Antigravity.output();
        let mut text = decoder.push(Stream::Stdout, &fixture[..split]);
        text.push_str(&decoder.push(Stream::Stderr, "stderr Grü".as_bytes()));
        text.push_str(&decoder.push(Stream::Stdout, &fixture[split..]));
        text.push_str(&decoder.finish());
        assert!(text.contains("Fixture task complete."), "{split}: {text}");
        assert!(text.contains("[stderr] stderr Grü"));
        let (status, detail) = decoder.interpret(status(0));
        assert_eq!(status, JobStatus::Succeeded, "{split}: {detail}");
        assert!(detail.contains("conversation 4e502687"));
    }
    // A complete terminal record need not end with a newline.
    let (status, ..) = outcome(SUCCESS.trim_end(), "", 0);
    assert_eq!(status, JobStatus::Succeeded);
}

#[test]
fn zero_exit_requires_initialization_consistent_identity_and_a_success_result() {
    let records = lines(SUCCESS);
    let without_result = join(&records[..records.len() - 1]);
    let (status, detail, text) = outcome(&without_result, "", 0);
    assert_eq!(status, JobStatus::Failed);
    assert!(detail.contains("incomplete"), "{detail}");
    assert!(text.contains("without a final result"));
    let (status, detail, _) = outcome(&join(&records[1..]), "", 0);
    assert_eq!(status, JobStatus::Failed);
    assert!(detail.contains("initialization"), "{detail}");
    // Activity after the latest result needs another result to finish.
    let mut trailing = records.clone();
    trailing.push(records[2].clone());
    assert_eq!(outcome(&join(&trailing), "", 0).0, JobStatus::Failed);
    let mut duplicate_init = records.clone();
    duplicate_init.insert(1, records[0].clone());
    assert_eq!(outcome(&join(&duplicate_init), "", 0).0, JobStatus::Failed);
    for (index, pointer, value) in [
        (0, "/conversation_id", json!("")),
        (0, "/conversation_id", json!(null)),
        (0, "/init/cwd", json!(null)),
        (0, "/init", json!("not an object")),
        (2, "/step_update/conversation_id", json!("other")),
        (2, "/step_update/state", json!("PENDING")),
        (2, "/step_update/step_index", json!(-1)),
        (2, "/step_update/text_delta", json!(7)),
        (2, "/step_update/step_type", json!(null)),
        (4, "/result/conversation_id", json!("other")),
        (4, "/result/status", json!(null)),
        (4, "/result/response", json!(null)),
        (4, "/result/num_turns", json!(0)),
        (4, "/result/num_turns", json!(null)),
        (4, "/result/error", json!("success with an error")),
        (4, "/event", json!("future_event")),
    ] {
        let mut changed = records.clone();
        let (parent, key) = pointer.rsplit_once('/').unwrap();
        changed[index].pointer_mut(parent).unwrap()[key] = value.clone();
        let (status, detail, _) = outcome(&join(&changed), "", 0);
        assert_eq!(status, JobStatus::Failed, "{pointer}={value}: {detail}");
    }
    // A non-zero exit fails even after a SUCCESS result.
    let (status, detail, _) = outcome(SUCCESS, "", 1);
    assert_eq!(status, JobStatus::Failed);
    assert!(detail.contains("process failed"), "{detail}");
}

#[test]
fn reported_failures_are_explicit_even_when_the_process_exits_zero() {
    let (status, detail, text) =
        outcome(include_str!("../fixtures/antigravity-error.jsonl"), "", 0);
    assert_eq!(status, JobStatus::Failed);
    assert!(detail.contains("Antigravity reported ERROR: fixture quota exhausted"));
    assert!(text.contains("Partial answer"));
    let (status, detail, _) = outcome(
        include_str!("../fixtures/antigravity-interrupted.jsonl"),
        "",
        130,
    );
    assert_eq!(status, JobStatus::Failed);
    assert!(detail.contains("INTERRUPTED") && detail.contains("stopped"));
    let records = lines(SUCCESS);
    for (reported, hint) in [
        ("CANCELED", "stopped"),
        ("WAITING", "waiting for input"),
        ("INVALID", "INVALID"),
        ("RUNNING", "RUNNING"),
    ] {
        let mut changed = records.clone();
        changed[4]["result"]["status"] = json!(reported);
        let (status, detail, _) = outcome(&join(&changed), "", 0);
        assert_eq!(status, JobStatus::Failed, "{reported}");
        assert!(detail.contains(hint), "{reported}: {detail}");
    }
    // A later success cannot erase an earlier failed turn.
    let mut follow_up = lines(include_str!("../fixtures/antigravity-error.jsonl"));
    let mut success = records[4].clone();
    success["result"]["conversation_id"] = follow_up[0]["conversation_id"].clone();
    follow_up.push(success);
    assert_eq!(outcome(&join(&follow_up), "", 0).0, JobStatus::Failed);
    // The structured stderr error line also prevents success.
    let (status, detail, _) = outcome(
        SUCCESS,
        "AGY_ERROR: {\"status\":\"UNAVAILABLE\",\"code\":503}\n",
        0,
    );
    assert_eq!(status, JobStatus::Failed);
    assert!(detail.contains("AGY_ERROR"), "{detail}");
}

#[test]
fn authentication_failures_are_classified_without_reading_credentials() {
    for (stderr, code) in [
        ("Error: authentication required\n", 1),
        ("Authentication expired. Please log back in.\n", 1),
    ] {
        let (status, detail, text) = outcome("", stderr, code);
        assert_eq!(status, JobStatus::Failed);
        assert!(
            detail.contains("authentication is required or expired"),
            "{detail}"
        );
        assert!(detail.contains("interactive agy session"));
        assert!(text.contains("[stderr]"));
    }
    // Split across chunks and reported in a result error as well.
    let mut decoder = Antigravity.output();
    decoder.push(Stream::Stderr, b"authentication req");
    decoder.push(Stream::Stderr, b"uired\n");
    decoder.finish();
    assert!(
        decoder
            .interpret(status(1))
            .1
            .contains("authentication is required")
    );
    let mut records = lines(include_str!("../fixtures/antigravity-error.jsonl"));
    records[3]["result"]["error"] = json!("Authentication expired. Please log back in.");
    let (_, detail, _) = outcome(&join(&records), "", 0);
    assert!(detail.contains("authentication is required or expired"));
}

#[test]
fn malformed_unknown_and_oversized_records_fail_closed() {
    let records = lines(SUCCESS);
    let mut malformed = join(&records[..2]);
    malformed.push_str("{broken-json\n");
    malformed.push_str(&join(&records[2..]));
    let (reported, detail, text) = outcome(&malformed, "", 0);
    assert_eq!(reported, JobStatus::Failed);
    assert!(
        detail.contains("Invalid Antigravity JSON record"),
        "{detail}"
    );
    assert!(text.contains("Invalid Antigravity JSON record"));
    for record in ["[]", "{}", "{\"event\":\"\"}", "{\"event\":\"result\"}"] {
        let mut stream = join(&records[..2]);
        stream.push_str(record);
        stream.push('\n');
        stream.push_str(&join(&records[2..]));
        assert_eq!(outcome(&stream, "", 0).0, JobStatus::Failed, "{record}");
    }
    let mut oversized = Antigravity.output();
    oversized.push(Stream::Stdout, format!("{}\n", records[0]).as_bytes());
    for _ in 0..1025 {
        oversized.push(Stream::Stdout, &[b'x'; 1024]);
    }
    oversized.push(Stream::Stdout, b"\n");
    oversized.push(Stream::Stdout, join(&records[1..]).as_bytes());
    oversized.finish();
    assert_eq!(oversized.interpret(status(0)).0, JobStatus::Failed);
}

#[test]
fn refused_actions_and_recoverable_tool_errors_stay_visible_on_success() {
    let (status, detail, text) = outcome(
        include_str!("../fixtures/antigravity-activity.jsonl"),
        "",
        0,
    );
    assert_eq!(status, JobStatus::Succeeded, "{detail}");
    assert!(detail.contains("refused 1 tool action(s)"), "{detail}");
    assert!(detail.contains("may be incomplete"));
    assert!(text.contains("tool failed: view_file"));
    assert!(!text.contains("raw-only tool output"));
    let mut records = lines(SUCCESS);
    records[4]["result"]["denied_actions"] = json!({"unexpected": "shape"});
    let (status, detail, _) = outcome(&join(&records), "", 0);
    assert_eq!(status, JobStatus::Succeeded);
    assert!(detail.contains("unrecognized form"), "{detail}");
}

#[test]
fn every_follow_up_result_must_succeed_for_the_same_conversation() {
    // Not documented for one stdin turn, but a successful background follow-up
    // must not be misclassified (CodeConvoy 1.0 Claude lesson).
    let records = lines(SUCCESS);
    let mut follow_up = records.clone();
    follow_up.push(records[2].clone());
    let mut second = records[4].clone();
    second["result"]["num_turns"] = json!(2);
    follow_up.push(second.clone());
    let (status, detail, _) = outcome(&join(&follow_up), "", 0);
    assert_eq!(status, JobStatus::Succeeded, "{detail}");
    second["result"]["conversation_id"] = json!("other");
    follow_up.pop();
    follow_up.push(second);
    assert_eq!(outcome(&join(&follow_up), "", 0).0, JobStatus::Failed);
}

#[test]
fn reported_working_directory_must_match_the_assigned_checkout() {
    let temp = tempfile::tempdir().unwrap();
    let checkout = temp.path().join("checkout ü");
    let other = temp.path().join("other");
    std::fs::create_dir(&checkout).unwrap();
    std::fs::create_dir(&other).unwrap();
    for (reported, expected) in [
        (checkout.clone(), JobStatus::Succeeded),
        (checkout.join("."), JobStatus::Succeeded),
        (other, JobStatus::Failed),
        (temp.path().join("missing"), JobStatus::Failed),
        ("relative".into(), JobStatus::Failed),
    ] {
        let mut records = lines(SUCCESS);
        records[0]["init"]["cwd"] = json!(reported);
        let mut decoder = Antigravity.output_for(&Default::default(), &checkout);
        decode(decoder.as_mut(), &join(&records), "");
        let (status, detail) = decoder.interpret(status(0));
        assert_eq!(status, expected, "{reported:?}: {detail}");
    }
}
