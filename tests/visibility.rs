#![allow(clippy::unwrap_used)]
use codeconvoy::{domain::*, visibility::*};

fn job(status: JobStatus, changed: Option<bool>) -> Job {
    let mut job = Job::queued(Repository {
        name: "Service-Alpha".into(),
        path: "/fixtures/service-alpha".into(),
    });
    job.status = status;
    job.completion_changes = changed.map(|changed| Changes {
        changed: Some(changed),
        files: Some(usize::from(changed) * 3),
    });
    job
}
fn run(id: u64, jobs: Vec<Job>) -> Run {
    Run {
        id,
        provenance: None,
        created_at: 0,
        task: TaskConfig {
            prompt: "Update Documentation for ÜberService".into(),
            agent: AgentId::Copilot,
            ..Default::default()
        },
        jobs,
    }
}
#[test]
fn history_search_matches_metadata_case_insensitively_without_mutating_history() {
    let runs = vec![
        run(123, vec![job(JobStatus::Succeeded, Some(false))]),
        run(456, vec![job(JobStatus::Failed, Some(true))]),
    ];
    let saved = serde_json::to_value(&runs).unwrap();
    let mut search = HistorySearch::default();
    for query in [
        "documentation",
        "DOCUMENTATION",
        "überservice",
        "service-ALPHA",
        "GITHUB COPILOT",
        " /fixtures/ ",
    ] {
        search.query = query.into();
        assert_eq!(search.matching(&runs), vec![123, 456], "{query}");
    }
    search.query = "#123".into();
    assert_eq!(search.matching(&runs), vec![123]);
    search.filter = HistoryFilter::Failed;
    assert!(search.matching(&runs).is_empty());
    search.query.clear();
    assert_eq!(search.matching(&runs), vec![456]);
    search.clear();
    assert_eq!(search.matching(&runs), vec![123, 456]);
    assert_eq!(serde_json::to_value(&runs).unwrap(), saved);
}
#[test]
fn history_filters_preserve_order_exclude_active_and_respect_review_resolution() {
    let mut resolved = job(JobStatus::Succeeded, Some(true));
    resolved.resolution = ResultResolution::Applied;
    let runs = vec![
        run(5, vec![job(JobStatus::Running, None)]),
        run(4, vec![job(JobStatus::Failed, Some(false))]),
        run(3, vec![job(JobStatus::Cancelled, Some(true))]),
        run(2, vec![job(JobStatus::Succeeded, None)]),
        run(1, vec![resolved]),
    ];
    let mut search = HistorySearch::default();
    for (filter, ids) in [
        (HistoryFilter::All, vec![4, 3, 2, 1]),
        (HistoryFilter::Completed, vec![2, 1]),
        (HistoryFilter::Failed, vec![4]),
        (HistoryFilter::Cancelled, vec![3]),
        (HistoryFilter::NeedsReview, vec![3, 2]),
    ] {
        search.filter = filter;
        assert_eq!(search.matching(&runs), ids);
    }
    assert!(search.matching(&[]).is_empty());
}
#[test]
fn comparisons_keep_outcomes_independent_and_totals_complete() {
    let run = run(
        1,
        vec![
            job(JobStatus::Succeeded, Some(true)),
            job(JobStatus::Succeeded, Some(false)),
            job(JobStatus::Failed, Some(true)),
            job(JobStatus::Cancelled, Some(false)),
            job(JobStatus::Cancelled, None),
        ],
    );
    assert_eq!(
        summary(&run),
        Summary {
            total: 5,
            succeeded: 2,
            failed: 1,
            cancelled: 2,
            changed: 2,
            unchanged: 2,
            unknown: 1,
            review: 3
        }
    );
    assert_eq!(
        run.jobs
            .iter()
            .filter(|j| RepositoryFilter::Changed.matches(j))
            .count(),
        2
    );
    assert!(RepositoryFilter::Failed.matches(&run.jobs[2]));
    assert!(RepositoryFilter::NeedsReview.matches(&run.jobs[2]));
    assert_eq!(changes(&run.jobs[2]).label(), "Changed · 3 files");
    assert_eq!(summary(&self::run(2, vec![])), Summary::default());
    let all_success = self::run(3, vec![job(JobStatus::Succeeded, Some(false)); 4]);
    assert_eq!(summary(&all_success).succeeded, 4);
    assert_eq!(summary(&all_success).unchanged, 4);
    assert_eq!(
        summary(&self::run(4, vec![job(JobStatus::Running, None)])).unknown,
        1
    );
}
#[test]
fn saved_observations_survive_serialization_and_ignore_current_review_and_recovery() {
    let mut job = job(JobStatus::Failed, Some(true));
    job.review = Some(Ok(codeconvoy::review::Statistics::default()));
    job.worktree_result = Some(WorktreeResult {
        exists: true,
        changed: Some(false),
        observed_this_session: true,
    });
    let restored: Job = serde_json::from_value(serde_json::to_value(&job).unwrap()).unwrap();
    assert_eq!(changes(&restored).changed, Some(true));
    assert_eq!(changes(&restored).files, Some(3));
    assert!(needs_review(&restored));
    let mut legacy = serde_json::to_value(&restored).unwrap();
    legacy.as_object_mut().unwrap().remove("completion_changes");
    let legacy: Job = serde_json::from_value(legacy).unwrap();
    assert_eq!(changes(&legacy), Changes::default());
    assert!(needs_review(&legacy));
    let state: AppState = serde_json::from_str(include_str!("fixtures/state-v0.1.json")).unwrap();
    let mut search = HistorySearch::default();
    let _ = search.matching(&state.runs);
    assert!(
        state
            .runs
            .iter()
            .flat_map(|r| &r.jobs)
            .all(|j| j.completion_changes.is_none())
    );
}
#[test]
fn unknown_and_resolved_review_are_orthogonal_to_execution() {
    let mut job = job(JobStatus::Succeeded, None);
    assert!(needs_review(&job));
    for resolution in [ResultResolution::Applied, ResultResolution::Discarded] {
        job.resolution = resolution;
        assert!(!needs_review(&job));
        assert_eq!(changes(&job).changed, None);
    }
    job.resolution = ResultResolution::ApplyPending;
    assert!(needs_review(&job));
    job.status = JobStatus::Running;
    assert!(!needs_review(&job));
}

#[test]
fn unchanged_retained_results_need_disposition_even_after_success() {
    let mut job = job(JobStatus::Succeeded, Some(false));
    job.execution_mode = ExecutionMode::IsolatedWorktree;
    job.worktree = Some(WorktreeMetadata {
        owner: "fixture".into(),
        run: 1,
        job: 0,
        repository: job.repository.clone(),
        common_dir: "/fixtures/service-alpha/.git".into(),
        path: "/fixtures/owned/tree".into(),
        base_commit: "abc".into(),
        execution_mode: ExecutionMode::IsolatedWorktree,
    });
    assert!(needs_review(&job));
    job.resolution = ResultResolution::Applied;
    assert!(!needs_review(&job));
    assert_eq!(changes(&job).changed, Some(false));
}

#[test]
fn cached_search_updates_after_completion_resolution_and_history_removal() {
    let mut runs = vec![run(1, vec![job(JobStatus::Running, Some(true))])];
    let mut search = HistorySearch::default();
    search.query = "documentation".into();
    search.filter = HistoryFilter::NeedsReview;
    assert!(search.matching(&runs).is_empty());
    runs[0].jobs[0].status = JobStatus::Succeeded;
    assert_eq!(search.matching(&runs), vec![1]);
    assert_eq!(search.matching(&runs), vec![1]);
    runs[0].jobs[0].resolution = ResultResolution::Applied;
    assert!(search.matching(&runs).is_empty());
    search.clear();
    assert_eq!(search.matching(&runs), vec![1]);
    runs.clear();
    assert!(search.matching(&runs).is_empty());
}
