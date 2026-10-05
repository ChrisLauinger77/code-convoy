use crate::domain::{AgentId, JobStatus, Run};

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
        JobStatus::Running => "> Running",
        JobStatus::Succeeded => "+ Succeeded",
        JobStatus::Failed => "× Failed",
        JobStatus::Cancelled => "– Cancelled",
    }
}

pub fn run_state(run: &Run) -> &'static str {
    if run.jobs.is_empty() {
        "No jobs"
    } else {
        run.status().label()
    }
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
