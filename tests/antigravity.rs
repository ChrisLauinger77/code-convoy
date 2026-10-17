#![allow(clippy::unwrap_used)]
//! Antigravity discovery/configuration contract. Never invokes agy, signs in or
//! reads credentials; execution is intentionally unavailable in this milestone.
use codeconvoy::{
    agents::{
        self, AgentBackend,
        antigravity::{Antigravity, MINIMUM_VERSION},
    },
    domain::{AgentId, AppState, JobStatus, Repository, TaskConfig},
    process::Stream,
};
use std::{path::Path, process::ExitStatus};

fn task() -> TaskConfig {
    TaskConfig {
        agent: AgentId::Antigravity,
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

#[test]
fn availability_checks_antigravity_identity_and_each_required_flag_without_authentication() {
    let help = include_str!("fixtures/antigravity-help-contract.txt");
    Antigravity.check_detection(help).unwrap();
    for flag in [
        "--print",
        "--output-format",
        "--input-format",
        "--model",
        "--effort",
        "--dangerously-skip-permissions",
        "--sandbox",
    ] {
        let error = Antigravity
            .check_detection(&help.replace(flag, "--unrelated"))
            .unwrap_err();
        assert!(error.to_string().contains(flag), "{error}");
    }
    assert!(
        Antigravity
            .check_detection(&help.replace("Antigravity", "Other"))
            .is_err()
    );
    // Claude Code shares several flag names; neither it nor Cursor is agy.
    for other in [
        include_str!("fixtures/claude-help-contract.txt"),
        include_str!("fixtures/cursor-help-contract.txt"),
    ] {
        assert!(Antigravity.check_detection(other).is_err());
    }
    let options = [("executable".into(), "agy".into())].into();
    let probe = Antigravity.detection(&options, Path::new(".")).unwrap();
    assert_eq!(probe.program, "agy");
    assert_eq!(probe.args, ["--help"]);
    assert!(probe.input.is_none());
    assert_eq!(
        Antigravity
            .version_command(&options, Path::new("."))
            .unwrap()
            .unwrap()
            .args,
        ["--version"]
    );
}

#[test]
fn version_gate_requires_a_readable_supported_release() {
    let (major, minor, patch) = MINIMUM_VERSION;
    for supported in [
        format!("{major}.{minor}.{patch}"),
        format!("agy version {major}.{minor}.{}", patch + 1),
        format!("v{major}.{}.0 (build abc123)", minor + 1),
        format!("{}.0.0", major + 1),
    ] {
        Antigravity.check_version(Some(&supported)).unwrap();
    }
    let minimum = format!("{major}.{minor}.{patch}");
    for unsupported in [
        format!("{major}.{minor}.{}", patch - 1),
        format!("{major}.{}.99", minor - 1),
        "0.9.0".into(),
    ] {
        let error = Antigravity.check_version(Some(&unsupported)).unwrap_err();
        assert!(error.to_string().contains("unsupported"), "{error}");
        assert!(error.to_string().contains(&minimum), "{error}");
    }
    for unreadable in [None, Some(""), Some("Version unavailable"), Some("1.4")] {
        let error = Antigravity.check_version(unreadable).unwrap_err();
        assert!(error.to_string().contains("could not be determined"));
        assert!(error.to_string().contains(&minimum), "{error}");
    }
    // Other backends keep their version output informational.
    for agent in AgentId::ALL {
        if agent != AgentId::Antigravity {
            agents::backend(agent).unwrap().check_version(None).unwrap();
        }
    }
}

#[test]
fn backend_rejects_invalid_options_secrets_and_relative_launchers() {
    let backend = agents::backend(AgentId::Antigravity).unwrap();
    let valid = [
        ("model", "gemini-fixture"),
        ("effort", "high"),
        ("effort", ""),
        ("permissions", "skip"),
        ("sandbox", "enabled"),
        ("timeout_seconds", "86400"),
        ("timeout_seconds", ""),
    ];
    for (key, value) in valid {
        let options = [(key.into(), value.into())].into();
        backend.detection(&options, Path::new(".")).unwrap();
    }
    for (key, value) in [
        ("api_key", "not accepted"),
        ("auth_token", "not accepted"),
        ("conversation", "fixture"),
        ("permissions", "yolo"),
        ("permissions", "always-proceed"),
        ("effort", "max"),
        ("sandbox", "disabled"),
        ("executable", ""),
        ("executable", "./agy"),
        ("executable", "bin/agy"),
        ("model", "bad\nmodel"),
        ("model", "bad\0model"),
        ("timeout_seconds", "0"),
        ("timeout_seconds", "86401"),
        ("timeout_seconds", "+5"),
        ("timeout_seconds", "1.5"),
    ] {
        let options = [(key.into(), value.into())].into();
        assert!(
            backend.detection(&options, Path::new(".")).is_err(),
            "{key}={value}"
        );
    }
    #[cfg(windows)]
    for executable in ["agy.cmd", "agy.bat", "agy.ps1"] {
        assert!(
            backend
                .detection(
                    &[("executable".into(), executable.into())].into(),
                    Path::new(".")
                )
                .is_err()
        );
    }
}

#[test]
fn execution_fails_closed_before_any_process_is_built() {
    let mut task = task();
    task.options.insert("permissions".into(), "skip".into());
    let error = Antigravity.build(&task, &repository()).unwrap_err();
    assert!(error.to_string().contains("not available"), "{error}");
    assert!(error.to_string().contains("No Antigravity process"));
    let mut invalid = task.clone();
    invalid.options.insert("effort".into(), "max".into());
    assert!(
        Antigravity
            .build(&invalid, &repository())
            .unwrap_err()
            .to_string()
            .contains("Unsupported value")
    );
    let mut wrong = task;
    wrong.agent = AgentId::Claude;
    assert!(
        Antigravity
            .build(&wrong, &repository())
            .unwrap_err()
            .to_string()
            .contains("Antigravity task")
    );
    // The placeholder decoder can never classify a run as successful.
    let mut decoder = Antigravity.output();
    decoder.push(
        Stream::Stdout,
        br#"{"event":"result","result":{"status":"SUCCESS"}}"#,
    );
    decoder.finish();
    assert_eq!(decoder.interpret(status(0)).0, JobStatus::Failed);
}

#[test]
fn execution_summary_states_unavailability_and_explicit_choices() {
    let configured = Antigravity.execution_summary(&Default::default());
    assert!(configured.contains("not available in this build"));
    assert!(configured.contains("configured permission rules"));
    assert!(!configured.contains("--dangerously-skip-permissions"));
    assert!(!configured.contains("--sandbox"));
    let explicit = Antigravity.execution_summary(
        &[
            ("permissions".into(), "skip".into()),
            ("sandbox".into(), "enabled".into()),
            ("timeout_seconds".into(), "90".into()),
        ]
        .into(),
    );
    assert!(explicit.contains("--dangerously-skip-permissions"));
    assert!(explicit.contains("--sandbox"));
    assert!(explicit.contains("90 seconds"));
    assert!(
        Antigravity
            .execution_summary(&[("timeout_seconds".into(), "never".into())].into())
            .contains("Invalid CodeConvoy time limit")
    );
}

#[test]
fn registry_preferences_and_persistence_keep_antigravity_independent() {
    assert!(AgentId::ALL.contains(&AgentId::Antigravity));
    assert_eq!(AgentId::Antigravity.label(), "Antigravity CLI");
    assert_eq!(
        agents::backend(AgentId::Antigravity).unwrap().id(),
        AgentId::Antigravity
    );
    assert_eq!(
        serde_json::to_value(AgentId::Antigravity).unwrap(),
        "antigravity"
    );
    assert_eq!(agents::discovery::default_name(AgentId::Antigravity), "agy");
    let mut state = AppState::default();
    state
        .draft
        .options
        .insert("model".into(), "codex-only".into());
    state.select_agent(AgentId::Antigravity);
    assert!(state.draft.options.is_empty());
    state
        .draft
        .options
        .insert("model".into(), "antigravity-only".into());
    state
        .draft
        .options
        .insert("permissions".into(), "skip".into());
    state.select_agent(AgentId::Cursor);
    assert!(state.draft.options.is_empty());
    state.select_agent(AgentId::Codex);
    assert_eq!(state.draft.options["model"], "codex-only");
    state.select_agent(AgentId::Antigravity);
    assert_eq!(state.draft.options["model"], "antigravity-only");
    let temp = tempfile::tempdir().unwrap();
    let store = codeconvoy::persistence::Store::open(temp.path()).unwrap();
    store.save(&state).unwrap();
    let restored = store.load().unwrap();
    assert_eq!(restored.draft.agent, AgentId::Antigravity);
    assert_eq!(restored.draft.options, state.draft.options);
    assert_eq!(restored.agent_options, state.agent_options);
}
