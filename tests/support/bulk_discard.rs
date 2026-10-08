use super::*;
use codeconvoy::persistence::bulk_discard::{Batch, Plan, Scope, Summary};

async fn add_result(f: &mut Fixture, run: u64) -> WorktreeMetadata {
    let index = f
        .state
        .runs
        .iter()
        .find(|r| r.id == run)
        .map_or(0, |r| r.jobs.len());
    let original = f.metadata();
    let m = worktrees::reserve(
        &f.store.directory().join("worktrees"),
        run,
        index,
        original.repository.clone(),
        original.common_dir,
        original.base_commit,
    )
    .unwrap();
    worktrees::create(&m, &Cancellation::default(), &AtomicBool::new(true))
        .await
        .unwrap();
    fs::write(
        m.path.join("tracked.txt"),
        format!("result {run}/{index}\n"),
    )
    .unwrap();
    let mut job = Job::queued(m.repository.clone());
    job.execution_mode = ExecutionMode::IsolatedWorktree;
    job.worktree = Some(m.clone());
    job.finish(JobStatus::Failed, Some(7), "fixture failure".into());
    job.raw_log.append("unchanged raw output");
    if let Some(r) = f.state.runs.iter_mut().find(|r| r.id == run) {
        r.jobs.push(job);
    } else {
        f.state.runs.push(Run {
            id: run,
            provenance: None,
            created_at: 0,
            task: f.state.runs[0].task.clone(),
            jobs: vec![job],
        });
    }
    f.state.next_run = f.state.next_run.max(run + 1);
    m
}
async fn execute(f: &mut Fixture, plan: Plan) -> Summary {
    let mut batch = Batch::new(plan, &mut f.state);
    while !batch.done() {
        if let Some(op) = batch.begin_next(&f.store, &mut f.state) {
            let completion = op.execute(&f.manager.lifecycle()).await;
            batch.finish(&f.store, &mut f.state, &op, completion);
        }
    }
    batch.activity(&mut f.state, &batch.summary.label(batch.plan.scope));
    batch.summary
}
async fn discard(f: &mut Fixture, scope: Scope) -> Summary {
    let plan = Plan::new(&f.state, scope);
    execute(f, plan).await
}

#[tokio::test]
async fn bulk_discard_terminal_convoy_unblocks_history_and_preserves_dirty_source_and_direct_job() {
    let mut f = Fixture::new().await;
    f.edit();
    let first = f.metadata();
    let second = add_result(&mut f, 1).await;
    let mut direct = Job::queued(f.state.repositories[0].clone());
    direct.finish(JobStatus::Succeeded, Some(0), "direct job".into());
    direct.raw_log.append("direct raw");
    f.state.runs[0].jobs.push(direct.clone());
    fs::write(f.source().join("tracked.txt"), "staged user change\n").unwrap();
    git_args(&f.source(), &["add", "tracked.txt"]);
    fs::write(f.source().join("tracked.txt"), "unstaged user change\n").unwrap();
    fs::write(f.source().join("user-untracked"), "keep\n").unwrap();
    let head = git_args(&f.source(), &["rev-parse", "HEAD"]);
    let index = fs::read(f.source().join(".git/index")).unwrap();
    assert!(!f.state.remove_from_history(1));
    assert_eq!(f.state.clear_history(), 0);
    let summary = discard(&mut f, Scope::Convoy(1)).await;
    assert_eq!(summary.discarded, 2);
    assert!(summary.failures.is_empty());
    assert!(!first.path.exists() && !second.path.exists());
    assert_eq!(
        serde_json::to_value(&f.state.runs[0].jobs[2]).unwrap(),
        serde_json::to_value(&direct).unwrap()
    );
    assert_eq!(f.state.runs[0].jobs[2].raw_log.text, "direct raw");
    assert_eq!(f.state.runs[0].jobs[1].raw_log.text, "unchanged raw output");
    assert_eq!(f.state.runs[0].jobs[1].status, JobStatus::Failed);
    assert_eq!(fs::read(f.source().join(".git/index")).unwrap(), index);
    assert_eq!(git_args(&f.source(), &["rev-parse", "HEAD"]), head);
    assert_eq!(
        fs::read_to_string(f.source().join("tracked.txt")).unwrap(),
        "unstaged user change\n"
    );
    assert_eq!(
        fs::read_to_string(f.source().join("user-untracked")).unwrap(),
        "keep\n"
    );
    assert!(
        f.state.runs[0].jobs[0]
            .log
            .text
            .contains("[CodeConvoy] Convoy discard completed: 2 discarded, 0 failed")
    );
    assert!(!f.state.runs[0].history_protected());
    let loaded = f.store.load().unwrap();
    assert!(
        loaded.runs[0].jobs[..2]
            .iter()
            .all(|j| j.resolution == ResultResolution::Discarded)
    );
    assert!(loaded.runs[0].history_protected()); // restart must verify physical cleanup
    assert!(f.state.remove_from_history(1));
    assert_eq!(f.state.next_run, 2);
}

#[tokio::test]
async fn bulk_discard_all_preserves_active_convoys_resolved_results_and_unrelated_worktrees() {
    let mut f = Fixture::new().await;
    let second = add_result(&mut f, 2).await;
    let active_result = add_result(&mut f, 3).await;
    let applied = add_result(&mut f, 4).await;
    let discarded = add_result(&mut f, 5).await;
    f.state.runs[3].jobs[0].resolution = ResultResolution::Applied;
    f.state.runs[4].jobs[0].resolution = ResultResolution::Discarded;
    let mut queued = Job::queued(f.state.repositories[0].clone());
    queued.execution_mode = ExecutionMode::IsolatedWorktree;
    f.state.runs[2].jobs.push(queued);
    let untouched = serde_json::to_value(&f.state.runs[2..]).unwrap();
    let foreign = f.root.join("user-worktree");
    git_cmd(
        &f.source(),
        &[
            "worktree".as_ref(),
            "add".as_ref(),
            "--detach".as_ref(),
            &git::path_argument(&foreign),
        ],
    );
    fs::write(foreign.join("user-file"), "user work").unwrap();
    assert!(Plan::new(&f.state, Scope::Convoy(3)).is_empty());
    let plan = Plan::new(&f.state, Scope::All);
    assert_eq!((plan.len(), plan.convoys()), (2, 2));
    let summary = execute(&mut f, plan).await;
    assert_eq!(summary.discarded, 2);
    assert!(summary.failures.is_empty());
    assert!(!second.path.exists());
    assert!(active_result.path.exists() && applied.path.exists() && discarded.path.exists());
    assert_eq!(serde_json::to_value(&f.state.runs[2..]).unwrap(), untouched);
    assert_eq!(
        fs::read_to_string(foreign.join("user-file")).unwrap(),
        "user work"
    );
    assert_eq!(f.state.clear_history(), 2);
    assert_eq!(f.state.runs.len(), 3);
}

#[tokio::test]
async fn bulk_discard_partial_ownership_failure_is_retryable_and_preserves_success_and_restart() {
    let mut f = Fixture::new().await;
    let good = add_result(&mut f, 1).await;
    fs::write(f.admin().join("locked"), "foreign owner\n").unwrap();
    let summary = discard(&mut f, Scope::Convoy(1)).await;
    assert_eq!((summary.discarded, summary.failures.len()), (1, 1));
    assert!(summary.label(Scope::Convoy(1)).contains("partially failed"));
    assert!(!good.path.exists());
    assert!(f.metadata().path.exists());
    assert_eq!(
        f.state.runs[0].jobs[0].result_availability,
        ResultAvailability::CleanupFailed
    );
    assert_eq!(
        f.state.runs[0].jobs[0].resolution,
        ResultResolution::DiscardPending
    );
    assert!(!f.state.remove_from_history(1));
    f.reload();
    assert_eq!(
        f.state.runs[0].jobs[1].resolution,
        ResultResolution::Discarded
    );
    assert_eq!(
        f.state.runs[0].jobs[0].result_availability,
        ResultAvailability::CleanupFailed
    );
    fs::write(f.admin().join("locked"), "CodeConvoy retained result\n").unwrap();
    let summary = discard(&mut f, Scope::Convoy(1)).await;
    assert_eq!(summary.discarded, 1);
    assert!(summary.failures.is_empty());
}

#[tokio::test]
async fn bulk_discard_revalidates_frozen_plan_and_never_expands_it() {
    for change in ["active", "identity", "applied", "registered", "uncertain"] {
        let mut f = Fixture::new().await;
        let plan = Plan::new(&f.state, Scope::All);
        match change {
            "active" => f.state.runs[0]
                .jobs
                .push(Job::queued(f.state.repositories[0].clone())),
            "identity" => f.state.runs[0].jobs[0].worktree.as_mut().unwrap().run = 999,
            "applied" => f.state.runs[0].jobs[0].resolution = ResultResolution::Applied,
            "registered" => f.state.repositories.push(Repository {
                path: f.metadata().path,
                name: "registered retained tree".into(),
            }),
            _ => f.state.runs[0].jobs[0].resolution = ResultResolution::ApplyPending,
        }
        let later = add_result(&mut f, 2).await;
        let summary = execute(&mut f, plan).await;
        assert_eq!(
            (summary.discarded, summary.failures.len()),
            (0, 1),
            "{change}"
        );
        assert!(
            f.metadata().path.exists() && later.path.exists(),
            "{change}"
        );
        assert_eq!(
            fs::read_to_string(f.source().join("tracked.txt")).unwrap(),
            "base\n"
        );
    }
}

#[tokio::test]
async fn bulk_discard_does_not_cancel_conflicting_active_work() {
    let mut f = Fixture::new().await;
    f.peer().await;
    let summary = discard(&mut f, Scope::All).await;
    assert_eq!((summary.discarded, summary.failures.len()), (0, 1));
    assert!(summary.failures[0].contains("in use"));
    assert!(f.manager.is_active(2));
    assert!(f.metadata().path.exists());
    f.stop_peer().await;
    assert_eq!(discard(&mut f, Scope::All).await.discarded, 1);
}

#[tokio::test]
async fn bulk_discard_save_failure_keeps_intent_and_history_protected() {
    let mut f = Fixture::new().await;
    let plan = Plan::new(&f.state, Scope::All);
    let mut batch = Batch::new(plan, &mut f.state);
    let op = batch.begin_next(&f.store, &mut f.state).unwrap();
    let completion = op.execute(&f.manager.lifecycle()).await;
    let saved = f.store.directory().join("state.json");
    fs::rename(&saved, f.store.directory().join("saved-intent.json")).unwrap();
    fs::create_dir(&saved).unwrap(); // deterministic replace failure, even as root
    batch.finish(&f.store, &mut f.state, &op, completion);
    assert_eq!(
        (batch.summary.discarded, batch.summary.failures.len()),
        (0, 1)
    );
    assert!(!f.metadata().path.exists());
    assert_eq!(
        f.state.runs[0].jobs[0].resolution,
        ResultResolution::DiscardPending
    );
    assert!(!f.state.remove_from_history(1));
    fs::remove_dir(&saved).unwrap();
    fs::rename(f.store.directory().join("saved-intent.json"), &saved).unwrap();
    f.state = f.store.load().unwrap();
    assert!(f.state.runs[0].history_protected());
    f.inspect(false).await;
    assert_eq!(
        f.state.runs[0].jobs[0].resolution,
        ResultResolution::Discarded
    );
    assert!(f.state.remove_from_history(1));
}
