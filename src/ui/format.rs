use crate::{
    agents::{OptionKind, OptionSpec},
    domain::{self, AgentId, Job, JobStatus, QueueReason, Run},
};

pub fn agent_name(agent: AgentId) -> &'static str {
    match agent {
        AgentId::Codex => "Codex",
        AgentId::Copilot => "Copilot",
        AgentId::OpenCode => "OpenCode",
        AgentId::Claude => "Claude Code",
    }
}

pub fn duration(seconds: u64) -> String {
    if seconds >= 3600 {
        format!(
            "{}:{:02}:{:02}",
            seconds / 3600,
            seconds / 60 % 60,
            seconds % 60
        )
    } else {
        format!("{}:{:02}", seconds / 60, seconds % 60)
    }
}

pub fn status_label(status: JobStatus) -> &'static str {
    match status {
        JobStatus::Queued => "· Queued",
        JobStatus::Preparing => "> Preparing",
        JobStatus::Running => "> Running",
        JobStatus::Succeeded => "Succeeded",
        JobStatus::Failed => "× Failed",
        JobStatus::Cancelled => "– Cancelled",
    }
}

pub fn run_state(run: &Run) -> &'static str {
    if run.jobs.is_empty() {
        "No jobs"
    } else if run.status() == JobStatus::Cancelled && run.jobs.iter().any(|j| j.interrupted) {
        "Interrupted"
    } else {
        run.status().label()
    }
}

pub fn job_label(job: &Job) -> &'static str {
    if job.interrupted {
        "– Interrupted"
    } else {
        status_label(job.status)
    }
}

pub fn output_empty(job: &Job, this_session: bool) -> String {
    if job.interrupted {
        return "Interrupted by application exit. Output from previous sessions is not retained; inspect the repository before retrying.".into();
    }
    match job.status {
        JobStatus::Queued => job.queue_reason.map_or_else(
            || "Queued; awaiting admission to run.".into(),
            QueueReason::label,
        ),
        JobStatus::Preparing => "Preparing isolated worktree from committed HEAD…".into(),
        JobStatus::Running => "Agent running; no output received yet.".into(),
        _ if !this_session => {
            "No retained output. Logs are session-only and are not restored after restarting."
                .into()
        }
        JobStatus::Cancelled if job.started_at.is_none() => {
            "Cancelled before execution; no agent output.".into()
        }
        JobStatus::Cancelled => {
            "Stopped without textual output. Any partial edits remain in the repository.".into()
        }
        JobStatus::Failed => {
            "No textual output was captured. Inspect the job diagnostics above.".into()
        }
        JobStatus::Succeeded => {
            "Completed without textual output. Inspect the current working-tree diff.".into()
        }
    }
}

pub fn option_label(spec: &OptionSpec) -> &str {
    match spec.key {
        "executable" => "Executable",
        "model_reasoning_effort" | "reasoning_effort" => "Reasoning",
        "temp_access" => "Temp directory",
        _ => spec.label,
    }
}

pub fn option_value(spec: &OptionSpec, saved: Option<&String>) -> String {
    let Some(value) = saved else {
        return "Not recorded".into();
    };
    match spec.kind {
        OptionKind::Text { hint } if value.is_empty() => hint.into(),
        OptionKind::Choice(choices) => choices.iter().find(|(v, _)| *v == value).map_or_else(
            || format!("Unrecognized saved value: {value}"),
            |(_, label)| (*label).into(),
        ),
        _ => value.clone(),
    }
}

/// UTC formatting avoids a timezone dependency and remains useful offline on
/// every platform. Bound malformed persisted timestamps before date arithmetic.
pub fn timestamp(seconds: u64) -> String {
    if seconds >= 253_402_300_800 {
        return "Date unavailable".into();
    }
    let days = seconds / 86_400 + 719_162; // Complete Gregorian days before 1970-01-01.
    let mut year = days / 146_097 * 400 + 1;
    let mut day = days % 146_097;
    let leap = |y: u64| y.is_multiple_of(4) && (!y.is_multiple_of(100) || y.is_multiple_of(400));
    loop {
        let length = if leap(year) { 366 } else { 365 };
        if day < length {
            break;
        }
        day -= length;
        year += 1;
    }
    let months = [
        31,
        if leap(year) { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    let mut month = 1;
    for length in months {
        if day < length {
            break;
        }
        day -= length;
        month += 1;
    }
    format!(
        "{year:04}-{month:02}-{:02} {:02}:{:02}:{:02} UTC",
        day + 1,
        seconds / 3600 % 24,
        seconds / 60 % 60,
        seconds % 60
    )
}

pub fn run_label(run: &Run) -> String {
    format!(
        "#{} · {} · {} repos · {} · {}/{}",
        run.id,
        agent_name(run.task.agent),
        run.jobs.len(),
        run_state(run),
        run.completed_jobs(),
        run.jobs.len()
    )
}

/// Last result observation is independent of agent completion and restart trust.
pub(super) fn isolated_result(job: &domain::Job) -> Option<&'static str> {
    if job.execution_mode != domain::ExecutionMode::IsolatedWorktree {
        return None;
    }
    Some(match &job.worktree_result {
        Some(result) if !result.observed_this_session => match (result.exists, result.changed) {
            (true, Some(true)) => "Saved result: changes retained · not checked after restart",
            (true, Some(false)) => "Saved result: no changes · not checked after restart",
            _ => "Saved result metadata · not checked after restart",
        },
        Some(result) => match (result.exists, result.changed) {
            (true, Some(true)) => "Isolated changes retained",
            (true, Some(false)) => "No repository changes · isolated worktree retained",
            (false, _) => "No isolated checkout found at last inspection",
            _ => "Isolated result changes unknown · worktree retained",
        },
        None if job.status == JobStatus::Preparing => {
            "Preparing isolated worktree from committed HEAD"
        }
        None if job.status == JobStatus::Running => "Agent running in isolated worktree",
        None if job.worktree.is_some() => "Isolated result metadata present · result not validated",
        None => "No isolated result recorded",
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{Job, Repository, TaskConfig};

    #[test]
    fn durations_keep_minutes_and_hours_readable() {
        for (seconds, expected) in [
            (0, "0:00"),
            (59, "0:59"),
            (60, "1:00"),
            (3599, "59:59"),
            (3600, "1:00:00"),
            (360001, "100:00:01"),
        ] {
            assert_eq!(duration(seconds), expected);
        }
    }

    #[test]
    fn timestamps_handle_epoch_leap_centuries_and_corrupt_values() {
        for (seconds, expected) in [
            (0, "1970-01-01 00:00:00 UTC"),
            (951_827_696, "2000-02-29 12:34:56 UTC"),
            (4_107_542_400, "2100-03-01 00:00:00 UTC"),
            (253_402_300_799, "9999-12-31 23:59:59 UTC"),
            (253_402_300_800, "Date unavailable"),
            (u64::MAX, "Date unavailable"),
        ] {
            assert_eq!(timestamp(seconds), expected);
        }
    }

    #[test]
    fn historical_options_use_backend_labels_without_inventing_missing_values() {
        for agent in AgentId::ALL {
            let backend = crate::agents::backend(agent).expect("built-in backend");
            for spec in backend.options() {
                assert_eq!(option_value(spec, None), "Not recorded");
                if let OptionKind::Choice(choices) = spec.kind {
                    for (value, label) in choices {
                        assert_eq!(option_value(spec, Some(&(*value).into())), *label);
                    }
                }
            }
        }
    }

    #[test]
    fn output_states_explain_waits_interruption_and_session_only_history() {
        let mut job = Job::queued(Repository {
            name: "repo".into(),
            path: "/repo".into(),
        });
        assert!(output_empty(&job, true).contains("awaiting admission"));
        for reason in [QueueReason::GlobalLimit, QueueReason::Repository(7)] {
            job.queue_reason = Some(reason);
            assert_eq!(output_empty(&job, true), reason.label());
        }
        job.status = JobStatus::Running;
        assert!(output_empty(&job, true).contains("running"));
        job.status = JobStatus::Succeeded;
        assert_eq!(job_label(&job), "Succeeded");
        assert!(output_empty(&job, true).contains("Completed"));
        assert!(output_empty(&job, false).contains("not restored"));
        job.status = JobStatus::Cancelled;
        assert!(output_empty(&job, true).contains("before execution"));
        job.started_at = Some(1);
        assert!(output_empty(&job, true).contains("partial edits"));
        job.interrupted = true;
        assert_eq!(job_label(&job), "– Interrupted");
        assert!(output_empty(&job, false).contains("Interrupted"));
    }

    #[test]
    fn run_summary_prioritizes_active_jobs_then_failures() {
        let job = Job::queued(Repository {
            name: "repo".into(),
            path: "/repo".into(),
        });
        let mut run = Run {
            id: 12,
            created_at: 0,
            task: TaskConfig::default(),
            jobs: vec![job.clone(), job],
        };
        run.jobs[0].status = JobStatus::Failed;
        assert_eq!(run_state(&run), "Queued");
        run.jobs[1].status = JobStatus::Running;
        assert_eq!(run_state(&run), "Running");
        run.jobs[1].status = JobStatus::Cancelled;
        assert_eq!(run_state(&run), "Failed");
        run.jobs[0].status = JobStatus::Succeeded;
        assert_eq!(run_state(&run), "Cancelled");
        run.jobs[1].status = JobStatus::Succeeded;
        assert_eq!(run_label(&run), "#12 · Codex · 2 repos · Succeeded · 2/2");
    }
}
