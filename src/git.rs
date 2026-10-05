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

fn command(path: &Path, args: &[&str]) -> CommandSpec {
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
async fn checked(path: &Path, args: &[&str]) -> Result<Vec<u8>> {
    let output = process::capture(command(path, args), LIMIT).await?;
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
pub async fn register(path: &Path) -> Result<Repository> {
    let path = path
        .canonicalize()
        .with_context(|| format!("Repository path does not exist: {}", path.display()))?;
    anyhow::ensure!(path.is_dir(), "Choose a repository directory.");
    let inside = checked(&path, &["rev-parse", "--is-inside-work-tree"]).await?;
    anyhow::ensure!(
        inside.starts_with(b"true"),
        "Select a Git working tree; bare repositories cannot run tasks."
    );
    // --show-prefix avoids decoding an absolute path emitted by Git (notably on Windows).
    let prefix = checked(&path, &["rev-parse", "--show-prefix"]).await?;
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
pub async fn status(path: &Path) -> Result<WorkingTree> {
    let repo = register(path).await?;
    anyhow::ensure!(
        repo.path == path,
        "Repository path now resolves to a different location; register it again."
    );
    let bytes = checked(
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
    let branch = process::capture(
        command(path, &["symbolic-ref", "--quiet", "--short", "HEAD"]),
        8192,
    )
    .await?;
    let branch = if branch.status.success() {
        String::from_utf8_lossy(&branch.stdout).trim().to_owned()
    } else {
        "(detached HEAD)".into()
    };
    let head = process::capture(command(path, &["rev-parse", "--verify", "HEAD"]), 8192).await?;
    let head = head
        .status
        .success()
        .then(|| String::from_utf8_lossy(&head.stdout).trim().to_owned());
    Ok(WorkingTree {
        summary: GitSummary {
            branch,
            head,
            changed: entries.len(),
        },
        entries,
    })
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
