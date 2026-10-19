#![cfg(feature = "test-support")]
#![allow(clippy::unwrap_used)]
//! Real detached launches of a probe executable standing in for a Git client.
//! No installed Git client is required or started.
#[path = "support/executable.rs"]
mod executable;

use codeconvoy::git_client::{self, Client, Error, Preferences};
use std::{
    fs,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

fn probe(root: &Path, behavior: &str) -> PathBuf {
    let directory = root.join("Git clients ü 日本語");
    fs::create_dir_all(&directory).unwrap();
    let path = directory.join(format!(
        "git-client-probe{behavior}{}",
        std::env::consts::EXE_SUFFIX
    ));
    executable::install(&path);
    path
}

fn record(probe: &Path) -> Option<serde_json::Value> {
    let bytes = fs::read(probe.with_extension("json")).ok()?;
    Some(serde_json::from_slice(&bytes).unwrap())
}

fn repository(root: &Path) -> PathBuf {
    // Valid on every platform, and harmful if anything ever reached a shell.
    let path = root.join("Repo with spaces ü 日本語 $(touch pwned) ; & 'q' %PATH%");
    fs::create_dir(&path).unwrap();
    path
}

fn preferences(client: Client, executable: &Path, arguments: &[&str]) -> Preferences {
    Preferences {
        client: Some(client),
        executable: executable.to_str().unwrap().into(),
        arguments: arguments.iter().map(|a| (*a).to_owned()).collect(),
    }
}

fn arguments(record: &serde_json::Value) -> Vec<String> {
    serde_json::from_value(record["arguments"].clone()).unwrap()
}

#[tokio::test]
async fn custom_arguments_reach_the_client_unchanged_and_separately() {
    let temp = executable::directory();
    let probe = probe(temp.path(), "");
    let repository = repository(temp.path());
    let target = repository.to_str().unwrap();
    git_client::open(
        preferences(
            Client::Custom,
            &probe,
            &["--path={repo}", "{repo}", "two words", "{repo}{repo}"],
        ),
        repository.clone(),
    )
    .await
    .unwrap();
    let record = record(&probe).unwrap();
    assert_eq!(
        arguments(&record),
        [
            format!("--path={target}"),
            target.to_owned(),
            "two words".to_owned(),
            format!("{target}{target}"),
        ]
    );
    // The client starts in the requested directory, not CodeConvoy's.
    let directory: PathBuf = serde_json::from_value(record["directory"].clone()).unwrap();
    assert_eq!(
        directory.canonicalize().unwrap(),
        repository.canonicalize().unwrap()
    );
    assert!(!repository.join("pwned").exists());
    assert_eq!(fs::read_dir(&repository).unwrap().count(), 0);
}

#[tokio::test]
async fn presets_pass_their_verified_arguments_to_a_configured_launcher() {
    let temp = executable::directory();
    let repository = repository(temp.path());
    let target = repository.to_str().unwrap().to_owned();
    let mut cases = vec![(Client::SublimeMerge, vec![target.clone()])];
    if !cfg!(target_os = "macos") {
        cases.push((Client::AngKorGit, vec!["open".into(), target.clone()]));
        cases.push((Client::SourceGit, vec![target.clone()]));
    }
    if cfg!(windows) {
        cases.push((Client::Fork, vec![target.clone()]));
    }
    if !cfg!(target_os = "macos") {
        cases.push((Client::GitKraken, vec![target.clone()]));
    }
    for (index, (client, expected)) in cases.into_iter().enumerate() {
        let probe = probe(&temp.path().join(index.to_string()), "");
        git_client::open(preferences(client, &probe, &[]), repository.clone())
            .await
            .unwrap();
        assert_eq!(arguments(&record(&probe).unwrap()), expected, "{client:?}");
    }
}

#[tokio::test]
async fn a_launcher_failure_is_reported_with_its_exit_status() {
    let temp = executable::directory();
    let probe = probe(temp.path(), "-fail");
    let repository = repository(temp.path());
    let error = git_client::open(preferences(Client::Custom, &probe, &["{repo}"]), repository)
        .await
        .unwrap_err();
    assert!(
        matches!(&error, Error::Exited { client: Client::Custom, status } if status.contains('3')),
        "{error}"
    );
    assert!(record(&probe).is_some());
}

#[tokio::test]
async fn a_running_client_is_left_running_and_reported_as_opened() {
    let temp = executable::directory();
    let probe = probe(temp.path(), "-wait");
    let repository = repository(temp.path());
    let started = Instant::now();
    git_client::open(
        preferences(Client::Custom, &probe, &["{repo}"]),
        repository.clone(),
    )
    .await
    .unwrap();
    // The observation window elapsed while the client kept running.
    let elapsed = started.elapsed();
    assert!(elapsed >= Duration::from_secs(2), "{elapsed:?}");
    let done = probe.with_extension("done");
    assert!(
        !done.exists(),
        "open returned while the client was still running"
    );
    assert_eq!(
        arguments(&record(&probe).unwrap()),
        [repository.to_str().unwrap()]
    );
    // Let the detached client finish before its temporary directory is removed.
    let deadline = Instant::now() + Duration::from_secs(10);
    while !done.exists() {
        assert!(Instant::now() < deadline, "detached client did not finish");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

#[tokio::test]
async fn missing_targets_and_executables_start_nothing() {
    let temp = executable::directory();
    let probe = probe(temp.path(), "");
    let repository = repository(temp.path());
    let deleted = temp.path().join("deleted worktree");
    let error = git_client::open(
        preferences(Client::Custom, &probe, &["{repo}"]),
        deleted.clone(),
    )
    .await
    .unwrap_err();
    assert_eq!(
        error,
        Error::Directory {
            path: deleted,
            problem: "the directory no longer exists."
        }
    );
    assert!(record(&probe).is_none());

    let missing = temp.path().join("uninstalled client");
    let error = git_client::open(
        preferences(Client::Custom, &missing, &["{repo}"]),
        repository.clone(),
    )
    .await
    .unwrap_err();
    assert!(
        matches!(
            &error,
            Error::Executable {
                problem: "does not exist.",
                ..
            }
        ),
        "{error}"
    );
    assert_eq!(
        git_client::open(Preferences::default(), repository).await,
        Err(Error::NotConfigured)
    );
}
