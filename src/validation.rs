//! Explicit local validation, separate from agent outcomes and scheduling.
pub mod configuration;
pub mod presets;
use crate::{
    domain::{self, ExecutionMode, Job},
    process::{self, Cancellation, CommandSpec},
    runner::LifecycleClient,
};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::{Mutex, atomic::Ordering},
    time::Instant,
};
use tokio::{runtime::Handle, sync::watch, task::JoinHandle};

pub const OUTPUT_LIMIT: usize = 64 * 1024;
const MAX_ACTIVE: usize = 4;
pub type Key = (u64, usize);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Command {
    pub executable: String,
    pub arguments: Vec<String>,
}
impl Command {
    pub fn validate(&self) -> Result<()> {
        anyhow::ensure!(
            !self.executable.trim().is_empty()
                && self.executable.len() <= 4096
                && !self.executable.chars().any(char::is_control),
            "Enter an executable name or absolute path (at most 4096 bytes)."
        );
        let path = Path::new(&self.executable);
        anyhow::ensure!(
            path.is_absolute()
                || (!self.executable.contains('/') && !self.executable.contains('\\')),
            "Use an executable name on PATH or an absolute executable path."
        );
        anyhow::ensure!(
            self.arguments.len() <= 128
                && self.arguments.iter().map(String::len).sum::<usize>() <= 32 * 1024
                && self.arguments.iter().all(|a| !a.contains('\0')),
            "Use at most 128 arguments / 32 KiB without NUL characters."
        );
        #[cfg(windows)]
        anyhow::ensure!(
            !path
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("cmd") || e.eq_ignore_ascii_case("bat")),
            "Batch launchers require shell interpretation. Configure a native executable instead."
        );
        Ok(())
    }
    pub fn label(&self) -> String {
        std::iter::once(&self.executable)
            .chain(&self.arguments)
            .map(|s| serde_json::to_string(s).unwrap_or_default())
            .collect::<Vec<_>>()
            .join(" ")
    }
    pub fn preview(&self) -> String {
        // JSON quoting is display-only; no parser or shell interprets this text.
        std::iter::once(&self.executable)
            .chain(&self.arguments)
            .map(|s| {
                if !s.is_empty()
                    && s.bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b"-_.:/".contains(&b))
                {
                    s.clone()
                } else {
                    serde_json::to_string(s).unwrap_or_default()
                }
            })
            .collect::<Vec<_>>()
            .join(" ")
    }
    fn spec(&self, directory: &Path) -> CommandSpec {
        let mut spec = CommandSpec::new(&self.executable, directory);
        spec.args = self.arguments.iter().map(Into::into).collect();
        // Preserve toolchain/auth environment, while keeping Git and PWD rooted here.
        spec.remove_env = std::env::vars_os()
            .filter_map(|(k, _)| k.into_string().ok())
            .filter(|k| repository_environment_override(k, cfg!(windows)))
            .collect();
        spec
    }
}

fn repository_environment_override(key: &str, windows: bool) -> bool {
    // Windows environment lookup ignores ASCII case. Keep the original key
    // spelling in remove_env so every inherited override is removed explicitly.
    if windows {
        key.eq_ignore_ascii_case("PWD")
            || key
                .get(..4)
                .is_some_and(|prefix| prefix.eq_ignore_ascii_case("GIT_"))
    } else {
        key == "PWD" || key.starts_with("GIT_")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Status {
    Running,
    Passed,
    Failed,
    Cancelled,
    Unavailable,
}
impl Status {
    pub fn label(self) -> &'static str {
        match self {
            Self::Running => "Running",
            Self::Passed => "Passed",
            Self::Failed => "Failed",
            Self::Cancelled => "Cancelled",
            Self::Unavailable => "Unavailable",
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Record {
    pub command: Command,
    pub directory: PathBuf,
    pub status: Status,
    pub started_at: u64,
    pub finished_at: Option<u64>,
    pub duration_ms: u64,
    pub exit_code: Option<i32>,
    // CLI output and diagnostics can contain secrets. Ignore legacy saved values too.
    #[serde(skip)]
    pub output: String,
    #[serde(skip)]
    pub truncated: bool,
    #[serde(skip)]
    pub detail: String,
}
impl Record {
    pub fn pending(command: Command, job: &Job) -> Self {
        Self {
            command,
            directory: directory(job).unwrap_or(Path::new("")).to_owned(),
            status: Status::Running,
            started_at: domain::now(),
            finished_at: None,
            duration_ms: 0,
            exit_code: None,
            output: String::new(),
            truncated: false,
            detail: String::new(),
        }
    }
    fn append(&mut self, text: &str) {
        self.output.push_str(text);
        if self.output.len() > OUTPUT_LIMIT {
            let mut cut = self.output.len() - OUTPUT_LIMIT;
            while !self.output.is_char_boundary(cut) {
                cut += 1;
            }
            self.output.drain(..cut);
            self.truncated = true;
        }
    }
    pub fn recover_interrupted(&mut self) {
        if self.status == Status::Running {
            self.status = Status::Cancelled;
            self.detail = "Interrupted by application exit; execution was not resumed. Duration is the last saved observation. Inspect files before running again.".into();
        }
    }
}

pub fn directory(job: &Job) -> Result<&Path> {
    match job.execution_mode {
        ExecutionMode::Direct => {
            anyhow::ensure!(
                job.worktree.is_none(),
                "Inconsistent original working directory metadata."
            );
            Ok(&job.repository.path)
        }
        ExecutionMode::IsolatedWorktree => job
            .worktree
            .as_ref()
            .map(|m| m.path.as_path())
            .context("Original isolated worktree was not recorded; validation is unavailable."),
    }
}
pub(crate) fn ensure_eligible(job: &Job) -> Result<()> {
    use domain::ResultResolution;
    anyhow::ensure!(
        job.status.is_terminal(),
        "Wait for the agent job to finish before validation."
    );
    match job.resolution {
        ResultResolution::Unresolved => Ok(()),
        ResultResolution::Applied => anyhow::bail!(
            "Validation is unavailable for Applied results; the retained Diff must stay unchanged until cleanup."
        ),
        ResultResolution::ApplyPending
        | ResultResolution::DiscardPending
        | ResultResolution::Discarded => anyhow::bail!(
            "Result is discarded or an operation is pending; validation is unavailable."
        ),
    }
}
pub fn label(job: &Job, configured: bool) -> &'static str {
    job.validation.as_ref().map_or(
        if configured {
            "Not run"
        } else {
            "Not configured"
        },
        |v| v.status.label(),
    )
}

/// Check saved in-flight intents before a result transaction changes metadata.
/// The manager still revalidates identity and acquires the authoritative lease.
pub(crate) fn blocks_result_operation(state: &domain::AppState, job: &Job) -> bool {
    fn common_dir(job: &Job) -> Option<&Path> {
        match job.execution_mode {
            ExecutionMode::Direct => job.before.as_ref()?.common_dir.as_deref(),
            ExecutionMode::IsolatedWorktree => Some(&job.worktree.as_ref()?.common_dir),
        }
    }
    fn paths(job: &Job) -> impl Iterator<Item = &PathBuf> {
        std::iter::once(&job.repository.path).chain(job.worktree.iter().map(|m| &m.path))
    }
    state.runs.iter().flat_map(|run| &run.jobs).any(|active| {
        active
            .validation
            .as_ref()
            .is_some_and(|v| v.status == Status::Running)
            && ((common_dir(job).is_some() && common_dir(job) == common_dir(active))
                || paths(job).any(|a| paths(active).any(|b| a.starts_with(b) || b.starts_with(a))))
    })
}

struct Worker {
    started: Instant,
    cancellation: Cancellation,
    updates: watch::Receiver<Record>,
    join: JoinHandle<()>,
}
/// A bounded set of user-started operations, with no queue or automatic retries.
#[derive(Default)]
pub struct Service {
    workers: BTreeMap<Key, Worker>,
    closing: bool,
}
impl Service {
    pub fn start(
        &mut self,
        runtime: &Handle,
        lifecycle: LifecycleClient,
        key: Key,
        job: Job,
        command: Command,
    ) -> Result<Record> {
        anyhow::ensure!(!self.closing, "Validation is shutting down.");
        anyhow::ensure!(
            !self.workers.contains_key(&key),
            "Validation is already running for this result."
        );
        anyhow::ensure!(
            self.workers.len() < MAX_ACTIVE,
            "At most four validations may run at once."
        );
        ensure_eligible(&job)?;
        command.validate()?;
        let record = Record::pending(command, &job);
        let (tx, updates) = watch::channel(record.clone());
        let cancellation = Cancellation::default();
        let cancel = cancellation.clone();
        let join = runtime.spawn(async move {
            execute(lifecycle, key, job, tx, cancel).await;
        });
        self.workers.insert(
            key,
            Worker {
                started: Instant::now(),
                cancellation,
                updates,
                join,
            },
        );
        Ok(record)
    }
    pub fn cancel(&self, key: Key) {
        if let Some(w) = self.workers.get(&key) {
            w.cancellation.cancel();
        }
    }
    pub fn is_idle(&self) -> bool {
        self.workers.is_empty()
    }
    pub fn shutdown(&mut self) {
        self.closing = true;
        for w in self.workers.values() {
            w.cancellation.cancel();
        }
    }
    pub fn finished(&self) -> bool {
        self.workers.values().all(|w| w.join.is_finished())
    }
    pub fn poll(&mut self) -> Vec<(Key, Record)> {
        let mut updates = Vec::new();
        self.workers.retain(|key, worker| {
            let finished = worker.join.is_finished();
            if finished || worker.updates.has_changed().unwrap_or(false) {
                let mut record = worker.updates.borrow_and_update().clone();
                if record.status == Status::Running { record.duration_ms = worker.started.elapsed().as_millis().min(u64::MAX as u128) as u64; }
                if finished && record.status == Status::Running {
                    record.status = Status::Unavailable;
                    record.detail = "Validation worker stopped unexpectedly. Inspect remaining processes and files before restarting.".into();
                }
                updates.push((*key, record));
            }
            !finished
        });
        updates
    }
}
impl Drop for Service {
    fn drop(&mut self) {
        self.shutdown();
    }
}

async fn execute(
    lifecycle: LifecycleClient,
    key: Key,
    job: Job,
    tx: watch::Sender<Record>,
    cancellation: Cancellation,
) {
    let start = Instant::now();
    // Do not drop lease acquisition/verification on cancellation: its Git children
    // must finish cleanup before access is released.
    let acquired = lifecycle
        .validation_permit(key.0, key.1, &job, &cancellation)
        .await;
    let outcome = match acquired {
        Err(error) => (
            if cancellation.is_cancelled() {
                Status::Cancelled
            } else {
                Status::Unavailable
            },
            None,
            format!("{error:#}"),
        ),
        Ok(permit) => {
            let command = tx.borrow().command.clone();
            let path = tx.borrow().directory.clone();
            if cancellation.is_cancelled() || permit.cancellation.is_cancelled() {
                (
                    Status::Cancelled,
                    None,
                    "Cancelled before execution.".into(),
                )
            } else {
                match process::spawn(&command.spec(&path)) {
                    Err(error) => (
                        if cancellation.is_cancelled() {
                            Status::Cancelled
                        } else {
                            Status::Unavailable
                        },
                        None,
                        format!("{error:#}"),
                    ),
                    Ok(child) => {
                        permit.safe.store(false, Ordering::Release);
                        let decoder = Mutex::new(crate::runner::raw_output::RawOutput::default());
                        let execution = process::execute(
                            child,
                            None,
                            &cancellation,
                            process::cancel,
                            |stream, bytes| {
                                if let Ok(mut decoder) = decoder.lock() {
                                    let text = decoder.push(stream, bytes);
                                    tx.send_modify(|record| record.append(&text));
                                }
                            },
                        );
                        tokio::pin!(execution);
                        let result = tokio::select! {
                            biased;
                            _ = permit.cancellation.cancelled() => { cancellation.cancel(); execution.await }
                            result = &mut execution => result,
                        };
                        if let Ok(mut decoder) = decoder.lock() {
                            tx.send_modify(|r| r.append(&decoder.finish()));
                        }
                        match result {
                            Ok(result) => {
                                permit.safe.store(true, Ordering::Release);
                                let status = if result.cancelled {
                                    Status::Cancelled
                                } else if result.status.is_some_and(|s| s.success()) {
                                    Status::Passed
                                } else {
                                    Status::Failed
                                };
                                (status, result.status.and_then(|s| s.code()), String::new())
                            }
                            Err(error) => (
                                Status::Unavailable,
                                None,
                                format!(
                                    "{error:#} Repository access remains blocked because cleanup is unconfirmed; inspect processes before restarting."
                                ),
                            ),
                        }
                    }
                }
            }
        }
    };
    tx.send_modify(|record| {
        record.status = outcome.0;
        record.exit_code = outcome.1;
        record.detail = outcome.2;
        record.finished_at = Some(domain::now());
        record.duration_ms = start.elapsed().as_millis().min(u64::MAX as u128) as u64;
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn repository_overrides_follow_platform_environment_case_rules() {
        for windows in [false, true] {
            for key in [
                "PWD",
                "GIT_DIR",
                "GIT_WORK_TREE",
                "GIT_INDEX_FILE",
                "GIT_CONFIG_COUNT",
            ] {
                assert!(repository_environment_override(key, windows), "{key}");
            }
            for key in [
                "pwd",
                "pWd",
                "git_dir",
                "Git_Work_Tree",
                "gIt_InDeX_fIlE",
                "git_config_count",
            ] {
                assert_eq!(
                    repository_environment_override(key, windows),
                    windows,
                    "{key}"
                );
            }
            for key in [
                "",
                "P",
                "GIT",
                "GITHUB_TOKEN",
                "PATH",
                "Path",
                "PWDX",
                "XGIT_DIR",
                "日本語",
            ] {
                assert!(!repository_environment_override(key, windows), "{key}");
            }
        }
    }
    #[test]
    fn command_spec_removes_repository_overrides_with_original_key_spelling() {
        const CHILD: &str = "CODECONVOY_VALIDATION_ENV_TEST";
        if std::env::var_os(CHILD).is_none() {
            // Use a fresh process: mutating this test runner's environment would
            // race other tests and requires unsafe set_var on multithreaded Rust.
            let status = std::process::Command::new(std::env::current_exe().expect("test executable"))
                .args(["--exact", "validation::tests::command_spec_removes_repository_overrides_with_original_key_spelling"])
                .env(CHILD, "1")
                .env("GIT_INDEX_FILE", "synthetic index")
                .env("Git_Dir", "synthetic git directory")
                .env("pWd", "synthetic working directory")
                .env("CODECONVOY_UNRELATED_ENV_TEST", "preserve")
                .status().expect("start isolated environment test");
            assert!(status.success());
            return;
        }
        let spec = Command {
            executable: "cargo".into(),
            arguments: vec!["test".into()],
        }
        .spec(Path::new("."));
        for (injected, removed) in [
            ("GIT_INDEX_FILE", true),
            ("Git_Dir", cfg!(windows)),
            ("pWd", cfg!(windows)),
        ] {
            // Windows can retain an inherited key's spelling when its value is
            // replaced by Command::env; assert against the actual child key.
            let key = std::env::vars_os()
                .filter_map(|(key, _)| key.into_string().ok())
                .find(|key| {
                    if cfg!(windows) {
                        key.eq_ignore_ascii_case(injected)
                    } else {
                        key == injected
                    }
                })
                .expect("injected environment key");
            assert_eq!(spec.remove_env.contains(&key), removed, "{key}");
        }
        assert!(
            !spec
                .remove_env
                .iter()
                .any(|k| k == "CODECONVOY_UNRELATED_ENV_TEST")
        );
    }
    #[test]
    fn commands_reject_ambiguous_paths_nuls_and_excessive_inputs() {
        let command = |executable: &str, arguments: Vec<String>| Command {
            executable: executable.into(),
            arguments,
        };
        for name in ["", "  ", "tool\n", "./tool", "dir/tool", "dir\\tool"] {
            assert!(command(name, vec![]).validate().is_err());
        }
        assert!(command("cargo", vec!["x\0y".into()]).validate().is_err());
        assert!(command("cargo", vec!["x".into(); 129]).validate().is_err());
        assert!(
            command("cargo", vec!["x".repeat(32769)])
                .validate()
                .is_err()
        );
        assert!(
            command("cargo", vec!["".into(), "a b".into(), "日本語".into()])
                .validate()
                .is_ok()
        );
    }
}
