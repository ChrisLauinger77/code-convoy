//! Retained detached worktrees and owned result lifecycle.
pub mod recovery;
use crate::{
    domain::{ExecutionMode, Repository, WorktreeMetadata, WorktreeResult},
    git,
    process::{self, Cancellation},
};
use anyhow::{Context, Result};
use std::{fs, io::Write, path::Path, sync::atomic::AtomicBool, time::Duration};

const LIMIT: usize = 2 * 1024 * 1024;
const OWNER: &str = "codeconvoy.worktree.v1";

/// Refuse symlinks and Windows junctions/reparse points, including dangling ones.
fn ordinary(path: &Path, directory: bool) -> Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        anyhow::ensure!(
            metadata.file_attributes() & 0x400 == 0,
            "Reparse point is not an owned resource: {}",
            path.display()
        );
    }
    anyhow::ensure!(
        !metadata.file_type().is_symlink(),
        "Symlink is not an owned resource: {}",
        path.display()
    );
    anyhow::ensure!(
        if directory {
            metadata.is_dir()
        } else {
            metadata.is_file()
        },
        "Unexpected resource type: {}",
        path.display()
    );
    Ok(())
}

/// Atomically reserve a random attempt directory, retaining it even on failure.
/// The manifest is a sibling of the checkout, never an agent-created file in it.
pub fn reserve(
    root: &Path,
    run: u64,
    job: usize,
    repository: Repository,
    common_dir: std::path::PathBuf,
    base_commit: String,
) -> Result<WorktreeMetadata> {
    anyhow::ensure!(
        valid_commit(&base_commit),
        "Invalid isolated base commit; start a fresh convoy."
    );
    // Inspect the root itself before canonicalization can follow a link. Dangling
    // links must also fail without creating directories at their target.
    match fs::symlink_metadata(root) {
        Ok(_) => ordinary(root, true)
            .context("Cannot use worktree storage; choose an ordinary directory.")?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(e).context("Cannot inspect worktree storage."),
    }
    fs::create_dir_all(root).with_context(|| {
        format!(
            "Cannot create worktree storage {}. Check permissions and free space.",
            root.display()
        )
    })?;
    ordinary(root, true).context("Cannot use worktree storage; choose an ordinary directory.")?;
    let root = root
        .canonicalize()
        .context("Cannot resolve worktree storage.")?;
    anyhow::ensure!(
        !root.starts_with(&repository.path),
        "Worktree storage must be outside the registered working tree."
    );
    let attempt = tempfile::Builder::new()
        .prefix(&format!("run-{run}-job-{job}-"))
        .tempdir_in(&root)
        .context("Cannot reserve a unique worktree directory. Check permissions and free space.")?
        .keep();
    let metadata = WorktreeMetadata {
        owner: OWNER.into(),
        run,
        job,
        repository,
        common_dir,
        path: attempt.join("tree"),
        base_commit,
        execution_mode: ExecutionMode::IsolatedWorktree,
    };
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(attempt.join("owner.json"))
        .context("Cannot write worktree ownership metadata; Git has not been started.")?;
    serde_json::to_writer_pretty(&mut file, &metadata)?;
    file.write_all(b"\n")?;
    file.sync_all()?;
    fs::create_dir(attempt.join("hooks"))?;
    #[cfg(unix)]
    fs::File::open(&attempt)?.sync_all()?;
    Ok(metadata)
}
fn valid_commit(commit: &str) -> bool {
    matches!(commit.len(), 40 | 64) && commit.bytes().all(|b| b.is_ascii_hexdigit())
}
fn verify_manifest(metadata: &WorktreeMetadata) -> Result<()> {
    anyhow::ensure!(
        metadata.owner == OWNER
            && metadata.execution_mode == ExecutionMode::IsolatedWorktree
            && valid_commit(&metadata.base_commit),
        "Invalid CodeConvoy worktree ownership metadata."
    );
    let parent = metadata
        .path
        .parent()
        .context("Missing worktree attempt directory.")?;
    anyhow::ensure!(
        metadata.path.file_name().is_some_and(|p| p == "tree"),
        "Invalid worktree checkout path."
    );
    let file = fs::File::open(parent.join("owner.json"))
        .context("Worktree ownership record is missing; no operation was performed.")?;
    let recorded: WorktreeMetadata = serde_json::from_reader(std::io::Read::take(file, 64 * 1024))?;
    anyhow::ensure!(
        &recorded == metadata,
        "Worktree ownership record does not match this job."
    );
    Ok(())
}
async fn verify(metadata: &WorktreeMetadata, inspection: &git::Inspection<'_>) -> Result<()> {
    let m = metadata.clone();
    tokio::task::spawn_blocking(move || verify_manifest(&m)).await??;
    anyhow::ensure!(
        inspection.register(&metadata.path).await?.path == metadata.path,
        "Worktree path now resolves elsewhere; inspect it before proceeding."
    );
    anyhow::ensure!(
        inspection.common_dir(&metadata.path).await? == metadata.common_dir,
        "Worktree belongs to a different Git repository; inspect it before proceeding."
    );
    Ok(())
}

/// Caller owns the common-repository administrative mutex until this returns.
/// Unlike a dropped future, cancellation awaits process-tree cleanup.
pub async fn create(
    metadata: &WorktreeMetadata,
    cancellation: &Cancellation,
    safe: &AtomicBool,
) -> Result<git::WorkingTree> {
    let inspection = git::Inspection { cancellation, safe };
    anyhow::ensure!(
        !cancellation.is_cancelled(),
        "Cancelled during worktree preparation."
    );
    let m = metadata.clone();
    tokio::task::spawn_blocking(move || -> Result<()> {
        verify_manifest(&m)?;
        anyhow::ensure!(fs::symlink_metadata(&m.path).is_err_and(|e| e.kind() == std::io::ErrorKind::NotFound),
            "Worktree destination already exists or cannot be inspected: {}. Start a fresh convoy; the existing path is retained.", m.path.display());
        Ok(())
    }).await??;
    anyhow::ensure!(
        inspection.register(&metadata.repository.path).await?.path == metadata.repository.path,
        "Registered repository now resolves elsewhere; register it again."
    );
    anyhow::ensure!(
        inspection.common_dir(&metadata.repository.path).await? == metadata.common_dir,
        "Repository identity changed after preflight; start a fresh convoy."
    );
    let base = inspection
        .capture(
            git::command(
                &metadata.repository.path,
                &[
                    "cat-file",
                    "-e",
                    &format!("{}^{{commit}}", metadata.base_commit),
                ],
            ),
            8192,
        )
        .await?;
    anyhow::ensure!(
        base.status.success(),
        "Base commit is unavailable. Start a fresh convoy after checking the repository. Git: {}",
        String::from_utf8_lossy(&base.stderr).trim()
    );
    let mut spec = git::command(&metadata.repository.path, &[]);
    // Suppress post-checkout hooks: preparation must not run user hook actions.
    let hooks = metadata
        .path
        .parent()
        .context("Missing attempt directory.")?
        .join("hooks");
    spec.args.push("-c".into());
    let mut config = std::ffi::OsString::from("core.hooksPath=");
    config.push(git::path_argument(&hooks));
    spec.args.push(config);
    spec.args.extend(
        [
            "worktree",
            "add",
            "--detach",
            "--lock",
            "--reason",
            "CodeConvoy retained result",
            "--",
        ]
        .map(Into::into),
    );
    spec.args.push(git::path_argument(&metadata.path));
    spec.args.push(metadata.base_commit.clone().into());
    let output = process::capture_owned(spec, LIMIT, cancellation, Duration::from_secs(300), safe).await
        .with_context(|| format!("Could not prepare isolated worktree at {}. Partial state and ownership metadata are retained; inspect Git, permissions and disk space.", metadata.path.display()))?;
    anyhow::ensure!(
        output.status.success(),
        "Git worktree creation failed for base {} at {}: {}. No direct execution was attempted; partial state is retained.",
        metadata.base_commit,
        metadata.path.display(),
        String::from_utf8_lossy(&output.stderr).trim()
    );
    verify(metadata, &inspection).await?;
    let state = inspection.status(&metadata.path).await?;
    anyhow::ensure!(
        state.summary.head.as_deref() == Some(&metadata.base_commit),
        "Isolated HEAD does not match the snapshotted base; agent was not started."
    );
    Ok(state)
}

async fn patch(
    metadata: &WorktreeMetadata,
    inspection: &git::Inspection<'_>,
    cached: bool,
) -> Result<process::Captured> {
    let mut spec = git::command(
        &metadata.path,
        &[
            "diff",
            "--no-ext-diff",
            "--no-textconv",
            "--no-color",
            "--ignore-submodules=none",
        ],
    );
    if cached {
        spec.args.push("--cached".into());
    }
    spec.args
        .extend([metadata.base_commit.as_str(), "--"].map(Into::into));
    let output = inspection.capture(spec, LIMIT).await?;
    anyhow::ensure!(
        output.status.success(),
        "Cannot compare retained worktree with base {}: {}",
        metadata.base_commit,
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(output)
}
pub async fn inspect(metadata: &WorktreeMetadata) -> Result<WorktreeResult> {
    inspect_owned(metadata, &Cancellation::default(), &AtomicBool::new(true)).await
}

pub(crate) async fn inspect_owned(
    metadata: &WorktreeMetadata,
    cancellation: &Cancellation,
    safe: &AtomicBool,
) -> Result<WorktreeResult> {
    let inspection = git::Inspection { cancellation, safe };
    let path = metadata.path.clone();
    let exists = tokio::task::spawn_blocking(move || path.try_exists()).await??;
    if !exists {
        return Ok(WorktreeResult {
            observed_this_session: true,
            exists: false,
            changed: None,
        });
    }
    verify(metadata, &inspection).await?;
    let state = inspection.status(&metadata.path).await?;
    // Exit 0 = equal; 1 = differs. Never infer changes from rendered patch size,
    // agent status, or timestamps. Ignored files follow the existing Git model.
    let mut tracked = false;
    for cached in [false, true] {
        let mut spec = git::command(
            &metadata.path,
            &[
                "diff",
                "--quiet",
                "--no-ext-diff",
                "--no-textconv",
                "--ignore-submodules=none",
            ],
        );
        if cached {
            spec.args.push("--cached".into());
        }
        spec.args
            .extend([metadata.base_commit.as_str(), "--"].map(Into::into));
        let comparison = inspection.capture(spec, 8192).await?;
        tracked |= match comparison.status.code() {
            Some(0) => false,
            Some(1) => true,
            _ => anyhow::bail!(
                "Cannot compare isolated result with its base commit. Git: {}",
                String::from_utf8_lossy(&comparison.stderr)
            ),
        };
    }
    Ok(WorktreeResult {
        observed_this_session: true,
        exists: true,
        changed: Some(tracked || state.entries.iter().any(|entry| entry.starts_with("?? "))),
    })
}
pub async fn diff(metadata: &WorktreeMetadata) -> Result<String> {
    diff_owned(metadata, &Cancellation::default(), &AtomicBool::new(true)).await
}
async fn diff_owned(
    metadata: &WorktreeMetadata,
    cancellation: &Cancellation,
    safe: &AtomicBool,
) -> Result<String> {
    let inspection = git::Inspection { cancellation, safe };
    verify(metadata, &inspection).await?;
    let worktree_patch = patch(metadata, &inspection, false).await?;
    let index_patch = patch(metadata, &inspection, true).await?;
    let state = inspection.status(&metadata.path).await?;
    Ok(format!(
        "Isolated worktree: {}\nBase commit: {}\n\n--- CHANGES FROM SNAPSHOTTED BASE ---\n{}{}\n--- INDEX CHANGES FROM SNAPSHOTTED BASE ---\n{}{}\n--- STATUS (untracked contents are not in Git diff) ---\n{}",
        metadata.path.display(),
        metadata.base_commit,
        if worktree_patch.stdout.is_empty() {
            "(none)".into()
        } else {
            String::from_utf8_lossy(&worktree_patch.stdout)
        },
        if worktree_patch.truncated {
            "\n[Diff truncated at 2 MiB]"
        } else {
            ""
        },
        if index_patch.stdout.is_empty() {
            "(none)".into()
        } else {
            String::from_utf8_lossy(&index_patch.stdout)
        },
        if index_patch.truncated {
            "\n[Diff truncated at 2 MiB]"
        } else {
            ""
        },
        if state.entries.is_empty() {
            "Clean working tree".into()
        } else {
            state.entries.join("\n")
        }
    ))
}
