#![allow(clippy::unwrap_used)]
use super::*;
use crate::domain::{AppState, Job, Repository, WorktreeResult};
use std::sync::{Mutex, mpsc};

fn run(statuses: &[JobStatus]) -> Run {
    Run {
        id: 17,
        provenance: None,
        created_at: 0,
        task: Default::default(),
        jobs: statuses
            .iter()
            .enumerate()
            .map(|(i, status)| {
                let mut job = Job::queued(Repository {
                    name: format!("repo{i}"),
                    path: format!("/repo{i}").into(),
                });
                job.status = *status;
                job.review = Some(Ok(Default::default()));
                job
            })
            .collect(),
    }
}

#[test]
fn preferences_defaults_partial_and_old_state_round_trip() {
    let defaults = Preferences::default();
    assert!(
        defaults.enabled
            && defaults.success
            && defaults.failure
            && defaults.review
            && defaults.suppress_foreground
    );
    assert!(!defaults.cancellation);
    let old: AppState = serde_json::from_str(r#"{"version":1,"next_run":42}"#).unwrap();
    assert_eq!(old.notifications, defaults);
    let partial: AppState =
        serde_json::from_str(r#"{"notifications":{"enabled":false,"cancellation":true}}"#).unwrap();
    assert_eq!(
        partial.notifications,
        Preferences {
            enabled: false,
            cancellation: true,
            ..defaults
        }
    );
    let temp = tempfile::tempdir().unwrap();
    let store = crate::persistence::Store::open(temp.path()).unwrap();
    store.save(&partial).unwrap();
    assert_eq!(store.load().unwrap().notifications, partial.notifications);
}

#[test]
fn every_filter_and_focus_combination_obeys_priority_event() {
    for mask in 0..64 {
        let p = Preferences {
            enabled: mask & 1 != 0,
            success: mask & 2 != 0,
            failure: mask & 4 != 0,
            cancellation: mask & 8 != 0,
            review: mask & 16 != 0,
            suppress_foreground: mask & 32 != 0,
        };
        for (outcome, filter) in [
            (Outcome::Success, p.success),
            (Outcome::Failure, p.failure),
            (Outcome::Cancellation, p.cancellation),
            (Outcome::Review, p.review),
        ] {
            for focused in [false, true] {
                assert_eq!(
                    p.allows(outcome, focused),
                    p.enabled
                        && filter
                        && (outcome == Outcome::Failure || !focused || !p.suppress_foreground)
                );
            }
        }
    }
    // Disabling the highest-priority event must not fall through to success.
    let mixed = Completion::classify(&run(&[JobStatus::Succeeded, JobStatus::Failed])).unwrap();
    assert!(
        !Preferences {
            failure: false,
            ..Default::default()
        }
        .allows(mixed.outcome, false)
    );
}

#[test]
fn terminal_classification_and_mixed_results() {
    use JobStatus::*;
    for statuses in [
        vec![],
        vec![Succeeded, Running],
        vec![Failed, Queued],
        vec![Cancelled, Preparing],
    ] {
        assert!(Completion::classify(&run(&statuses)).is_none());
    }
    for (statuses, outcome) in [
        (vec![Succeeded, Succeeded], Outcome::Success),
        (vec![Succeeded, Failed], Outcome::Failure),
        (vec![Cancelled, Failed], Outcome::Failure),
        (vec![Succeeded, Cancelled], Outcome::Cancellation),
        (vec![Cancelled], Outcome::Cancellation),
    ] {
        assert_eq!(
            Completion::classify(&run(&statuses)).unwrap().outcome,
            outcome
        );
    }
    let mixed = Completion::classify(&run(&[Succeeded, Succeeded, Succeeded, Failed])).unwrap();
    assert!(mixed.title.contains("#17"));
    assert!(mixed.body.contains("3 successful, 1 failed"));
}

#[test]
fn review_is_orthogonal_and_unknown_is_never_unchanged() {
    let mut run = run(&[JobStatus::Succeeded]);
    run.jobs[0].review = None;
    assert!(Completion::classify(&run).is_none());
    run.jobs[0].review = Some(Err("unavailable".into()));
    let result = Completion::classify(&run).unwrap();
    assert_eq!(result.outcome, Outcome::Review);
    assert!(result.body.contains("could not be checked"));
    run.jobs[0].review = Some(Ok(crate::review::Statistics {
        index_alternatives: 1,
        ..Default::default()
    }));
    assert_eq!(Completion::classify(&run).unwrap().outcome, Outcome::Review);
    run.jobs[0].execution_mode = ExecutionMode::IsolatedWorktree;
    run.jobs[0].review = None;
    run.jobs[0].worktree_result = Some(WorktreeResult {
        observed_this_session: true,
        exists: true,
        changed: Some(true),
    });
    assert_eq!(Completion::classify(&run).unwrap().outcome, Outcome::Review);
    assert_eq!(run.jobs[0].status, JobStatus::Succeeded);
    run.jobs[0].resolution = ResultResolution::Applied;
    assert_eq!(
        Completion::classify(&run).unwrap().outcome,
        Outcome::Success
    );
    run.jobs[0].resolution = ResultResolution::Unresolved;
    run.jobs[0]
        .worktree_result
        .as_mut()
        .unwrap()
        .observed_this_session = false;
    assert!(Completion::classify(&run).is_none()); // persisted observation is not evidence
    run.jobs[0].status = JobStatus::Failed;
    assert_eq!(
        Completion::classify(&run).unwrap().outcome,
        Outcome::Failure
    );
}

#[test]
fn exactly_once_only_after_all_jobs_finish_and_never_from_restoration() {
    let mut tracker = CompletionTracker::default();
    let mut r = run(&[JobStatus::Succeeded, JobStatus::Running]);
    assert!(tracker.completed(&r).is_none());
    tracker.launched(r.id);
    assert!(tracker.completed(&r).is_none());
    r.jobs[1].status = JobStatus::Succeeded;
    r.jobs[0].log.append("private output");
    r.task.prompt = "private prompt".into();
    let snapshot = tracker.completed(&r).unwrap();
    assert!(snapshot.task.prompt.is_empty());
    assert!(snapshot.jobs[0].log.text.is_empty());
    assert!(tracker.completed(&r).is_none());
    r.jobs[0].review = Some(Err("later inspection".into()));
    assert!(tracker.completed(&r).is_none());
    let mut restored = CompletionTracker::default();
    assert!(restored.completed(&r).is_none());
    let mut state = AppState {
        runs: vec![run(&[JobStatus::Running])],
        ..Default::default()
    };
    state.recover_interrupted();
    assert!(restored.completed(&state.runs[0]).is_none());
}

struct MockBackend {
    calls: Arc<Mutex<Vec<Completion>>>,
    fail: bool,
}
impl Backend for MockBackend {
    fn deliver(&self, completion: Completion, events: Events) -> Delivery {
        let calls = self.calls.clone();
        let fail = self.fail;
        Box::pin(async move {
            calls.lock().unwrap().push(completion.clone());
            if fail {
                return Err("notification permission denied".into());
            }
            events.emit(Event::Activated(completion.run));
            Ok(())
        })
    }
}

async fn receive(rx: &mpsc::Receiver<Event>) -> Event {
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            if let Ok(event) = rx.try_recv() {
                return event;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap()
}

#[tokio::test]
async fn mock_backend_failure_is_visible_and_does_not_retry_or_affect_other_convoys() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let (tx, rx) = mpsc::channel();
    let mut service = Service::new(
        Arc::new(MockBackend {
            calls: calls.clone(),
            fail: true,
        }),
        Events::new(move |event| {
            tx.send(event).unwrap();
        }),
    );
    for id in [17, 18] {
        let mut completion = Completion::classify(&run(&[JobStatus::Failed])).unwrap();
        completion.run = id;
        service.submit(&Handle::current(), completion);
        assert_eq!(
            receive(&rx).await,
            Event::Failed(id, "notification permission denied".into())
        );
    }
    assert_eq!(calls.lock().unwrap().len(), 2);
    service.reap();
    assert!(rx.try_recv().is_err());
}

#[tokio::test]
async fn multiple_notification_callbacks_preserve_stable_run_ids() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let (tx, rx) = mpsc::channel();
    let mut service = Service::new(
        Arc::new(MockBackend { calls, fail: false }),
        Events::new(move |event| {
            tx.send(event).unwrap();
        }),
    );
    for id in [700, 9] {
        let mut completion = Completion::classify(&run(&[JobStatus::Succeeded])).unwrap();
        completion.run = id;
        service.submit(&Handle::current(), completion);
        assert_eq!(receive(&rx).await, Event::Activated(id));
    }
}

#[tokio::test]
async fn assessment_survives_history_removal_and_reports_unavailable_repository() {
    let (tx, rx) = mpsc::channel();
    let mut service = Service::new(
        Arc::new(MockBackend {
            calls: Default::default(),
            fail: false,
        }),
        Events::new(move |event| {
            tx.send(event).unwrap();
        }),
    );
    let mut tracker = CompletionTracker::default();
    let mut r = run(&[JobStatus::Succeeded]);
    let temp = tempfile::tempdir().unwrap();
    r.jobs[0].repository.path = temp.path().join("missing");
    r.jobs[0].review = None;
    tracker.launched(r.id);
    service.assess(&Handle::current(), tracker.completed(&r).unwrap());
    drop(r);
    tracker.retain(&[]);
    let Event::Ready(result) = receive(&rx).await else {
        panic!("Expected completion");
    };
    assert_eq!(result.run, 17);
    assert_eq!(result.outcome, Outcome::Review);
    assert!(rx.try_recv().is_err()); // assessment does not bypass policy and send
}

struct WaitingBackend(Arc<std::sync::atomic::AtomicBool>);
impl Backend for WaitingBackend {
    fn deliver(&self, _: Completion, _: Events) -> Delivery {
        let stopped = self.0.clone();
        Box::pin(async move {
            struct Guard(Arc<std::sync::atomic::AtomicBool>);
            impl Drop for Guard {
                fn drop(&mut self) {
                    self.0.store(true, std::sync::atomic::Ordering::SeqCst);
                }
            }
            let _guard = Guard(stopped);
            std::future::pending().await
        })
    }
}

#[tokio::test]
async fn shutdown_cancels_action_listeners_without_waiting_for_user() {
    let stopped = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let mut service = Service::new(
        Arc::new(WaitingBackend(stopped.clone())),
        Events::new(|_| {}),
    );
    service.submit(
        &Handle::current(),
        Completion::classify(&run(&[JobStatus::Failed])).unwrap(),
    );
    for _ in 0..4 {
        tokio::task::yield_now().await;
    }
    service.stop();
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        while !stopped.load(std::sync::atomic::Ordering::SeqCst) {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn direct_git_observation_distinguishes_clean_dirty_staged_and_untracked() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().canonicalize().unwrap();
    let git = |args: &[&str]| {
        let output = std::process::Command::new("git")
            .arg("-c")
            .arg("user.name=Notification Test")
            .arg("-c")
            .arg("user.email=notification@example.invalid")
            .arg("-c")
            .arg("commit.gpgsign=false")
            .args(args)
            .current_dir(&path)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    };
    git(&["init", "--quiet"]);
    std::fs::write(path.join("tracked"), "base\n").unwrap();
    git(&["add", "tracked"]);
    git(&["commit", "--quiet", "-m", "fixture"]);
    let (tx, rx) = mpsc::channel();
    let mut service = Service::new(
        Arc::new(MockBackend {
            calls: Default::default(),
            fail: false,
        }),
        Events::new(move |event| {
            tx.send(event).unwrap();
        }),
    );
    for (index, expected) in [
        Outcome::Success,
        Outcome::Review,
        Outcome::Review,
        Outcome::Review,
    ]
    .into_iter()
    .enumerate()
    {
        match index {
            1 => std::fs::write(path.join("tracked"), "modified\n").unwrap(),
            2 => {
                git(&["add", "tracked"]);
                std::fs::write(path.join("tracked"), "base\n").unwrap();
            }
            3 => {
                git(&["add", "tracked"]);
                std::fs::write(path.join("untracked"), "new\n").unwrap();
            }
            _ => {}
        }
        let before_index = std::fs::read(path.join(".git/index")).unwrap();
        let before_head = std::fs::read(path.join(".git/HEAD")).unwrap();
        let mut r = run(&[JobStatus::Succeeded]);
        r.jobs[0].repository.path = path.clone();
        r.jobs[0].review = None;
        service.assess(&Handle::current(), r);
        let Event::Ready(completion) = receive(&rx).await else {
            panic!("Expected assessment");
        };
        assert_eq!(completion.outcome, expected);
        assert_eq!(
            std::fs::read(path.join(".git/index")).unwrap(),
            before_index
        );
        assert_eq!(std::fs::read(path.join(".git/HEAD")).unwrap(), before_head);
    }
}

struct PanickingBackend;
impl Backend for PanickingBackend {
    fn deliver(&self, _: Completion, _: Events) -> Delivery {
        Box::pin(async { panic!("native transport panic") })
    }
}
#[tokio::test]
async fn native_backend_panics_are_contained() {
    let (tx, rx) = mpsc::channel();
    let mut service = Service::new(
        Arc::new(PanickingBackend),
        Events::new(move |event| {
            tx.send(event).unwrap();
        }),
    );
    service.submit(
        &Handle::current(),
        Completion::classify(&run(&[JobStatus::Failed])).unwrap(),
    );
    let Event::Failed(17, error) = receive(&rx).await else {
        panic!("Expected diagnostic");
    };
    assert!(error.contains("Notification worker failed"));
}
