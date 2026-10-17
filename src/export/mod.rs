//! Portable historical reports. Capture only recorded metadata; no filesystem,
//! Git, scheduler, validation service or Store access is needed to take a snapshot.
pub mod file;
pub(crate) mod markdown;
pub(crate) mod time;

use crate::{
    domain::{AgentId, Job, JobStatus, ResultResolution, Run},
    validation, visibility,
};
use anyhow::{Result, bail, ensure};
use serde::Serialize;
use std::{collections::HashSet, io::Write};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Format {
    #[default]
    Markdown,
    Json,
}
impl Format {
    pub fn extension(self) -> &'static str {
        match self {
            Self::Markdown => "md",
            Self::Json => "json",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Markdown => "Markdown",
            Self::Json => "JSON",
        }
    }
}

/// IDs are resolved against current history at confirmation, in history order.
/// Filtered IDs come from the existing HistorySearch, never a second search policy.
pub enum Scope {
    Selected(Vec<u64>),
    Filtered(Vec<u64>),
    All,
}
impl Scope {
    pub fn resolve<'a>(&self, runs: &'a [Run]) -> Result<Vec<&'a Run>> {
        let ids = match self {
            Self::Selected(ids) | Self::Filtered(ids) => {
                Some(ids.iter().copied().collect::<HashSet<_>>())
            }
            Self::All => None,
        };
        if let Self::Selected(selected) = self {
            for id in selected {
                let Some(run) = runs.iter().find(|r| r.id == *id) else {
                    bail!("Selected convoy #{id} is no longer in history.");
                };
                ensure!(!run.active(), "Convoy #{id} is still active.");
            }
        }
        let mut seen = HashSet::new();
        let selected: Vec<_> = runs
            .iter()
            .filter(|r| {
                !r.active()
                    && ids.as_ref().is_none_or(|ids| ids.contains(&r.id))
                    && seen.insert(r.id)
            })
            .collect();
        ensure!(
            !selected.is_empty(),
            "No historical convoys in this export scope."
        );
        Ok(selected)
    }
}

#[derive(Debug, Serialize)]
pub struct Snapshot {
    schema_version: u32,
    export_type: &'static str,
    pub convoys: Vec<Convoy>,
}
impl Snapshot {
    pub fn capture(runs: &[Run], scope: &Scope) -> Result<Self> {
        let selected = scope.resolve(runs)?;
        Ok(Self {
            schema_version: 1,
            export_type: if matches!(scope, Scope::Selected(ids) if ids.iter().copied().collect::<HashSet<_>>().len() == 1)
            {
                "single_convoy"
            } else {
                "history"
            },
            convoys: selected.into_iter().map(Convoy::capture).collect(),
        })
    }
    pub fn suggested_name(&self, format: Format, now: u64) -> String {
        if self.export_type == "single_convoy" {
            format!(
                "codeconvoy-convoy-{}.{}",
                self.convoys[0].id,
                format.extension()
            )
        } else {
            let date = time::rfc3339(now)
                .map(|s| s[..10].to_owned())
                .unwrap_or_else(|| "undated".into());
            format!("codeconvoy-history-{date}.{}", format.extension())
        }
    }
    /// Call on a worker. Stream JSON and build at most one convoy's Markdown at a time.
    pub fn write(&self, format: Format, mut writer: impl Write) -> Result<()> {
        match format {
            Format::Json => {
                serde_json::to_writer_pretty(&mut writer, self)?;
                writeln!(writer)?;
            }
            Format::Markdown => {
                if self.export_type == "history" {
                    writeln!(
                        writer,
                        "# CodeConvoy History\n\nExported convoys: {}\n",
                        self.convoys.len()
                    )?;
                }
                for (index, convoy) in self.convoys.iter().enumerate() {
                    if index > 0 {
                        writeln!(writer, "\n---\n")?;
                    }
                    let text = markdown::convoy_snapshot(convoy);
                    if self.export_type == "history" {
                        // Nest the shared single-convoy report under the document heading.
                        for line in text.lines() {
                            if line.starts_with('#') {
                                write!(writer, "#")?;
                            }
                            writeln!(writer, "{line}")?;
                        }
                    } else {
                        writer.write_all(text.as_bytes())?;
                    }
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug, Serialize)]
pub struct Convoy {
    pub id: u64,
    status: &'static str,
    backend: &'static str,
    #[serde(skip)]
    backend_label: &'static str,
    task_summary: Option<String>,
    created_at: Option<String>,
    started_at: Option<String>,
    completed_at: Option<String>,
    duration_ms: Option<u64>,
    summary: Summary,
    repositories: Vec<Repository>,
}
#[derive(Debug, Serialize)]
struct Summary {
    repositories: usize,
    succeeded: usize,
    failed: usize,
    cancelled: usize,
    interrupted: usize,
    changed: usize,
    unchanged: usize,
    unknown_changes: usize,
    needs_review: usize,
}
#[derive(Debug, Serialize)]
struct Repository {
    name: String,
    status: &'static str,
    interrupted: bool,
    started_at: Option<String>,
    completed_at: Option<String>,
    duration_ms: Option<u64>,
    changes: Changes,
    review: &'static str,
    validation: Option<Validation>,
}
#[derive(Debug, Serialize)]
struct Changes {
    changed: Option<bool>,
    changed_files: Option<usize>,
}
impl Changes {
    fn label(&self) -> String {
        visibility::Changes {
            changed: self.changed,
            files: self.changed_files,
        }
        .label()
    }
}
#[derive(Debug, Serialize)]
struct Validation {
    status: &'static str,
    command: String,
    started_at: Option<String>,
    completed_at: Option<String>,
    duration_ms: u64,
    exit_code: Option<i32>,
}
impl Convoy {
    fn capture(run: &Run) -> Self {
        let completion = if run.jobs.is_empty()
            || run.active()
            || run.jobs.iter().any(|j| j.finished_at.is_none())
        {
            None
        } else {
            run.jobs.iter().filter_map(|j| j.finished_at).max()
        };
        let summary = visibility::summary(run);
        Self {
            id: run.id,
            status: if run.jobs.is_empty() {
                "no_jobs"
            } else if run.status() == JobStatus::Cancelled && run.jobs.iter().any(|j| j.interrupted)
            {
                "interrupted"
            } else {
                status(run.status())
            },
            backend: match run.task.agent {
                AgentId::Codex => "codex",
                AgentId::Copilot => "copilot",
                AgentId::OpenCode => "opencode",
                AgentId::Claude => "claude",
                AgentId::Cursor => "cursor",
            },
            backend_label: run.task.agent.label(),
            task_summary: run
                .task
                .prompt
                .lines()
                .find(|line| !line.trim().is_empty())
                .map(|line| markdown::plain_field(line, 160)),
            created_at: time::rfc3339(run.created_at),
            started_at: run
                .jobs
                .iter()
                .filter_map(|j| j.started_at)
                .min()
                .and_then(time::rfc3339),
            completed_at: completion.and_then(time::rfc3339),
            duration_ms: duration_ms(Some(run.created_at), completion),
            summary: Summary {
                repositories: summary.total,
                succeeded: summary.succeeded,
                failed: summary.failed,
                cancelled: summary.cancelled,
                interrupted: run.jobs.iter().filter(|j| j.interrupted).count(),
                changed: summary.changed,
                unchanged: summary.unchanged,
                unknown_changes: summary.unknown,
                needs_review: summary.review,
            },
            repositories: run
                .jobs
                .iter()
                .map(|job| {
                    let changes = visibility::changes(job);
                    Repository {
                        name: if job.repository.name.trim().is_empty() {
                            "Unnamed repository".into()
                        } else {
                            markdown::plain_field(&job.repository.name, 80)
                        },
                        status: status(job.status),
                        interrupted: job.interrupted,
                        started_at: job.started_at.and_then(time::rfc3339),
                        completed_at: job.finished_at.and_then(time::rfc3339),
                        duration_ms: duration_ms(job.started_at, job.finished_at),
                        changes: Changes {
                            changed: changes.changed,
                            changed_files: changes.files,
                        },
                        review: review_status(job),
                        validation: job.validation.as_ref().map(|v| Validation {
                            status: validation_status(v.status),
                            command: markdown::command_text(&v.command),
                            started_at: time::rfc3339(v.started_at),
                            completed_at: v.finished_at.and_then(time::rfc3339),
                            duration_ms: v.duration_ms,
                            exit_code: v.exit_code,
                        }),
                    }
                })
                .collect(),
        }
    }
}
fn duration_ms(start: Option<u64>, end: Option<u64>) -> Option<u64> {
    let (start, end) = start.zip(end)?;
    if start >= 253_402_300_800 || end >= 253_402_300_800 {
        return None;
    }
    end.checked_sub(start)?.checked_mul(1000)
}
fn status(status: JobStatus) -> &'static str {
    match status {
        JobStatus::Queued => "queued",
        JobStatus::Preparing => "preparing",
        JobStatus::Running => "running",
        JobStatus::Succeeded => "succeeded",
        JobStatus::Failed => "failed",
        JobStatus::Cancelled => "cancelled",
    }
}
fn status_label(status: &str) -> &'static str {
    match status {
        "queued" => "Queued",
        "preparing" => "Preparing",
        "running" => "Running",
        "succeeded" => "Succeeded",
        "failed" => "Failed",
        "cancelled" => "Cancelled",
        "interrupted" => "Interrupted",
        _ => "No jobs",
    }
}
fn repository_status_label(job: &Repository) -> &'static str {
    if job.interrupted {
        "Interrupted"
    } else {
        status_label(job.status)
    }
}
fn validation_status(status: validation::Status) -> &'static str {
    match status {
        validation::Status::Passed => "passed",
        validation::Status::Failed => "failed",
        validation::Status::Cancelled => "cancelled",
        validation::Status::Unavailable => "unavailable",
        validation::Status::Running => "running",
    }
}
fn validation_label(validation: Option<&Validation>) -> &'static str {
    validation.map_or("Not run / not recorded", |v| match v.status {
        "passed" => "Passed",
        "failed" => "Failed",
        "cancelled" => "Cancelled",
        "unavailable" => "Unavailable",
        _ => "Running",
    })
}
fn review_status(job: &Job) -> &'static str {
    match job.resolution {
        ResultResolution::Applied => "applied",
        ResultResolution::Discarded => "discarded",
        ResultResolution::ApplyPending => "apply_pending",
        ResultResolution::DiscardPending => "discard_pending",
        ResultResolution::Unresolved if visibility::needs_review(job) => "required",
        ResultResolution::Unresolved if job.status.is_terminal() => "not_required",
        ResultResolution::Unresolved => "unavailable",
    }
}
fn review_label(status: &str) -> &'static str {
    match status {
        "applied" => "Applied",
        "discarded" => "Discarded",
        "apply_pending" => "Apply pending / uncertain",
        "discard_pending" => "Discard pending",
        "required" => "Required",
        "not_required" => "Not required",
        _ => "Unavailable",
    }
}

#[cfg(test)]
mod tests;
