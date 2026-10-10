//! Location preferences never grant cleanup authority. Custom attempts are bound
//! to the original Store by a second, immutable record outside the chosen base.
use super::ordinary;
use crate::domain::WorktreeMetadata;
use anyhow::{Context, Result};
use std::{
    fs,
    io::{Read, Write},
    path::{Component, Path, PathBuf},
};

const RECORDS: &str = ".locations";

fn absolute(path: &Path) -> Result<()> {
    anyhow::ensure!(
        path.is_absolute()
            && !path
                .components()
                .any(|c| matches!(c, Component::ParentDir | Component::CurDir)),
        "Worktree location must be an absolute directory without '.' or '..' components."
    );
    // Path::components normalizes interior dots; reject them in the original spelling too.
    let text = path
        .to_str()
        .context("Worktree location must be a Unicode path.")?;
    anyhow::ensure!(
        !text
            .split(std::path::is_separator)
            .any(|p| p == "." || p == ".."),
        "Worktree location cannot contain path traversal components."
    );
    Ok(())
}

fn exists(path: &Path) -> Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => {
            Err(e).with_context(|| format!("Cannot inspect worktree location {}.", path.display()))
        }
    }
}

/// Filesystem-only validation, run on a blocking worker. Missing directories are
/// resolved without creating them; reservation checks again and creates them.
pub fn validate_base(path: &Path) -> Result<PathBuf> {
    absolute(path)?;
    let mut missing = Vec::new();
    let mut parent = path;
    while !exists(parent)? {
        missing.push(parent.file_name().context("Invalid worktree location.")?);
        parent = parent
            .parent()
            .context("Worktree location has no existing parent.")?;
    }
    ordinary(parent, true).context("Cannot use worktree storage; choose an ordinary directory.")?;
    // Resolve OS aliases such as macOS /var before checking the canonical chain.
    let mut resolved = parent
        .canonicalize()
        .context("Cannot resolve worktree location.")?;
    for ancestor in resolved.ancestors() {
        ordinary(ancestor, true)?;
        anyhow::ensure!(
            !exists(&ancestor.join(".git"))? && !exists(&ancestor.join("owner.json"))?,
            "Worktree location must be outside Git working trees and managed worktree attempts."
        );
    }
    for part in missing.iter().rev() {
        resolved.push(part);
    }
    Ok(resolved)
}

/// Resolve a preference without permitting use of the Store's private children.
pub fn resolve_base(storage: &Path, custom: Option<&Path>) -> Result<PathBuf> {
    let base = validate_base(custom.unwrap_or(storage))?;
    anyhow::ensure!(
        base == storage || !base.starts_with(storage),
        "Choose a worktree location outside CodeConvoy's internal worktree storage."
    );
    Ok(base)
}

fn record_path(root: &Path, m: &WorktreeMetadata) -> Result<PathBuf> {
    let name = m
        .path
        .parent()
        .and_then(Path::file_name)
        .and_then(|n| n.to_str())
        .context("Invalid worktree attempt identity.")?;
    let prefix = format!("run-{}-job-{}-", m.run, m.job);
    anyhow::ensure!(
        name.starts_with(&prefix)
            && name.len() > prefix.len()
            && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-'),
        "Invalid worktree attempt identity."
    );
    Ok(root.join(RECORDS).join(format!("{name}.json")))
}

fn trusted_records(root: &Path) -> Result<PathBuf> {
    ordinary(root, true)?;
    anyhow::ensure!(
        root.canonicalize()? == root,
        "Worktree storage now resolves elsewhere."
    );
    let records = root.join(RECORDS);
    ordinary(&records, true)?;
    anyhow::ensure!(
        records.canonicalize()? == records,
        "Worktree location records now resolve elsewhere."
    );
    Ok(records)
}

/// Durable exact ownership binding, written before Git creates a custom checkout.
pub(super) fn record(root: &Path, m: &WorktreeMetadata) -> Result<()> {
    if m.path.parent().and_then(Path::parent) == Some(root) {
        return Ok(());
    }
    anyhow::ensure!(
        validate_base(root)? == root,
        "Worktree ownership storage changed location."
    );
    fs::create_dir_all(root).context("Cannot create application worktree ownership storage.")?;
    ordinary(root, true)?;
    anyhow::ensure!(
        root.canonicalize()? == root,
        "Worktree ownership storage changed location."
    );
    let records = root.join(RECORDS);
    match fs::create_dir(&records) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(e) => return Err(e).context("Cannot create worktree location records."),
    }
    trusted_records(root)?;
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(record_path(root, m)?)
        .context("Cannot record original worktree location; Git has not been started.")?;
    serde_json::to_writer_pretty(&mut file, m)?;
    file.write_all(b"\n")?;
    file.sync_all()?;
    #[cfg(unix)]
    {
        fs::File::open(&records)?.sync_all()?;
        fs::File::open(root)?.sync_all()?;
    }
    Ok(())
}

/// Never authorize a path from the preference or a sibling manifest alone.
pub(super) fn recorded_root(root: &Path, m: &WorktreeMetadata) -> Result<PathBuf> {
    absolute(&m.path)?;
    let original = m
        .path
        .parent()
        .and_then(Path::parent)
        .context("Missing original worktree location.")?;
    if original == root {
        return Ok(root.to_owned());
    }
    trusted_records(root)?;
    let path = record_path(root, m)?;
    ordinary(&path, false)
        .context("Original worktree location record is missing or unsafe; cleanup is blocked.")?;
    let mut bytes = Vec::new();
    fs::File::open(path)?
        .take(64 * 1024 + 1)
        .read_to_end(&mut bytes)?;
    anyhow::ensure!(
        bytes.len() <= 64 * 1024,
        "Worktree location record is too large."
    );
    let recorded: WorktreeMetadata = serde_json::from_slice(&bytes)?;
    anyhow::ensure!(
        recorded == *m,
        "Original worktree location record does not match this result."
    );
    Ok(original.to_owned())
}

/// Include original custom locations in orphan diagnostics after settings change.
pub(super) fn recorded_attempts(root: &Path) -> Result<Vec<PathBuf>> {
    if !exists(&root.join(RECORDS))? {
        return Ok(Vec::new());
    }
    let records = trusted_records(root)?;
    let mut paths = Vec::new();
    for entry in fs::read_dir(records)? {
        let path = entry?.path();
        ordinary(&path, false)?;
        let m: WorktreeMetadata = serde_json::from_reader(fs::File::open(&path)?.take(64 * 1024))?;
        anyhow::ensure!(
            record_path(root, &m)? == path,
            "Worktree location record identity mismatch."
        );
        recorded_root(root, &m)?;
        paths.push(
            m.path
                .parent()
                .context("Missing attempt directory.")?
                .to_owned(),
        );
    }
    Ok(paths)
}
