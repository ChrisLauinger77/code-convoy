//! Opt-in installed-CLI probes use the real builder, decoder and process owner.
use super::*;
use codeconvoy::process::{Cancellation, ProcessResult};
use std::{fs, path::PathBuf, sync::Mutex, time::Duration};

fn git(directory: &Path, args: &[&str]) {
    assert!(
        std::process::Command::new("git")
            .args(args)
            .current_dir(directory)
            .status()
            .unwrap()
            .success()
    );
}

#[tokio::test]
#[ignore = "Uses existing Claude authentication and model calls; set CODECONVOY_CLAUDE_AUDIT_EXECUTABLE"]
async fn real_headless_permission_audit() {
    let executable = std::env::var("CODECONVOY_CLAUDE_AUDIT_EXECUTABLE").unwrap();
    let options = [("executable".into(), executable.clone())].into();
    println!(
        "{}",
        agents::detect(&Claude, &options, Path::new("."))
            .await
            .unwrap()
    );
    let mut failures = Vec::new();
    for case in [
        "normal",
        "denied",
        "mcp-confirm",
        "mcp-elicit",
        "auth",
        "cancel",
    ] {
        if std::env::var("CODECONVOY_CLAUDE_AUDIT_CASES")
            .is_ok_and(|cases| !cases.split(',').any(|selected| selected == case))
        {
            continue;
        }
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source");
        let worktree = root.path().join("isolated");
        fs::create_dir(&source).unwrap();
        git(&source, &["init", "--quiet"]);
        fs::write(source.join("fixture.txt"), "HEADLESS_FIXTURE\n").unwrap();
        git(&source, &["add", "fixture.txt"]);
        git(
            &source,
            &[
                "-c",
                "user.name=Fixture",
                "-c",
                "user.email=fixture@example.invalid",
                "commit",
                "--quiet",
                "-m",
                "fixture",
            ],
        );
        git(
            &source,
            &[
                "worktree",
                "add",
                "--quiet",
                "--detach",
                worktree.to_str().unwrap(),
                "HEAD",
            ],
        );
        fs::create_dir(worktree.join(".claude")).unwrap();
        // Restrict the disposable fixture's Bash operation. Specific approval
        // of the harmless MCP test tool is passed separately below.
        fs::write(
            worktree.join(".claude/settings.json"),
            json!({"permissions":{
                "deny":["Bash(printf *)"]
            }})
            .to_string(),
        )
        .unwrap();
        if case.starts_with("mcp-") || case == "cancel" {
            fs::write(
                worktree.join(".mcp.json"),
                json!({"mcpServers":{"audit":{
                    "command":env!("CARGO_BIN_EXE_codeconvoy-test-agent"),
                    "args":["claude-audit-mcp",root.path()],
                    "timeout":if case == "cancel" {60000} else {1000}
                }}})
                .to_string(),
            )
            .unwrap();
        }
        let mut task = task();
        task.options = [
            ("executable".into(), executable.clone()),
            ("max_turns".into(), "4".into()),
        ]
        .into();
        task.prompt = match case {
            "normal" => "Reply exactly HEADLESS_OK. Do not use tools.",
            "denied" => "In this disposable audit fixture attempt Bash exactly once with command `printf DENIED_PROBE`. Do not run any alternative command or use another tool. Then report the outcome.",
            "mcp-confirm" => "Use ToolSearch to load mcp__audit__confirm, then call it exactly once with {}. Do not use any other tools or retry the call. Then report the outcome.",
            "mcp-elicit" | "cancel" => "Use ToolSearch to load mcp__audit__elicit, then call it exactly once with {}. Do not use any other tools or retry the call. Then report the outcome.",
            _ => "Reply exactly AUTH_UNEXPECTED. Do not use tools.",
        }.into();
        let mut spec = Claude
            .build(
                &task,
                &Repository {
                    name: "audit".into(),
                    path: worktree,
                },
            )
            .unwrap();
        if case.starts_with("mcp-") || case == "cancel" {
            // Approve only this disposable, side-effect-free test tool. No
            // bypass or sandbox change: a human-only tool must still be denied.
            spec.args.push(
                if case == "mcp-confirm" {
                    "--allowedTools=mcp__audit__confirm"
                } else {
                    "--allowedTools=mcp__audit__elicit"
                }
                .into(),
            );
        }
        spec.env.push(("DISABLE_AUTOUPDATER".into(), "1".into()));
        if case == "auth" {
            let config = root.path().join("empty-config");
            fs::create_dir(&config).unwrap();
            spec.env
                .push(("CLAUDE_CONFIG_DIR".into(), config.to_str().unwrap().into()));
            spec.remove_env
                .extend(std::env::vars_os().filter_map(|(key, _)| {
                    let key = key.to_str()?;
                    (key.starts_with("ANTHROPIC_") || key.starts_with("CLAUDE_CODE_"))
                        .then(|| key.to_owned())
                }));
        }
        let cancellation = Cancellation::default();
        let captured = Mutex::new((Vec::new(), Vec::new()));
        let execute = process::execute_until(
            process::spawn(&spec).unwrap(),
            spec.input,
            &cancellation,
            Some(tokio::time::Instant::now() + Duration::from_secs(90)),
            process::cancel,
            |stream, bytes| {
                let mut c = captured.lock().unwrap();
                if stream == Stream::Stdout {
                    c.0.extend_from_slice(bytes)
                } else {
                    c.1.extend_from_slice(bytes)
                }
            },
        );
        let stop = async {
            if case == "cancel" {
                let ready = tokio::time::timeout(Duration::from_secs(80), async {
                    while !root.path().join("elicitation.ready").exists() {
                        tokio::time::sleep(Duration::from_millis(10)).await;
                    }
                })
                .await;
                cancellation.cancel();
                ready.is_ok()
            } else {
                true
            }
        };
        let (result, ready) = tokio::join!(execute, stop);
        let result: ProcessResult = result.unwrap();
        let (stdout, stderr) = captured.into_inner().unwrap();
        // Optional private captures for audit/sanitization, never printed wholesale.
        if let Some(path) = std::env::var_os("CODECONVOY_CLAUDE_AUDIT_CAPTURE") {
            let path = PathBuf::from(path);
            fs::create_dir_all(&path).unwrap();
            fs::write(path.join(format!("{case}.stdout.jsonl")), &stdout).unwrap();
            fs::write(path.join(format!("{case}.stderr.txt")), &stderr).unwrap();
            if let Ok(response) = fs::read(root.path().join("question-response.json")) {
                fs::write(
                    path.join(format!("{case}.question-response.json")),
                    response,
                )
                .unwrap();
            }
        }
        let mut output = Claude.output_with_options(&task.options);
        output.push(Stream::Stdout, &stdout);
        output.push(Stream::Stderr, &stderr);
        output.finish();
        let outcome = output.interpret(result.status.unwrap()).0;
        let events: Vec<Value> = String::from_utf8_lossy(&stdout)
            .lines()
            .filter_map(|l| serde_json::from_str(l).ok())
            .collect();
        println!(
            "{case}: exit={:?}, cancelled={}, timeout={}, adapter={outcome:?}",
            result.status.unwrap().code(),
            result.cancelled,
            result.timed_out
        );
        for e in events.iter().filter(|e| e["type"] == "result") {
            println!(
                "  result: subtype={}, is_error={}, terminal_reason={}, denials={}",
                e["subtype"],
                e["is_error"],
                e["terminal_reason"],
                e["permission_denials"].as_array().map_or(0, Vec::len)
            );
        }
        assert!(!result.timed_out, "{case}: process deadline expired");
        assert!(ready, "{case}: MCP elicitation did not start");
        if case == "cancel" {
            assert!(result.cancelled);
            assert!(root.path().join("elicitation.ready").exists());
        } else if outcome
            != if case == "normal" {
                JobStatus::Succeeded
            } else {
                JobStatus::Failed
            }
        {
            failures.push(case);
        }
        if case == "mcp-elicit" {
            assert!(root.path().join("elicitation.ready").exists());
            let response: Value = serde_json::from_slice(
                &fs::read(root.path().join("question-response.json")).unwrap(),
            )
            .unwrap();
            assert_eq!(response["result"]["action"], "cancel");
        }
        assert!(!root.path().join("confirm.called").exists());
        if case == "normal" {
            assert!(
                events
                    .iter()
                    .any(|e| e["type"] == "result" && e["result"] == "HEADLESS_OK")
            );
        }
        if case.starts_with("mcp-") {
            let tool = if case == "mcp-confirm" {
                "mcp__audit__confirm"
            } else {
                "mcp__audit__elicit"
            };
            assert!(
                events.iter().any(|e| e["type"] == "assistant"
                    && e["message"]["content"]
                        .as_array()
                        .is_some_and(|blocks| blocks
                            .iter()
                            .any(|b| b["type"] == "tool_use" && b["name"] == tool))),
                "{case}: required tool was never attempted"
            );
        }
        if root.path().join("mcp.lock").exists() {
            let lock = fs::OpenOptions::new()
                .write(true)
                .open(root.path().join("mcp.lock"))
                .unwrap();
            assert!(
                fs2::FileExt::try_lock_exclusive(&lock).is_ok(),
                "MCP process survived {case}"
            );
        }
    }
    assert!(failures.is_empty(), "Misclassified cases: {failures:?}");
}
