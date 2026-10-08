//! Restart validation and the internal, fail-closed removal primitive.
//! Callers must hold a manager lifecycle lease and the repository admin mutex.
use super::{OWNER, inspect_owned, ordinary, valid_commit, verify_manifest};
use crate::{
    domain::{Job, ResultAvailability as Availability, WorktreeMetadata, WorktreeResult},
    git,
    process::Cancellation,
};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    ffi::OsString,
    fs,
    io::{Read, Write},
    path::{Component, Path, PathBuf},
    sync::atomic::AtomicBool,
};

#[derive(Debug, Clone)]
pub struct Report {
    pub availability: Availability,
    pub result: Option<WorktreeResult>,
    pub detail: String,
    pub diff: Option<String>,
    pub statistics: Option<Result<crate::review::Statistics, String>>,
}
impl Report {
    pub fn unavailable(availability: Availability, detail: impl Into<String>) -> Self {
        Self {
            availability,
            result: None,
            detail: detail.into(),
            diff: None,
            statistics: None,
        }
    }
    pub fn apply(&self, job: &mut Job) {
        let transition = !job.result_checked || job.result_availability != self.availability;
        job.result_availability = self.availability;
        job.result_checked = true;
        job.review = Some(
            self.statistics
                .clone()
                .unwrap_or_else(|| Err(self.detail.clone())),
        );
        if self.availability == Availability::Cleaned
            && job.resolution == crate::domain::ResultResolution::DiscardPending
        {
            job.resolution = crate::domain::ResultResolution::Discarded;
            job.resolved_at = Some(crate::domain::now());
        }
        if let Some(result) = &self.result {
            job.worktree_result = Some(result.clone());
        }
        job.worktree_detail = self.detail.clone();
        if job.resolution == crate::domain::ResultResolution::ApplyPending {
            job.worktree_detail.push_str(" Apply outcome is pending or uncertain. Inspect the registered destination manually; automatic retry is blocked and the isolated copy is retained.");
        }
        if transition {
            let message = match self.availability {
                Availability::Available => "Recovered isolated result",
                Availability::Missing => "Isolated result missing",
                Availability::Cleaned => "Cleanup completed",
                Availability::CleanupFailed => "Cleanup failed",
                _ => "Isolated result unavailable",
            };
            job.log.append(&format!("\n[CodeConvoy] {message}\n"));
        }
    }
}

fn absent(path: &Path) -> Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(false),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(true),
        Err(e) => Err(e.into()),
    }
}
fn bounded(path: &Path) -> Result<Vec<u8>> {
    ordinary(path, false)?;
    let mut bytes = Vec::new();
    fs::File::open(path)?
        .take(64 * 1024 + 1)
        .read_to_end(&mut bytes)?;
    anyhow::ensure!(bytes.len() <= 64 * 1024, "Ownership record is too large.");
    Ok(bytes)
}
pub(crate) fn native(bytes: &[u8]) -> Result<PathBuf> {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStringExt;
        Ok(OsString::from_vec(bytes.to_vec()).into())
    }
    #[cfg(not(unix))]
    {
        Ok(
            OsString::from(std::str::from_utf8(bytes).context("Git path is not valid Unicode.")?)
                .into(),
        )
    }
}
pub(crate) fn line(bytes: &[u8]) -> &[u8] {
    let bytes = bytes.strip_suffix(b"\n").unwrap_or(bytes);
    #[cfg(windows)]
    let bytes = bytes.strip_suffix(b"\r").unwrap_or(bytes);
    bytes
}
fn resolve(path: &Path) -> Result<PathBuf> {
    if absent(path)? {
        Ok(resolve(path.parent().context("Missing path parent.")?)?
            .join(path.file_name().context("Missing path name.")?))
    } else {
        Ok(path.canonicalize()?)
    }
}

/// The root comes from the application's Store, never from persisted job paths.
fn layout(root: &Path, m: &WorktreeMetadata) -> Result<PathBuf> {
    anyhow::ensure!(
        root.is_absolute()
            && m.path.is_absolute()
            && !m
                .path
                .components()
                .any(|c| matches!(c, Component::ParentDir | Component::CurDir)),
        "Invalid isolated storage path."
    );
    let attempt = m.path.parent().context("Missing attempt directory.")?;
    anyhow::ensure!(
        attempt.parent() == Some(root) && m.path.file_name().is_some_and(|n| n == "tree"),
        "Isolated result is outside its owned storage location."
    );
    let prefix = format!("run-{}-job-{}-", m.run, m.job);
    anyhow::ensure!(
        attempt
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.starts_with(&prefix) && n.len() > prefix.len()),
        "Isolated attempt identity does not match its job."
    );
    anyhow::ensure!(
        m.owner == OWNER
            && m.execution_mode == crate::domain::ExecutionMode::IsolatedWorktree
            && valid_commit(&m.base_commit),
        "Invalid isolated ownership metadata."
    );
    anyhow::ensure!(
        !m.path.starts_with(&m.repository.path) && !m.repository.path.starts_with(&m.path),
        "Registered working trees cannot be cleanup targets."
    );
    Ok(attempt.to_owned())
}
fn manifest(root: &Path, m: &WorktreeMetadata) -> Result<()> {
    let attempt = layout(root, m)?;
    ordinary(root, true)?;
    anyhow::ensure!(
        root.canonicalize()? == root,
        "Worktree storage now resolves elsewhere."
    );
    ordinary(&attempt, true)?;
    anyhow::ensure!(
        attempt.canonicalize()? == attempt,
        "Attempt directory now resolves elsewhere."
    );
    ordinary(&attempt.join("owner.json"), false)?;
    verify_manifest(m)?;
    if !absent(&m.path)? {
        ordinary(&m.path, true)?;
    }
    Ok(())
}

#[derive(Serialize, Deserialize)]
struct CleanupRecord {
    metadata: WorktreeMetadata,
    completed: bool,
}
fn journal(m: &WorktreeMetadata) -> Result<Option<CleanupRecord>> {
    let path = m
        .path
        .parent()
        .context("Missing attempt directory.")?
        .join("cleanup.json");
    if absent(&path)? {
        return Ok(None);
    }
    let record: CleanupRecord = serde_json::from_slice(&bounded(&path)?)?;
    anyhow::ensure!(record.metadata == *m, "Cleanup record ownership mismatch.");
    Ok(Some(record))
}
fn write_journal(root: &Path, m: &WorktreeMetadata, completed: bool) -> Result<()> {
    manifest(root, m)?;
    let parent = m.path.parent().context("Missing attempt directory.")?;
    // Never follow a substituted journal. The atomic replacement is only in the
    // verified attempt directory, and contains no repository file contents.
    journal(m)?;
    let mut file = tempfile::NamedTempFile::new_in(parent)?;
    serde_json::to_writer(
        &mut file,
        &CleanupRecord {
            metadata: m.clone(),
            completed,
        },
    )?;
    file.write_all(b"\n")?;
    file.as_file().sync_all()?;
    file.persist(parent.join("cleanup.json"))?;
    #[cfg(unix)]
    fs::File::open(parent)?.sync_all()?;
    Ok(())
}

struct Verified {
    exists: bool,
    registered: bool,
}
struct Failure(Availability, anyhow::Error);
type VerifiedResult<T> = std::result::Result<T, Failure>;
fn invalid(e: anyhow::Error) -> Failure {
    Failure(Availability::Invalid, e)
}
fn stale(e: anyhow::Error) -> Failure {
    Failure(Availability::Stale, e)
}

async fn verify(
    root: &Path,
    m: &WorktreeMetadata,
    i: &git::Inspection<'_>,
) -> VerifiedResult<Verified> {
    layout(root, m).map_err(invalid)?;
    // Removed storage is missing, not an invitation to recreate or delete it.
    if absent(root).map_err(stale)?
        || absent(
            m.path
                .parent()
                .ok_or_else(|| invalid(anyhow::anyhow!("Missing attempt directory.")))?,
        )
        .map_err(stale)?
    {
        return Err(Failure(
            Availability::Missing,
            anyhow::anyhow!(
                "Isolated storage is missing. Restore its original location to inspect the result."
            ),
        ));
    }
    manifest(root, m).map_err(invalid)?;
    let source = i.register(&m.repository.path).await.map_err(stale)?;
    if source.path != m.repository.path
        || i.common_dir(&source.path).await.map_err(stale)? != m.common_dir
    {
        return Err(invalid(anyhow::anyhow!(
            "Registered repository identity changed. Restore its original location; no cleanup was performed."
        )));
    }
    let base = i
        .capture(
            git::command(
                &source.path,
                &["cat-file", "-e", &format!("{}^{{commit}}", m.base_commit)],
            ),
            8192,
        )
        .await
        .map_err(stale)?;
    if !base.status.success() {
        return Err(stale(anyhow::anyhow!(
            "Base commit is unavailable. Restore repository objects before inspecting or cleaning this result. Git: {}",
            String::from_utf8_lossy(&base.stderr)
        )));
    }
    let listing = i
        .checked(&source.path, &["worktree", "list", "--porcelain", "-z"])
        .await
        .map_err(stale)?;
    let mut registered = false;
    let mut owned_registration = false;
    // -z is lossless on Unix and unquoted on every platform, even with newlines.
    for record in listing
        .split(|b| *b == 0)
        .collect::<Vec<_>>()
        .split(|field| field.is_empty())
    {
        if let Some(path) = record.first().and_then(|f| f.strip_prefix(b"worktree ")) {
            let path = native(path).map_err(stale)?;
            if resolve(&path).is_ok_and(|p| p == m.path) {
                registered = true;
                owned_registration = record.contains(&b"detached".as_slice())
                    && record.contains(&b"locked CodeConvoy retained result".as_slice());
            }
        }
    }
    let exists = !absent(&m.path).map_err(stale)?;
    if registered && !owned_registration {
        return Err(invalid(anyhow::anyhow!(
            "Worktree registration no longer has the expected detached ownership lock. Inspect it in Git; cleanup is blocked."
        )));
    }
    if exists && !registered {
        return Err(stale(anyhow::anyhow!(
            "Git no longer recognizes this retained worktree. Preserve its files and inspect its Git registration."
        )));
    }
    if registered {
        verify_backlink(m, exists).map_err(invalid)?;
    }
    if exists
        && (i.register(&m.path).await.map_err(stale)?.path != m.path
            || i.common_dir(&m.path).await.map_err(stale)? != m.common_dir)
    {
        return Err(invalid(anyhow::anyhow!(
            "Retained worktree identity does not match the registered repository."
        )));
    }
    Ok(Verified { exists, registered })
}

fn verify_backlink(m: &WorktreeMetadata, exists: bool) -> Result<()> {
    let entries = m.common_dir.join("worktrees");
    ordinary(&entries, true)?;
    anyhow::ensure!(
        entries.canonicalize()? == entries,
        "Git administration now resolves elsewhere."
    );
    let expected = m.path.join(".git");
    let mut matches = Vec::new();
    for entry in fs::read_dir(&entries)? {
        let entry = entry?;
        let admin = entry.path();
        if ordinary(&admin, true).is_err() {
            continue;
        }
        let Ok(bytes) = bounded(&admin.join("gitdir")) else {
            continue;
        };
        let backlink = native(line(&bytes))?;
        // For missing checkouts the parent cannot be canonicalized. Compare to
        // the exact native path emitted by Git for the originally canonical tree.
        let same = backlink == expected || resolve(&backlink).is_ok_and(|p| p == expected);
        if same {
            matches.push(admin);
        }
    }
    anyhow::ensure!(
        matches.len() == 1,
        "Missing or ambiguous Git ownership backlink."
    );
    let admin = &matches[0];
    anyhow::ensure!(
        line(&bounded(&admin.join("locked"))?) == b"CodeConvoy retained result",
        "Git ownership lock changed."
    );
    if exists {
        let bytes = bounded(&expected)?;
        let target = native(line(
            bytes
                .strip_prefix(b"gitdir: ")
                .context("Invalid worktree Git link.")?,
        ))?;
        let target = if target.is_absolute() {
            target
        } else {
            m.path.join(target)
        };
        anyhow::ensure!(
            target.canonicalize()? == *admin,
            "Worktree Git link does not match its ownership backlink."
        );
    }
    Ok(())
}

pub(crate) async fn reconcile(
    root: &Path,
    m: &WorktreeMetadata,
    previous: Availability,
    with_diff: bool,
    cancellation: &Cancellation,
    safe: &AtomicBool,
) -> Report {
    let inspection = git::Inspection { cancellation, safe };
    let verified = match verify(root, m, &inspection).await {
        Ok(v) => v,
        Err(Failure(availability, e)) => {
            return Report::unavailable(
                if matches!(
                    previous,
                    Availability::CleanupFailed | Availability::CleanupPending
                ) {
                    Availability::CleanupFailed
                } else {
                    availability
                },
                format!("Isolated result unavailable. {e:#}"),
            );
        }
    };
    let record = match journal(m) {
        Ok(r) => r,
        Err(e) => {
            return Report::unavailable(
                Availability::Invalid,
                format!("Invalid cleanup record. {e:#}"),
            );
        }
    };
    if let Some(record) = &record
        && !verified.exists
        && !verified.registered
    {
        if !record.completed
            && let Err(e) = write_journal(root, m, true)
        {
            return Report::unavailable(
                Availability::CleanupFailed,
                format!(
                    "Could not record completed cleanup. Ownership metadata is preserved. {e:#}"
                ),
            );
        }
        return Report::unavailable(
            Availability::Cleaned,
            "Cleanup completed; no checkout or Git registration remains.",
        );
    }
    let cleanup_unfinished = matches!(
        previous,
        Availability::CleanupPending | Availability::CleanupFailed | Availability::Cleaned
    ) || record.is_some();
    let cleanup_detail = "Cleanup did not finish, or the resource reappeared. Files and ownership metadata are preserved; inspect the worktree before retrying cleanup.";
    let unavailable = if cleanup_unfinished {
        Availability::CleanupFailed
    } else {
        Availability::Stale
    };
    if !verified.exists {
        return Report {
            result: Some(WorktreeResult {
                observed_this_session: true,
                exists: false,
                changed: None,
            }),
            ..Report::unavailable(
                if cleanup_unfinished {
                    Availability::CleanupFailed
                } else {
                    Availability::Missing
                },
                if cleanup_unfinished {
                    cleanup_detail
                } else {
                    "Isolated result directory is missing. Restore its original location to inspect it; nothing was recreated."
                },
            )
        };
    }
    let mut result = match inspect_owned(m, cancellation, safe).await {
        Ok(r) => r,
        Err(e) => {
            return Report::unavailable(
                unavailable,
                format!("Cannot inspect isolated result. {e:#}"),
            );
        }
    };
    let diff = if with_diff {
        match super::diff_owned(m, cancellation, safe).await {
            Ok(d) => Some(d),
            Err(e) => {
                return Report::unavailable(
                    unavailable,
                    format!("Cannot inspect isolated diff. {e:#}"),
                );
            }
        }
    } else {
        None
    };
    let statistics = crate::review::Snapshot::capture(&m.path, &m.base_commit, &inspection)
        .await
        .map(|s| s.statistics)
        .map_err(|e| format!("{e:#}"));
    if let Ok(s) = &statistics {
        result.changed = Some(s.files > 0 || s.index_alternatives > 0);
    }
    Report {
        availability: if cleanup_unfinished {
            Availability::CleanupFailed
        } else {
            Availability::Available
        },
        result: Some(result),
        detail: if cleanup_unfinished {
            cleanup_detail.into()
        } else {
            String::new()
        },
        diff,
        statistics: Some(statistics),
    }
}

pub(crate) async fn verify_available(
    root: &Path,
    m: &WorktreeMetadata,
    i: &git::Inspection<'_>,
) -> Result<()> {
    let v = verify(root, m, i).await.map_err(|Failure(_, e)| e)?;
    anyhow::ensure!(
        v.exists && v.registered && journal(m)?.is_none(),
        "Result is stale, missing or has pending cleanup; Apply is blocked."
    );
    Ok(())
}

pub(crate) async fn cleanup(
    root: &Path,
    m: &WorktreeMetadata,
    cancellation: &Cancellation,
    safe: &AtomicBool,
) -> Result<Report> {
    let i = git::Inspection { cancellation, safe };
    verify(root, m, &i).await.map_err(|Failure(_, e)| e)?;
    write_journal(root, m, false)?;
    // Recheck after durable intent and immediately before the destructive command.
    let verified = verify(root, m, &i).await.map_err(|Failure(_, e)| e)?;
    if verified.registered {
        let mut spec = git::command(
            &m.repository.path,
            &["worktree", "remove", "--force", "--force", "--"],
        );
        spec.args.push(git::path_argument(&m.path));
        let output = i.capture(spec, 8192).await?;
        anyhow::ensure!(
            output.status.success(),
            "Could not remove isolated worktree. Close programs using its files and inspect Git before retrying. Git: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    let after = verify(root, m, &i).await.map_err(|Failure(_, e)| e)?;
    anyhow::ensure!(
        !after.exists && !after.registered,
        "Cleanup is incomplete; result metadata has been retained."
    );
    write_journal(root, m, true)?;
    Ok(Report::unavailable(
        Availability::Cleaned,
        "Cleanup completed; ownership record retained.",
    ))
}

/// Discover unreferenced attempts without following links or removing anything.
/// Bounded diagnostics avoid turning a large storage directory into a UI log.
pub fn orphans(root: &Path, referenced: &HashSet<PathBuf>) -> Result<Vec<PathBuf>> {
    if absent(root)? {
        return Ok(Vec::new());
    }
    ordinary(root, true)?;
    anyhow::ensure!(
        root.canonicalize()? == root,
        "Worktree storage now resolves elsewhere."
    );
    let mut found = Vec::new();
    for entry in fs::read_dir(root)? {
        let path = entry?.path();
        let tree = path.join("tree");
        if referenced.contains(&tree) {
            continue;
        }
        let resolved = (|| -> Result<bool> {
            ordinary(&path, true)?;
            let m: WorktreeMetadata = serde_json::from_slice(&bounded(&path.join("owner.json"))?)?;
            anyhow::ensure!(m.path == tree, "Manifest location mismatch.");
            manifest(root, &m)?;
            Ok(journal(&m)?.is_some_and(|r| r.completed) && absent(&tree)?)
        })()
        .unwrap_or(false);
        if !resolved {
            found.push(path);
        }
        if found.len() >= 100 {
            break;
        }
    }
    Ok(found)
}
