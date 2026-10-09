#![allow(clippy::unwrap_used)]
use codeconvoy::git;
use std::{path::Path, process::Command};
fn command(path: &Path, args: &[&str]) -> Vec<u8> {
    let out = Command::new("git")
        .arg("-C")
        .arg(path)
        .args(args)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    out.stdout
}
fn repository() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    command(dir.path(), &["init", "--quiet", "--initial-branch=main"]);
    std::fs::write(dir.path().join("tracked.txt"), "base\n").unwrap();
    command(dir.path(), &["add", "."]);
    command(
        dir.path(),
        &[
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=fixture@invalid",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "--quiet",
            "-m",
            "base",
        ],
    );
    dir
}
#[tokio::test]
async fn health_clean_dirty_detached_and_refresh_preserve_head_index_and_files() {
    let dir = repository();
    let path = dir.path().canonicalize().unwrap();
    let head = command(&path, &["rev-parse", "HEAD"]);
    let index = std::fs::read(path.join(".git/index")).unwrap();
    let state = git::status(&path).await.unwrap();
    assert_eq!(state.summary.branch, "main");
    assert_eq!(state.summary.changed, 0);
    std::fs::write(path.join("tracked.txt"), "changed\n").unwrap();
    std::fs::write(path.join("untracked.txt"), "new\n").unwrap();
    let state = git::status(&path).await.unwrap();
    assert_eq!(state.summary.changed, 2);
    assert_eq!(std::fs::read(path.join(".git/index")).unwrap(), index);
    assert_eq!(command(&path, &["rev-parse", "HEAD"]), head);
    assert_eq!(
        std::fs::read_to_string(path.join("tracked.txt")).unwrap(),
        "changed\n"
    );
    assert!(!path.join(".git/index.lock").exists());
    command(&path, &["checkout", "--detach", "--quiet"]);
    assert_eq!(
        git::status(&path).await.unwrap().summary.branch,
        "(detached HEAD)"
    );
}
#[tokio::test]
async fn health_reports_missing_non_git_invalid_and_git_command_errors() {
    let temp = tempfile::tempdir().unwrap();
    assert!(
        git::status(&temp.path().join("missing"))
            .await
            .unwrap_err()
            .to_string()
            .contains("does not exist")
    );
    assert!(
        git::status(&temp.path().canonicalize().unwrap())
            .await
            .is_err()
    );
    let dir = repository();
    let path = dir.path().canonicalize().unwrap();
    std::fs::write(path.join(".git/config"), "[malformed config\n").unwrap();
    let error = git::status(&path).await.unwrap_err().to_string();
    assert!(error.contains("Git inspection failed"));
    let unavailable = tempfile::tempdir().unwrap();
    std::fs::write(
        unavailable.path().join(".git"),
        "gitdir: /missing/codeconvoy-health-fixture\n",
    )
    .unwrap();
    assert!(
        git::status(&unavailable.path().canonicalize().unwrap())
            .await
            .is_err()
    );
}
