//! Which directory to open comes only from what CodeConvoy recorded: the
//! registration in repository management, and each job's saved execution
//! context in convoy results. Verification reads Git's metadata files; it never
//! runs Git and never creates, restores, repairs or substitutes a directory.
use super::Error;
use crate::domain::{ExecutionMode, Job, JobStatus, Repository, ResultAvailability};
use crate::worktrees::recovery::{line, native};
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// A registered repository, or the working tree a Direct job ran in.
    Repository,
    /// The isolated worktree an IsolatedWorktree job ran in.
    Worktree,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    pub kind: Kind,
    pub path: PathBuf,
    /// The Git common directory recorded for this tree, compared when known.
    pub common_dir: Option<PathBuf>,
}
impl Target {
    pub fn repository(repository: &Repository) -> Self {
        Self {
            kind: Kind::Repository,
            path: repository.path.clone(),
            common_dir: None,
        }
    }

    /// The job's recorded execution location, independent of current settings,
    /// registrations or branches. Never falls back to another directory.
    pub fn job(job: &Job) -> Result<Self, Error> {
        match job.execution_mode {
            ExecutionMode::Direct => Ok(Self {
                kind: Kind::Repository,
                path: job.repository.path.clone(),
                common_dir: job.before.as_ref().and_then(|git| git.common_dir.clone()),
            }),
            ExecutionMode::IsolatedWorktree => {
                let Some(worktree) = &job.worktree else {
                    return Err(Error::Target {
                        path: None,
                        problem: if job.status.is_terminal() {
                            "no isolated worktree was created for this job. Its original repository is not opened instead.".into()
                        } else {
                            "its isolated worktree has not been created yet.".into()
                        },
                    });
                };
                let unavailable = |problem: &str| Error::Target {
                    path: Some(worktree.path.clone()),
                    problem: problem.into(),
                };
                if matches!(job.status, JobStatus::Queued | JobStatus::Preparing) {
                    return Err(unavailable(
                        "the isolated worktree is still being prepared.",
                    ));
                }
                if job.result_availability == ResultAvailability::Cleaned {
                    return Err(unavailable(
                        "this isolated worktree was removed by cleanup. CodeConvoy does not recreate it.",
                    ));
                }
                Ok(Self {
                    kind: Kind::Worktree,
                    path: worktree.path.clone(),
                    common_dir: Some(worktree.common_dir.clone()),
                })
            }
        }
    }

    pub fn describe(&self) -> String {
        match self.kind {
            Kind::Repository => format!("repository {}", self.path.display()),
            Kind::Worktree => format!("isolated worktree {}", self.path.display()),
        }
    }
}

/// Filesystem-only proof that the recorded path is still the expected Git
/// working tree. Run on a blocking worker, never while rendering.
pub(crate) fn verify(target: &Target) -> Result<(), Error> {
    let path = &target.path;
    let fail = |problem: String| Error::Target {
        path: Some(path.clone()),
        problem,
    };
    if !path.is_absolute() {
        return Err(fail("the recorded path is not absolute.".into()));
    }
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(fail(match target.kind {
                Kind::Repository => "the repository no longer exists there. If it was moved or renamed, register it again at its new location.".into(),
                Kind::Worktree => "the isolated worktree no longer exists. CodeConvoy does not recreate it or open another directory instead.".into(),
            }));
        }
        Err(error) => return Err(fail(format!("the directory cannot be accessed: {error}."))),
        Ok(_) => {}
    }
    // Recorded paths are canonical. A path that now resolves elsewhere (for
    // example through a symlink left after a move) is a different location.
    let canonical = path
        .canonicalize()
        .map_err(|error| fail(format!("the directory cannot be resolved: {error}.")))?;
    if canonical != *path {
        return Err(fail(format!(
            "the path now resolves to {}. Nothing else is opened in its place.",
            canonical.display()
        )));
    }
    if !canonical.is_dir() {
        return Err(fail("the path is not a directory.".into()));
    }
    let common = common_dir(path, target.kind).map_err(fail)?;
    if let Some(expected) = &target.common_dir
        && common != *expected
    {
        return Err(fail(format!(
            "it now belongs to a different Git repository ({}).",
            common.display()
        )));
    }
    Ok(())
}

/// Reads `.git` the way Git resolves a working tree, returning the canonical
/// common directory. Linked worktrees must have a consistent Git backlink.
fn common_dir(tree: &Path, kind: Kind) -> Result<PathBuf, String> {
    let dot_git = tree.join(".git");
    let metadata = match fs::symlink_metadata(&dot_git) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err("it is no longer a Git working tree: .git is missing.".into());
        }
        Err(error) => return Err(format!("its Git metadata cannot be read: {error}.")),
    };
    if metadata.is_dir() {
        if kind == Kind::Worktree {
            return Err("it is no longer a linked worktree: .git is a directory.".into());
        }
        return dot_git
            .canonicalize()
            .map_err(|error| format!("its Git directory cannot be resolved: {error}."));
    }
    if !metadata.is_file() {
        return Err("its .git entry is neither a file nor a directory.".into());
    }
    let link = read(&dot_git)?;
    let target = line(&link)
        .strip_prefix(b"gitdir: ")
        .ok_or("its .git file is not a Git link.")?;
    let admin = absolute(tree, target)?
        .canonicalize()
        .map_err(|_| "its Git administration directory is missing.".to_owned())?;
    // Git records which checkout owns a linked worktree's administration.
    let owner = admin.join("gitdir");
    let linked = owner.exists();
    if linked {
        let backlink = absolute(&admin, line(&read(&owner)?))?;
        let backlink = backlink.canonicalize().unwrap_or(backlink);
        if backlink != dot_git {
            return Err(format!(
                "its Git administration belongs to another worktree ({}).",
                backlink.display()
            ));
        }
    } else if kind == Kind::Worktree {
        return Err("its Git administration has no worktree backlink.".into());
    }
    let common = match fs::symlink_metadata(admin.join("commondir")) {
        Ok(_) => absolute(&admin, line(&read(&admin.join("commondir"))?))?
            .canonicalize()
            .map_err(|_| "its Git common directory is missing.".to_owned())?,
        Err(_) => admin.clone(),
    };
    if kind == Kind::Worktree && admin.parent() != Some(common.join("worktrees").as_path()) {
        return Err("its Git administration is outside the repository's worktrees.".into());
    }
    Ok(common)
}

fn read(path: &Path) -> Result<Vec<u8>, String> {
    let invalid = || format!("Git metadata {} cannot be read.", path.display());
    let mut bytes = Vec::new();
    fs::File::open(path)
        .map_err(|_| invalid())?
        .take(64 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| invalid())?;
    if bytes.len() > 64 * 1024 {
        return Err(invalid());
    }
    Ok(bytes)
}

fn absolute(base: &Path, bytes: &[u8]) -> Result<PathBuf, String> {
    let path = native(bytes).map_err(|error| format!("{error:#}"))?;
    Ok(if path.is_absolute() {
        path
    } else {
        base.join(path)
    })
}

#[cfg(test)]
#[path = "target_tests.rs"]
mod tests;
