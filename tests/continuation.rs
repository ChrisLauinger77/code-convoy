#![cfg(feature = "test-support")]
#![allow(clippy::unwrap_used)]
use codeconvoy::{
    attachments::Attachment,
    continuation::{FollowUp, Provenance, Retry},
    domain::*,
    git,
    persistence::Store,
    runner,
};
use std::{collections::HashSet, fs, path::Path, process::Command};

fn command(path: &Path, args: &[&str]) {
    let out = Command::new("git")
        .arg("-C")
        .arg(path)
        .args(["-c", "commit.gpgsign=false"])
        .args(args)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}
async fn fixture() -> (tempfile::TempDir, AppState) {
    let temp = tempfile::Builder::new()
        .prefix("retry ü ")
        .tempdir()
        .unwrap();
    let path = temp.path().join("repository with spaces");
    fs::create_dir(&path).unwrap();
    command(&path, &["init", "--quiet"]);
    fs::write(path.join("file.txt"), "base\n").unwrap();
    command(&path, &["add", "."]);
    command(
        &path,
        &[
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=f@invalid",
            "commit",
            "-qm",
            "base",
        ],
    );
    let repository = git::register(&path).await.unwrap();
    let attachment = temp.path().join("context.txt");
    fs::write(&attachment, "original context").unwrap();
    let task = TaskConfig {
        prompt: "original task".into(),
        attachments: vec![Attachment::inspect(&attachment).unwrap()],
        options: [
            (
                "executable".into(),
                env!("CARGO_BIN_EXE_codeconvoy-test-agent").into(),
            ),
            ("model".into(), "original-model".into()),
        ]
        .into(),
        concurrency: 3,
        ..Default::default()
    };
    let prepared = runner::prepare(task, vec![repository.clone()])
        .await
        .unwrap();
    let mut run = prepared.snapshot(41);
    run.jobs[0].finish(JobStatus::Failed, Some(7), "original failure".into());
    run.jobs[0].started_at = Some(1);
    run.jobs[0].log.append("original Activity");
    run.jobs[0].raw_log.append("original Raw");
    (
        temp,
        AppState {
            next_run: 42,
            repositories: vec![repository],
            runs: vec![run],
            global_concurrency: 7,
            ..Default::default()
        },
    )
}

#[tokio::test]
async fn eligibility_and_new_identity_preserve_original_evidence_and_settings() {
    let (_temp, mut state) = fixture().await;
    for (status, interrupted, allowed) in [
        (JobStatus::Failed, false, true),
        (JobStatus::Cancelled, false, true),
        (JobStatus::Failed, true, true),
        (JobStatus::Succeeded, false, false),
        (JobStatus::Running, false, false),
        (JobStatus::Preparing, false, false),
        (JobStatus::Queued, false, false),
    ] {
        state.runs[0].jobs[0].status = status;
        state.runs[0].jobs[0].interrupted = interrupted;
        assert_eq!(Retry::from_state(&state, 41, 0).is_ok(), allowed);
    }
    state.runs[0].jobs[0].status = JobStatus::Failed;
    state.draft.prompt = "unrelated draft".into();
    state.draft.options.clear();
    let before = serde_json::to_value(&state).unwrap();
    let (prepared, provenance) = Retry::from_state(&state, 41, 0)
        .unwrap()
        .prepare()
        .await
        .unwrap();
    let mut attempt = prepared.snapshot(42);
    attempt.provenance = Some(provenance.clone());
    assert_eq!(
        provenance,
        Provenance::Retry {
            run: 41,
            job: 0,
            attempt: 2
        }
    );
    assert_eq!(
        serde_json::to_value(&attempt.task).unwrap(),
        before["runs"][0]["task"]
    );
    assert_eq!(attempt.jobs[0].status, JobStatus::Queued);
    assert!(attempt.jobs[0].started_at.is_none() && attempt.jobs[0].finished_at.is_none());
    assert!(attempt.jobs[0].log.text.is_empty() && attempt.jobs[0].raw_log.text.is_empty());
    assert_eq!(serde_json::to_value(&state).unwrap(), before);
    assert_eq!(state.runs[0].jobs[0].log.text, "original Activity");
    assert_eq!(state.runs[0].jobs[0].raw_log.text, "original Raw");
    attempt.jobs[0].finish(JobStatus::Cancelled, None, "stopped retry".into());
    state.runs.push(attempt);
    assert_eq!(
        Retry::from_state(&state, 42, 0).unwrap().provenance,
        Provenance::Retry {
            run: 42,
            job: 0,
            attempt: 3
        }
    );
}

#[tokio::test]
async fn retry_rechecks_direct_baseline_context_executable_and_registration() {
    let (_temp, mut state) = fixture().await;
    let repo = state.repositories[0].path.clone();
    fs::write(repo.join("file.txt"), "current user edit\n").unwrap();
    let (prepared, _) = Retry::from_state(&state, 41, 0)
        .unwrap()
        .prepare()
        .await
        .unwrap();
    assert!(prepared.dirty());
    assert_eq!(
        prepared.repositories[0].state.summary,
        git::status(&repo).await.unwrap().summary
    );
    assert_eq!(
        fs::read_to_string(repo.join("file.txt")).unwrap(),
        "current user edit\n"
    );
    let attachment = state.runs[0].task.attachments[0].path.clone();
    fs::write(&attachment, "changed context").unwrap();
    let error = Retry::from_state(&state, 41, 0)
        .unwrap()
        .prepare()
        .await
        .err()
        .unwrap();
    assert!(format!("{error:#}").contains("Attachment changed since"));
    fs::remove_file(attachment).unwrap();
    let error = Retry::from_state(&state, 41, 0)
        .unwrap()
        .prepare()
        .await
        .err()
        .unwrap();
    assert!(format!("{error:#}").contains("Attachment is missing or unavailable"));
    state.runs[0].task.attachments.clear();
    state.runs[0].task.options.insert(
        "executable".into(),
        repo.join("missing-cli").display().to_string(),
    );
    assert!(
        Retry::from_state(&state, 41, 0)
            .unwrap()
            .prepare()
            .await
            .is_err()
    );
    fs::rename(&repo, repo.with_extension("moved")).unwrap();
    assert!(
        Retry::from_state(&state, 41, 0)
            .unwrap()
            .prepare()
            .await
            .err()
            .unwrap()
            .to_string()
            .contains("repository is unavailable")
    );
    state.repositories.clear();
    assert!(
        Retry::from_state(&state, 41, 0)
            .err()
            .unwrap()
            .to_string()
            .contains("no longer registered")
    );
}

#[tokio::test]
async fn followup_selection_is_independent_of_status_resolution_resources_and_display_names() {
    let (_temp, mut state) = fixture().await;
    let repo = state.repositories[0].clone();
    state.repositories[0].name = "renamed registration".into();
    let duplicate = state.runs[0].jobs[0].clone();
    state.runs[0].jobs.push(duplicate);
    state.runs[0].jobs[0].repository.name = "same label".into();
    state.runs[0].jobs[1].repository.name = "same label".into();
    let missing = Repository {
        name: "same label".into(),
        path: repo.path.join("no longer registered"),
    };
    state.runs[0].jobs.push(Job::queued(missing));
    let unavailable = Repository {
        name: "unavailable".into(),
        path: repo.path.join("missing"),
    };
    state.repositories.push(unavailable.clone());
    state.runs[0].jobs.push(Job::queued(unavailable));
    state.groups.push(RepositoryGroup {
        name: "GNOME Extensions".into(),
        repositories: vec![repo.path.clone()],
    });
    state.templates.push(TaskTemplate {
        name: "Follow-up".into(),
        prompt: "Review remaining issues".into(),
    });
    for mode in [ExecutionMode::Direct, ExecutionMode::IsolatedWorktree] {
        state.runs[0].task.execution_mode = mode;
        for resolution in [
            ResultResolution::Unresolved,
            ResultResolution::Applied,
            ResultResolution::Discarded,
            ResultResolution::ApplyPending,
        ] {
            state.runs[0].jobs[0].resolution = resolution;
            state.runs[0].jobs[0].result_availability = ResultAvailability::Missing;
            let original = serde_json::to_value(&state.runs).unwrap();
            let groups = state.groups.clone();
            let templates = state.templates.clone();
            let followup = FollowUp::from_state(&state, 41, &[0, 1, 2, 3].into())
                .unwrap()
                .check()
                .await;
            assert_eq!(followup.repositories.len(), 1);
            assert_eq!(followup.repositories[0].name, "renamed registration");
            let (selection, message) = followup.populate(&mut state);
            assert_eq!(selection, HashSet::from([repo.path.clone()]));
            assert!(message.contains("no longer registered") && message.contains("unavailable"));
            assert!(state.draft.prompt.is_empty() && state.draft.attachments.is_empty());
            assert_eq!(state.draft.options, state.runs[0].task.options);
            assert_eq!(state.draft.agent, state.runs[0].task.agent);
            assert_eq!(state.draft.execution_mode, mode);
            assert_eq!(state.draft.concurrency, 3);
            assert_eq!(state.global_concurrency, 7);
            assert_eq!(state.groups, groups);
            assert_eq!(state.templates, templates);
            assert_eq!(serde_json::to_value(&state.runs).unwrap(), original);
            assert_eq!(state.next_run, 42);
        }
    }
    assert!(FollowUp::from_state(&state, 41, &HashSet::new()).is_err());
    assert!(FollowUp::from_state(&state, 41, &[9].into()).is_err());
    let followup = FollowUp::from_state(&state, 41, &[0].into())
        .unwrap()
        .check()
        .await;
    state.repositories.clear(); // Registration can disappear while a worker is checking.
    let (selection, message) = followup.populate(&mut state);
    assert!(selection.is_empty() && message.contains("no longer registered"));
}

#[tokio::test]
async fn persisted_provenance_survives_source_removal_and_interrupted_retry_restart() {
    let (temp, mut state) = fixture().await;
    let store = Store::open(&temp.path().join("state")).unwrap();
    let (prepared, provenance) = Retry::from_state(&state, 41, 0)
        .unwrap()
        .prepare()
        .await
        .unwrap();
    let mut attempt = prepared.snapshot(42);
    attempt.provenance = Some(provenance.clone());
    attempt.jobs[0].status = JobStatus::Running;
    state.runs.push(attempt);
    let followup = FollowUp::from_state(&state, 41, &[0].into())
        .unwrap()
        .check()
        .await;
    followup.populate(&mut state);
    state.draft.prompt = "new explicit task".into();
    assert!(state.remove_from_history(41));
    state.next_run = 43;
    store.save(&state).unwrap();
    let loaded = store.load().unwrap();
    assert_eq!(loaded.runs[0].provenance, Some(provenance));
    assert_eq!(loaded.draft_provenance, state.draft_provenance);
    assert_eq!(loaded.runs[0].jobs[0].status, JobStatus::Cancelled);
    assert!(loaded.runs[0].jobs[0].interrupted && loaded.runs[0].jobs[0].retryable());
    assert_eq!(loaded.next_run, 43);
    assert_eq!(loaded.draft.prompt, "new explicit task");
    assert!(
        runner::prepare(loaded.draft, loaded.repositories)
            .await
            .is_ok()
    );
    assert!(!store.directory().join("worktrees").exists());
}

#[tokio::test]
async fn multi_repository_followup_uses_explicit_registered_paths_and_backend_settings() {
    let (_first, mut state) = fixture().await;
    let (_second, other) = fixture().await;
    state.repositories.push(other.repositories[0].clone());
    state.runs[0].jobs.push(other.runs[0].jobs[0].clone());
    for agent in AgentId::ALL {
        state.runs[0].task.agent = agent;
        let before = serde_json::to_value(&state.runs).unwrap();
        let (selection, _) = FollowUp::from_state(&state, 41, &[0, 1].into())
            .unwrap()
            .check()
            .await
            .populate(&mut state);
        assert_eq!(
            selection,
            state.repositories.iter().map(|r| r.path.clone()).collect()
        );
        assert_eq!(state.draft.agent, agent);
        assert_eq!(state.draft.options, state.runs[0].task.options);
        assert_eq!(serde_json::to_value(&state.runs).unwrap(), before);
    }
}

#[tokio::test]
async fn populated_v02_upgrade_retains_libraries_context_options_and_direct_history() {
    let (temp, mut state) = fixture().await;
    state.groups.push(RepositoryGroup {
        name: "Package Repositories".into(),
        repositories: vec![state.repositories[0].path.clone()],
    });
    state.templates.push(TaskTemplate {
        name: "Maintenance".into(),
        prompt: "Inspect compatibility".into(),
    });
    state.draft = state.runs[0].task.clone();
    for agent in AgentId::ALL {
        state.agent_options.insert(
            agent,
            [("model".into(), format!("{agent:?}-preference"))].into(),
        );
    }
    let mut legacy = serde_json::to_value(&state).unwrap();
    legacy["draft"]
        .as_object_mut()
        .unwrap()
        .remove("execution_mode");
    legacy["runs"][0]["task"]
        .as_object_mut()
        .unwrap()
        .remove("execution_mode");
    let job = legacy["runs"][0]["jobs"][0].as_object_mut().unwrap();
    for field in [
        "execution_mode",
        "worktree",
        "worktree_result",
        "result_availability",
        "resolution",
        "resolved_at",
    ] {
        job.remove(field);
    }
    let store = Store::open(&temp.path().join("upgrade")).unwrap();
    fs::write(
        store.directory().join("state.json"),
        serde_json::to_vec(&legacy).unwrap(),
    )
    .unwrap();
    let loaded = store.load().unwrap();
    assert_eq!(loaded.repositories, state.repositories);
    assert_eq!(loaded.groups, state.groups);
    assert_eq!(loaded.templates, state.templates);
    assert_eq!(loaded.agent_options, state.agent_options);
    assert_eq!(loaded.global_concurrency, state.global_concurrency);
    assert_eq!(
        serde_json::to_value(&loaded.draft).unwrap(),
        serde_json::to_value(&state.draft).unwrap()
    );
    assert_eq!(
        serde_json::to_value(&loaded.runs).unwrap(),
        serde_json::to_value(&state.runs).unwrap()
    );
    assert!(loaded.draft_provenance.is_none() && loaded.runs[0].provenance.is_none());
    assert_eq!(loaded.appearance, Appearance::System);
    assert!(loaded.runs[0].jobs[0].worktree.is_none());
    store.save(&loaded).unwrap();
    assert_eq!(
        serde_json::to_value(store.load().unwrap()).unwrap(),
        serde_json::to_value(&loaded).unwrap()
    );
    assert!(!store.directory().join("worktrees").exists());
}
