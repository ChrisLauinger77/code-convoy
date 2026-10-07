#![cfg(feature = "test-support")]
#![allow(clippy::unwrap_used)]
use codeconvoy::{
    agents,
    attachments::Attachment,
    domain::*,
    git,
    runner::{self, Event, RunManager},
};
use sha2::{Digest, Sha256};
use std::{fs, path::Path, process::Command, time::Duration};

async fn repository(root: &Path, name: &str, fixture: bool) -> Repository {
    let path = root.join(name);
    assert!(
        Command::new("git")
            .args(["init", "--quiet"])
            .arg(&path)
            .status()
            .unwrap()
            .success()
    );
    if fixture {
        fs::write(path.join(".git/codeconvoy-workflow.json"), b"{}").unwrap();
    }
    git::register(&path).await.unwrap()
}

fn context(root: &Path) -> Vec<Attachment> {
    let requirements = root.join("requirements.md");
    let screenshot = root.join("screenshot.png");
    fs::write(
        &requirements,
        "Validation specification. Report this exact token: convoy-blue-lantern.\n",
    )
    .unwrap();
    fs::write(&screenshot, include_bytes!("../assets/screenshot.png")).unwrap();
    [requirements, screenshot]
        .iter()
        .map(|path| Attachment::inspect(path).unwrap())
        .collect()
}

async fn execute(task: TaskConfig, repos: Vec<Repository>) -> (Run, Vec<Event>) {
    let prepared = runner::prepare(task, repos).await.unwrap();
    let snapshot = prepared.snapshot(1);
    let (tx, mut rx) = tokio::sync::mpsc::channel(256);
    let mut manager = RunManager::new(2, tx);
    manager
        .start(1, prepared, agents::backend(snapshot.task.agent).unwrap())
        .unwrap();
    let outcome = tokio::time::timeout(Duration::from_secs(180), async {
        let mut events = Vec::new();
        let mut finished = 0;
        while finished < snapshot.jobs.len() {
            let event = rx.recv().await.unwrap();
            if matches!(event, Event::Finished { .. }) {
                finished += 1;
            }
            events.push(event);
        }
        events
    })
    .await;
    manager.shutdown();
    let events = outcome.expect("Workflow timed out; remaining fixture/agent jobs were cancelled");
    for event in &events {
        if let Event::Finished { status, detail, .. } = event {
            assert_eq!(*status, JobStatus::Succeeded, "{detail}");
        }
    }
    (snapshot, events)
}

#[tokio::test]
async fn maintenance_groups_and_image_context_reach_independent_jobs_for_every_backend() {
    for agent in AgentId::ALL {
        let temp = tempfile::tempdir().unwrap();
        let attachments = context(temp.path());
        let repos = vec![
            repository(temp.path(), "homebrew-cask", true).await,
            repository(temp.path(), "scoop-bucket", true).await,
        ];
        let mut state = AppState {
            repositories: repos.clone(),
            ..Default::default()
        };
        state
            .save_group(
                None,
                RepositoryGroup {
                    name: "Package Repositories".into(),
                    repositories: repos.iter().map(|r| r.path.clone()).collect(),
                },
            )
            .unwrap();
        state.save_template(None, TaskTemplate { name: "Packaged application release".into(), prompt: "Review this repository's package definition and README for the attached release requirements. Do not commit or push.".into() }).unwrap();
        let task = TaskConfig {
            execution_mode: ExecutionMode::Direct,
            agent,
            prompt: state.templates[0].prompt.clone(),
            attachments: attachments.clone(),
            concurrency: 2,
            options: [(
                "executable".into(),
                env!("CARGO_BIN_EXE_codeconvoy-test-agent").into(),
            )]
            .into(),
        };
        let (snapshot, events) = execute(task, repos.clone()).await;
        assert_eq!(
            snapshot
                .jobs
                .iter()
                .map(|j| &j.repository)
                .collect::<Vec<_>>(),
            repos.iter().collect::<Vec<_>>()
        );
        let original = serde_json::to_value(&snapshot).unwrap();
        state.groups.clear();
        state.templates.clear();
        assert_eq!(serde_json::to_value(&snapshot).unwrap(), original);
        for (index, repo) in repos.iter().enumerate() {
            let receipt: serde_json::Value = serde_json::from_slice(
                &fs::read(repo.path.join(".git/codeconvoy-received.json")).unwrap(),
            )
            .unwrap();
            assert_eq!(receipt["repository"], repo.name);
            assert!(
                receipt["text"]
                    .as_str()
                    .unwrap()
                    .contains("convoy-blue-lantern")
            );
            assert!(
                receipt["text"]
                    .as_str()
                    .unwrap()
                    .contains(&snapshot.task.prompt)
            );
            assert_eq!(
                receipt["image_sha256"],
                serde_json::json!([Sha256::digest(include_bytes!("../assets/screenshot.png"))
                    .iter()
                    .map(|byte| format!("{byte:02x}"))
                    .collect::<String>()])
            );
            let activity: String = events
                .iter()
                .filter_map(|event| match event {
                    Event::Output { job, text, .. } if *job == index => Some(text.as_str()),
                    _ => None,
                })
                .collect();
            let raw: String = events
                .iter()
                .filter_map(|event| match event {
                    Event::Output { job, raw, .. } if *job == index => Some(raw.as_str()),
                    _ => None,
                })
                .collect();
            assert!(activity.contains(&format!("Fixture result for {}", repo.name)));
            assert!(!activity.contains(&format!("Fixture result for {}", repos[1 - index].name)));
            assert!(raw.contains(&repo.name));
            assert_eq!(git::status(&repo.path).await.unwrap().summary.changed, 0);
        }
    }
}

#[tokio::test]
#[ignore = "Deliberate provider probe: set CODECONVOY_E2E_CODEX to an installed authenticated CLI; read-only disposable repositories"]
async fn installed_codex_receives_text_and_image_in_two_disposable_repositories() {
    let executable = std::env::var("CODECONVOY_E2E_CODEX")
        .expect("Set CODECONVOY_E2E_CODEX explicitly to opt in");
    assert!(Path::new(&executable).is_absolute());
    let temp = tempfile::tempdir().unwrap();
    let repos = vec![
        repository(temp.path(), "attachment-alpha", false).await,
        repository(temp.path(), "attachment-beta", false).await,
    ];
    let task = TaskConfig {
        prompt: "Read-only validation. Do not modify any files or run network tools. From the attached requirements report its exact validation token. From the attached screenshot report the two repository names and the version mentioned in the displayed maintenance task. Also report the basename of your current repository directory. Do not inspect unrelated files. Respond concisely with these facts.".into(),
        attachments: context(temp.path()), concurrency: 2,
        options: [("executable".into(), executable), ("sandbox".into(), "read-only".into())].into(),
        ..Default::default()
    };
    let (_, events) = execute(task, repos.clone()).await;
    for (index, repo) in repos.iter().enumerate() {
        let activity: String = events
            .iter()
            .filter_map(|event| match event {
                Event::Output { job, text, .. } if *job == index => Some(text.as_str()),
                _ => None,
            })
            .collect();
        for expected in [
            "convoy-blue-lantern",
            "homebrew-cask",
            "scoop-bucket",
            "0.1.0",
            &repo.name,
        ] {
            assert!(
                activity.contains(expected),
                "Missing expected validation fact {expected:?} for {}: {activity}",
                repo.name
            );
        }
        println!(
            "Verified read-only text/image context and independent result for {}",
            repo.name
        );
        assert_eq!(git::status(&repo.path).await.unwrap().summary.changed, 0);
    }
}
