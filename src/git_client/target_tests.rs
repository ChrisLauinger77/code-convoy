#![allow(clippy::unwrap_used)]
//! Real Git repositories and linked worktrees in disposable directories.
use super::*;
use crate::domain::{AppState, GitSummary, ResultResolution, Run, TaskConfig, WorktreeMetadata};
use std::process::Command;

fn git(directory: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(["-c", "commit.gpgsign=false", "-c", "user.name=Fixture"])
        .args(["-c", "user.email=fixture@example.invalid"])
        .arg("-C")
        .arg(directory)
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}

/// A canonical repository root with one commit, like a registration.
fn repository(root: &Path, name: &str) -> Repository {
    let path = root.join(name);
    fs::create_dir_all(&path).unwrap();
    git(&path, &["init", "--quiet"]);
    git(&path, &["commit", "--quiet", "--allow-empty", "-m", "base"]);
    let path = path.canonicalize().unwrap();
    Repository {
        name: name.into(),
        path,
    }
}

fn worktree(repository: &Repository, path: &Path) -> PathBuf {
    git(
        &repository.path,
        &[
            "worktree",
            "add",
            "--quiet",
            "--detach",
            path.to_str().unwrap(),
        ],
    );
    path.canonicalize().unwrap()
}

fn common(repository: &Repository) -> PathBuf {
    repository.path.join(".git").canonicalize().unwrap()
}

fn direct(repository: &Repository) -> Job {
    let mut job = Job::queued(repository.clone());
    job.status = JobStatus::Succeeded;
    job.before = Some(GitSummary {
        common_dir: Some(common(repository)),
        branch: "main".into(),
        head: None,
        changed: 0,
    });
    job
}

fn isolated(repository: &Repository, path: &Path, status: JobStatus) -> Job {
    let mut job = direct(repository);
    job.execution_mode = ExecutionMode::IsolatedWorktree;
    job.status = status;
    job.worktree = Some(WorktreeMetadata {
        owner: "test".into(),
        run: 1,
        job: 0,
        repository: repository.clone(),
        common_dir: common(repository),
        path: path.to_owned(),
        base_commit: "0".repeat(40),
        execution_mode: ExecutionMode::IsolatedWorktree,
    });
    job
}

fn problem(result: Result<impl std::fmt::Debug, Error>) -> String {
    match result.unwrap_err() {
        Error::Target { problem, .. } => problem,
        other => panic!("unexpected error {other:?}"),
    }
}

#[test]
fn direct_jobs_open_their_repository_and_isolated_jobs_their_worktree() {
    let temp = tempfile::tempdir().unwrap();
    let repo = repository(temp.path(), "source ü with spaces");
    let tree = worktree(&repo, &temp.path().join("worktree 日本語"));

    let target = Target::job(&direct(&repo)).unwrap();
    assert_eq!(target.kind, Kind::Repository);
    assert_eq!(target.path, repo.path);
    assert_eq!(target.common_dir, Some(common(&repo)));
    assert_eq!(verify(&target), Ok(()));

    let target = Target::job(&isolated(&repo, &tree, JobStatus::Succeeded)).unwrap();
    assert_eq!(target.kind, Kind::Worktree);
    assert_eq!(target.path, tree);
    assert_eq!(verify(&target), Ok(()));
    assert!(target.describe().starts_with("isolated worktree "));

    let registered = Target::repository(&repo);
    assert_eq!(
        (registered.kind, &registered.path),
        (Kind::Repository, &repo.path)
    );
    assert_eq!(verify(&registered), Ok(()));
}

#[test]
fn isolated_jobs_never_fall_back_to_the_original_repository() {
    let temp = tempfile::tempdir().unwrap();
    let repo = repository(temp.path(), "source");
    let tree = temp.path().join("not yet created");
    let mut failed = isolated(&repo, &tree, JobStatus::Failed);
    failed.worktree = None;
    assert!(problem(Target::job(&failed)).contains("original repository is not opened instead"));
    failed.status = JobStatus::Queued;
    assert!(problem(Target::job(&failed)).contains("has not been created yet"));
    for status in [JobStatus::Queued, JobStatus::Preparing] {
        assert!(problem(Target::job(&isolated(&repo, &tree, status))).contains("being prepared"));
    }
    let mut cleaned = isolated(&repo, &tree, JobStatus::Succeeded);
    cleaned.result_availability = ResultAvailability::Cleaned;
    cleaned.resolution = ResultResolution::Discarded;
    assert!(problem(Target::job(&cleaned)).contains("removed by cleanup"));
}

#[test]
fn running_jobs_open_their_recorded_location() {
    let temp = tempfile::tempdir().unwrap();
    let repo = repository(temp.path(), "source");
    let tree = worktree(&repo, &temp.path().join("tree"));
    // Opening never needs a lease: it reads metadata only.
    let target = Target::job(&isolated(&repo, &tree, JobStatus::Running)).unwrap();
    assert_eq!((target.kind, target.path.clone()), (Kind::Worktree, tree));
    let mut active = direct(&repo);
    active.status = JobStatus::Running;
    assert_eq!(Target::job(&active).unwrap().path, repo.path);
}

#[test]
fn multi_repository_convoys_resolve_each_job_independently() {
    let temp = tempfile::tempdir().unwrap();
    let first = repository(temp.path(), "first");
    let second = repository(temp.path(), "second");
    let first_tree = worktree(&first, &temp.path().join("trees/first"));
    let second_tree = worktree(&second, &temp.path().join("trees/second"));
    let mut failed = isolated(&second, &temp.path().join("trees/third"), JobStatus::Failed);
    failed.worktree = None;
    let jobs = [
        isolated(&first, &first_tree, JobStatus::Succeeded),
        isolated(&second, &second_tree, JobStatus::Cancelled),
        failed,
    ];
    let targets: Vec<_> = jobs.iter().map(Target::job).collect();
    assert_eq!(targets[0].as_ref().unwrap().path, first_tree);
    assert_eq!(targets[1].as_ref().unwrap().path, second_tree);
    assert!(
        targets[2].is_err(),
        "no worktree means no target, not a fallback"
    );
    for target in targets.iter().flatten() {
        assert_eq!(verify(target), Ok(()));
    }
}

#[test]
fn historical_runs_keep_their_recorded_location_after_settings_change() {
    let temp = tempfile::tempdir().unwrap();
    let repo = repository(temp.path(), "source");
    let tree = worktree(&repo, &temp.path().join("tree"));
    let mut state = AppState {
        repositories: vec![repo.clone()],
        ..Default::default()
    };
    state.runs.push(Run {
        id: 1,
        provenance: None,
        created_at: 0,
        task: TaskConfig {
            execution_mode: ExecutionMode::IsolatedWorktree,
            ..Default::default()
        },
        jobs: vec![isolated(&repo, &tree, JobStatus::Succeeded)],
    });
    state.runs.push(Run {
        id: 2,
        provenance: None,
        created_at: 0,
        task: TaskConfig::default(),
        jobs: vec![direct(&repo)],
    });
    let before: Vec<_> = state.runs.iter().map(|r| Target::job(&r.jobs[0])).collect();
    // Later configuration: other default mode, another worktree base, and the
    // registration removed. Saved runs still resolve as they executed.
    state.draft.execution_mode = ExecutionMode::Direct;
    state.worktree_base = Some(temp.path().join("other base"));
    state.repositories.clear();
    let reloaded: AppState =
        serde_json::from_value(serde_json::to_value(state.persistence_snapshot()).unwrap())
            .unwrap();
    let after: Vec<_> = reloaded
        .runs
        .iter()
        .map(|r| Target::job(&r.jobs[0]))
        .collect();
    assert_eq!(before, after);
    assert_eq!(after[0].as_ref().unwrap().path, tree);
    assert_eq!(after[1].as_ref().unwrap().path, repo.path);

    // Saves from before isolated execution existed load as Direct.
    let mut legacy = serde_json::to_value(&state.runs[1].jobs[0]).unwrap();
    for field in [
        "execution_mode",
        "worktree",
        "result_availability",
        "resolution",
    ] {
        legacy.as_object_mut().unwrap().remove(field);
    }
    let legacy: Job = serde_json::from_value(legacy).unwrap();
    assert_eq!(Target::job(&legacy).unwrap().path, repo.path);
}

#[test]
fn deleted_worktrees_are_reported_and_never_recreated() {
    let temp = tempfile::tempdir().unwrap();
    let repo = repository(temp.path(), "source");
    let tree = worktree(&repo, &temp.path().join("tree"));
    let target = Target::job(&isolated(&repo, &tree, JobStatus::Succeeded)).unwrap();
    fs::remove_dir_all(&tree).unwrap();
    assert!(problem(verify(&target)).contains("no longer exists"));
    assert!(!tree.exists(), "verification must not recreate anything");
    assert!(repo.path.join(".git").is_dir());
}

#[test]
fn moved_repositories_are_reported_without_opening_the_new_location() {
    let temp = tempfile::tempdir().unwrap();
    let repo = repository(temp.path(), "before move");
    let target = Target::job(&direct(&repo)).unwrap();
    fs::rename(&repo.path, temp.path().join("after move")).unwrap();
    let message = problem(verify(&target));
    assert!(message.contains("register it again"), "{message}");
    assert!(problem(verify(&Target::repository(&repo))).contains("no longer exists"));
}

#[test]
fn invalid_git_metadata_is_rejected() {
    let temp = tempfile::tempdir().unwrap();
    let plain = temp.path().join("plain").canonicalize_or_create();
    let target = Target::repository(&Repository {
        name: "plain".into(),
        path: plain.clone(),
    });
    assert!(problem(verify(&target)).contains(".git is missing"));
    fs::write(plain.join(".git"), "not a link\n").unwrap();
    assert!(problem(verify(&target)).contains("not a Git link"));
    fs::write(plain.join(".git"), "gitdir: /nonexistent/admin\n").unwrap();
    assert!(problem(verify(&target)).contains("administration directory is missing"));
    let file = temp.path().join("file").canonicalize_or_create();
    fs::remove_dir(&file).unwrap();
    fs::write(&file, "").unwrap();
    let file_target = Target::repository(&Repository {
        name: "file".into(),
        path: file,
    });
    assert!(problem(verify(&file_target)).contains("not a directory"));
    let relative = Target::repository(&Repository {
        name: "relative".into(),
        path: "relative/repo".into(),
    });
    assert!(problem(verify(&relative)).contains("not absolute"));
}

#[test]
fn a_worktree_path_holding_another_tree_is_rejected() {
    let temp = tempfile::tempdir().unwrap();
    let repo = repository(temp.path(), "source");
    let other = repository(temp.path(), "other");
    let first = worktree(&repo, &temp.path().join("first"));
    let second = worktree(&repo, &temp.path().join("second"));
    let target = Target::job(&isolated(&repo, &second, JobStatus::Succeeded)).unwrap();
    assert_eq!(verify(&target), Ok(()));

    // Second's checkout now points at first's administration: wrong worktree.
    fs::copy(first.join(".git"), second.join(".git")).unwrap();
    assert!(problem(verify(&target)).contains("belongs to another worktree"));

    // A worktree of a different repository at the recorded path.
    fs::remove_dir_all(&second).unwrap();
    git(&repo.path, &["worktree", "prune"]);
    worktree(&other, &second);
    assert!(problem(verify(&target)).contains("different Git repository"));

    // An ordinary clone at the recorded path is not a linked worktree.
    fs::remove_dir_all(&second).unwrap();
    git(&other.path, &["worktree", "prune"]);
    fs::create_dir(&second).unwrap();
    git(&second, &["init", "--quiet"]);
    assert!(problem(verify(&target)).contains("no longer a linked worktree"));
}

#[test]
fn a_direct_job_detects_a_tree_now_belonging_to_another_repository() {
    let temp = tempfile::tempdir().unwrap();
    let repo = repository(temp.path(), "source");
    let other = repository(temp.path(), "other");
    // Linked worktrees can be registered as repository roots.
    let path = worktree(&repo, &temp.path().join("registered tree"));
    let registered = Repository {
        name: "registered tree".into(),
        path: path.clone(),
    };
    let mut job = direct(&registered);
    job.before.as_mut().unwrap().common_dir = Some(common(&repo));
    let target = Target::job(&job).unwrap();
    assert_eq!(verify(&target), Ok(()));
    fs::remove_dir_all(&path).unwrap();
    git(&repo.path, &["worktree", "prune"]);
    worktree(&other, &path);
    assert!(problem(verify(&target)).contains("different Git repository"));
}

#[cfg(unix)]
#[test]
fn symlinks_never_redirect_to_another_location() {
    let temp = tempfile::tempdir().unwrap();
    let repo = repository(temp.path(), "moved");
    let tree = worktree(&repo, &temp.path().join("tree"));
    let link = temp.path().canonicalize().unwrap().join("link to moved");
    std::os::unix::fs::symlink(&repo.path, &link).unwrap();
    // A recorded path that is now a symlink resolves elsewhere: rejected.
    let target = Target::repository(&Repository {
        name: "link".into(),
        path: link,
    });
    assert!(problem(verify(&target)).contains("now resolves to"));
    // Same for a deleted worktree replaced by a symlink to its repository.
    let job = isolated(&repo, &tree, JobStatus::Succeeded);
    fs::remove_dir_all(&tree).unwrap();
    std::os::unix::fs::symlink(&repo.path, &tree).unwrap();
    assert!(problem(verify(&Target::job(&job).unwrap())).contains("now resolves to"));
}

#[cfg(unix)]
#[test]
fn repositories_may_use_a_git_directory_symlink_but_worktrees_may_not() {
    use std::os::unix::fs::symlink;
    let temp = tempfile::tempdir().unwrap();
    let repo = repository(temp.path(), "linked .git");
    let store = temp.path().canonicalize().unwrap().join("store/real.git");
    fs::create_dir_all(store.parent().unwrap()).unwrap();
    fs::rename(repo.path.join(".git"), &store).unwrap();
    symlink(&store, repo.path.join(".git")).unwrap();
    // Git accepts this layout; CodeConvoy records the resolved directory.
    git(&repo.path, &["status", "--porcelain"]);
    let mut job = direct(&repo);
    job.before.as_mut().unwrap().common_dir = Some(store.canonicalize().unwrap());
    assert_eq!(verify(&Target::job(&job).unwrap()), Ok(()));
    assert_eq!(verify(&Target::repository(&repo)), Ok(()));

    // The symlink is followed, never trusted: its target is still compared.
    let other = repository(temp.path(), "other");
    fs::remove_file(repo.path.join(".git")).unwrap();
    symlink(other.path.join(".git"), repo.path.join(".git")).unwrap();
    assert!(problem(verify(&Target::job(&job).unwrap())).contains("different Git repository"));
    fs::remove_file(repo.path.join(".git")).unwrap();
    symlink(temp.path().join("missing.git"), repo.path.join(".git")).unwrap();
    assert!(problem(verify(&Target::repository(&repo))).contains("points to nothing"));

    // CodeConvoy creates isolated worktrees with a regular .git link file.
    let source = repository(temp.path(), "source");
    let tree = worktree(&source, &temp.path().join("tree"));
    let link = fs::read(tree.join(".git")).unwrap();
    let moved = temp.path().join("moved link");
    fs::write(&moved, link).unwrap();
    fs::remove_file(tree.join(".git")).unwrap();
    symlink(&moved, tree.join(".git")).unwrap();
    let target = Target::job(&isolated(&source, &tree, JobStatus::Succeeded)).unwrap();
    assert!(problem(verify(&target)).contains(".git is a symlink"));
}

#[test]
fn verification_changes_nothing_in_the_repository() {
    let temp = tempfile::tempdir().unwrap();
    let repo = repository(temp.path(), "source");
    let tree = worktree(&repo, &temp.path().join("tree"));
    fs::write(repo.path.join("dirty.txt"), "uncommitted\n").unwrap();
    let snapshot = || {
        (
            git(
                &repo.path,
                &["status", "--porcelain=v1", "--untracked-files=all"],
            ),
            git(&repo.path, &["worktree", "list", "--porcelain"]),
            listing(&repo.path.join(".git")),
        )
    };
    let before = snapshot();
    for target in [
        Target::job(&direct(&repo)).unwrap(),
        Target::job(&isolated(&repo, &tree, JobStatus::Succeeded)).unwrap(),
    ] {
        assert_eq!(verify(&target), Ok(()));
    }
    assert_eq!(snapshot(), before);
}

/// Every path, size and modification time below a Git directory.
fn listing(root: &Path) -> Vec<(PathBuf, u64, std::time::SystemTime)> {
    let mut entries = Vec::new();
    let mut pending = vec![root.to_owned()];
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(directory).unwrap() {
            let entry = entry.unwrap();
            let metadata = entry.metadata().unwrap();
            if metadata.is_dir() {
                pending.push(entry.path());
            }
            entries.push((entry.path(), metadata.len(), metadata.modified().unwrap()));
        }
    }
    entries.sort();
    entries
}

trait CanonicalizeOrCreate {
    fn canonicalize_or_create(&self) -> PathBuf;
}
impl CanonicalizeOrCreate for PathBuf {
    fn canonicalize_or_create(&self) -> PathBuf {
        fs::create_dir_all(self).unwrap();
        self.canonicalize().unwrap()
    }
}
