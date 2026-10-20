#![allow(clippy::unwrap_used)]
#[cfg(feature = "test-support")]
#[path = "support/executable.rs"]
mod executable;

use codeconvoy::{
    agents::{self, AgentBackend, OptionKind, opencode::OpenCode},
    domain::{AgentId, AppState, JobStatus, Repository, TaskConfig},
    persistence::Store,
    process::{self, Stream},
};
use std::{path::Path, process::ExitStatus};

fn task() -> TaskConfig {
    TaskConfig {
        agent: AgentId::OpenCode,
        prompt: "--help\nTreat $(echo hello), `whoami`, quotes and Grüße 🌍 literally.".into(),
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
const STOP: &[u8] = b"{\"type\":\"step_finish\",\"sessionID\":\"ses_fixture\",\"part\":{\"type\":\"step-finish\",\"reason\":\"stop\",\"cost\":0,\"tokens\":{}}}\n";

#[test]
fn default_command_uses_exact_stdin_native_repository_path_and_existing_rules() {
    let task = task();
    let repo = repo();
    let spec = OpenCode.build(&task, &repo).unwrap();
    assert_eq!(spec.program, "opencode");
    assert_eq!(spec.directory, repo.path);
    assert_eq!(spec.args[..4], ["run", "--format", "json", "--dir"]);
    assert_eq!(spec.args[4], repo.path.as_os_str());
    assert_eq!(spec.args.len(), 5);
    assert_eq!(spec.input.as_deref(), Some(task.prompt.as_bytes()));
    assert!(spec.remove_env.iter().any(|key| key == "PWD"));
    for key in [
        "OPENCODE_CONFIG",
        "OPENCODE_CONFIG_CONTENT",
        "OPENCODE_CONFIG_DIR",
        "OPENCODE_PERMISSION",
        "OPENAI_API_KEY",
        "ANTHROPIC_API_KEY",
    ] {
        assert!(!spec.remove_env.iter().any(|k| k == key));
    }
    assert!(spec.env.is_empty());
    assert!(
        OpenCode
            .execution_summary(&task.options)
            .contains("not an OS sandbox")
    );
    let backend = agents::backend(AgentId::OpenCode).unwrap();
    assert_eq!(backend.id(), AgentId::OpenCode);
    for spec in backend.options() {
        if let OptionKind::Choice(choices) = spec.kind {
            assert!(choices.iter().any(|(value, _)| *value == spec.default));
        }
    }
    assert!(agents::backend(AgentId::Claude).is_ok());
}

fn opencode2(mut task: TaskConfig) -> TaskConfig {
    task.options.insert("version".into(), "2".into());
    task
}

#[test]
#[cfg(windows)]
fn opencode_2_runs_standalone_in_the_job_directory_without_dir() {
    let task = opencode2(task());
    let repo = repo();
    let spec = OpenCode.build(&task, &repo).unwrap();
    assert_eq!(spec.program, "opencode");
    // The process directory is OpenCode 2's only workspace signal; --standalone
    // gives this job a private server instead of the shared background service.
    assert_eq!(spec.directory, repo.path);
    assert_eq!(spec.args, ["run", "--standalone", "--format", "json"]);
    assert!(
        !spec
            .args
            .iter()
            .any(|a| a == "--dir" || a == repo.path.as_os_str())
    );
    assert_eq!(spec.input.as_deref(), Some(task.prompt.as_bytes()));
    assert!(spec.remove_env.iter().any(|key| key == "PWD"));
    assert_eq!(
        spec.remove_env,
        OpenCode.build(&self::task(), &repo).unwrap().remove_env
    );
    assert!(spec.env.is_empty());
    assert!(
        OpenCode
            .execution_summary(&task.options)
            .contains("private standalone server")
    );
    assert!(
        !OpenCode
            .execution_summary(&self::task().options)
            .contains("standalone")
    );
}

#[test]
#[cfg(windows)]
fn opencode_2_joins_variant_to_the_model_and_rejects_unrepresentable_variants() {
    for mode in ["existing", "auto"] {
        let mut task = opencode2(task());
        for (key, value) in [
            ("model", "provider/model/submodel"),
            ("agent", "--literal agent;$(no shell)"),
            ("variant", "custom-high"),
            ("permissions", mode),
        ] {
            task.options.insert(key.into(), value.into());
        }
        let spec = OpenCode.build(&task, &repo()).unwrap();
        let mut expected = vec![
            "run",
            "--standalone",
            "--format",
            "json",
            "--model=provider/model/submodel#custom-high",
            "--agent=--literal agent;$(no shell)",
        ];
        if mode == "auto" {
            expected.push("--auto");
        }
        assert_eq!(spec.args, expected);
        assert!(
            !spec
                .args
                .iter()
                .any(|a| a.to_string_lossy().starts_with("--variant"))
        );
    }
    let mut model_only = opencode2(task());
    model_only
        .options
        .insert("model".into(), "provider/model".into());
    assert_eq!(
        OpenCode.build(&model_only, &repo()).unwrap().args[4],
        "--model=provider/model"
    );
    for (model, variant, message) in [
        ("", "high", "Enter a model or clear Variant"),
        ("provider/model#low", "high", "provider/model#variant"),
        ("provider/model", "hi#gh", "provider/model#variant"),
        ("provider/model", "very high", "provider/model#variant"),
    ] {
        let mut task = opencode2(task());
        task.options.insert("model".into(), model.into());
        task.options.insert("variant".into(), variant.into());
        let error = OpenCode.build(&task, &repo()).unwrap_err().to_string();
        assert!(error.contains(message), "{model} {variant}: {error}");
        assert!(OpenCode.detection(&task.options, Path::new(".")).is_err());
        // OpenCode 1 keeps its own --variant semantics for the same settings.
        task.options.remove("version");
        assert!(OpenCode.build(&task, &repo()).is_ok());
    }
    let mut unsupported = task();
    unsupported.options.insert("version".into(), "3".into());
    assert!(OpenCode.build(&unsupported, &repo()).is_err());
}

#[test]
fn both_versions_use_the_exact_absolute_directory_with_spaces_and_unicode() {
    let repo = Repository {
        name: "unicode".into(),
        path: std::env::temp_dir()
            .join("Grüße repo 日本 🌍")
            .join("with spaces"),
    };
    let v1 = OpenCode.build(&task(), &repo).unwrap();
    assert_eq!(v1.directory, repo.path);
    assert_eq!(v1.args[3], "--dir");
    assert_eq!(v1.args[4], repo.path.as_os_str());
    #[cfg(windows)]
    {
        let v2 = OpenCode.build(&opencode2(task()), &repo).unwrap();
        assert_eq!(v2.directory, repo.path);
        assert_eq!(v2.args.len(), 4);
    }
    for task in [task(), opencode2(task())] {
        // A relative directory would resolve against CodeConvoy's own cwd.
        let error = OpenCode
            .build(
                &task,
                &Repository {
                    name: "relative".into(),
                    path: "relative/repo".into(),
                },
            )
            .unwrap_err();
        assert!(error.to_string().contains("absolute"), "{error}");
    }
}

#[test]
#[cfg(windows)]
fn help_detection_verifies_the_selected_major_version_and_rejects_unknown_clis() {
    let v1 = include_str!("fixtures/opencode-1.19.3-run-help.txt");
    let v2 = include_str!("fixtures/opencode-2.0.32-run-help.txt");
    // Launch preflight materializes the version setting.
    let one = [("version".into(), "1".into())].into();
    let two = opencode2(TaskConfig::default()).options;
    OpenCode.check_detection_for(&one, v1).unwrap();
    OpenCode.check_detection_for(&two, v2).unwrap();
    // The option-free contract is OpenCode 1, the default selection.
    OpenCode.check_detection(v1).unwrap();
    for (options, help, detected, selected) in [
        (&one, v2, "OpenCode 2", "OpenCode 1"),
        (&two, v1, "OpenCode 1", "OpenCode 2"),
    ] {
        let error = OpenCode
            .check_detection_for(options, help)
            .unwrap_err()
            .to_string();
        assert!(
            error.contains(&format!("offers the {detected} run interface"))
                && error.contains(&format!("{selected} is selected"))
                && error.contains("OpenCode version"),
            "{error}"
        );
    }
    for flag in ["--standalone", "--format", "--model", "--agent", "--auto"] {
        let error = OpenCode
            .check_detection_for(&two, &v2.replace(flag, &format!("{flag}-unrelated")))
            .unwrap_err()
            .to_string();
        assert!(
            error.contains(&format!("OpenCode 2 run is missing {flag}")),
            "{error}"
        );
    }
    // --standalone in another flag's description is not the flag itself.
    let without_flag: String = v2
        .lines()
        .filter(|line| !line.trim_start().starts_with("--standalone"))
        .map(|line| format!("{line}\n"))
        .collect();
    assert!(without_flag.contains("require --standalone"));
    assert!(
        OpenCode
            .check_detection_for(&two, &without_flag)
            .unwrap_err()
            .to_string()
            .contains("missing --standalone")
    );
    for help in [
        "",
        "codex exec --json",
        "unrelated executable",
        &v2.replace("run [flags]", "run [options]"),
        &v2.replace("json", "text"),
    ] {
        for options in [&one, &two] {
            let error = OpenCode
                .check_detection_for(options, help)
                .unwrap_err()
                .to_string();
            assert!(
                error.contains("Expected OpenCode") || error.contains("is missing"),
                "{error}"
            );
        }
    }
}

#[test]
#[cfg(unix)]
fn opencode_2_is_refused_on_macos_and_linux_before_any_command_runs() {
    // Its standalone server would run outside process-group cancellation.
    const REFUSED: &str = "not supported on macOS and Linux";
    let v1 = include_str!("fixtures/opencode-1.19.3-run-help.txt");
    let v2 = include_str!("fixtures/opencode-2.0.32-run-help.txt");
    // Launch preflight materializes the version setting.
    let one = [("version".into(), "1".into())].into();
    let two = opencode2(TaskConfig::default()).options;
    OpenCode.check_detection_for(&one, v1).unwrap();
    OpenCode.check_detection(v1).unwrap();
    for error in [
        OpenCode.check_detection_for(&two, v2).unwrap_err(),
        OpenCode.check_detection_for(&two, v1).unwrap_err(),
        OpenCode.detection(&two, Path::new(".")).unwrap_err(),
        OpenCode.version_command(&two, Path::new(".")).unwrap_err(),
        OpenCode.build(&opencode2(task()), &repo()).unwrap_err(),
    ] {
        assert!(error.to_string().contains(REFUSED), "{error}");
    }
    // An OpenCode 2 executable is named, without advice to select OpenCode 2.
    let error = OpenCode
        .check_detection_for(&one, v2)
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("offers the OpenCode 2 run interface")
            && error.contains(REFUSED)
            && !error.contains("Choose OpenCode 2"),
        "{error}"
    );
    for help in [
        "",
        "codex exec --json",
        &v1.replace("--dir", "--dir-unrelated"),
    ] {
        let error = OpenCode
            .check_detection_for(&one, help)
            .unwrap_err()
            .to_string();
        assert!(
            error.contains("Expected OpenCode 1") || error.contains("OpenCode 1 run is missing"),
            "{error}"
        );
    }
    let version = &OpenCode.options()[1];
    assert_eq!(version.key, "version");
    assert!(version.help.contains("only on Windows"));
}

#[test]
fn check_cli_without_a_version_accepts_either_supported_interface() {
    let v1 = include_str!("fixtures/opencode-1.19.3-run-help.txt");
    let v2 = include_str!("fixtures/opencode-2.0.32-run-help.txt");
    // Availability checks pass only the executable, never job settings.
    let probe = [("executable".into(), "opencode".into())].into();
    OpenCode.check_detection_for(&probe, v1).unwrap();
    let broken = v2.replace("--auto", "--auto-unrelated");
    if cfg!(windows) {
        OpenCode.check_detection_for(&probe, v2).unwrap();
        let error = OpenCode
            .check_detection_for(&probe, &broken)
            .unwrap_err()
            .to_string();
        assert!(
            error.contains("OpenCode 2 run is missing --auto"),
            "{error}"
        );
    } else {
        for help in [v2, broken.as_str()] {
            let error = OpenCode
                .check_detection_for(&probe, help)
                .unwrap_err()
                .to_string();
            assert!(
                error.contains("offers the OpenCode 2 run interface")
                    && error.contains("not supported on macOS and Linux"),
                "{error}"
            );
        }
    }
    let error = OpenCode
        .check_detection_for(&probe, &v1.replace("--dir", "--dir-unrelated"))
        .unwrap_err()
        .to_string();
    assert!(error.contains("OpenCode 1 run is missing --dir"), "{error}");
    // Launch passes the materialized setting, which must match the executable.
    let one = [("version".into(), "1".into())].into();
    let error = OpenCode
        .check_detection_for(&one, v2)
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("offers the OpenCode 2 run interface"),
        "{error}"
    );
}

fn decode(options: &codeconvoy::domain::AgentOptions, data: &str) -> Box<dyn agents::AgentOutput> {
    let mut output = OpenCode.output_with_options(options);
    output.push(Stream::Stdout, data.as_bytes());
    output.finish();
    output
}

#[test]
fn captured_runs_complete_under_each_versions_own_rule() {
    // Real `run --format json` output from a read-tool task in a disposable
    // repository (unauthenticated sandbox, OpenCode's free default model).
    let v1 = include_str!("fixtures/opencode-1.19.3-run.jsonl");
    let v2 = include_str!("fixtures/opencode-2.0.32-run.jsonl");
    let one = task().options;
    let two = opencode2(task()).options;
    // OpenCode 1 ends with step_finish reason=stop.
    let mut output = OpenCode.output_with_options(&one);
    let decoded = output.push(Stream::Stdout, v1.as_bytes()) + &output.finish();
    assert!(decoded.contains("smoke"));
    let activity = output.take_activity().unwrap();
    assert!(
        activity.contains("Tool (completed): read · /private/tmp/"),
        "{activity}"
    );
    assert_eq!(output.interpret(status(0)).0, JobStatus::Succeeded);
    // OpenCode 2 never reports its final step: its last event is the text.
    assert!(v2.lines().last().unwrap().contains(r#""type":"text""#));
    let mut output = OpenCode.output_with_options(&two);
    let decoded = output.push(Stream::Stdout, v2.as_bytes()) + &output.finish();
    assert!(decoded.contains("smoke"));
    let activity = output.take_activity().unwrap();
    assert!(
        activity.contains("Tool (completed): read · README.md"),
        "{activity}"
    );
    assert!(activity.contains("OpenCode step finished: tool-calls"));
    assert_eq!(output.interpret(status(0)).0, JobStatus::Succeeded);
    // OpenCode 2 exits 1 after reported errors and 130 after an interrupt.
    for code in [1, 130] {
        assert_eq!(output.interpret(status(code)).0, JobStatus::Failed);
    }
    // The OpenCode 1 rule still needs the final stop that OpenCode 2 omits.
    assert_eq!(decode(&one, v2).interpret(status(0)).0, JobStatus::Failed);
}

#[test]
fn opencode_2_success_still_needs_an_answered_final_step_and_no_errors() {
    let v2 = include_str!("fixtures/opencode-2.0.32-run.jsonl");
    let two = opencode2(task()).options;
    let lines: Vec<&str> = v2.lines().collect();
    let error = r#"{"type":"error","timestamp":1,"sessionID":"","error":{"type":"unknown","message":"Failed to change directory to /missing"}}"#;
    let stop = r#"{"type":"step_finish","part":{"type":"step-finish","reason":"stop"}}"#;
    let length = r#"{"type":"step_finish","part":{"type":"step-finish","reason":"length"}}"#;
    // A final step_finish, when one does arrive, must be a stop.
    assert_eq!(
        decode(&two, &format!("{v2}{stop}\n"))
            .interpret(status(0))
            .0,
        JobStatus::Succeeded
    );
    for data in [
        // Exit 0 alone, or no step that started and answered, is not success.
        String::new(),
        lines[0].to_owned() + "\n",
        r#"{"type":"text","part":{"type":"text","text":"no step"}}"#.to_owned() + "\n",
        // A last step that finished for tool calls was never continued, and a
        // final step without its own response does not count earlier answers.
        lines[..3].join("\n") + "\n",
        lines[..4].join("\n") + "\n",
        format!("{v2}{length}\n"),
        // A reported error or a malformed or incomplete record overrides it.
        format!("{error}\n{v2}"),
        format!("{v2}{error}\n"),
        format!("{v2}{{\"type\":\"text\",\"part\":\n"),
        format!("{}\n{{broken", lines.join("\n")),
    ] {
        assert_eq!(
            decode(&two, &data).interpret(status(0)).0,
            JobStatus::Failed,
            "{data}"
        );
    }
}

#[test]
fn optional_model_agent_variant_and_permission_modes_are_opencode_specific() {
    for mode in ["existing", "auto"] {
        let mut task = task();
        task.options = [
            ("model".into(), "provider/model/submodel".into()),
            ("agent".into(), "--literal agent;$(no shell)".into()),
            ("variant".into(), "custom-high".into()),
            ("permissions".into(), mode.into()),
        ]
        .into();
        let spec = OpenCode.build(&task, &repo()).unwrap();
        assert_eq!(
            spec.args[5..8],
            [
                "--model=provider/model/submodel",
                "--agent=--literal agent;$(no shell)",
                "--variant=custom-high"
            ]
        );
        assert_eq!(spec.args.iter().any(|a| a == "--auto"), mode == "auto");
        assert_eq!(spec.args.len(), if mode == "auto" { 9 } else { 8 });
        assert_eq!(spec.input.as_deref(), Some(task.prompt.as_bytes()));
        assert!(spec.env.is_empty()); // No permission/provider configuration is rewritten.
    }
    let mut blank = task();
    for key in ["model", "agent", "variant"] {
        blank.options.insert(key.into(), "  ".into());
    }
    assert_eq!(OpenCode.build(&blank, &repo()).unwrap().args.len(), 5);
}

#[test]
fn rejects_invalid_configuration_without_a_provider_catalog_or_credentials() {
    for (key, value) in [
        ("model", "unqualified"),
        ("model", "/model"),
        ("model", "provider/"),
        ("model", "provider/model with spaces"),
        ("agent", "bad\nname"),
        ("variant", "bad\0name"),
        ("permissions", "allow-all"),
        ("sandbox", "workspace-write"),
        ("reasoning_effort", "high"),
        ("api_key", "never-accepted"),
        ("executable", ""),
        ("executable", "./opencode"),
    ] {
        let mut task = task();
        task.options.insert(key.into(), value.into());
        assert!(
            OpenCode.build(&task, &repo()).is_err(),
            "accepted {key}={value}"
        );
        assert!(OpenCode.detection(&task.options, Path::new(".")).is_err());
    }
    let mut invalid = task();
    invalid.prompt.clear();
    assert!(OpenCode.build(&invalid, &repo()).is_err());
    invalid = task();
    invalid.agent = AgentId::Copilot;
    assert!(OpenCode.build(&invalid, &repo()).is_err());
    invalid = task();
    invalid.options.insert("variant".into(), "x".repeat(4097));
    assert!(OpenCode.build(&invalid, &repo()).is_err());
}

#[test]
fn help_detection_checks_complete_flags_and_uses_the_selected_executable_for_version() {
    let exe = std::env::temp_dir().join("OpenCode executable");
    let options = [("executable".into(), exe.to_str().unwrap().into())].into();
    let detection = OpenCode.detection(&options, Path::new(".")).unwrap();
    let version = OpenCode
        .version_command(&options, Path::new("."))
        .unwrap()
        .unwrap();
    assert_eq!(detection.program, exe);
    assert_eq!(detection.program, version.program);
    assert_eq!(detection.args, ["run", "--help"]);
    assert_eq!(version.args, ["--version"]);
    assert!(detection.input.is_none() && version.input.is_none());
    let help = include_str!("fixtures/opencode-1.19.3-run-help.txt");
    OpenCode.check_detection(help).unwrap();
    // OpenCode 1 prints this help to stderr, OpenCode 2 to stdout.
    let text = OpenCode.help_text(b"", help.as_bytes());
    OpenCode
        .check_detection_for(&Default::default(), &text)
        .unwrap();
    let v2 = include_str!("fixtures/opencode-2.0.32-run-help.txt");
    let text = OpenCode.help_text(v2.as_bytes(), b"");
    let result = OpenCode.check_detection_for(&Default::default(), &text);
    if cfg!(windows) {
        result.unwrap();
    } else {
        // Recognized as OpenCode 2, which this platform still refuses.
        let error = result.unwrap_err().to_string();
        assert!(
            error.contains("offers the OpenCode 2 run interface"),
            "{error}"
        );
    }
    for flag in [
        "--format",
        "--dir",
        "--model",
        "--agent",
        "--variant",
        "--auto",
    ] {
        let error = OpenCode
            .check_detection(&help.replace(flag, &format!("{flag}-unrelated")))
            .unwrap_err();
        assert!(error.to_string().contains(flag));
    }
    assert!(OpenCode.check_detection("codex exec --json").is_err());
    assert!(
        OpenCode
            .check_detection(&help.replace("json", "text"))
            .is_err()
    );
}

#[test]
fn json_events_handle_split_utf8_and_preserve_tools_unknown_events_and_diagnostics() {
    let mut output = OpenCode.output();
    let text = serde_json::json!({"type":"text", "part":{"text":"Grüße 🌍"}}).to_string() + "\n";
    let mut decoded = String::new();
    for byte in text.bytes() {
        decoded.push_str(&output.push(Stream::Stdout, &[byte]));
    }
    assert_eq!(decoded, "Grüße 🌍\n");
    let tool = "{\"type\":\"tool_use\",\"part\":{\"tool\":\"bash\",\"state\":{\"status\":\"error\",\"error\":\"denied\"}}}\n";
    assert_eq!(output.push(Stream::Stdout, tool.as_bytes()), tool);
    let unknown = b"{\"type\":\"future_opencode_event\",\"details\":42}\n";
    assert_eq!(
        output.push(Stream::Stdout, unknown),
        String::from_utf8_lossy(unknown)
    );
    assert_eq!(
        output.push(Stream::Stdout, b"permission requested; auto-rejecting\n"),
        "permission requested; auto-rejecting\n"
    );
    output.push(Stream::Stderr, b"provider diagnostic");
    output.push(Stream::Stdout, STOP);
    assert_eq!(output.finish(), "[stderr] provider diagnostic");
    assert!(output.finish().is_empty());
    // A tool error can be recovered by the model. Session errors cannot.
    assert_eq!(output.interpret(status(0)).0, JobStatus::Succeeded);
    for code in [1, 7, 130] {
        assert_eq!(output.interpret(status(code)).0, JobStatus::Failed);
    }
}

#[test]
fn success_requires_a_final_stop_and_no_session_errors_even_when_exit_is_zero() {
    for data in [
        "",
        "{\"type\":\"turn.completed\"}\n",
        "{\"type\":\"text\",\"part\":{\"text\":\"done\"}}\n",
        "{\"type\":\"step_finish\",\"part\":{\"reason\":\"tool-calls\"}}\n",
        "{\"type\":\"step_finish\",\"part\":{\"reason\":\"length\"}}\n",
        "{\"type\":\"step_finish\",\"part\":{\"reason\":\"content-filter\"}}\n",
    ] {
        let mut output = OpenCode.output();
        output.push(Stream::Stdout, data.as_bytes());
        output.finish();
        assert_eq!(output.interpret(status(0)).0, JobStatus::Failed, "{data}");
    }
    for failure in [
        b"{\"type\":\"error\",\"error\":{\"name\":\"ProviderAuthError\"}}\n".as_slice(),
        b"{invalid JSON\n",
    ] {
        for failure_first in [true, false] {
            let mut output = OpenCode.output();
            for data in if failure_first {
                [failure, STOP]
            } else {
                [STOP, failure]
            } {
                output.push(Stream::Stdout, data);
            }
            output.finish();
            assert_eq!(output.interpret(status(0)).0, JobStatus::Failed);
        }
    }
    let mut output = OpenCode.output();
    output.push(Stream::Stdout, STOP);
    output.push(Stream::Stdout, b"{\"type\":\"step_start\"}\n");
    assert_eq!(output.interpret(status(0)).0, JobStatus::Failed);
    output.push(Stream::Stdout, &STOP[..STOP.len() - 1]);
    output.finish();
    assert_eq!(output.interpret(status(0)).0, JobStatus::Succeeded);
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
fn oversized_records_do_not_reassemble_a_fake_success_or_hide_output() {
    let mut output = OpenCode.output();
    let prefix = vec![b'x'; 256 * 1024];
    assert!(
        output
            .push(Stream::Stdout, &prefix)
            .contains("exceeded 256 KiB")
    );
    // This looks valid only if an overlong record is split into new JSON records.
    output.push(Stream::Stdout, STOP);
    output.push(Stream::Stdout, STOP);
    assert_eq!(output.interpret(status(0)).0, JobStatus::Failed);
    let mut stderr = OpenCode.output();
    assert!(
        stderr
            .push(Stream::Stderr, &prefix)
            .starts_with("[stderr] ")
    );
    stderr.push(Stream::Stderr, b"\n");
    stderr.push(Stream::Stdout, STOP);
    assert_eq!(stderr.interpret(status(0)).0, JobStatus::Succeeded);
}

#[test]
fn version_one_state_roundtrips_opencode_drafts_preferences_and_history() {
    for existing in ["codex", "copilot"] {
        let mut state: AppState = serde_json::from_value(serde_json::json!({"version":1, "draft":{"agent":existing,"prompt":"old task","options":{"model":"original"}}})).unwrap();
        let previous = state.draft.clone();
        state.select_agent(AgentId::OpenCode);
        assert!(state.draft.options.is_empty());
        state.draft = task();
        state.draft.options = [
            ("model".into(), "provider/model".into()),
            ("agent".into(), "build".into()),
            ("variant".into(), "high".into()),
            ("permissions".into(), "auto".into()),
        ]
        .into();
        let original = state.draft.clone();
        state.runs.push(codeconvoy::domain::Run {
            provenance: None,
            id: 1,
            created_at: 0,
            task: original.clone(),
            jobs: vec![],
        });
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(dir.path()).unwrap();
        store.save(&state).unwrap();
        let mut state = store.load().unwrap();
        assert_eq!(
            serde_json::to_value(&state.draft).unwrap(),
            serde_json::to_value(&original).unwrap()
        );
        assert_eq!(state.runs[0].task.agent, AgentId::OpenCode);
        state.select_agent(previous.agent);
        assert_eq!(state.draft.options, previous.options);
        state.reuse_task(state.runs[0].task.clone());
        assert_eq!(state.draft.options, original.options);
        // Loading repairs a counter that would otherwise reuse retained ID 1.
        assert_eq!(state.next_run, 2);
        assert_eq!(serde_json::to_value(AgentId::OpenCode).unwrap(), "opencode");
        let persisted = std::fs::read_to_string(dir.path().join("state.json")).unwrap();
        assert!(!persisted.contains("API_KEY"));
    }
}

#[tokio::test]
async fn missing_executable_is_actionable_and_other_backends_still_exist() {
    let dir = tempfile::tempdir().unwrap();
    let versions: &[&str] = if cfg!(windows) { &["1", "2"] } else { &["1"] };
    for version in versions {
        let options = [
            (
                "executable".into(),
                dir.path().join("missing-opencode").to_str().unwrap().into(),
            ),
            ("version".into(), (*version).into()),
        ]
        .into();
        let error = agents::detect(&OpenCode, &options, dir.path())
            .await
            .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("Check the executable path and installation")
        );
    }
    for agent in [AgentId::Codex, AgentId::Copilot] {
        assert!(agents::backend(agent).is_ok());
    }
}

#[tokio::test]
#[cfg(feature = "test-support")]
async fn detects_fixture_help_and_displays_version_without_parsing_a_release_number() {
    let dir = tempfile::tempdir().unwrap();
    let options = [(
        "executable".into(),
        env!("CARGO_BIN_EXE_codeconvoy-test-agent").into(),
    )]
    .into();
    let report = agents::detect(&OpenCode, &options, dir.path())
        .await
        .unwrap();
    assert!(report.starts_with("1.18.34 (CodeConvoy fixture) detected."));
    assert!(report.contains("authentication is used when a job runs"));
}

#[tokio::test]
#[cfg(all(feature = "test-support", unix))]
async fn an_opencode_2_executable_is_refused_on_macos_and_linux_without_running_it() {
    let dir = executable::directory();
    // Probe tracking records every help/version invocation of the fixture.
    std::fs::write(dir.path().join("probe-tracking"), "track").unwrap();
    let link = dir
        .path()
        .join(format!("opencode2{}", std::env::consts::EXE_SUFFIX));
    executable::install(&link);
    let mut options: codeconvoy::domain::AgentOptions = [
        ("executable".into(), link.to_str().unwrap().into()),
        ("version".into(), "2".into()),
    ]
    .into();
    let error = agents::detect(&OpenCode, &options, dir.path())
        .await
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("not supported on macOS and Linux"),
        "{error}"
    );
    assert!(!dir.path().join("opencode2.probes").exists());
    // With OpenCode 1 selected, the help check identifies OpenCode 2.
    options.insert("version".into(), "1".into());
    let error = agents::detect(&OpenCode, &options, dir.path())
        .await
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("offers the OpenCode 2 run interface")
            && error.contains("not supported on macOS and Linux"),
        "{error}"
    );
    assert_eq!(
        std::fs::read_to_string(dir.path().join("opencode2.probes")).unwrap(),
        "help\n"
    );
}

#[tokio::test]
#[cfg(all(feature = "test-support", windows))]
async fn detects_opencode_2_fixture_and_refuses_a_mismatched_version_setting() {
    let dir = executable::directory();
    let link = dir
        .path()
        .join(format!("opencode2{}", std::env::consts::EXE_SUFFIX));
    executable::install(&link);
    let path = link.to_str().unwrap();
    let mut options: codeconvoy::domain::AgentOptions = [
        ("executable".into(), path.into()),
        ("version".into(), "2".into()),
    ]
    .into();
    let report = agents::detect(&OpenCode, &options, dir.path())
        .await
        .unwrap();
    assert!(
        report.starts_with("opencode v2.0.32 (CodeConvoy fixture) detected."),
        "{report}"
    );
    options.insert("version".into(), "1".into());
    let error = agents::detect(&OpenCode, &options, dir.path())
        .await
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("Choose OpenCode 2 under OpenCode version"),
        "{error}"
    );
    // The OpenCode 1 fixture is refused when OpenCode 2 is selected.
    let error = agents::detect(
        &OpenCode,
        &[
            (
                "executable".into(),
                env!("CARGO_BIN_EXE_codeconvoy-test-agent").into(),
            ),
            ("version".into(), "2".into()),
        ]
        .into(),
        dir.path(),
    )
    .await
    .unwrap_err()
    .to_string();
    assert!(
        error.contains("Choose OpenCode 1 under OpenCode version"),
        "{error}"
    );
}

#[tokio::test]
#[ignore = "Requires installed OpenCode; help/version only, no authenticated task"]
async fn installed_opencode_accepts_generated_arguments() {
    let dir = tempfile::tempdir().unwrap();
    // Select the major version whose help contract the installed CLI satisfies.
    let mut version = None;
    for candidate in ["1", "2"] {
        let options = [("version".into(), candidate.into())].into();
        if let Ok(report) = agents::detect(&OpenCode, &options, dir.path()).await {
            println!("OpenCode {candidate}: {report}");
            version = Some(candidate);
        }
    }
    let version = version.expect("installed OpenCode matches neither help contract");
    for mode in ["existing", "auto"] {
        let mut task = task();
        task.options = [
            ("version".into(), version.into()),
            ("model".into(), "provider/model".into()),
            ("agent".into(), "build".into()),
            ("variant".into(), "high".into()),
            ("permissions".into(), mode.into()),
        ]
        .into();
        let mut command = OpenCode
            .build(
                &task,
                &Repository {
                    name: "help only".into(),
                    path: dir.path().into(),
                },
            )
            .unwrap();
        command.args.push("--help".into());
        command.input = None;
        let captured = process::capture(command, 128 * 1024).await.unwrap();
        assert!(
            captured.status.success(),
            "{}",
            String::from_utf8_lossy(&captured.stderr)
        );
        assert!(!captured.truncated);
        OpenCode
            .check_detection_for(
                &task.options,
                &OpenCode.help_text(&captured.stdout, &captured.stderr),
            )
            .unwrap();
    }
}
