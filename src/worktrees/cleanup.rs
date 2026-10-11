//! Ordinary cleanup: read-only preview and non-force Git removal under a lifecycle
//! lease. Unresolved results are protected by the Store before reaching this code.
use super::{ordinary, recovery};
use crate::{domain::WorktreeMetadata, git, process::Cancellation};
use anyhow::{Context, Result, ensure};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Proof {
    // Session-only filesystem identity; never authorize a replacement directory
    // using just the recorded pathname or a persisted observation.
    stamps: Vec<(PathBuf, u64, u64)>,
}
#[derive(Debug)]
pub(crate) enum Failure {
    /// This attempt did not reach the journal write and its processes stopped.
    Blocked(anyhow::Error),
    /// A journal write was attempted, or process cleanup was not confirmed.
    Uncertain(anyhow::Error),
}
#[derive(Clone, Debug, Default)]
pub struct Inspection {
    pub exists: Option<bool>,
    pub registered: Option<bool>,
    pub detached: Option<bool>,
    pub changes: Vec<String>,
    pub detail: String,
    pub proof: Option<Proof>,
}
impl Inspection {
    pub fn unknown(detail: impl Into<String>) -> Self {
        Self {
            detail: detail.into(),
            ..Default::default()
        }
    }
}

fn chain(path: &Path) -> Result<()> {
    for parent in path.ancestors() {
        ordinary(parent, true)?;
    }
    ensure!(
        path.canonicalize()? == path,
        "Path no longer has its recorded canonical identity."
    );
    Ok(())
}
fn identity(m: &WorktreeMetadata, admin: &Path) -> Result<Proof> {
    for directory in [&m.path, &m.repository.path, &m.common_dir, admin] {
        chain(directory)?;
    }
    let mut paths = vec![
        m.path.clone(),
        m.path
            .parent()
            .context("Missing attempt directory.")?
            .to_owned(),
        m.repository.path.clone(),
        m.repository.path.join(".git"),
        m.common_dir.clone(),
        admin.to_owned(),
    ];
    paths.extend([
        m.path.join(".git"),
        m.path
            .parent()
            .context("Missing attempt directory.")?
            .join("owner.json"),
        admin.join("gitdir"),
    ]);
    let mut stamps = Vec::new();
    for path in paths {
        let metadata = fs::symlink_metadata(&path)?;
        ensure!(
            !metadata.file_type().is_symlink(),
            "Symlink substitution detected."
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            stamps.push((path, metadata.dev(), metadata.ino()));
        }
        #[cfg(not(unix))]
        {
            let _ = metadata;
            stamps.push((path, 0, 0));
        }
    }
    Ok(Proof { stamps })
}

/// Force Git to read tracked contents without trusting the real index's stat
/// cache. Only a disposable index is written; no real index/object is changed.
async fn fresh_tracked_changes(m: &WorktreeMetadata, i: &git::Inspection<'_>) -> Result<bool> {
    let entries = i.checked(&m.path, &["ls-files", "--stage", "-z"]).await?;
    let temporary = tempfile::tempdir()?;
    let index = git::path_argument(&temporary.path().join("index"))
        .into_string()
        .map_err(|_| anyhow::anyhow!("Temporary index path is not Unicode."))?;
    let make = |args: &[&str]| {
        let mut spec = git::command(&m.path, args);
        let mut hooks = std::ffi::OsString::from("core.hooksPath=");
        hooks.push(git::path_argument(temporary.path()));
        spec.args.splice(
            0..0,
            [
                "-c".into(),
                hooks,
                "-c".into(),
                "core.ignorestat=false".into(),
                "-c".into(),
                "core.splitIndex=false".into(),
                "-c".into(),
                "core.filemode=true".into(),
            ],
        );
        spec.env.push(("GIT_INDEX_FILE".into(), index.clone()));
        spec
    };
    let mut import = make(&["update-index", "-z", "--index-info"]);
    import.input = Some(entries);
    crate::review::checked(i, import).await?;
    // diff-files alone treats missing stat data as changed without comparing
    // contents. Refresh this disposable index so Git hashes every tracked file.
    let refresh = i
        .capture(make(&["update-index", "--really-refresh"]), 8192)
        .await?;
    match refresh.status.code() {
        Some(0) => {}
        Some(1) => return Ok(true),
        _ => anyhow::bail!(
            "Cannot refresh temporary content check: {}",
            String::from_utf8_lossy(&refresh.stderr)
        ),
    }
    let result = i
        .capture(
            make(&[
                "diff-files",
                "--quiet",
                "--no-ext-diff",
                "--no-textconv",
                "--ignore-submodules=none",
                "--",
            ]),
            8192,
        )
        .await?;
    match result.status.code() {
        Some(0) => Ok(false),
        Some(1) => Ok(true),
        _ => anyhow::bail!(
            "Cannot verify tracked file contents: {}",
            String::from_utf8_lossy(&result.stderr)
        ),
    }
}
async fn changes(
    m: &WorktreeMetadata,
    admin: &Path,
    i: &git::Inspection<'_>,
) -> Result<Vec<String>> {
    // Status may execute configured clean filters while rehashing files. Reject
    // unsupported conversions before asking Git to inspect any working content.
    super::apply::supported(&m.path, &m.base_commit, i).await?;
    // `checked` uses GIT_OPTIONAL_LOCKS=0: even a metadata-only file change must
    // not let status persist a stat-cache refresh to the retained index.
    let bytes = i
        .checked(
            &m.path,
            &[
                "-c",
                "core.untrackedCache=false",
                "status",
                "--porcelain=v1",
                "-z",
                "--untracked-files=all",
                "--ignored=matching",
                "--ignore-submodules=none",
            ],
        )
        .await?;
    let mut changes = git::parse_status(&bytes)?;
    let head = i
        .checked(&m.path, &["rev-parse", "--verify", "HEAD"])
        .await?;
    if recovery::line(&head) != m.base_commit.as_bytes() {
        changes.push("HEAD differs from the saved base; committed work needs review.".into());
    }
    for name in [
        "MERGE_HEAD",
        "CHERRY_PICK_HEAD",
        "REVERT_HEAD",
        "rebase-merge",
        "rebase-apply",
        "sequencer",
        "BISECT_LOG",
        "index.lock",
    ] {
        match fs::symlink_metadata(admin.join(name)) {
            Ok(_) => changes.push(format!("Git operation evidence: {name}")),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
    }
    // Dirty/untracked/ignored/conflicted trees are already protected; don't run
    // content conversion or a costly tracked-file pass for those trees.
    if changes.is_empty() && fresh_tracked_changes(m, i).await? {
        changes.push("Tracked contents differ from the index (fresh content check).".into());
    }
    Ok(changes)
}

pub(crate) async fn inspect(
    root: &Path,
    m: &WorktreeMetadata,
    i: &git::Inspection<'_>,
) -> Inspection {
    let mut report = Inspection::default();
    let result = async {
        report.exists = match fs::symlink_metadata(&m.path) {
            Ok(_) => Some(true),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Some(false),
            Err(e) => return Err(e.into()),
        };
        // Validate ancestors before any Git command or ownership file is read.
        chain(root)?;
        chain(m.path.parent().context("Missing attempt directory.")?)?;
        chain(&m.repository.path)?;
        chain(&m.common_dir)?;
        let source_git = m.repository.path.join(".git");
        ordinary(&source_git, fs::symlink_metadata(&source_git)?.is_dir())?;
        let (exists, registered, admin) = recovery::cleanup_observation(root, m, i, true).await?;
        report.exists = Some(exists);
        report.registered = Some(registered);
        report.detached = registered.then_some(true);
        ensure!(exists && registered, "Worktree is missing or unregistered; inventory and Git records are preserved.");
        let admin = admin.context("Git ownership backlink is unavailable.")?;
        let before = identity(m, &admin)?;
        recovery::no_cleanup_journal(m)?;
        report.changes = changes(m, &admin, i).await?;
        ensure!(identity(m, &admin)? == before, "Worktree identity changed during inspection.");
        if report.changes.is_empty() {
            ensure!(cfg!(unix), "Ordinary cleanup is disabled on this platform: stable filesystem identity checks are unavailable.");
            report.proof = Some(before);
        }
        Ok::<(), anyhow::Error>(())
    }.await;
    if let Err(e) = result {
        report.detail = format!("{e:#}");
    }
    report
}

async fn worktree_command(
    m: &WorktreeMetadata,
    i: &git::Inspection<'_>,
    args: &[&str],
) -> Result<()> {
    let mut spec = git::command(&m.repository.path, args);
    spec.args.push(git::path_argument(&m.path));
    let output = i.capture(spec, 8192).await?;
    ensure!(
        output.status.success() && !output.truncated,
        "Git worktree operation failed: {}",
        String::from_utf8_lossy(&output.stderr).trim()
    );
    Ok(())
}

pub(crate) async fn remove(
    root: &Path,
    m: &WorktreeMetadata,
    proof: &Proof,
    cancellation: &Cancellation,
    safe: &AtomicBool,
) -> Result<recovery::Report, Failure> {
    remove_checked(root, m, proof, cancellation, safe, |_| {}).await
}

#[derive(Clone, Copy)]
enum RemovalStep {
    Unlocked,
    BeforeGitRemove,
}

// Narrow test seam around the real Git operations, without replacing Git or
// changing global PATH. Production calls use a no-op callback.
async fn remove_checked(
    root: &Path,
    m: &WorktreeMetadata,
    proof: &Proof,
    cancellation: &Cancellation,
    safe: &AtomicBool,
    at: impl FnMut(RemovalStep),
) -> Result<recovery::Report, Failure> {
    let preflight = async {
        ensure!(cfg!(unix), "Ordinary cleanup is disabled on this platform.");
        let i = git::Inspection { cancellation, safe };
        let current = inspect(root, m, &i).await;
        ensure!(
            current.proof.as_ref() == Some(proof),
            "Worktree changed since confirmation; preserved. {} {}",
            current.detail,
            current.changes.join("; ")
        );
        ensure!(
            !cancellation.is_cancelled(),
            "Cleanup cancelled before removal."
        );
        Ok(())
    }
    .await;
    if let Err(error) = preflight {
        return Err(if safe.load(Ordering::Acquire) {
            Failure::Blocked(error)
        } else {
            Failure::Uncertain(error)
        });
    }
    // Never infer retry safety later from a missing journal: even its atomic
    // write can fail after replacement. All errors past this boundary are sticky.
    remove_journaled(root, m, proof, safe, at)
        .await
        .map_err(Failure::Uncertain)
}

async fn remove_journaled(
    root: &Path,
    m: &WorktreeMetadata,
    proof: &Proof,
    safe: &AtomicBool,
    mut at: impl FnMut(RemovalStep),
) -> Result<recovery::Report> {
    recovery::write_journal(root, m, false)?;
    // From unlocking until relocking/removal is verified, shutdown must await
    // this critical section. Every child still has the ordinary 20-second bound.
    let critical = Cancellation::default();
    let i = git::Inspection {
        cancellation: &critical,
        safe,
    };
    let admin = recovery::cleanup_target(root, m, &i, true).await?;
    ensure!(
        &identity(m, &admin)? == proof,
        "Worktree identity changed before removal."
    );
    ensure!(
        changes(m, &admin, &i).await?.is_empty(),
        "Worktree became modified; cleanup blocked."
    );
    worktree_command(m, &i, &["worktree", "unlock", "--"]).await?;
    at(RemovalStep::Unlocked);
    let removal = async {
        let admin = recovery::cleanup_target(root, m, &i, false).await?;
        ensure!(
            &identity(m, &admin)? == proof,
            "Worktree identity changed after unlocking."
        );
        ensure!(
            changes(m, &admin, &i).await?.is_empty(),
            "Worktree became modified; cleanup blocked."
        );
        recovery::manifest(root, m)?;
        ensure!(
            &identity(m, &admin)? == proof,
            "Worktree path changed immediately before removal."
        );
        at(RemovalStep::BeforeGitRemove);
        worktree_command(m, &i, &["worktree", "remove", "--"]).await?;
        recovery::confirm_removed(root, m, &i).await
    }
    .await;
    if let Err(error) = removal {
        // Restore only our still-verified, unchanged ownership. Never manipulate
        // a substituted path. A failure retains the journal and blocks retries.
        let restored = async {
            let admin = recovery::cleanup_target(root, m, &i, false).await?;
            ensure!(
                &identity(m, &admin)? == proof,
                "Ownership changed; cannot restore lock."
            );
            worktree_command(
                m,
                &i,
                &[
                    "worktree",
                    "lock",
                    "--reason",
                    "CodeConvoy retained result",
                    "--",
                ],
            )
            .await
        }
        .await;
        return match restored {
            Ok(()) => {
                Err(error).context("Cleanup failed; ownership lock restored and evidence retained.")
            }
            Err(e) => Err(error).context(format!(
                "Cleanup uncertain; journal retained. Lock restoration: {e:#}"
            )),
        };
    }
    Ok(recovery::Report::unavailable(
        crate::domain::ResultAvailability::Cleaned,
        "Managed worktree removed; historical observations and ownership record retained.",
    ))
}

#[cfg(all(test, unix))]
#[path = "cleanup_tests.rs"]
mod tests;
