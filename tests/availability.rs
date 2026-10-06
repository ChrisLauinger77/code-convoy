#![cfg(feature = "test-support")]
#![allow(clippy::unwrap_used)]

use codeconvoy::{
    agents::availability::CliChecks,
    domain::{AgentId, AppState},
};
use std::{
    path::{Path, PathBuf},
    sync::mpsc,
    time::Duration,
};

struct Fixture {
    directory: tempfile::TempDir,
    checks: CliChecks,
    completions: mpsc::Receiver<()>,
}
impl Fixture {
    fn new() -> Self {
        let directory = tempfile::tempdir().unwrap();
        std::fs::write(directory.path().join("probe-tracking"), "track").unwrap();
        let (tx, completions) = mpsc::channel();
        let checks = CliChecks::new(
            directory.path().to_owned(),
            tokio::runtime::Handle::current(),
            move || {
                let _ = tx.send(());
            },
        );
        Self {
            directory,
            checks,
            completions,
        }
    }
    fn executable(&self, name: &str) -> String {
        let path = self
            .directory
            .path()
            .join(name)
            .with_extension(std::env::consts::EXE_EXTENSION);
        std::fs::copy(env!("CARGO_BIN_EXE_codeconvoy-test-agent"), &path).unwrap();
        path.to_str().unwrap().into()
    }
    fn marker(&self, name: &str) -> PathBuf {
        self.directory.path().join(name)
    }
    fn mark(&self, name: &str) {
        std::fs::write(self.marker(name), "fixture").unwrap();
    }
    fn probes(&self, name: &str) -> Vec<String> {
        std::fs::read_to_string(self.marker(&format!("{name}.probes")))
            .unwrap_or_default()
            .lines()
            .map(str::to_owned)
            .collect()
    }
    async fn complete(&mut self) {
        tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                self.checks.poll();
                if !self.checks.any_checking() {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .unwrap();
    }
    async fn ready(&self, name: &str) {
        wait_for(|| self.marker(name).exists()).await;
    }
    async fn completion_queued(&self) {
        wait_for(|| self.completions.try_recv().is_ok()).await;
    }
    fn select_backend(&mut self, state: &mut AppState, agent: AgentId) {
        state.select_agent(agent);
        // Exercise the lifecycle request made by the UI selection handler, with
        // real per-backend preference restoration and counted native probes.
        self.checks
            .ensure(agent, state.draft.options.get("executable").cloned());
    }
}
async fn wait_for(condition: impl Fn() -> bool) {
    tokio::time::timeout(Duration::from_secs(10), async {
        while !condition() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn all_backends_validate_at_startup_and_again_after_restart() {
    let mut fixture = Fixture::new();
    let executables: Vec<_> = AgentId::ALL
        .into_iter()
        .enumerate()
        .map(|(index, agent)| (agent, fixture.executable(&format!("agent{index}"))))
        .collect();
    for (agent, executable) in &executables {
        fixture.checks.ensure(*agent, Some(executable.clone()));
        assert!(fixture.checks.checking(*agent));
        assert!(fixture.checks.result(*agent, Some(executable)).is_none());
    }
    fixture.complete().await;
    for (agent, executable) in &executables {
        let info = fixture
            .checks
            .result(*agent, Some(executable))
            .unwrap()
            .as_ref()
            .unwrap();
        assert_eq!(&info.executable, executable);
        assert!(info.detail.contains("CLI compatibility checked"));
        if *agent != AgentId::Codex {
            assert!(info.detail.contains("fixture"));
        }
    }
    // Runtime results are never restored as permanent availability.
    fixture.checks = CliChecks::new(
        fixture.directory.path().to_owned(),
        tokio::runtime::Handle::current(),
        || {},
    );
    for (agent, executable) in &executables {
        fixture.checks.ensure(*agent, Some(executable.clone()));
        assert!(fixture.checks.checking(*agent));
    }
    fixture.complete().await;
    for (index, (agent, _)) in executables.iter().enumerate() {
        assert_eq!(
            fixture.probes(&format!("agent{index}")).len(),
            if *agent == AgentId::Codex { 2 } else { 4 }
        );
    }
    assert!(!fixture.marker("agent-input").exists());
}

#[tokio::test]
async fn backend_switch_checks_unchecked_executables_reuses_results_and_manual_refresh_bypasses_cache()
 {
    let mut fixture = Fixture::new();
    let codex = fixture.executable("codex");
    let copilot = fixture.executable("copilot");
    let mut state = AppState::default();
    state
        .draft
        .options
        .insert("executable".into(), codex.clone());
    state.agent_options.insert(
        AgentId::Copilot,
        [("executable".into(), copilot.clone())].into(),
    );
    fixture.select_backend(&mut state, AgentId::Codex);
    fixture.complete().await;

    assert!(
        fixture
            .checks
            .result(AgentId::Copilot, Some(&copilot))
            .is_none()
    );
    fixture.select_backend(&mut state, AgentId::Copilot);
    assert!(fixture.checks.checking(AgentId::Copilot));
    fixture.complete().await;
    assert!(
        fixture
            .checks
            .result(AgentId::Copilot, Some(&copilot))
            .unwrap()
            .is_ok()
    );
    for agent in [
        AgentId::Codex,
        AgentId::Copilot,
        AgentId::Codex,
        AgentId::Copilot,
    ] {
        fixture.select_backend(&mut state, agent);
        assert!(!fixture.checks.any_checking());
        assert!(
            fixture
                .checks
                .result(
                    agent,
                    state.draft.options.get("executable").map(String::as_str)
                )
                .unwrap()
                .is_ok()
        );
    }
    assert_eq!(fixture.probes("codex"), vec!["help"]);
    assert_eq!(fixture.probes("copilot"), vec!["help", "version"]);

    fixture.checks.recheck(state.draft.agent);
    assert!(fixture.checks.checking(AgentId::Copilot));
    assert!(
        fixture
            .checks
            .result(AgentId::Copilot, Some(&copilot))
            .is_none()
    );
    // Selection during forced refresh reuses its in-flight check.
    fixture.select_backend(&mut state, AgentId::Codex);
    fixture.select_backend(&mut state, AgentId::Copilot);
    fixture.complete().await;
    assert_eq!(
        fixture.probes("copilot"),
        vec!["help", "version", "help", "version"]
    );
    assert_eq!(fixture.probes("codex"), vec!["help"]);
}

#[tokio::test]
async fn backend_switch_checks_a_changed_executable_then_reuses_its_new_result() {
    let mut fixture = Fixture::new();
    let codex = fixture.executable("codex");
    let old = fixture.executable("copilot-old");
    let new = fixture.executable("copilot-new");
    let mut state = AppState::default();
    state
        .draft
        .options
        .insert("executable".into(), codex.clone());
    state.agent_options.insert(
        AgentId::Copilot,
        [("executable".into(), old.clone())].into(),
    );
    fixture.select_backend(&mut state, AgentId::Copilot);
    fixture.complete().await;
    fixture.select_backend(&mut state, AgentId::Codex);
    fixture.complete().await;
    state
        .agent_options
        .get_mut(&AgentId::Copilot)
        .unwrap()
        .insert("executable".into(), new.clone());
    fixture.select_backend(&mut state, AgentId::Copilot);
    assert!(fixture.checks.checking(AgentId::Copilot));
    assert!(
        fixture
            .checks
            .result(AgentId::Copilot, Some(&old))
            .is_none()
    );
    assert!(
        fixture
            .checks
            .result(AgentId::Copilot, Some(&new))
            .is_none()
    );
    fixture.complete().await;
    assert_eq!(
        fixture
            .checks
            .result(AgentId::Copilot, Some(&new))
            .unwrap()
            .as_ref()
            .unwrap()
            .executable,
        new
    );
    fixture.select_backend(&mut state, AgentId::Codex);
    fixture.select_backend(&mut state, AgentId::Copilot);
    assert!(!fixture.checks.any_checking());
    assert_eq!(fixture.probes("copilot-old"), vec!["help", "version"]);
    assert_eq!(fixture.probes("copilot-new"), vec!["help", "version"]);
    assert_eq!(fixture.probes("codex"), vec!["help"]);
}

#[tokio::test]
async fn stale_results_after_backend_switches_cannot_replace_current_backend_or_executable() {
    let mut fixture = Fixture::new();
    let old = fixture.executable("codex-old");
    let new = fixture.executable("codex-new");
    let copilot = fixture.executable("copilot");
    let mut state = AppState::default();
    state.draft.options.insert("executable".into(), old.clone());
    state.agent_options.insert(
        AgentId::Copilot,
        [("executable".into(), copilot.clone())].into(),
    );
    fixture.select_backend(&mut state, AgentId::Codex);
    fixture.completion_queued().await; // Old Codex success is waiting for the UI.
    fixture.mark("copilot.help.block");
    fixture.select_backend(&mut state, AgentId::Copilot);
    fixture.ready("copilot.help.ready").await;
    state
        .agent_options
        .get_mut(&AgentId::Codex)
        .unwrap()
        .insert("executable".into(), new.clone());
    fixture.select_backend(&mut state, AgentId::Codex);
    fixture.select_backend(&mut state, AgentId::Copilot);
    fixture.checks.poll();
    assert_eq!(state.draft.agent, AgentId::Copilot);
    assert!(
        fixture
            .checks
            .result(AgentId::Copilot, Some(&copilot))
            .is_none()
    );
    assert!(fixture.checks.checking(AgentId::Copilot));
    assert!(fixture.checks.result(AgentId::Codex, Some(&old)).is_none());
    assert!(fixture.checks.result(AgentId::Codex, Some(&new)).is_none());
    std::fs::remove_file(fixture.marker("copilot.help.block")).unwrap();
    fixture.complete().await;
    assert_eq!(
        fixture
            .checks
            .result(state.draft.agent, Some(&copilot))
            .unwrap()
            .as_ref()
            .unwrap()
            .executable,
        copilot
    );
    fixture.select_backend(&mut state, AgentId::Codex);
    assert_eq!(
        fixture
            .checks
            .result(state.draft.agent, Some(&new))
            .unwrap()
            .as_ref()
            .unwrap()
            .executable,
        new
    );
    assert!(!fixture.checks.any_checking());
    assert_eq!(fixture.probes("codex-old"), vec!["help"]);
    assert_eq!(fixture.probes("codex-new"), vec!["help"]);
    assert_eq!(fixture.probes("copilot"), vec!["help", "version"]);
}

#[tokio::test]
async fn missing_and_invalid_executables_are_local_failures_and_manual_recheck_recovers() {
    let mut fixture = Fixture::new();
    let good = fixture.executable("good");
    let invalid = fixture.executable("invalid");
    fixture.mark("invalid.invalid");
    let missing = fixture.marker("missing").to_str().unwrap().to_owned();
    fixture.checks.ensure(AgentId::Codex, Some(good.clone()));
    fixture
        .checks
        .ensure(AgentId::Copilot, Some(invalid.clone()));
    fixture
        .checks
        .ensure(AgentId::Claude, Some(missing.clone()));
    fixture
        .checks
        .ensure(AgentId::OpenCode, Some("relative/invalid".into()));
    fixture.complete().await;
    assert!(
        fixture
            .checks
            .result(AgentId::Codex, Some(&good))
            .unwrap()
            .is_ok()
    );
    let error = fixture
        .checks
        .result(AgentId::Copilot, Some(&invalid))
        .unwrap()
        .as_ref()
        .unwrap_err();
    assert!(!error.missing);
    assert!(
        fixture
            .checks
            .result(AgentId::Claude, Some(&missing))
            .unwrap()
            .as_ref()
            .unwrap_err()
            .missing
    );
    assert!(
        !fixture
            .checks
            .result(AgentId::OpenCode, Some("relative/invalid"))
            .unwrap()
            .as_ref()
            .unwrap_err()
            .missing
    );
    assert_eq!(fixture.probes("invalid"), vec!["help"]);
    std::fs::remove_file(fixture.marker("invalid.invalid")).unwrap();
    for _ in 0..20 {
        fixture.checks.recheck(AgentId::Copilot);
    }
    assert!(fixture.checks.checking(AgentId::Copilot));
    assert!(
        fixture
            .checks
            .result(AgentId::Copilot, Some(&invalid))
            .is_none()
    );
    fixture.complete().await;
    assert!(
        fixture
            .checks
            .result(AgentId::Copilot, Some(&invalid))
            .unwrap()
            .is_ok()
    );
    assert_eq!(fixture.probes("invalid"), vec!["help", "help", "version"]);
    assert_eq!(fixture.probes("good"), vec!["help"]);
}

#[tokio::test]
async fn executable_edits_cancel_old_probe_and_coalesce_to_latest_without_duplicates() {
    let mut fixture = Fixture::new();
    let a = fixture.executable("a");
    let b = fixture.executable("b");
    let c = fixture.executable("c");
    fixture.mark("a.help.block");
    fixture.checks.ensure(AgentId::Copilot, Some(a.clone()));
    fixture.ready("a.help.ready").await;
    for _ in 0..20 {
        fixture.checks.ensure(AgentId::Copilot, Some(a.clone()));
        fixture.checks.recheck(AgentId::Copilot);
    }
    assert_eq!(fixture.probes("a"), vec!["help"]);
    fixture.checks.ensure(AgentId::Copilot, Some(b.clone()));
    fixture.checks.ensure(AgentId::Copilot, Some(c.clone()));
    assert!(fixture.checks.result(AgentId::Copilot, Some(&a)).is_none());
    assert!(fixture.checks.checking(AgentId::Copilot));
    fixture.complete().await; // A is still gated: only cancellation can finish it.
    assert!(
        fixture
            .checks
            .result(AgentId::Copilot, Some(&c))
            .unwrap()
            .is_ok()
    );
    assert!(fixture.checks.result(AgentId::Copilot, Some(&b)).is_none());
    assert!(fixture.probes("b").is_empty());
    assert_eq!(fixture.probes("a"), vec!["help"]);
    assert_eq!(fixture.probes("c"), vec!["help", "version"]);
    // An unchanged selection and repeated UI polls never rediscover or respawn.
    for _ in 0..20 {
        fixture.checks.ensure(AgentId::Copilot, Some(c.clone()));
        fixture.checks.poll();
    }
    assert!(!fixture.checks.any_checking());
    assert_eq!(fixture.probes("c"), vec!["help", "version"]);
}

#[tokio::test]
async fn queued_success_cannot_overwrite_a_newer_selection_even_when_switching_back() {
    let mut fixture = Fixture::new();
    let a = fixture.executable("a");
    let b = fixture.executable("b");
    fixture.checks.ensure(AgentId::Codex, Some(a.clone()));
    fixture.completion_queued().await; // Result A reached the mailbox, not the UI.
    fixture.checks.ensure(AgentId::Codex, Some(b.clone()));
    fixture.checks.poll();
    assert!(fixture.checks.result(AgentId::Codex, Some(&b)).is_none());
    assert!(fixture.checks.result(AgentId::Codex, Some(&a)).is_none());
    assert!(fixture.checks.checking(AgentId::Codex));
    fixture.checks.ensure(AgentId::Codex, Some(a.clone()));
    fixture.complete().await;
    assert!(
        fixture
            .checks
            .result(AgentId::Codex, Some(&a))
            .unwrap()
            .is_ok()
    );
    assert_eq!(fixture.probes("a"), vec!["help", "help"]);
}

#[tokio::test]
async fn stop_cancels_gated_checks_and_refuses_new_checks() {
    let mut fixture = Fixture::new();
    let a = fixture.executable("a");
    fixture.mark("a.version.block");
    fixture.checks.ensure(AgentId::Claude, Some(a.clone()));
    fixture.ready("a.version.ready").await;
    fixture.checks.stop();
    fixture.checks.recheck(AgentId::Claude);
    fixture.checks.ensure(AgentId::Copilot, Some(a.clone()));
    fixture.completion_queued().await; // The gated process was cancelled and reaped.
    fixture.checks.poll();
    assert!(!fixture.checks.any_checking());
    assert!(fixture.checks.result(AgentId::Claude, Some(&a)).is_none());
    assert_eq!(fixture.probes("a"), vec!["help", "version"]);
}

#[tokio::test]
async fn explicit_missing_selection_never_falls_back_to_discovery() {
    let mut fixture = Fixture::new();
    // Existing discovery offers this other candidate, but a configured path wins.
    let other = fixture.executable("codex");
    assert!(Path::new(&other).exists());
    let selected = fixture
        .marker("manual-missing-codex")
        .to_str()
        .unwrap()
        .to_owned();
    fixture
        .checks
        .ensure(AgentId::Codex, Some(selected.clone()));
    fixture.complete().await;
    assert!(
        fixture
            .checks
            .result(AgentId::Codex, Some(&selected))
            .unwrap()
            .as_ref()
            .unwrap_err()
            .missing
    );
    assert!(fixture.probes("codex").is_empty());
}

#[tokio::test]
async fn launch_preflight_and_startup_do_not_spawn_simultaneous_backend_probes() {
    let mut fixture = Fixture::new();
    let executable = fixture.executable("copilot");
    fixture.mark("copilot.help.block");
    fixture
        .checks
        .ensure(AgentId::Copilot, Some(executable.clone()));
    fixture.ready("copilot.help.ready").await;
    let directory = fixture.directory.path().to_owned();
    let selected = executable.clone();
    let preflight = tokio::spawn(async move {
        let backend = codeconvoy::agents::backend(AgentId::Copilot).unwrap();
        // This is the same fresh help/version check used by runner::prepare.
        codeconvoy::agents::detect(
            backend.as_ref(),
            &[("executable".into(), selected)].into(),
            &directory,
        )
        .await
    });
    std::fs::remove_file(fixture.marker("copilot.help.block")).unwrap();
    fixture.complete().await;
    tokio::time::timeout(Duration::from_secs(10), preflight)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert!(!fixture.marker("copilot.overlap").exists());
    assert_eq!(
        fixture.probes("copilot"),
        vec!["help", "version", "help", "version"]
    );
}

#[test]
fn automatic_discovery_and_exact_manual_names_use_gui_safe_locations() {
    const ROOT: &str = "CODECONVOY_TEST_DISCOVERY_ROOT";
    let Ok(root) = std::env::var(ROOT) else {
        let directory = tempfile::tempdir().unwrap();
        for name in ["codex", "chosen-codex"] {
            let path = directory
                .path()
                .join(name)
                .with_extension(std::env::consts::EXE_EXTENSION);
            std::fs::copy(env!("CARGO_BIN_EXE_codeconvoy-test-agent"), path).unwrap();
        }
        std::fs::write(directory.path().join("probe-tracking"), "track").unwrap();
        // A separate test process keeps PATH changes isolated from parallel tests.
        // An empty GUI PATH forces the normal common-location fallback.
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "automatic_discovery_and_exact_manual_names_use_gui_safe_locations",
                "--nocapture",
            ])
            .env(ROOT, directory.path())
            .env("PATH", "")
            .env("XDG_BIN_DIR", directory.path())
            .status()
            .unwrap();
        assert!(status.success());
        return;
    };
    let root = PathBuf::from(root);
    let default = root
        .join("codex")
        .with_extension(std::env::consts::EXE_EXTENSION)
        .to_str()
        .unwrap()
        .to_owned();
    let chosen = root
        .join("chosen-codex")
        .with_extension(std::env::consts::EXE_EXTENSION)
        .to_str()
        .unwrap()
        .to_owned();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        let (tx, rx) = mpsc::channel();
        let mut checks =
            CliChecks::new(root.clone(), tokio::runtime::Handle::current(), move || {
                let _ = tx.send(());
            });
        checks.ensure(AgentId::Codex, None);
        assert!(checks.checking(AgentId::Codex));
        wait_for(|| rx.try_recv().is_ok()).await;
        let resolved = checks.poll();
        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].path, default);
        assert!(resolved[0].configured.is_none());
        assert!(
            checks
                .result(AgentId::Codex, Some(&default))
                .unwrap()
                .is_ok()
        );
        // Adopting the discovered setting does not start a duplicate probe.
        checks.ensure(AgentId::Codex, Some(default.clone()));
        assert!(!checks.checking(AgentId::Codex));

        // An explicitly named launcher is resolved exactly, even with no shell PATH.
        checks.ensure(AgentId::Codex, Some("chosen-codex".into()));
        wait_for(|| rx.try_recv().is_ok()).await;
        let resolved = checks.poll();
        assert_eq!(resolved[0].configured.as_deref(), Some("chosen-codex"));
        assert_eq!(resolved[0].path, chosen);
        assert!(
            checks
                .result(AgentId::Codex, Some(&chosen))
                .unwrap()
                .is_ok()
        );
        checks.ensure(AgentId::Codex, Some(chosen.clone()));
        assert!(!checks.checking(AgentId::Codex));

        // A queued automatic discovery result cannot replace a later manual path.
        checks.ensure(AgentId::Codex, None);
        wait_for(|| rx.try_recv().is_ok()).await;
        let missing = root
            .join("manually-selected-missing")
            .to_str()
            .unwrap()
            .to_owned();
        checks.ensure(AgentId::Codex, Some(missing.clone()));
        assert!(checks.poll().is_empty());
        assert!(checks.result(AgentId::Codex, Some(&default)).is_none());
        wait_for(|| rx.try_recv().is_ok()).await;
        assert!(checks.poll().is_empty());
        assert!(
            checks
                .result(AgentId::Codex, Some(&missing))
                .unwrap()
                .as_ref()
                .unwrap_err()
                .missing
        );
    });
}
