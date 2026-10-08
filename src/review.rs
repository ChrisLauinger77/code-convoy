//! Cached, Git-derived review of the effective working tree. No live index edits.
use crate::{git, process::CommandSpec};
use anyhow::{Context, Result};
use std::{collections::HashSet, path::Path};

const LIMIT: usize = 32 * 1024 * 1024;
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Statistics {
    pub files: usize,
    pub additions: u64,
    pub deletions: u64,
    pub binary: usize,
    /// Alternative staged contents cannot be flattened into the working image.
    pub index_alternatives: usize,
    pub(crate) tree: Option<String>,
}
impl Statistics {
    pub fn label(&self) -> String {
        if self.files == 0 && self.index_alternatives == 0 {
            return "No changes".into();
        }
        let mut label = format!(
            "{} {}  +{} −{}",
            self.files,
            if self.files == 1 { "file" } else { "files" },
            self.additions,
            self.deletions
        );
        if self.binary > 0 {
            label.push_str(&format!(" · {} binary", self.binary));
        }
        if self.index_alternatives > 0 {
            label.push_str(&format!(
                " · {} staged alternatives (see Diff)",
                self.index_alternatives
            ));
        }
        label
    }
}
#[derive(Default)]
pub struct Totals {
    pub repositories: usize,
    pub succeeded: usize,
    pub failed: usize,
    pub cancelled: usize,
    pub measured: usize,
    pub statistics: Statistics,
}
pub fn totals(run: &crate::domain::Run) -> Totals {
    use crate::domain::JobStatus;
    let mut total = Totals {
        repositories: run.jobs.len(),
        ..Totals::default()
    };
    for job in &run.jobs {
        match job.status {
            JobStatus::Succeeded => total.succeeded += 1,
            JobStatus::Failed => total.failed += 1,
            JobStatus::Cancelled => total.cancelled += 1,
            _ => {}
        }
        if let Some(Ok(s)) = &job.review {
            total.measured += 1;
            total.statistics.files += s.files;
            total.statistics.additions += s.additions;
            total.statistics.deletions += s.deletions;
            total.statistics.binary += s.binary;
            total.statistics.index_alternatives += s.index_alternatives;
        }
    }
    total
}
pub(crate) async fn checked(i: &git::Inspection<'_>, spec: CommandSpec) -> Result<Vec<u8>> {
    let out = i.capture(spec, LIMIT).await?;
    anyhow::ensure!(
        out.status.success(),
        "Git result inspection failed: {}",
        String::from_utf8_lossy(&out.stderr).trim()
    );
    anyhow::ensure!(
        !out.truncated,
        "Result exceeds the 32 MiB inspection/patch limit; retained files were preserved."
    );
    Ok(out.stdout)
}
fn names(bytes: &[u8]) -> HashSet<&[u8]> {
    bytes.split(|b| *b == 0).filter(|p| !p.is_empty()).collect()
}

pub(crate) struct Snapshot {
    pub tree: String,
    pub statistics: Statistics,
}
impl Snapshot {
    pub async fn capture(path: &Path, base: &str, i: &git::Inspection<'_>) -> Result<Self> {
        let flags = i.checked(path, &["ls-files", "-v", "-z"]).await?;
        anyhow::ensure!(
            flags
                .split(|b| *b == 0)
                .filter(|f| !f.is_empty())
                .all(|f| f[0] == b'H'),
            "Sparse, unmerged or assume-unchanged index entries are unsupported; inspect the result in Git."
        );
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
        let mut attrs = git::command(path, &["check-attr", "-z", "--stdin", "filter"]);
        attrs.input = Some(files);
        let attrs = checked(i, attrs).await?;
        let fields: Vec<_> = attrs.split(|b| *b == 0).filter(|r| !r.is_empty()).collect();
        anyhow::ensure!(
            fields
                .chunks(3)
                .all(|r| r.len() == 3 && matches!(r[2], b"unspecified" | b"unset")),
            "Custom content filters are unsupported for Review statistics/Apply; inspect the result in Git."
        );
        // Import entries without stat-cache data. Copying the real index can lose
        // Git's racy-clean protection when the copy gets a newer modification time.
        // Keep staged/ignored additions, but force every working file to be read.
        let entries = i.checked(path, &["ls-files", "--stage", "-z"]).await?;
        let directory = tempfile::tempdir()?;
        let index = directory.path().join("index");
        let index_argument = git::path_argument(&index)
            .into_string()
            .map_err(|_| anyhow::anyhow!("Temporary index path is not Unicode."))?;
        let make = |args: &[&str]| {
            let mut spec = git::command(path, args);
            let mut hooks = std::ffi::OsString::from("core.hooksPath=");
            hooks.push(git::path_argument(directory.path()));
            spec.args.splice(
                0..0,
                [
                    std::ffi::OsString::from("-c"),
                    hooks,
                    "-c".into(),
                    "core.ignorestat=false".into(),
                    "-c".into(),
                    "core.splitIndex=false".into(),
                ],
            );
            spec.env
                .push(("GIT_INDEX_FILE".into(), index_argument.clone()));
            spec
        };
        let mut import = make(&["update-index", "-z", "--index-info"]);
        import.input = Some(entries);
        checked(i, import).await?;
        checked(i, make(&["add", "--all", "--", "."])).await?;
        let tree = String::from_utf8(checked(i, make(&["write-tree"])).await?)?
            .trim()
            .to_owned();
        for revision in [base, tree.as_str()] {
            let entries = i.checked(path, &["ls-tree", "-r", "-z", revision]).await?;
            anyhow::ensure!(
                !entries
                    .split(|b| *b == 0)
                    .any(|r| r.starts_with(b"160000 ")),
                "Submodule/nested repository statistics are unavailable; inspect these changes in Git."
            );
        }
        let stats = i
            .checked(
                path,
                &[
                    "diff",
                    "--numstat",
                    "-z",
                    "--no-renames",
                    "--no-ext-diff",
                    "--no-textconv",
                    base,
                    &tree,
                    "--",
                ],
            )
            .await?;
        let mut statistics = parse_numstat(&stats)?;
        let staged = i
            .checked(
                path,
                &[
                    "diff",
                    "--cached",
                    "--name-only",
                    "-z",
                    "--no-renames",
                    "--no-ext-diff",
                    base,
                    "--",
                ],
            )
            .await?;
        let alternatives = i
            .checked(
                path,
                &[
                    "diff",
                    "--cached",
                    "--name-only",
                    "-z",
                    "--no-renames",
                    "--no-ext-diff",
                    &tree,
                    "--",
                ],
            )
            .await?;
        statistics.index_alternatives = names(&staged).intersection(&names(&alternatives)).count();
        statistics.tree = Some(tree.clone());
        Ok(Self { tree, statistics })
    }
    pub async fn patch(&self, path: &Path, base: &str, i: &git::Inspection<'_>) -> Result<Vec<u8>> {
        checked(
            i,
            git::command(
                path,
                &[
                    "diff",
                    "--binary",
                    "--full-index",
                    "--no-renames",
                    "--no-ext-diff",
                    "--no-textconv",
                    "--no-color",
                    "--src-prefix=a/",
                    "--dst-prefix=b/",
                    base,
                    &self.tree,
                    "--",
                ],
            ),
        )
        .await
    }
}
pub(crate) fn parse_numstat(bytes: &[u8]) -> Result<Statistics> {
    let mut s = Statistics::default();
    for record in bytes.split(|b| *b == 0).filter(|r| !r.is_empty()) {
        let mut parts = record.splitn(3, |b| *b == b'\t');
        let additions = parts.next().context("Missing additions")?;
        let deletions = parts.next().context("Missing deletions")?;
        anyhow::ensure!(
            parts.next().is_some_and(|p| !p.is_empty()),
            "Invalid Git statistics path"
        );
        s.files += 1;
        if additions == b"-" && deletions == b"-" {
            s.binary += 1;
        } else {
            s.additions += std::str::from_utf8(additions)?.parse::<u64>()?;
            s.deletions += std::str::from_utf8(deletions)?.parse::<u64>()?;
        }
    }
    Ok(s)
}
pub async fn direct(path: &Path) -> Result<Statistics> {
    let i = git::Inspection {
        cancellation: &crate::process::Cancellation::default(),
        safe: &std::sync::atomic::AtomicBool::new(true),
    };
    let state = i.status(path).await?;
    let base = state
        .summary
        .head
        .context("Current working tree has no commit; statistics unavailable.")?;
    Ok(Snapshot::capture(path, &base, &i).await?.statistics)
}
