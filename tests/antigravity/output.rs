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
        (4, "/result/error", json!({"message": "structured error"})),
        (4, "/result/error", json!(["error"])),
        (4, "/result/error", json!(1)),
        (4, "/result/error", json!(false)),
        (4, "/event", json!("future_event")),
    ] {
        let mut changed = records.clone();
        let (parent, key) = pointer.rsplit_once('/').unwrap();
        changed[index].pointer_mut(parent).unwrap()[key] = value.clone();
        let (status, detail, _) = outcome(&join(&changed), "", 0);
        assert_eq!(status, JobStatus::Failed, "{pointer}={value}: {detail}");
    }
    // An absent or empty error remains a consistent successful result.
    let mut empty_error = records.clone();
    empty_error[4]["result"]["error"] = json!("");
    assert_eq!(outcome(&join(&empty_error), "", 0).0, JobStatus::Succeeded);
    let mut structured = records.clone();
    structured[4]["result"]["error"] = json!({"message": "structured error"});
    assert!(
        outcome(&join(&structured), "", 0)
            .1
            .contains("non-string error")
    );
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
    assert!(detail.contains("Antigravity error — fixture quota exhausted"));
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
            detail.contains("sign-in is required or has expired"),
            "{detail}"
        );
        assert!(detail.contains("Run agy once in a terminal"));
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
            .contains("sign-in is required")
    );
    let mut records = lines(include_str!("../fixtures/antigravity-error.jsonl"));
    records[3]["result"]["error"] = json!("Authentication expired. Please log back in.");
    let (_, detail, _) = outcome(&join(&records), "", 0);
    assert!(detail.contains("sign-in is required or has expired"));
}

#[test]
fn installed_1_4_2_unauthenticated_output_is_an_authentication_failure() {
    // Captured from agy 1.4.2 with CodeConvoy's exact invocation, no sign-in:
    // a lone ERROR result without init, exit 1, and a stderr notice.
    let stdout = include_str!("../fixtures/antigravity-1.4.2-authentication.stdout.jsonl");
    let stderr = include_str!("../fixtures/antigravity-1.4.2-authentication.stderr.txt");
    let (status, detail, text) = outcome(stdout, stderr, 1);
    assert_eq!(status, JobStatus::Failed);
    assert!(
        detail.contains("sign-in is required or has expired"),
        "{detail}"
    );
    assert!(text.contains("Antigravity result: ERROR · authentication failed"));
    // The result error alone is recognized too, even if stderr wording changes.
    let (status, detail, _) = outcome(stdout, "", 1);
    assert_eq!(status, JobStatus::Failed);
    assert!(
        detail.contains("sign-in is required or has expired"),
        "{detail}"
    );
}

#[test]
fn failure_details_lead_with_actionable_text_and_recovery_hints() {
    let records = lines(SUCCESS);
    for (error, advice) in [
        (
            "invalid model selection (--model \"x\" --effort \"\"): model x is not recognized as a known model or custom model in settings",
            "choose a slug listed by agy models",
        ),
        (
            "Your AI credits balance is too low to continue.",
            "check agy -p /usage",
        ),
        ("fixture quota exhausted", "check agy -p /usage"),
        (
            "/model is answered by the CLI itself and is unavailable with --input-format stream-json",
            "start the task with ordinary text",
        ),
    ] {
        let mut changed = records.clone();
        changed[4]["result"]["status"] = json!("ERROR");
        changed[4]["result"]["error"] = json!(error);
        let (status, detail, _) = outcome(&join(&changed), "", 0);
        assert_eq!(status, JobStatus::Failed);
        // The compact summary ends at the first colon, which provider error
        // text may contain; the advice must come before it.
        let summary = detail.split(": ").next().unwrap();
        assert!(summary.contains(advice), "{error}: {summary}");
        assert!(
            detail.contains("(exit code 0; conversation 4e502687"),
            "{detail}"
        );
    }
    // Advice from a structured stderr error, without a result error.
    let (_, detail, _) = outcome(
        SUCCESS,
        "AGY_ERROR: {\"status\":\"RESOURCE_EXHAUSTED\",\"code\":429}\n",
        0,
    );
    assert!(detail.contains("AGY_ERROR") && detail.contains("check agy -p /usage"));
    // No advice is invented for an unrecognized error, and success stays clean.
    let (_, detail, _) = outcome(
        include_str!("../fixtures/antigravity-interrupted.jsonl"),
        "",
        130,
    );
    assert!(!detail.contains("agy models") && !detail.contains("/usage"));
    assert!(detail.contains("(exit code 130;"), "{detail}");
    // Summaries show text before the first colon; keep it actionable.
    for (stdout, stderr, code) in [
        ("", "Error: authentication required\n", 1),
        (include_str!("../fixtures/antigravity-error.jsonl"), "", 0),
        (SUCCESS, "", 7),
    ] {
        let (_, detail, _) = outcome(stdout, stderr, code);
        assert!(!detail.contains(": "), "{detail}");
    }
}

#[test]
fn every_failure_keeps_a_known_conversation_id() {
    let records = lines(SUCCESS);
    let initialized = join(&records[..2]);
    let without_cwd = {
        let mut changed = records.clone();
        changed[0]["init"]["cwd"] = Value::Null;
        join(&changed)
    };
    for (stdout, stderr, code, expected) in [
        (
            initialized.as_str(),
            "Authentication expired. Please log back in.\n",
            1,
            "sign-in is required",
        ),
        (SUCCESS, "", 1, "process failed"),
        (without_cwd.as_str(), "", 0, "missing its working directory"),
        (initialized.as_str(), "", 0, "incomplete"),
    ] {
        let (status, detail, _) = outcome(stdout, stderr, code);
        assert_eq!(status, JobStatus::Failed);
        assert!(detail.contains(expected), "{detail}");
        assert!(
            detail.contains(&format!("(exit code {code}; conversation 4e502687")),
            "{detail}"
        );
        assert!(!detail.contains(": "), "{detail}");
    }
    // No conversation is invented when none was reported.
    let (_, detail, _) = outcome("", "Error: authentication required\n", 1);
    assert!(detail.contains("(exit code 1)") && !detail.contains("conversation"));
}

#[cfg(unix)]
#[test]
fn signal_terminations_keep_the_signal_without_a_colon() {
    use std::os::unix::process::ExitStatusExt;
    let records = lines(SUCCESS);
    let initialized = join(&records[..2]);
    // Raw wait statuses: signal in the low bits, 0x80 marks a core dump.
    for (raw, expected) in [
        (libc::SIGKILL, "terminated by signal 9 (SIGKILL)"),
        (
            libc::SIGSEGV | 0x80,
            "terminated by signal 11 (SIGSEGV), core dumped",
        ),
        (libc::SIGTERM, "(SIGTERM)"),
    ] {
        let mut output = Antigravity.output();
        output.push(Stream::Stdout, initialized.as_bytes());
        output.finish();
        let (status, detail) = output.interpret(ExitStatus::from_raw(raw));
        assert_eq!(status, JobStatus::Failed);
        assert!(detail.contains(expected), "{detail}");
        assert!(detail.contains("; conversation 4e502687"), "{detail}");
        assert!(!detail.contains(": "), "{detail}");
    }
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
fn success_is_rejected_while_any_step_is_still_active() {
    let records = lines(SUCCESS);
    let conversation = records[0]["conversation_id"].clone();
    let step = |index: u64, state: &str, kind: &str| {
        let mut step = json!({"event":"step_update","step_update":{"conversation_id":conversation,"step_index":index,"state":state,"step_type":kind}});
        if kind == "tool" {
            step["step_update"]["tool_name"] = json!("run_command");
        } else if kind == "agent_response" {
            step["step_update"]["text_delta"] = json!("partial ");
        }
        step
    };
    for kind in [
        "tool",
        "agent_response",
        "user_input",
        "checkpoint",
        "future_step",
    ] {
        let mut active = records.clone();
        active.insert(2, step(7, "ACTIVE", kind));
        let (status, detail, _) = outcome(&join(&active), "", 0);
        assert_eq!(status, JobStatus::Failed, "{kind}");
        assert!(detail.contains("step was still active"), "{kind}: {detail}");
        // Repeated ACTIVE updates for one step finish with its single DONE update.
        let mut finished = records.clone();
        for (offset, state) in ["ACTIVE", "ACTIVE", "DONE"].into_iter().enumerate() {
            finished.insert(2 + offset, step(7, state, kind));
        }
        let (status, detail, text) = outcome(&join(&finished), "", 0);
        assert_eq!(status, JobStatus::Succeeded, "{kind}: {detail}");
        if kind == "tool" {
            assert_eq!(text.matches("tool started: run_command").count(), 1);
        }
    }
    // More unfinished steps than can be tracked cannot be accepted silently.
    let mut overflow = records[..2].to_vec();
    overflow.extend((0..1025).map(|index| step(100 + index, "ACTIVE", "tool")));
    overflow.extend((0..1025).map(|index| step(100 + index, "DONE", "tool")));
    overflow.extend_from_slice(&records[2..]);
    let (status, detail, _) = outcome(&join(&overflow), "", 0);
    assert_eq!(status, JobStatus::Failed);
    assert!(detail.contains("Too many unfinished"), "{detail}");
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
