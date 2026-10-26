#![allow(clippy::unwrap_used)]
use codeconvoy::{
    agents::{AgentBackend, codex::Codex},
    domain::JobStatus,
    process::Stream,
};
use std::{path::PathBuf, process::ExitStatus};

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
/// A CLI CodeConvoy stopped: SIGKILL on Unix, a terminated Job Object on Windows.
fn stopped() -> ExitStatus {
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        ExitStatus::from_raw(9)
    }
    #[cfg(windows)]
    {
        status(1)
    }
}
fn audit() -> (PathBuf, serde_json::Value) {
    let root =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/codex-0.164-audit");
    let manifest = serde_json::from_slice(&std::fs::read(root.join("captures.json")).unwrap());
    (root, manifest.unwrap())
}

#[test]
fn official_0_163_and_0_164_captures_keep_the_existing_completion_rule() {
    // Official binaries against a local deterministic provider: a rejected
    // escalation, a sandbox denial, a failed request and runner cancellation.
    // The provider has no WebSocket endpoint, so streams also carry the CLI's
    // retried `error` records, which must not fail a completed turn.
    let (root, manifest) = audit();
    for capture in manifest["captures"].as_array().unwrap() {
        let file = capture["file"].as_str().unwrap();
        let data = std::fs::read(root.join(file)).unwrap();
        let mut output = Codex.output();
        for chunk in data.chunks(37) {
            output.push(Stream::Stdout, chunk);
        }
        output.finish();
        let exit = capture["exit_code"]
            .as_i64()
            .map_or_else(stopped, |code| status(code as i32));
        let expected = match capture["expected"].as_str().unwrap() {
            "Succeeded" => JobStatus::Succeeded,
            "Failed" => JobStatus::Failed,
            other => panic!("unexpected capture outcome: {other}"),
        };
        assert_eq!(output.interpret(exit).0, expected, "{file}");
        if expected == JobStatus::Succeeded {
            // exec exits 1 after a non-retried turn error or an interrupted
            // turn, so a reported turn.completed alone is never success.
            for code in [1, 130] {
                assert_eq!(
                    output.interpret(status(code)).0,
                    JobStatus::Failed,
                    "{file}"
                );
            }
            assert_eq!(output.interpret(stopped()).0, JobStatus::Failed, "{file}");
        }
        if capture["runner"] == "Cancelled" {
            // The runner owns Cancelled. Without turn.completed, even a zero
            // exit cannot pass the decoder's success rule.
            assert_eq!(output.interpret(status(0)).0, JobStatus::Failed, "{file}");
        }
        let activity = output.take_activity().unwrap();
        match capture["scenario"].as_str().unwrap() {
            "escalation-rejected" => assert!(activity.contains("AUDIT_AFTER_ESCALATION_DENIAL")),
            "sandbox-denied" => assert!(activity.contains("AUDIT_AFTER_SANDBOX_DENIAL")),
            "failed-request" => assert!(activity.contains("Codex turn failed")),
            _ => {}
        }
    }
}

#[test]
fn official_0_163_and_0_164_help_passes_detection() {
    let (root, manifest) = audit();
    for help in manifest["help"].as_array().unwrap() {
        let file = help["file"].as_str().unwrap();
        let text = std::fs::read_to_string(root.join(file)).unwrap();
        Codex.check_detection(&text).unwrap();
    }
    assert!(
        Codex
            .check_detection("codex exec --ask-for-approval")
            .is_err()
    );
}
