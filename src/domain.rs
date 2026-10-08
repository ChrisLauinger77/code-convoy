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

/// Selection conveniences reference the canonical identities of registrations.
/// References remain when a registration is removed, so repair is explicit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepositoryGroup {
    pub name: String,
    pub repositories: Vec<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskTemplate {
    pub name: String,
    pub prompt: String,
}

/// Execution context is part of the immutable task snapshot, never a backend option.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionMode {
    #[default]
    Direct,
    IsolatedWorktree,
}
impl ExecutionMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Direct => "Current working tree",
            Self::IsolatedWorktree => "Isolated worktree",
        }
    }
}

/// Also written outside the checkout before Git is allowed to create it.
/// A path prefix alone is never evidence of ownership.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorktreeMetadata {
    pub owner: String,
    pub run: u64,
    pub job: usize,
    pub repository: Repository,
    pub common_dir: PathBuf,
    pub path: PathBuf,
    pub base_commit: String,
    pub execution_mode: ExecutionMode,
}

/// Availability is separate from agent status and last observed Git changes.
/// Persisted values must be reconciled before they are trusted in this session.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResultAvailability {
    #[default]
    Unchecked,
    Available,
    Missing,
    Stale,
    Invalid,
    CleanupPending,
    CleanupFailed,
    Cleaned,
}

/// Human resolution stays independent of physical availability and agent status.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResultResolution {
    #[default]
    Unresolved,
    ApplyPending,
    Applied,
    DiscardPending,
    Discarded,
}

/// Last observation, independent of the agent's success/failure status.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct WorktreeResult {
    /// Saved observations are never evidence of recovery validation.
    #[serde(skip)]
    pub observed_this_session: bool,
    pub exists: bool,
    /// None means unavailable/partial; never present that as unchanged.
    pub changed: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TaskConfig {
    pub prompt: String,
    pub attachments: Vec<crate::attachments::Attachment>,
    pub agent: AgentId,
    pub options: AgentOptions,
    pub concurrency: usize,
    pub execution_mode: ExecutionMode,
}
impl Default for TaskConfig {
    fn default() -> Self {
        Self {
            prompt: String::new(),
            attachments: Vec::new(),
            agent: AgentId::Codex,
            options: AgentOptions::new(),
            concurrency: 2,
            execution_mode: ExecutionMode::Direct,
        }
    }
}
impl TaskConfig {
    pub fn validate(&self, repositories: &[Repository]) -> anyhow::Result<()> {
        crate::attachments::validate_limits(&self.attachments)?;
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
    Preparing,
    Running,
    Succeeded,
    Failed,
    Cancelled,
}
impl JobStatus {
    pub fn is_terminal(self) -> bool {
        !matches!(self, Self::Queued | Self::Preparing | Self::Running)
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Queued => "Queued",
            Self::Preparing => "Preparing",
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
    Maintenance,
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
            Self::Maintenance => "Waiting for isolated result inspection or cleanup.".into(),
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
    #[serde(default)]
    pub execution_mode: ExecutionMode,
    #[serde(default)]
    pub worktree: Option<WorktreeMetadata>,
    #[serde(default)]
    pub worktree_result: Option<WorktreeResult>,
    #[serde(default)]
    pub result_availability: ResultAvailability,
    #[serde(skip)]
    pub result_checked: bool,
    #[serde(default)]
    pub resolution: ResultResolution,
    #[serde(default)]
    pub resolved_at: Option<u64>,
    #[serde(skip)]
    pub review: Option<Result<crate::review::Statistics, String>>,
    #[serde(skip)]
    pub worktree_detail: String,
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
    pub raw_log: LogBuffer,
    #[serde(skip)]
    pub queue_reason: Option<QueueReason>,
}
impl Job {
    pub fn queued(repository: Repository) -> Self {
        Self {
            repository,
            execution_mode: ExecutionMode::Direct,
            worktree: None,
            worktree_result: None,
            result_availability: ResultAvailability::Unchecked,
            result_checked: false,
            resolution: ResultResolution::Unresolved,
            resolved_at: None,
            review: None,
            worktree_detail: String::new(),
            status: JobStatus::Queued,
            started_at: None,
            finished_at: None,
            exit_code: None,
            before: None,
            interrupted: false,
            detail: String::new(),
            log: LogBuffer::default(),
            raw_log: LogBuffer::default(),
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
    #[serde(default)]
    pub common_dir: Option<PathBuf>,
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
    pub fn unresolved_results(&self) -> bool {
        self.jobs.iter().any(|job| {
            job.worktree.is_some()
                && !matches!(
                    job.resolution,
                    ResultResolution::Applied | ResultResolution::Discarded
                )
                && (job.result_availability != ResultAvailability::Cleaned || !job.result_checked)
        })
    }
    pub fn history_protected(&self) -> bool {
        self.active()
            || self.unresolved_results()
            || self.jobs.iter().any(|j| {
                j.worktree.is_some()
                    && (j.result_availability != ResultAvailability::Cleaned || !j.result_checked)
            })
    }

    pub fn status(&self) -> JobStatus {
        if self.jobs.iter().any(|j| j.status == JobStatus::Running) {
            JobStatus::Running
        } else if self.jobs.iter().any(|j| j.status == JobStatus::Preparing) {
            JobStatus::Preparing
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
    pub groups: Vec<RepositoryGroup>,
    pub templates: Vec<TaskTemplate>,
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
            groups: Vec::new(),
            templates: Vec::new(),
            draft: TaskConfig::default(),
            agent_options: BTreeMap::new(),
            runs: Vec::new(),
            global_concurrency: DEFAULT_GLOBAL_CONCURRENCY,
        }
    }
}
impl AppState {
    pub fn save_group(
        &mut self,
        index: Option<usize>,
        mut group: RepositoryGroup,
    ) -> anyhow::Result<()> {
        validate_saved_name(&group.name)?;
        anyhow::ensure!(
            index.is_none_or(|i| i < self.groups.len()),
            "This group no longer exists."
        );
        anyhow::ensure!(
            self.groups
                .iter()
                .enumerate()
                .all(|(i, existing)| Some(i) == index || existing.name.trim() != group.name.trim()),
            "A group with this name already exists."
        );
        group.name = group.name.trim().to_owned();
        let mut seen = std::collections::HashSet::new();
        group.repositories.retain(|path| seen.insert(path.clone()));
        if let Some(index) = index {
            self.groups[index] = group;
        } else {
            self.groups.push(group);
        }
        Ok(())
    }

    pub fn save_template(
        &mut self,
        index: Option<usize>,
        mut template: TaskTemplate,
    ) -> anyhow::Result<()> {
        validate_saved_name(&template.name)?;
        anyhow::ensure!(
            index.is_none_or(|i| i < self.templates.len()),
            "This template no longer exists."
        );
        anyhow::ensure!(
            self.templates
                .iter()
                .enumerate()
                .all(|(i, existing)| Some(i) == index
                    || existing.name.trim() != template.name.trim()),
            "A template with this name already exists."
        );
        anyhow::ensure!(
            !template.prompt.trim().is_empty() && template.prompt.len() <= 128 * 1024,
            "Template task must contain text and be at most 128 KiB."
        );
        template.name = template.name.trim().to_owned();
        if let Some(index) = index {
            self.templates[index] = template;
        } else {
            self.templates.push(template);
        }
        Ok(())
    }
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
        self.runs
            .retain(|run| run.id != id || run.history_protected());
        self.runs.len() != before
    }
    pub fn clear_history(&mut self) -> usize {
        let before = self.runs.len();
        self.runs.retain(Run::history_protected);
        before - self.runs.len()
    }
    /// Never evict active convoys or unresolved isolated ownership records.
    pub fn trim_history(&mut self) {
        let mut completed = 0;
        self.runs.retain(|run| {
            if run.history_protected() {
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
            .map(|j| j.log.text.len() + j.raw_log.text.len())
            .sum();
        for job in self.runs.iter_mut().rev().flat_map(|r| &mut r.jobs) {
            if total <= TOTAL_LOG_LIMIT {
                break;
            }
            total -= job.log.text.len() + job.raw_log.text.len();
            if !job.log.text.is_empty() {
                job.log.text.clear();
                job.log.truncated = true;
            }
            if !job.raw_log.text.is_empty() {
                job.raw_log.text.clear();
                job.raw_log.truncated = true;
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

fn validate_saved_name(name: &str) -> anyhow::Result<()> {
    anyhow::ensure!(
        !name.trim().is_empty() && name.len() <= 128 && !name.chars().any(char::is_control),
        "Enter a name of at most 128 bytes without control characters."
    );
    Ok(())
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
