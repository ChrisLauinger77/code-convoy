//! Deterministic metadata fixtures; no agent, repository or clock access.
use codeconvoy::{domain::*, validation, visibility::Changes};

pub const NOW: u64 = 2_000_000_000;

pub fn state(count: usize) -> AppState {
    let repositories: Vec<_> = (0..4)
        .map(|i| Repository {
            name: format!("Service-{i}-Überprüfung"),
            path: format!("/synthetic-codeconvoy/service-{i}").into(),
        })
        .collect();
    let mut state = AppState {
        next_run: count as u64 + 1,
        repositories: repositories.clone(),
        ..Default::default()
    };
    for id in (1..=count as u64).rev() {
        let created_at = NOW - (count as u64 - id + 1) * 3600;
        let jobs = repositories
            .iter()
            .enumerate()
            .map(|(i, repo)| {
                let mut job = Job::queued(repo.clone());
                job.status = match (id + i as u64) % 7 {
                    0 => JobStatus::Failed,
                    1 => JobStatus::Cancelled,
                    _ => JobStatus::Succeeded,
                };
                job.started_at = Some(created_at + 1);
                job.finished_at = Some(created_at + 30);
                job.exit_code = Some(i32::from(job.status != JobStatus::Succeeded));
                if id % 5 != 0 {
                    job.completion_changes = Some(Changes {
                        changed: Some(i % 2 == 0),
                        files: Some(if i % 2 == 0 { 3 } else { 0 }),
                    });
                }
                if id % 3 == 0 {
                    job.validation = Some(validation::Record {
                        command: validation::Command {
                            executable: "cargo".into(),
                            arguments: vec!["test".into()],
                        },
                        directory: repo.path.clone(),
                        status: if id % 2 == 0 {
                            validation::Status::Passed
                        } else {
                            validation::Status::Failed
                        },
                        started_at: created_at + 31,
                        finished_at: Some(created_at + 40),
                        duration_ms: 9000,
                        exit_code: Some((id % 2) as i32),
                        output: String::new(),
                        truncated: false,
                        detail: String::new(),
                    });
                }
                job
            })
            .collect();
        state.runs.push(Run {
            id,
            provenance: None,
            created_at,
            task: TaskConfig {
                prompt: format!(
                    "Task {id}: review Unicode 日本語\n{}",
                    "Detailed reproducible context. ".repeat(if id % 10 == 0 { 512 } else { 8 })
                ),
                agent: AgentId::ALL[id as usize % AgentId::ALL.len()],
                ..Default::default()
            },
            jobs,
        });
    }
    state
}
