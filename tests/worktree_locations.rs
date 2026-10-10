#![cfg(feature = "test-support")]
#![allow(clippy::unwrap_used)]
use codeconvoy::{
    domain::*, git, persistence::Store, process::Cancellation, runner::RunManager, worktrees,
};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::AtomicBool,
};

fn git_cmd(path: &Path, args: &[&str]) {
    let out = Command::new("git")
        .arg("-C")
        .arg(path)
        .args([
            "-c",
            "commit.gpgsign=false",
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=fixture@example.invalid",
        ])
        .args(args)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}
async fn repository(root: &Path, name: &str) -> Repository {
    let path = root.join(name);
    fs::create_dir(&path).unwrap();
    git_cmd(&path, &["init", "--quiet"]);
    // Keep fixture bytes independent of Git for Windows' global CRLF policy.
    git_cmd(&path, &["config", "core.autocrlf", "false"]);
    fs::write(path.join("tracked"), "base\n").unwrap();
    git_cmd(&path, &["add", "tracked"]);
    git_cmd(&path, &["commit", "-qm", "base"]);
    git::register(&path).await.unwrap()
}
async fn reserve(
    storage: &Path,
    base: Option<&Path>,
    repo: &Repository,
    run: u64,
) -> WorktreeMetadata {
    let summary = git::status(&repo.path).await.unwrap().summary;
    let reservation = worktrees::reserve_at(
        storage,
        base,
        run,
        0,
        repo.clone(),
        summary.common_dir.unwrap(),
        summary.head.unwrap(),
    )
    .unwrap();
    reservation.preparation.unwrap();
    reservation.metadata
}
fn record(storage: &Path, m: &WorktreeMetadata) -> PathBuf {
    storage.join(".locations").join(format!(
        "{}.json",
        m.path
            .parent()
            .unwrap()
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
    ))
}
fn state(repo: Repository, m: WorktreeMetadata, base: Option<PathBuf>) -> AppState {
    let mut job = Job::queued(repo.clone());
    job.execution_mode = ExecutionMode::IsolatedWorktree;
    job.worktree = Some(m.clone());
    job.finish(JobStatus::Succeeded, Some(0), String::new());
    AppState {
        worktree_base: base,
        repositories: vec![repo],
        next_run: m.run + 1,
        runs: vec![Run {
            id: m.run,
            created_at: 0,
            provenance: None,
            task: TaskConfig {
                execution_mode: ExecutionMode::IsolatedWorktree,
                ..Default::default()
            },
            jobs: vec![job],
        }],
        ..Default::default()
    }
}

#[test]
fn preference_roundtrips_and_old_state_keeps_default() {
    let old: AppState = serde_json::from_str(include_str!("fixtures/state-v0.1.json")).unwrap();
    assert_eq!(old.worktree_base, None);
    let temp = tempfile::tempdir().unwrap();
    let store = Store::open(temp.path()).unwrap();
    let mut state = AppState {
        worktree_base: Some(temp.path().join("custom ü 日本語")),
        ..Default::default()
    };
    store.save(&state).unwrap();
    assert_eq!(store.load().unwrap().worktree_base, state.worktree_base);
    state.worktree_base = None;
    store.save(&state).unwrap();
    assert_eq!(store.load().unwrap().worktree_base, None);
    assert!(
        !fs::read_to_string(temp.path().join("state.json"))
            .unwrap()
            .contains("worktree_base")
    );
}

#[tokio::test]
async fn default_and_missing_custom_base_resolve_without_relocating_anything() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    let repo = repository(&root, "repo").await;
    let storage = root.join("state/worktrees");
    let custom = root.join("missing/with spaces ü 日本語");
    assert_eq!(worktrees::location::validate_base(&custom).unwrap(), custom);
    assert!(!custom.exists());
    let original = reserve(&storage, None, &repo, 1).await;
    assert_eq!(original.path.parent().unwrap().parent().unwrap(), storage);
    let external = reserve(&storage, Some(&custom), &repo, 2).await;
    assert_eq!(external.path.parent().unwrap().parent().unwrap(), custom);
    assert!(record(&storage, &external).exists());
    assert!(original.path.parent().unwrap().join("owner.json").exists());
    for m in [&original, &external] {
        worktrees::create(m, &Cancellation::default(), &AtomicBool::new(true))
            .await
            .unwrap();
        assert_eq!(
            fs::read_to_string(m.path.join("tracked")).unwrap(),
            "base\n"
        );
    }
    let orphans = worktrees::recovery::orphans(&storage, &Default::default()).unwrap();
    assert_eq!(orphans.len(), 2);
    assert!(orphans.contains(&external.path.parent().unwrap().to_owned()));
}

#[tokio::test]
async fn invalid_locations_and_nested_attempts_fail_before_git_or_fallback() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    let repo = repository(&root, "repo").await;
    let summary = git::status(&repo.path).await.unwrap().summary;
    let storage = root.join("default");
    let file = root.join("file");
    fs::write(&file, "keep").unwrap();
    let reserved = reserve(&storage, None, &repo, 1).await;
    // PathBuf::join removes dot components on Windows verbatim paths. Preserve
    // the original spelling so these cases actually reach traversal validation.
    let raw_path = |suffix: &str| {
        let mut path = root.as_os_str().to_owned();
        path.push(std::path::MAIN_SEPARATOR_STR);
        path.push(suffix.replace('/', std::path::MAIN_SEPARATOR_STR));
        PathBuf::from(path)
    };
    for path in [
        PathBuf::new(),
        "relative".into(),
        raw_path("missing/../escape"),
        raw_path("./dot"),
        root.join("nul\0path"),
        file.clone(),
        file.join("child"),
        repo.path.join("nested/new"),
        reserved.path.join("inside"),
        storage.join(".locations"),
    ] {
        let result = worktrees::reserve_at(
            &storage,
            Some(&path),
            2,
            0,
            repo.clone(),
            summary.common_dir.clone().unwrap(),
            summary.head.clone().unwrap(),
        );
        assert!(result.is_err(), "accepted {}", path.display());
    }
    assert_eq!(fs::read_to_string(file).unwrap(), "keep");
    assert_eq!(fs::read_dir(&storage).unwrap().count(), 1);
    assert!(!root.join("missing").exists());
    assert!(!repo.path.join("nested").exists());
    assert!(!reserved.path.exists());
}

#[tokio::test]
async fn concurrent_repositories_and_convoys_get_unique_attempts() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    let first = repository(&root, "one").await;
    let second = repository(&root, "two").await;
    let mut tasks = Vec::new();
    for n in 0..32 {
        let repo = if n % 2 == 0 {
            first.clone()
        } else {
            second.clone()
        };
        let summary = git::status(&repo.path).await.unwrap().summary;
        let storage = root.join("default");
        let base = root.join("shared base");
        tasks.push(tokio::task::spawn_blocking(move || {
            worktrees::reserve_at(
                &storage,
                Some(&base),
                n % 4,
                0,
                repo,
                summary.common_dir.unwrap(),
                summary.head.unwrap(),
            )
            .unwrap()
        }));
    }
    let mut paths = std::collections::HashSet::new();
    for task in tasks {
        let reservation = task.await.unwrap();
        reservation.preparation.unwrap();
        assert!(paths.insert(reservation.metadata.path));
    }
    assert_eq!(paths.len(), 32);
    assert_eq!(
        fs::read_dir(root.join("default/.locations"))
            .unwrap()
            .count(),
        32
    );
}

#[tokio::test]
async fn cleanup_after_setting_change_and_restart_uses_only_original_location() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    let repo = repository(&root, "repo").await;
    let store = Store::open(&root.join("state")).unwrap();
    let storage = store.directory().join("worktrees");
    let original_base = root.join("original");
    let m = reserve(&storage, Some(&original_base), &repo, 1).await;
    worktrees::create(&m, &Cancellation::default(), &AtomicBool::new(true))
        .await
        .unwrap();
    fs::write(m.path.join("tracked"), "retained edits\n").unwrap();
    let new_base = root.join("new");
    let peer = reserve(&storage, Some(&new_base), &repo, 2).await;
    worktrees::create(&peer, &Cancellation::default(), &AtomicBool::new(true))
        .await
        .unwrap();
    let original_index = fs::read(repo.path.join(".git/index")).unwrap();
    let mut state = state(repo.clone(), m.clone(), Some(new_base.clone()));
    store.save(&state).unwrap();
    state = store.load().unwrap();
    let (tx, _rx) = tokio::sync::mpsc::channel(256);
    let mut manager = RunManager::with_worktree_directory(2, tx, storage.clone());
    manager.set_worktree_base(state.worktree_base.clone());
    let report = manager
        .lifecycle()
        .inspect(1, 0, &state.runs[0].jobs[0], true)
        .await;
    assert_eq!(
        report.availability,
        ResultAvailability::Available,
        "{}",
        report.detail
    );
    store
        .cleanup_result(&mut state, &manager, 1, 0)
        .await
        .unwrap();
    assert!(!m.path.exists());
    assert!(peer.path.exists());
    assert!(record(&storage, &m).exists());
    assert_eq!(
        fs::read_to_string(repo.path.join("tracked")).unwrap(),
        "base\n"
    );
    assert_eq!(
        fs::read(repo.path.join(".git/index")).unwrap(),
        original_index
    );
    assert!(
        worktrees::recovery::orphans(&storage, &[peer.path.clone()].into())
            .unwrap()
            .is_empty()
    );
    manager.shutdown();
    (&mut manager.join).await.unwrap();
}

#[tokio::test]
async fn custom_cleanup_requires_store_record_and_exact_manifest_not_current_setting() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    let repo = repository(&root, "repo").await;
    let store = Store::open(&root.join("state")).unwrap();
    let storage = store.directory().join("worktrees");
    let base = root.join("external");
    let m = reserve(&storage, Some(&base), &repo, 1).await;
    worktrees::create(&m, &Cancellation::default(), &AtomicBool::new(true))
        .await
        .unwrap();
    let mut state = state(repo, m.clone(), Some(base));
    let (tx, _rx) = tokio::sync::mpsc::channel(256);
    let mut manager = RunManager::with_worktree_directory(2, tx, storage.clone());
    manager.set_worktree_base(state.worktree_base.clone());
    let binding = record(&storage, &m);
    let bytes = fs::read(&binding).unwrap();
    fs::remove_file(&binding).unwrap();
    assert!(
        store
            .cleanup_result(&mut state, &manager, 1, 0)
            .await
            .is_err()
    );
    assert!(m.path.exists());
    let mut tampered = m.clone();
    tampered.run += 1;
    fs::write(&binding, serde_json::to_vec(&tampered).unwrap()).unwrap();
    assert!(
        store
            .cleanup_result(&mut state, &manager, 1, 0)
            .await
            .is_err()
    );
    assert!(m.path.exists());
    fs::write(&binding, bytes).unwrap();
    store
        .cleanup_result(&mut state, &manager, 1, 0)
        .await
        .unwrap();
    assert!(!m.path.exists());
    manager.shutdown();
    (&mut manager.join).await.unwrap();
}

#[cfg(unix)]
#[tokio::test]
async fn links_permissions_and_replaced_base_fail_closed() {
    use std::os::unix::fs::{PermissionsExt, symlink};
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    let repo = repository(&root, "repo").await;
    let store = Store::open(&root.join("state")).unwrap();
    let storage = store.directory().join("worktrees");
    let base = root.join("external");
    let m = reserve(&storage, Some(&base), &repo, 1).await;
    worktrees::create(&m, &Cancellation::default(), &AtomicBool::new(true))
        .await
        .unwrap();
    let link = root.join("link");
    symlink(&base, &link).unwrap();
    assert!(worktrees::location::validate_base(&link).is_err());
    fs::remove_file(&link).unwrap();
    symlink(root.join("missing"), &link).unwrap();
    assert!(worktrees::location::validate_base(&link).is_err());
    assert!(!root.join("missing").exists());
    let readonly = root.join("readonly");
    fs::create_dir(&readonly).unwrap();
    fs::set_permissions(&readonly, fs::Permissions::from_mode(0o500)).unwrap();
    let summary = git::status(&repo.path).await.unwrap().summary;
    // Root/admin runners can bypass permissions, so only assert where writes fail.
    if tempfile::NamedTempFile::new_in(&readonly).is_err() {
        assert!(
            worktrees::reserve_at(
                &storage,
                Some(&readonly),
                2,
                0,
                repo.clone(),
                summary.common_dir.unwrap(),
                summary.head.unwrap()
            )
            .is_err()
        );
    }
    fs::set_permissions(&readonly, fs::Permissions::from_mode(0o700)).unwrap();
    let moved = root.join("moved");
    fs::rename(&base, &moved).unwrap();
    symlink(&moved, &base).unwrap();
    let mut state = state(repo, m.clone(), None);
    let (tx, _rx) = tokio::sync::mpsc::channel(256);
    let mut manager = RunManager::with_worktree_directory(2, tx, storage);
    assert!(
        store
            .cleanup_result(&mut state, &manager, 1, 0)
            .await
            .is_err()
    );
    assert!(m.path.join("tracked").exists());
    manager.shutdown();
    (&mut manager.join).await.unwrap();
}
