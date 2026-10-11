#![allow(clippy::unwrap_used)]
use super::*;
use crate::domain::{AppState, Repository as DomainRepository, TaskConfig};

fn run(id: u64, statuses: &[JobStatus]) -> Run {
    Run {
        id,
        provenance: None,
        created_at: 1_700_000_000,
        task: TaskConfig {
            prompt: "Brief task\nPRIVATE_FULL_PROMPT".into(),
            ..Default::default()
        },
        jobs: statuses
            .iter()
            .enumerate()
            .map(|(index, status)| {
                let mut job = Job::queued(DomainRepository {
                    name: format!("日本語|repo{id}-{index}"),
                    path: "/SECRET_SOURCE_PATH".into(),
                });
                job.status = *status;
                job.started_at = Some(1_700_000_010);
                job.finished_at = Some(1_700_000_020);
                job.completion_changes = Some(visibility::Changes {
                    changed: Some(index == 0),
                    files: Some(usize::from(index == 0)),
                });
                job
            })
            .collect(),
    }
}
fn render(snapshot: &Snapshot, format: Format) -> String {
    let mut bytes = Vec::new();
    snapshot.write(format, &mut bytes).unwrap();
    String::from_utf8(bytes).unwrap()
}
fn json(snapshot: &Snapshot) -> serde_json::Value {
    serde_json::from_str(&render(snapshot, Format::Json)).unwrap()
}
#[test]
fn explicit_schema_statuses_timestamps_counts_and_associations() {
    let mut runs = vec![
        run(
            42,
            &[
                JobStatus::Succeeded,
                JobStatus::Failed,
                JobStatus::Cancelled,
            ],
        ),
        run(9, &[JobStatus::Succeeded]),
        run(1, &[JobStatus::Cancelled]),
    ];
    runs[0].jobs[2].interrupted = true;
    let snapshot = Snapshot::capture(&runs, &Scope::All).unwrap();
    let value = json(&snapshot);
    assert_eq!(value["schema_version"], 1);
    assert_eq!(value["export_type"], "history");
    let a = &value["convoys"][0];
    assert_eq!(a["id"], 42);
    assert_eq!(a["status"], "failed");
    assert_eq!(a["backend"], "codex");
    assert_eq!(a["created_at"], "2023-11-14T22:13:20Z");
    assert_eq!(a["started_at"], "2023-11-14T22:13:30Z");
    assert_eq!(a["completed_at"], "2023-11-14T22:13:40Z");
    assert_eq!(a["duration_ms"], 20_000);
    for key in ["succeeded", "failed", "cancelled", "interrupted", "changed"] {
        assert_eq!(a["summary"][key], 1);
    }
    assert_eq!(a["summary"]["unchanged"], 2);
    for (index, status) in ["succeeded", "failed", "cancelled"].iter().enumerate() {
        assert_eq!(a["repositories"][index]["status"], *status);
        assert_eq!(
            a["repositories"][index]["name"],
            format!("日本語|repo42-{index}")
        );
    }
    assert_eq!(
        value["convoys"][1]["repositories"][0]["name"],
        "日本語|repo9-0"
    );
    assert_eq!(value["convoys"][1]["status"], "succeeded");
    assert_eq!(value["convoys"][2]["status"], "cancelled");
    assert_eq!(
        render(&snapshot, Format::Json),
        render(&snapshot, Format::Json)
    );
    assert_eq!(
        a.as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        [
            "backend",
            "completed_at",
            "created_at",
            "duration_ms",
            "id",
            "repositories",
            "started_at",
            "status",
            "summary",
            "task_summary"
        ]
    );
}
#[test]
fn single_markdown_is_the_copy_formatter_and_multi_nests_separates_and_escapes() {
    let runs = vec![
        run(9, &[JobStatus::Succeeded]),
        run(2, &[JobStatus::Failed]),
    ];
    let single = Snapshot::capture(&runs, &Scope::Selected(vec![9])).unwrap();
    assert_eq!(
        render(&single, Format::Markdown),
        markdown::convoy(&runs[0])
    );
    assert_eq!(json(&single)["export_type"], "single_convoy");
    let multi = Snapshot::capture(&runs, &Scope::All).unwrap();
    let text = render(&multi, Format::Markdown);
    assert!(text.starts_with("# CodeConvoy History\n\nExported convoys: 2\n"));
    assert!(text.contains("\n---\n\n## Convoy #2"));
    assert!(text.contains("### Repository Results"));
    assert!(text.contains("日本語\\|repo9-0"));
    assert!(text.find("## Convoy #9").unwrap() < text.find("## Convoy #2").unwrap());
}
#[test]
fn scopes_follow_history_order_deduplicate_and_exclude_active() {
    let runs = vec![
        run(4, &[JobStatus::Succeeded]),
        run(3, &[JobStatus::Running]),
        run(2, &[JobStatus::Failed]),
        run(1, &[JobStatus::Cancelled]),
    ];
    for (scope, expected) in [
        (Scope::Selected(vec![1, 4, 1]), vec![4, 1]),
        (Scope::Filtered(vec![1, 2, 3, 2]), vec![2, 1]),
        (Scope::All, vec![4, 2, 1]),
    ] {
        assert_eq!(
            Snapshot::capture(&runs, &scope)
                .unwrap()
                .convoys
                .iter()
                .map(|c| c.id)
                .collect::<Vec<_>>(),
            expected
        );
    }
    for scope in [
        Scope::Selected(vec![]),
        Scope::Filtered(vec![]),
        Scope::Selected(vec![404]),
        Scope::Selected(vec![4, 404]),
        Scope::Selected(vec![3]),
    ] {
        assert!(Snapshot::capture(&runs, &scope).is_err());
    }
    assert!(Snapshot::capture(&[], &Scope::All).is_err());
    let duplicates = vec![runs[0].clone(), runs[0].clone()];
    assert_eq!(
        Snapshot::capture(&duplicates, &Scope::All)
            .unwrap()
            .convoys
            .len(),
        1
    );
}
#[test]
fn filtering_snapshot_is_independent_of_later_changes_and_retention() {
    let mut state = AppState {
        runs: vec![
            run(7, &[JobStatus::Succeeded]),
            run(4, &[JobStatus::Failed]),
        ],
        ..Default::default()
    };
    let before = serde_json::to_value(&state).unwrap();
    let mut search = visibility::HistorySearch::default();
    search.query = "repo7".into();
    let snapshot =
        Snapshot::capture(&state.runs, &Scope::Filtered(search.matching(&state.runs))).unwrap();
    assert_eq!(serde_json::to_value(&state).unwrap(), before);
    search.query = "repo4".into();
    assert_eq!(search.matching(&state.runs), [4]);
    let text = render(&snapshot, Format::Json);
    state.runs[0].jobs[0].status = JobStatus::Failed;
    state.runs.clear();
    assert_eq!(render(&snapshot, Format::Json), text);
    assert_eq!(json(&snapshot)["convoys"][0]["id"], 7);
}
#[test]
fn missing_legacy_observations_validation_and_invalid_dates_remain_unknown() {
    let mut r = run(1, &[JobStatus::Succeeded]);
    r.jobs[0].completion_changes = None;
    r.jobs[0].started_at = None;
    r.jobs[0].finished_at = None;
    r.jobs[0].review = Some(Ok(crate::review::Statistics {
        files: 999,
        ..Default::default()
    }));
    r.jobs[0].worktree_result = Some(crate::domain::WorktreeResult {
        exists: true,
        changed: Some(true),
        observed_this_session: true,
    });
    r.created_at = u64::MAX;
    let a = json(&Snapshot::capture(&[r], &Scope::All).unwrap())["convoys"][0].clone();
    for field in ["created_at", "started_at", "completed_at", "duration_ms"] {
        assert!(a[field].is_null());
    }
    let j = &a["repositories"][0];
    assert!(j["changes"]["changed"].is_null());
    assert!(j["changes"]["changed_files"].is_null());
    assert!(j["validation"].is_null());
    assert_eq!(a["summary"]["unknown_changes"], 1);
    assert_eq!(time::rfc3339(0).unwrap(), "1970-01-01T00:00:00Z");
    assert_eq!(time::rfc3339(951_782_400).unwrap(), "2000-02-29T00:00:00Z");
    assert_eq!(
        time::rfc3339(253_402_300_799).unwrap(),
        "9999-12-31T23:59:59Z"
    );
    let state: AppState =
        serde_json::from_str(include_str!("../../tests/fixtures/state-v0.1.json")).unwrap();
    for run in state.runs.iter().filter(|r| !r.active()) {
        assert!(Snapshot::capture(std::slice::from_ref(run), &Scope::All).is_ok());
    }
}
#[test]
fn validation_and_privacy_are_metadata_only() {
    let mut r = run(
        5,
        &[
            JobStatus::Succeeded,
            JobStatus::Failed,
            JobStatus::Cancelled,
        ],
    );
    for (index, status) in [validation::Status::Passed, validation::Status::Failed]
        .into_iter()
        .enumerate()
    {
        r.jobs[index].validation = Some(validation::Record {
            command: validation::Command {
                executable: "/secret/bin/cargo".into(),
                arguments: vec!["test".into(), "--manifest-path=/SECRET_MANIFEST".into()],
            },
            directory: "/SECRET_VALIDATION_DIRECTORY".into(),
            status,
            started_at: 1_700_000_100,
            finished_at: Some(1_700_000_101),
            duration_ms: 1000,
            exit_code: Some(index as i32),
            output: "SECRET_VALIDATION_LOG".into(),
            detail: "SECRET_VALIDATION_DETAIL".into(),
            truncated: false,
        });
        r.jobs[index].raw_log.append("SECRET_RAW_LOG");
    }
    r.task.attachments.push(crate::attachments::Attachment {
        path: "/SECRET_ATTACHMENT_PATH".into(),
        filename: "SECRET_ATTACHMENT_NAME".into(),
        kind: crate::attachments::AttachmentKind::Text,
        size: 99,
        sha256: "SECRET_ATTACHMENT_HASH".into(),
    });
    r.jobs[0].worktree = Some(crate::domain::WorktreeMetadata {
        owner: "SECRET_OWNER".into(),
        run: r.id,
        job: 0,
        repository: r.jobs[0].repository.clone(),
        common_dir: "/SECRET_COMMON_DIR".into(),
        path: "/SECRET_WORKTREE".into(),
        base_commit: "SECRET_BASE_COMMIT".into(),
        execution_mode: crate::domain::ExecutionMode::IsolatedWorktree,
    });
    r.jobs[0].worktree_detail = "SECRET_DIFF".into();
    r.jobs[0].log.append("SECRET_ACTIVITY");
    r.task
        .options
        .insert("environment".into(), "SECRET_ENVIRONMENT".into());
    let snapshot = Snapshot::capture(&[r], &Scope::All).unwrap();
    let value = json(&snapshot);
    for (index, status) in ["passed", "failed"].iter().enumerate() {
        assert_eq!(
            value["convoys"][0]["repositories"][index]["validation"]["status"],
            *status
        );
        assert_eq!(
            value["convoys"][0]["repositories"][index]["validation"]["exit_code"],
            index
        );
    }
    for format in [Format::Json, Format::Markdown] {
        let text = render(&snapshot, format);
        assert!(!text.contains("SECRET"));
        assert!(!text.contains("PRIVATE_FULL_PROMPT"));
        assert!(!text.contains("/secret"));
        for key in [
            "worktree_result",
            "ownership",
            "agent_options",
            "attachments",
            "provenance",
            "directory",
            "output",
            "review_fingerprint",
        ] {
            assert!(!text.contains(key));
        }
    }
}
#[test]
fn large_history_is_stable_and_names_are_portable() {
    let runs: Vec<_> = (1..=2500)
        .rev()
        .map(|id| run(id, &[JobStatus::Succeeded]))
        .collect();
    let snapshot = Snapshot::capture(&runs, &Scope::All).unwrap();
    assert_eq!(json(&snapshot)["convoys"].as_array().unwrap().len(), 2500);
    assert_eq!(
        snapshot.suggested_name(Format::Json, 0),
        "codeconvoy-history-1970-01-01.json"
    );
    assert_eq!(
        Snapshot::capture(&runs, &Scope::Selected(vec![42]))
            .unwrap()
            .suggested_name(Format::Markdown, 0),
        "codeconvoy-convoy-42.md"
    );
    assert!(render(&snapshot, Format::Markdown).contains("Exported convoys: 2500"));
}
