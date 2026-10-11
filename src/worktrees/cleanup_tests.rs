#![allow(clippy::unwrap_used)]
use super::*;
use crate::domain::ResultAvailability;

fn git(path: &Path, args: &[&str]) -> String {
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(path)
        .args([
            "-c",
            "commit.gpgsign=false",
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=f@invalid",
        ])
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().into()
}
struct Fixture {
    _temp: tempfile::TempDir,
    root: PathBuf,
    m: WorktreeMetadata,
}
impl Fixture {
    async fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let directory = temp.path().canonicalize().unwrap();
        let source = directory.join("repository");
        fs::create_dir(&source).unwrap();
        git(&source, &["init", "--quiet"]);
        git(&source, &["config", "core.autocrlf", "false"]);
        fs::write(source.join("tracked"), "keep\n").unwrap();
        git(&source, &["add", "."]);
        git(&source, &["commit", "-qm", "base"]);
        let root = directory.join("worktrees");
        let m = crate::worktrees::reserve(
            &root,
            1,
            0,
            crate::git::register(&source).await.unwrap(),
            crate::git::common_dir(&source).await.unwrap(),
            git(&source, &["rev-parse", "HEAD"]),
        )
        .unwrap();
        crate::worktrees::create(&m, &Cancellation::default(), &AtomicBool::new(true))
            .await
            .unwrap();
        Self {
            _temp: temp,
            root,
            m,
        }
    }
    async fn proof(&self) -> Proof {
        let report = inspect(
            &self.root,
            &self.m,
            &git::Inspection {
                cancellation: &Cancellation::default(),
                safe: &AtomicBool::new(true),
            },
        )
        .await;
        assert!(report.proof.is_some(), "{report:?}");
        report.proof.unwrap()
    }
    fn admin(&self) -> PathBuf {
        PathBuf::from(
            fs::read_to_string(self.m.path.join(".git"))
                .unwrap()
                .trim()
                .strip_prefix("gitdir: ")
                .unwrap(),
        )
    }
}

#[tokio::test]
async fn failed_nonforce_git_removal_restores_lock_and_keeps_all_evidence() {
    let f = Fixture::new().await;
    let proof = f.proof().await;
    let admin = f.admin();
    let error = remove_checked(
        &f.root,
        &f.m,
        &proof,
        &Cancellation::default(),
        &AtomicBool::new(true),
        |step| {
            if matches!(step, RemovalStep::BeforeGitRemove) {
                fs::write(f.m.path.join("late-untracked"), "valuable").unwrap();
            }
        },
    )
    .await
    .unwrap_err();
    assert!(error.to_string().contains("lock restored"), "{error:#}");
    assert_eq!(
        fs::read_to_string(f.m.path.join("late-untracked")).unwrap(),
        "valuable"
    );
    assert!(f.m.path.join("tracked").exists());
    assert_eq!(
        fs::read_to_string(admin.join("locked")).unwrap().trim(),
        "CodeConvoy retained result"
    );
    let journal: serde_json::Value =
        serde_json::from_slice(&fs::read(f.m.path.parent().unwrap().join("cleanup.json")).unwrap())
            .unwrap();
    assert_eq!(journal["completed"], false);
    assert!(recovery::no_cleanup_journal(&f.m).is_err());
    assert!(!git(&f.m.repository.path, &["worktree", "list", "--porcelain"]).is_empty());
}
#[tokio::test]
async fn ignored_data_created_after_unlock_is_preserved_and_registration_relocked() {
    let f = Fixture::new().await;
    git(
        &f.m.repository.path,
        &["config", "core.excludesfile", "/dev/null"],
    );
    fs::write(f.m.common_dir.join("info/exclude"), "valuable\n").unwrap();
    let proof = f.proof().await;
    let admin = f.admin();
    let result = remove_checked(
        &f.root,
        &f.m,
        &proof,
        &Cancellation::default(),
        &AtomicBool::new(true),
        |step| {
            if matches!(step, RemovalStep::Unlocked) {
                fs::write(f.m.path.join("valuable"), "keep").unwrap();
            }
        },
    )
    .await;
    assert!(result.is_err());
    assert_eq!(
        fs::read_to_string(f.m.path.join("valuable")).unwrap(),
        "keep"
    );
    assert!(admin.join("locked").exists());
}
#[tokio::test]
async fn shutdown_after_unlock_finishes_critical_removal_and_records_completion() {
    let f = Fixture::new().await;
    let proof = f.proof().await;
    let cancel = Cancellation::default();
    let safe = AtomicBool::new(true);
    let report = remove_checked(&f.root, &f.m, &proof, &cancel, &safe, |step| {
        if matches!(step, RemovalStep::Unlocked) {
            cancel.cancel();
        }
    })
    .await
    .unwrap();
    assert_eq!(report.availability, ResultAvailability::Cleaned);
    assert!(!f.m.path.exists());
    assert!(recovery::physically_cleaned(&f.m).unwrap());
    assert!(safe.load(std::sync::atomic::Ordering::Acquire));
}
#[tokio::test]
async fn substitution_after_unlock_never_removes_external_data_or_repairs_foreign_ownership() {
    let f = Fixture::new().await;
    let proof = f.proof().await;
    let result = remove_checked(
        &f.root,
        &f.m,
        &proof,
        &Cancellation::default(),
        &AtomicBool::new(true),
        |step| {
            if matches!(step, RemovalStep::Unlocked) {
                fs::rename(&f.m.path, f.m.path.with_extension("saved")).unwrap();
                std::os::unix::fs::symlink(&f.m.repository.path, &f.m.path).unwrap();
            }
        },
    )
    .await;
    assert!(result.is_err());
    assert_eq!(
        fs::read_to_string(f.m.repository.path.join("tracked")).unwrap(),
        "keep\n"
    );
    assert!(f.m.path.with_extension("saved").join("tracked").exists());
}

#[tokio::test]
async fn unsupported_filters_are_rejected_before_status_can_execute_them() {
    let f = Fixture::new().await;
    fs::write(f.m.path.join(".gitattributes"), "tracked filter=fixture\n").unwrap();
    git(
        &f.m.path,
        &[
            "config",
            "filter.fixture.clean",
            "touch filter-executed; cat",
        ],
    );
    fs::write(f.m.path.join("tracked"), "changed\n").unwrap();
    let report = inspect(
        &f.root,
        &f.m,
        &git::Inspection {
            cancellation: &Cancellation::default(),
            safe: &AtomicBool::new(true),
        },
    )
    .await;
    assert!(report.proof.is_none());
    assert!(report.detail.contains("conversion"));
    assert!(!f.m.path.join("filter-executed").exists());
}
