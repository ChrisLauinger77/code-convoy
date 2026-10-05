use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

pub const MAX_HISTORY: usize = 30;
pub const DEFAULT_GLOBAL_CONCURRENCY: usize = 4;
pub const MAX_CONCURRENCY: usize = 16;
pub const MAX_REPOSITORIES: usize = 128;
pub const LOG_LIMIT: usize = 512 * 1024;
pub const TOTAL_LOG_LIMIT: usize = 32 * 1024 * 1024;
pub type AgentOptions = BTreeMap<String, String>;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentId {
    #[default]
    Codex,
    Claude,
    Copilot,
    #[serde(rename = "opencode")]
    OpenCode,
}
impl AgentId {
    pub const ALL: [Self; 4] = [Self::Codex, Self::Copilot, Self::OpenCode, Self::Claude];
    pub fn label(self) -> &'static str {
        match self {
            Self::Codex => "OpenAI Codex CLI",
            Self::Claude => "Claude Code",
            Self::Copilot => "GitHub Copilot CLI",
            Self::OpenCode => "OpenCode",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Repository {
    pub path: PathBuf,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TaskConfig {
    pub prompt: String,
    pub agent: AgentId,
    pub options: AgentOptions,
    pub concurrency: usize,
}
impl Default for TaskConfig {
    fn default() -> Self {
        Self {
            prompt: String::new(),
            agent: AgentId::Codex,
            options: AgentOptions::new(),
            concurrency: 2,
        }
    }
}
impl TaskConfig {
    pub fn validate(&self, repositories: &[Repository]) -> anyhow::Result<()> {
        anyhow::ensure!(
            !self.prompt.trim().is_empty(),
            "Enter a task before running."
        );
        anyhow::ensure!(
            self.prompt.len() <= 128 * 1024,
            "Task must be smaller than 128 KiB."
        );
        anyhow::ensure!(
            (1..=16).contains(&self.concurrency),
            "Concurrency must be between 1 and 16."
        );
        anyhow::ensure!(!repositories.is_empty(), "Select at least one repository.");
        anyhow::ensure!(
            repositories.len() <= MAX_REPOSITORIES,
            "Select at most {MAX_REPOSITORIES} repositories."
        );
        for (i, repo) in repositories.iter().enumerate() {
            anyhow::ensure!(
                !repositories[..i].iter().any(|r| r.path == repo.path),
                "Repository selected twice: {}",
                repo.path.display()
            );
            anyhow::ensure!(
                !repositories[..i]
                    .iter()
                    .any(|r| r.path.starts_with(&repo.path) || repo.path.starts_with(&r.path)),
                "Selected repositories overlap: {}. Run nested repositories separately.",
                repo.path.display()
            );
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobStatus {
    Queued,
    Running,
    Succeeded,
    Failed,
    Cancelled,
}
impl JobStatus {
    pub fn is_terminal(self) -> bool {
        !matches!(self, Self::Queued | Self::Running)
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Queued => "Queued",
            Self::Running => "Running",
            Self::Succeeded => "Succeeded",
            Self::Failed => "Failed",
            Self::Cancelled => "Cancelled",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueueReason {
    ConvoyLimit,
    GlobalLimit,
    Repository(u64),
    CheckingRepository,
    CleanupFailed(u64),
}
impl QueueReason {
    pub fn label(self) -> String {
        match self {
            Self::ConvoyLimit => "Waiting for this convoy's concurrency slot.".into(),
            Self::GlobalLimit => "Waiting for a global job slot.".into(),
            Self::Repository(run) => {
                format!("Waiting for repository access held by convoy #{run}.")
            }
            Self::CheckingRepository => "Checking Git state before execution…".into(),
            Self::CleanupFailed(run) => format!(
                "Repository blocked: process cleanup was not confirmed in convoy #{run}. Inspect processes before restarting CodeConvoy."
            ),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Job {
    pub repository: Repository,
    pub status: JobStatus,
    pub started_at: Option<u64>,
    pub finished_at: Option<u64>,
    pub exit_code: Option<i32>,
    pub before: Option<GitSummary>,
    pub interrupted: bool,
    // Untrusted CLI diagnostics and source code never enter application persistence.
    #[serde(skip)]
    pub detail: String,
    #[serde(skip)]
    pub log: LogBuffer,
    #[serde(skip)]
    pub queue_reason: Option<QueueReason>,
}
impl Job {
    pub fn queued(repository: Repository) -> Self {
        Self {
            repository,
            status: JobStatus::Queued,
            started_at: None,
            finished_at: None,
            exit_code: None,
            before: None,
            interrupted: false,
            detail: String::new(),
            log: LogBuffer::default(),
            queue_reason: None,
        }
    }
    pub fn finish(&mut self, status: JobStatus, exit_code: Option<i32>, detail: String) {
        if !self.status.is_terminal() {
            self.status = status;
            self.queue_reason = None;
            self.exit_code = exit_code;
            self.detail = detail;
            self.finished_at = Some(now());
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitSummary {
    pub branch: String,
    pub head: Option<String>,
    pub changed: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Run {
    pub id: u64,
    pub created_at: u64,
    pub task: TaskConfig,
    pub jobs: Vec<Job>,
}
impl Run {
    pub fn status(&self) -> JobStatus {
        if self.jobs.iter().any(|j| j.status == JobStatus::Running) {
            JobStatus::Running
        } else if self.jobs.iter().any(|j| j.status == JobStatus::Queued) {
            JobStatus::Queued
        } else if self.jobs.iter().any(|j| j.status == JobStatus::Failed) {
            JobStatus::Failed
        } else if self.jobs.iter().any(|j| j.status == JobStatus::Cancelled) || self.jobs.is_empty()
        {
            JobStatus::Cancelled
        } else {
            JobStatus::Succeeded
        }
    }
    pub fn completed_jobs(&self) -> usize {
        self.jobs.iter().filter(|j| j.status.is_terminal()).count()
    }
    pub fn elapsed(&self, current_time: u64) -> u64 {
        let end = if self.active() {
            current_time
        } else {
            self.jobs
                .iter()
                .filter_map(|j| j.finished_at)
                .max()
                .unwrap_or(self.created_at)
        };
        end.saturating_sub(self.created_at)
    }
    pub fn active(&self) -> bool {
        self.jobs.iter().any(|j| !j.status.is_terminal())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppState {
    pub version: u32,
    pub next_run: u64,
    pub repositories: Vec<Repository>,
    pub draft: TaskConfig,
    pub agent_options: BTreeMap<AgentId, AgentOptions>,
    pub runs: Vec<Run>,
    pub global_concurrency: usize,
}
impl Default for AppState {
    fn default() -> Self {
        Self {
            version: 1,
            next_run: 1,
            repositories: Vec::new(),
            draft: TaskConfig::default(),
            agent_options: BTreeMap::new(),
            runs: Vec::new(),
            global_concurrency: DEFAULT_GLOBAL_CONCURRENCY,
        }
    }
}
impl AppState {
    /// Keep backend preferences separate without changing old draft/run schemas.
    pub fn select_agent(&mut self, agent: AgentId) {
        if agent != self.draft.agent {
            self.agent_options
                .insert(self.draft.agent, self.draft.options.clone());
            self.draft.agent = agent;
            self.draft.options = self.agent_options.get(&agent).cloned().unwrap_or_default();
        }
    }
    pub fn reuse_task(&mut self, task: TaskConfig) {
        self.agent_options
            .insert(self.draft.agent, self.draft.options.clone());
        self.draft = task;
    }
    /// History cleanup only touches local run metadata, never execution or Git.
    pub fn remove_from_history(&mut self, id: u64) -> bool {
        let before = self.runs.len();
        self.runs.retain(|run| run.id != id || run.active());
        self.runs.len() != before
    }
    pub fn clear_history(&mut self) -> usize {
        let before = self.runs.len();
        self.runs.retain(Run::active);
        before - self.runs.len()
    }
    /// Never evict an active convoy, even when it is older than the history cap.
    pub fn trim_history(&mut self) {
        let mut completed = 0;
        self.runs.retain(|run| {
            if run.active() {
                true
            } else {
                completed += 1;
                completed <= MAX_HISTORY
            }
        });
    }
    pub fn trim_logs(&mut self) {
        let mut total: usize = self
            .runs
            .iter()
            .flat_map(|r| &r.jobs)
            .map(|j| j.log.text.len())
            .sum();
        for job in self.runs.iter_mut().rev().flat_map(|r| &mut r.jobs) {
            if total <= TOTAL_LOG_LIMIT {
                break;
            }
            total -= job.log.text.len();
            if !job.log.text.is_empty() {
                job.log.text.clear();
                job.log.truncated = true;
            }
        }
    }
    pub fn recover_interrupted(&mut self) {
        for job in self.runs.iter_mut().flat_map(|r| &mut r.jobs) {
            if !job.status.is_terminal() {
                job.finish(
                    JobStatus::Cancelled,
                    None,
                    "Interrupted by application exit; inspect the working tree before retrying."
                        .into(),
                );
                job.interrupted = true;
            }
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct LogBuffer {
    pub text: String,
    pub truncated: bool,
}
impl LogBuffer {
    pub fn append(&mut self, text: &str) {
        self.text.push_str(text);
        if self.text.len() > LOG_LIMIT {
            let mut cut = self.text.len() - LOG_LIMIT;
            while !self.text.is_char_boundary(cut) {
                cut += 1;
            }
            self.text.drain(..cut);
            self.truncated = true;
        }
    }
}
pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
