use crate::{
    domain::{GitSummary, Repository},
    process::{self, CommandSpec},
};
use anyhow::{Context, Result};
use std::path::Path;

const LIMIT: usize = 2 * 1024 * 1024;
#[derive(Debug, Clone)]
pub struct WorkingTree {
    pub summary: GitSummary,
    pub entries: Vec<String>,
}

/// Spell a native path for Git's command-line parser without changing the
/// canonical identity stored by CodeConvoy. Git for Windows rejects the verbatim
/// prefix produced by Rust's canonicalize when creating a worktree. Preserve OS
/// strings (including Unicode) and convert only drive and UNC prefixes.
pub fn path_argument(path: &Path) -> std::ffi::OsString {
    #[cfg(windows)]
    {
        use std::{
            ffi::OsString,
            path::{Component, Prefix},
        };
        let mut components = path.components();
        if let Some(Component::Prefix(prefix)) = components.next() {
            let mut argument = match prefix.kind() {
                Prefix::VerbatimDisk(drive) => OsString::from(format!("{}:", char::from(drive))),
                Prefix::VerbatimUNC(server, share) => {
                    let mut unc = OsString::from(r"\\");
                    unc.push(server);
                    unc.push(r"\");
                    unc.push(share);
                    unc
                }
                _ => return path.as_os_str().to_owned(),
            };
            argument.push(components.as_path());
            return argument;
        }
    }
    path.as_os_str().to_owned()
}

pub(crate) fn command(path: &Path, args: &[&str]) -> CommandSpec {
    let mut spec = CommandSpec::new("git", path).args(&[
        "--no-pager",
        "-c",
        "core.fsmonitor=false",
        "-c",
        "core.quotePath=true",
    ]);
    spec.args.extend(args.iter().map(std::ffi::OsString::from));
    // A desktop app launched inside a Git hook must still inspect the chosen repo.
    spec.remove_env = std::env::vars_os()
        .filter_map(|(key, _)| {
            let key = key.to_str()?;
            key.starts_with("GIT_").then(|| key.to_owned())
        })
        .collect();
    spec.env = vec![
        ("GIT_OPTIONAL_LOCKS".into(), "0".into()),
        ("GIT_TERMINAL_PROMPT".into(), "0".into()),
    ];
    spec
}
/// Inspection owned by a worker. Every subprocess observes cancellation and
/// confirms cleanup before the worker may release its repository lease.
pub(crate) struct Inspection<'a> {
    pub cancellation: &'a crate::process::Cancellation,
    pub safe: &'a std::sync::atomic::AtomicBool,
}
impl Inspection<'_> {
    /// Count distinct paths in HEAD, index and working files against the reviewed
    /// HEAD, plus nonignored untracked files. No index/object writes.
    pub(crate) async fn completion_changes(
        &self,
        path: &Path,
        base: Option<&str>,
    ) -> Result<crate::visibility::Changes> {
        let repo = self.register(path).await?;
        anyhow::ensure!(
            repo.path == path,
            "Repository identity changed during execution."
        );
        let mut names = std::collections::HashSet::new();
        let mut add = |bytes: Vec<u8>| {
            names.extend(
                bytes
                    .split(|b| *b == 0)
                    .filter(|p| !p.is_empty())
                    .map(Vec::from),
            );
        };
        if let Some(base) = base {
            // HEAD can differ even when the agent stages the reviewed image
            // again. Neither single-commit diff form includes that change.
            for target in [
                vec![base, "--"],
                vec!["--cached", base, "--"],
                vec![base, "HEAD", "--"],
            ] {
                let mut args = vec![
                    "diff",
                    "--name-only",
                    "-z",
                    "--no-renames",
                    "--no-ext-diff",
                    "--no-textconv",
                    "--ignore-submodules=none",
                ];
                args.extend(target);
                add(self.checked(path, &args).await?);
            }
            add(self
                .checked(path, &["ls-files", "--others", "--exclude-standard", "-z"])
                .await?);
        } else {
            // An unborn reviewed HEAD has an empty tracked baseline.
            // Include a newly committed tree even if its paths are now deleted
            // from the working tree and index. No hard-coded empty-tree hash.
            let head = self
                .capture(
                    command(path, &["rev-parse", "--verify", "--quiet", "HEAD"]),
                    8192,
                )
                .await?;
            match head.status.code() {
                Some(0) => {
                    anyhow::ensure!(!head.truncated, "Git HEAD inspection was truncated.");
                    let head =
                        String::from_utf8(head.stdout).context("Git returned an invalid HEAD.")?;
                    add(self
                        .checked(path, &["ls-tree", "-r", "--name-only", "-z", head.trim()])
                        .await?);
                }
                Some(1) => {} // Still unborn: no committed paths to include.
                _ => anyhow::bail!(
                    "Cannot inspect completion HEAD: {}",
                    String::from_utf8_lossy(&head.stderr).trim()
                ),
            }
            add(self
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
                .await?);
        }
        Ok(crate::visibility::Changes {
            changed: Some(!names.is_empty()),
            files: Some(names.len()),
        })
    }
    pub(crate) async fn capture(
        &self,
        spec: CommandSpec,
        limit: usize,
    ) -> Result<process::Captured> {
        anyhow::ensure!(
            self.safe.load(std::sync::atomic::Ordering::Acquire),
            "Git inspection skipped because process cleanup is unconfirmed."
        );
        process::capture_owned(
            spec,
            limit,
            self.cancellation,
            std::time::Duration::from_secs(20),
            self.safe,
        )
        .await
    }
    pub(crate) async fn checked(&self, path: &Path, args: &[&str]) -> Result<Vec<u8>> {
        let output = self.capture(command(path, args), LIMIT).await?;
        anyhow::ensure!(
            output.status.success(),
            "Git inspection failed in {}: {}",
            path.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        );
        anyhow::ensure!(
            !output.truncated,
            "Git output exceeded 2 MiB; inspect this repository in Git before running."
        );
        Ok(output.stdout)
    }
    pub(crate) async fn register(&self, path: &Path) -> Result<Repository> {
        let path = path
            .canonicalize()
            .with_context(|| format!("Repository path does not exist: {}", path.display()))?;
        anyhow::ensure!(path.is_dir(), "Choose a repository directory.");
        let inside = self
            .checked(&path, &["rev-parse", "--is-inside-work-tree"])
            .await?;
        anyhow::ensure!(
            inside.starts_with(b"true"),
            "Select a Git working tree; bare repositories cannot run tasks."
        );
        // --show-prefix avoids decoding an absolute path emitted by Git (notably on Windows).
        let prefix = self.checked(&path, &["rev-parse", "--show-prefix"]).await?;
        anyhow::ensure!(
            prefix == b"\n" || prefix == b"\r\n",
            "Select the repository root, not a subdirectory: {}",
            path.display()
        );
        let name = path
            .file_name()
            .unwrap_or(path.as_os_str())
            .to_string_lossy()
            .into_owned();
        Ok(Repository { name, path })
    }
    pub(crate) async fn status(&self, path: &Path) -> Result<WorkingTree> {
        let repo = self.register(path).await?;
        anyhow::ensure!(
            repo.path == path,
            "Repository path now resolves to a different location; register it again."
        );
        let bytes = self
            .checked(
                path,
                &[
                    "status",
                    "--porcelain=v1",
                    "-z",
                    "--untracked-files=all",
                    "--ignore-submodules=none",
                ],
            )
            .await?;
        let entries = parse_status(&bytes)?;
        let branch = self
            .capture(
                command(path, &["symbolic-ref", "--quiet", "--short", "HEAD"]),
                8192,
            )
            .await?;
        let branch = if branch.status.success() {
            String::from_utf8_lossy(&branch.stdout).trim().to_owned()
        } else if branch.status.code() == Some(1) {
            "(detached HEAD)".into()
        } else {
            anyhow::bail!(
                "Cannot inspect branch: {}",
                String::from_utf8_lossy(&branch.stderr).trim()
            );
        };
        let head = self
            .capture(command(path, &["rev-parse", "--verify", "HEAD"]), 8192)
            .await?;
        let head = head
            .status
            .success()
            .then(|| String::from_utf8_lossy(&head.stdout).trim().to_owned());
        Ok(WorkingTree {
            summary: GitSummary {
                common_dir: Some(self.common_dir(path).await?),
                branch,
                head,
                changed: entries.len(),
            },
            entries,
        })
    }
    pub(crate) async fn common_dir(&self, path: &Path) -> Result<std::path::PathBuf> {
        let mut bytes = self
            .checked(path, &["rev-parse", "--git-common-dir"])
            .await?;
        if bytes.last() == Some(&b'\n') {
            bytes.pop();
        }
        #[cfg(windows)]
        if bytes.last() == Some(&b'\r') {
            bytes.pop();
        }
        #[cfg(unix)]
        let value = {
            use std::os::unix::ffi::OsStringExt;
            std::ffi::OsString::from_vec(bytes)
        };
        #[cfg(not(unix))]
        let value = std::ffi::OsString::from(
            String::from_utf8(bytes).context("Git returned an invalid repository path.")?,
        );
        // Git may return a relative path for the main checkout and an absolute
        // path for a linked worktree. Resolve either without requiring newer flags.
        path.join(value)
            .canonicalize()
            .context("Cannot resolve Git common directory; register the repository again.")
    }
}
pub async fn register(path: &Path) -> Result<Repository> {
    Inspection {
        cancellation: &crate::process::Cancellation::default(),
        safe: &std::sync::atomic::AtomicBool::new(true),
    }
    .register(path)
    .await
}
pub async fn status(path: &Path) -> Result<WorkingTree> {
    Inspection {
        cancellation: &crate::process::Cancellation::default(),
        safe: &std::sync::atomic::AtomicBool::new(true),
    }
    .status(path)
    .await
}
/// Canonical common Git administration identity, shared by linked worktrees.
pub async fn common_dir(path: &Path) -> Result<std::path::PathBuf> {
    Inspection {
        cancellation: &crate::process::Cancellation::default(),
        safe: &std::sync::atomic::AtomicBool::new(true),
    }
    .common_dir(path)
    .await
}
pub fn parse_status(bytes: &[u8]) -> Result<Vec<String>> {
    let mut parts = bytes.split(|b| *b == 0).filter(|p| !p.is_empty());
    let mut entries = Vec::new();
    while let Some(entry) = parts.next() {
        anyhow::ensure!(
            entry.len() >= 4 && entry[2] == b' ',
            "Unexpected Git status format."
        );
        let mut label = String::from_utf8_lossy(entry).escape_debug().to_string();
        if entry[..2].iter().any(|b| *b == b'R' || *b == b'C') {
            let source = parts.next().context("Incomplete Git rename record.")?;
            label.push_str(&format!(
                " (from {})",
                String::from_utf8_lossy(source).escape_debug()
            ));
        }
        entries.push(label);
    }
    Ok(entries)
}
/// Always a live working-tree inspection, not a historical patch snapshot.
pub async fn diff(path: &Path) -> Result<String> {
    let state = status(path).await?;
    let mut text = format!(
        "Current working tree: {}\nBranch: {}\n\n",
        path.display(),
        state.summary.branch
    );
    for (title, args) in [
        (
            "STAGED CHANGES",
            vec![
                "diff",
                "--cached",
                "--no-ext-diff",
                "--no-textconv",
                "--no-color",
                "--",
            ],
        ),
        (
            "UNSTAGED CHANGES",
            vec!["diff", "--no-ext-diff", "--no-textconv", "--no-color", "--"],
        ),
    ] {
        let output = process::capture(command(path, &args), LIMIT).await?;
        anyhow::ensure!(
            output.status.success(),
            "Cannot read diff: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        text.push_str(&format!("--- {title} ---\n"));
        text.push_str(&String::from_utf8_lossy(&output.stdout));
        if output.stdout.is_empty() {
            text.push_str("(none)\n");
        }
        if output.truncated {
            text.push_str("\n[Diff truncated at 2 MiB]\n");
        }
    }
    text.push_str(
        "\n--- STATUS (includes untracked files; their contents are not in Git diff) ---\n",
    );
    text.push_str(&state.entries.join("\n"));
    if state.entries.is_empty() {
        text.push_str("Clean working tree");
    }
    Ok(text)
}

pub async fn diff_job(job: &crate::domain::Job) -> Result<String> {
    match job.execution_mode {
        crate::domain::ExecutionMode::Direct => diff(&job.repository.path).await,
        crate::domain::ExecutionMode::IsolatedWorktree => {
            crate::worktrees::diff(
                job.worktree
                    .as_ref()
                    .context("The isolated worktree has not been prepared.")?,
            )
            .await
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(windows)]
    #[test]
    fn git_paths_preserve_drive_unc_unicode_and_ordinary_paths() {
        for (native, argument) in [
            (r"\\?\C:\storage ü\日本語\tree", r"C:\storage ü\日本語\tree"),
            (
                r"\\?\UNC\server\share\storage ü\tree",
                r"\\server\share\storage ü\tree",
            ),
            (r"C:\storage ü\tree", r"C:\storage ü\tree"),
            (r"\\server\share\tree", r"\\server\share\tree"),
            (r"relative\tree", r"relative\tree"),
            (r"\\.\device", r"\\.\device"),
        ] {
            assert_eq!(path_argument(Path::new(native)), argument);
        }
        // Do not turn an unpaired Windows code unit into a replacement character.
        use std::os::windows::ffi::{OsStrExt, OsStringExt};
        let mut units: Vec<u16> = r"\\?\C:\storage\".encode_utf16().collect();
        units.push(0xd800);
        let native = std::ffi::OsString::from_wide(&units);
        let argument = path_argument(Path::new(&native));
        assert_eq!(argument.encode_wide().collect::<Vec<_>>(), units[4..]);
    }

    #[cfg(unix)]
    #[test]
    fn git_paths_preserve_native_unix_bytes() {
        use std::os::unix::ffi::OsStrExt;
        let native = std::ffi::OsStr::from_bytes(b"/storage \xff/tree");
        assert_eq!(path_argument(Path::new(native)), native);
    }
}

#[cfg(test)]
mod completion_tests {
    #![allow(clippy::unwrap_used)]
    use super::*;
    fn git(path: &Path, args: &[&str]) -> String {
        let out = std::process::Command::new("git")
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
        String::from_utf8(out.stdout).unwrap().trim().into()
    }
    #[tokio::test]
    async fn completion_counts_fixed_base_index_alternatives_and_untracked_without_writes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().canonicalize().unwrap();
        git(&path, &["init", "--quiet"]);
        let cancellation = crate::process::Cancellation::default();
        let safe = std::sync::atomic::AtomicBool::new(true);
        let inspection = Inspection {
            cancellation: &cancellation,
            safe: &safe,
        };
        assert_eq!(
            inspection
                .completion_changes(&path, None)
                .await
                .unwrap()
                .files,
            Some(0)
        );
        std::fs::write(path.join("tracked"), "base").unwrap();
        git(&path, &["add", "."]);
        git(
            &path,
            &[
                "-c",
                "user.name=Fixture",
                "-c",
                "user.email=f@invalid",
                "-c",
                "commit.gpgsign=false",
                "commit",
                "-qm",
                "base",
            ],
        );
        let base = git(&path, &["rev-parse", "HEAD"]);
        assert_eq!(
            inspection
                .completion_changes(&path, Some(&base))
                .await
                .unwrap()
                .changed,
            Some(false)
        );
        std::fs::write(path.join("tracked"), "staged alternative").unwrap();
        git(&path, &["add", "tracked"]);
        std::fs::write(path.join("tracked"), "base").unwrap();
        std::fs::write(path.join("new"), "untracked").unwrap();
        let index = std::fs::read(path.join(".git/index")).unwrap();
        let observation = inspection
            .completion_changes(&path, Some(&base))
            .await
            .unwrap();
        assert_eq!(observation.files, Some(2));
        assert_eq!(observation.changed, Some(true));
        assert_eq!(std::fs::read(path.join(".git/index")).unwrap(), index);
        assert_eq!(git(&path, &["rev-parse", "HEAD"]), base);
        assert!(!path.join(".git/index.lock").exists());
        git(
            &path,
            &[
                "-c",
                "user.name=Fixture",
                "-c",
                "user.email=f@invalid",
                "-c",
                "commit.gpgsign=false",
                "commit",
                "-qm",
                "agent commit",
            ],
        );
        // A later commit cannot hide the changes relative to the reviewed HEAD.
        std::fs::write(path.join("tracked"), "staged alternative").unwrap();
        assert_eq!(
            inspection
                .completion_changes(&path, Some(&base))
                .await
                .unwrap()
                .files,
            Some(2)
        );
        safe.store(false, std::sync::atomic::Ordering::Release);
        assert!(
            inspection
                .completion_changes(&path, Some(&base))
                .await
                .is_err()
        );
    }
    fn commit(path: &Path, message: &str) {
        git(
            path,
            &[
                "-c",
                "user.name=Fixture",
                "-c",
                "user.email=f@invalid",
                "-c",
                "commit.gpgsign=false",
                "commit",
                "-qm",
                message,
            ],
        );
    }

    #[tokio::test]
    async fn completion_keeps_committed_changes_when_original_contents_are_staged_again() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().canonicalize().unwrap();
        git(&path, &["init", "--quiet"]);
        std::fs::write(path.join("tracked"), "reviewed contents").unwrap();
        git(&path, &["add", "."]);
        commit(&path, "base");
        let base = git(&path, &["rev-parse", "HEAD"]);
        std::fs::write(path.join("tracked"), "committed agent changes").unwrap();
        git(&path, &["add", "."]);
        commit(&path, "agent change");
        std::fs::write(path.join("tracked"), "reviewed contents").unwrap();
        git(&path, &["add", "."]);
        assert!(git(&path, &["diff", "--name-only", &base, "--"]).is_empty());
        assert!(git(&path, &["diff", "--cached", "--name-only", &base, "--"]).is_empty());
        assert_committed_observation(&path, Some(&base)).await;
        assert_eq!(
            std::fs::read_to_string(path.join("tracked")).unwrap(),
            "reviewed contents"
        );
    }

    #[tokio::test]
    async fn completion_keeps_first_commit_when_all_committed_paths_are_staged_for_deletion() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().canonicalize().unwrap();
        git(&path, &["init", "--quiet"]);
        // The reviewed repository has no HEAD. The agent creates its first commit,
        // then restores the empty reviewed image in both the index and worktree.
        std::fs::write(path.join("tracked"), "committed agent changes").unwrap();
        git(&path, &["add", "."]);
        commit(&path, "first agent commit");
        git(&path, &["rm", "tracked"]);
        assert!(
            git(
                &path,
                &["ls-files", "--cached", "--others", "--exclude-standard"]
            )
            .is_empty()
        );
        assert_committed_observation(&path, None).await;
        assert!(!path.join("tracked").exists());
    }

    async fn assert_committed_observation(path: &Path, base: Option<&str>) {
        let head = git(path, &["rev-parse", "HEAD"]);
        let index = std::fs::read(path.join(".git/index")).unwrap();
        let cancellation = crate::process::Cancellation::default();
        let safe = std::sync::atomic::AtomicBool::new(true);
        let changes = Inspection {
            cancellation: &cancellation,
            safe: &safe,
        }
        .completion_changes(path, base)
        .await
        .unwrap();
        assert_eq!(changes.changed, Some(true));
        assert_eq!(changes.files, Some(1));
        let mut job = crate::domain::Job::queued(Repository {
            path: path.into(),
            name: "fixture".into(),
        });
        job.status = crate::domain::JobStatus::Succeeded;
        job.completion_changes = Some(changes);
        assert!(crate::visibility::needs_review(&job));
        assert_eq!(git(path, &["rev-parse", "HEAD"]), head);
        assert_eq!(std::fs::read(path.join(".git/index")).unwrap(), index);
        assert!(!path.join(".git/index.lock").exists());
    }
}
