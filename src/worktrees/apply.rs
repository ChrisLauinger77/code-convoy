//! Explicit, conservative transfer into a clean registered working tree.
//! Caller holds the manager's exclusive lifecycle lease and admin mutex.
use crate::{
    domain::WorktreeMetadata,
    git,
    review::{self, Snapshot, Statistics},
};
use anyhow::{Context, Result};
use std::path::Path;

#[derive(Debug)]
pub enum Outcome {
    Applied(Statistics),
    Blocked(String),
    /// A write was attempted: never infer rollback or automatically retry.
    Uncertain(String),
}

async fn destination(m: &WorktreeMetadata, i: &git::Inspection<'_>) -> Result<()> {
    super::ordinary(&m.repository.path, true)?;
    supported(&m.repository.path, &m.base_commit, i).await?;
    let state = i.status(&m.repository.path).await?;
    anyhow::ensure!(
        state.summary.common_dir.as_ref() == Some(&m.common_dir),
        "Registered repository identity changed."
    );
    anyhow::ensure!(
        state.summary.head.as_ref() == Some(&m.base_commit),
        "Repository HEAD changed from the isolated result's base."
    );
    anyhow::ensure!(
        state.entries.is_empty(),
        "Registered working tree has local changes."
    );
    Ok(())
}

/// Do not silently flatten submodules, filters, sparse trees, or hidden index edits.
pub(crate) async fn supported(path: &Path, base: &str, i: &git::Inspection<'_>) -> Result<()> {
    let flags = i.checked(path, &["ls-files", "-v", "-z"]).await?;
    anyhow::ensure!(
        flags
            .split(|b| *b == 0)
            .filter(|r| !r.is_empty())
            .all(|r| r[0] == b'H'),
        "Sparse, unmerged or assume-unchanged entries are unsupported."
    );
    for args in [
        vec!["ls-tree", "-r", "-z", base],
        vec!["ls-files", "--stage", "-z"],
    ] {
        let entries = i.checked(path, &args).await?;
        anyhow::ensure!(
            !entries
                .split(|b| *b == 0)
                .any(|r| r.starts_with(b"160000 ")),
            "Submodule/nested repository results are unsupported; retained files were preserved."
        );
    }
    let files = i
        .checked(
            path,
            &[
                "ls-files",
                "--cached",
                "--others",
                "--exclude-standard",
                "-z",
            ],
        )
        .await?;
    let mut spec = git::command(
        path,
        &[
            "check-attr",
            "-z",
            "--stdin",
            "filter",
            "working-tree-encoding",
            "text",
            "eol",
            "ident",
        ],
    );
    spec.input = Some(files);
    let attrs = review::checked(i, spec).await?;
    let fields: Vec<_> = attrs.split(|b| *b == 0).filter(|r| !r.is_empty()).collect();
    anyhow::ensure!(
        fields
            .chunks(3)
            .all(|r| r.len() == 3 && matches!(r[2], b"unspecified" | b"unset")),
        "Content conversion attributes (filter, encoding, text, eol or ident) are unsupported for safe Apply."
    );
    let config = i
        .capture(
            git::command(path, &["config", "--get", "core.autocrlf"]),
            8192,
        )
        .await?;
    anyhow::ensure!(
        !config.status.success() || config.stdout.starts_with(b"false"),
        "core.autocrlf conversion is unsupported for safe Apply."
    );
    Ok(())
}
async fn platform_modes(m: &WorktreeMetadata, tree: &str, i: &git::Inspection<'_>) -> Result<()> {
    let raw = i
        .checked(
            &m.path,
            &[
                "diff",
                "--raw",
                "--no-renames",
                "-z",
                &m.base_commit,
                tree,
                "--",
            ],
        )
        .await?;
    let mut modes = false;
    let mut symlinks = false;
    // Raw -z alternates a header and a native path (no rename pairs).
    let records: Vec<_> = raw.split(|b| *b == 0).filter(|r| !r.is_empty()).collect();
    for pair in records.chunks(2) {
        let fields: Vec<_> = pair[0].split(|b| *b == b' ').collect();
        anyhow::ensure!(
            fields.len() == 5 && pair.len() == 2,
            "Invalid Git mode diff."
        );
        let old = fields[0]
            .strip_prefix(b":")
            .context("Invalid Git mode header")?;
        let new = fields[1];
        if old != new {
            modes |= old == b"100755" || new == b"100755";
        }
        symlinks |= old == b"120000" || new == b"120000";
    }
    for (needed, key, message) in [
        (
            modes,
            "core.filemode",
            "Executable mode changes cannot be preserved with core.filemode=false.",
        ),
        (
            symlinks,
            "core.symlinks",
            "Symlink changes cannot be preserved with core.symlinks=false.",
        ),
    ] {
        if needed {
            let out = i
                .capture(
                    git::command(&m.repository.path, &["config", "--bool", "--get", key]),
                    8192,
                )
                .await?;
            anyhow::ensure!(
                out.status.code() == Some(1)
                    || (out.status.success() && out.stdout.starts_with(b"true")),
                "{message}"
            );
        }
    }
    Ok(())
}

async fn preflight(
    root: &Path,
    m: &WorktreeMetadata,
    expected: &str,
    i: &git::Inspection<'_>,
) -> Result<(Snapshot, Vec<u8>)> {
    super::recovery::verify_available(root, m, i).await?;
    destination(m, i).await?;
    supported(&m.path, &m.base_commit, i).await?;
    let snapshot = Snapshot::capture(&m.path, &m.base_commit, i).await?;
    anyhow::ensure!(
        snapshot.tree == expected,
        "Isolated result changed since Review; refresh Review before applying."
    );
    supported(&m.path, &snapshot.tree, i).await?;
    anyhow::ensure!(
        snapshot.statistics.index_alternatives == 0,
        "Result has staged contents that differ from the working files; inspect both versions in Diff. Apply cannot preserve both."
    );
    anyhow::ensure!(
        snapshot.statistics.files > 0,
        "Result has no changes to apply."
    );
    platform_modes(m, &snapshot.tree, i).await?;
    let patch = snapshot.patch(&m.path, &m.base_commit, i).await?;
    let mut spec = git::command(
        &m.repository.path,
        &["apply", "--check", "--binary", "--whitespace=nowarn", "-"],
    );
    spec.input = Some(patch.clone());
    review::checked(i, spec)
        .await
        .context("Patch preflight failed; destination was not changed.")?;
    // No await by the UI and no release of the exclusive lease between these steps.
    super::recovery::verify_available(root, m, i).await?;
    let recheck = Snapshot::capture(&m.path, &m.base_commit, i).await?;
    anyhow::ensure!(
        snapshot.tree == recheck.tree && recheck.statistics.index_alternatives == 0,
        "Isolated result changed during preflight; review it again."
    );
    destination(m, i).await?;
    Ok((snapshot, patch))
}
pub(crate) async fn apply(
    root: &Path,
    m: &WorktreeMetadata,
    expected: &str,
    i: &git::Inspection<'_>,
) -> Outcome {
    let (snapshot, patch) = match preflight(root, m, expected, i).await {
        Ok(p) => p,
        Err(e) => return Outcome::Blocked(format!("Apply blocked: {e:#}")),
    };
    let mut spec = git::command(
        &m.repository.path,
        &["apply", "--binary", "--whitespace=nowarn", "-"],
    );
    spec.input = Some(patch);
    let result = async {
        review::checked(i, spec).await?;
        let actual = Snapshot::capture(&m.repository.path, &m.base_commit, i).await?;
        anyhow::ensure!(
            actual.tree == snapshot.tree,
            "Destination does not exactly match the reviewed result."
        );
        let head = i
            .checked(&m.repository.path, &["rev-parse", "HEAD"])
            .await?;
        anyhow::ensure!(
            String::from_utf8_lossy(&head).trim() == m.base_commit,
            "Destination HEAD changed during Apply."
        );
        let staged = i
            .checked(
                &m.repository.path,
                &[
                    "diff",
                    "--cached",
                    "--name-only",
                    "-z",
                    &m.base_commit,
                    "--",
                ],
            )
            .await?;
        anyhow::ensure!(staged.is_empty(), "Destination index changed during Apply.");
        Ok::<_, anyhow::Error>(())
    }
    .await;
    match result {
        Ok(()) => Outcome::Applied(snapshot.statistics),
        Err(e) => Outcome::Uncertain(format!(
            "Apply preflight passed, but application or verification failed. Destination may contain changes; inspect it manually. No rollback was attempted. Isolated result retained. {e:#}"
        )),
    }
}
