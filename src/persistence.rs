use crate::domain::{AppState, MAX_CONCURRENCY, MAX_REPOSITORIES};
use anyhow::{Context, Result};
use directories::ProjectDirs;
use fs2::FileExt;
use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

pub fn data_directory() -> Result<PathBuf> {
    Ok(ProjectDirs::from("", "", "CodeConvoy")
        .context("Cannot locate your application data directory.")?
        .data_local_dir()
        .to_owned())
}

pub struct Store {
    directory: PathBuf,
    _lock: File,
}
impl Store {
    pub fn open_default() -> Result<Self> {
        Self::open(&data_directory()?)
    }
    pub fn open(directory: &Path) -> Result<Self> {
        fs::create_dir_all(directory)
            .with_context(|| format!("Cannot create {}", directory.display()))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(directory, fs::Permissions::from_mode(0o700))?;
        }
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(directory.join("app.lock"))?;
        lock.try_lock_exclusive()
            .context("CodeConvoy is already running, or its data directory cannot be locked.")?;
        Ok(Self {
            directory: directory.canonicalize()?,
            _lock: lock,
        })
    }
    pub fn directory(&self) -> &Path {
        &self.directory
    }
    pub fn load(&self) -> Result<AppState> {
        let path = self.directory.join("state.json");
        let mut state: AppState = match fs::read(&path) {
            Ok(bytes) => serde_json::from_slice(&bytes)
                .with_context(|| format!("Invalid state in {}. Back up and move this file aside to start fresh; it has not been overwritten.", path.display()))?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => AppState::default(),
            Err(e) => return Err(e).context("Cannot read CodeConvoy state."),
        };
        anyhow::ensure!(
            state.version == 1,
            "Unsupported state version {}; use a compatible CodeConvoy version. State has not been overwritten.",
            state.version
        );
        anyhow::ensure!(
            state.repositories.len() <= MAX_REPOSITORIES,
            "Too many registered repositories in saved state."
        );
        state.recover_interrupted();
        state.trim_history();
        state.global_concurrency = state.global_concurrency.clamp(1, MAX_CONCURRENCY);
        state.draft.concurrency = state.draft.concurrency.clamp(1, 16);
        Ok(state)
    }
    /// Internal foundation for a future explicit Discard action. No UI calls this.
    /// The caller owns application state and the Store lock for this transaction.
    pub async fn cleanup_result(
        &self,
        state: &mut AppState,
        manager: &crate::runner::RunManager,
        run: u64,
        index: usize,
    ) -> Result<()> {
        use crate::domain::{ExecutionMode, ResultAvailability as A};
        let job = state
            .runs
            .iter_mut()
            .find(|r| r.id == run)
            .and_then(|r| r.jobs.get_mut(index))
            .context("Historical result was not found.")?;
        anyhow::ensure!(
            job.status.is_terminal() && !manager.is_active(run),
            "Active work cannot be cleaned up."
        );
        let metadata = job
            .worktree
            .clone()
            .context("No isolated result was recorded.")?;
        anyhow::ensure!(
            metadata.run == run
                && metadata.job == index
                && metadata.repository == job.repository
                && job.execution_mode == ExecutionMode::IsolatedWorktree,
            "Result ownership does not match this job; cleanup is blocked."
        );
        let previous = job.result_availability;
        anyhow::ensure!(
            !state
                .repositories
                .iter()
                .any(|r| r.path.starts_with(&metadata.path) || metadata.path.starts_with(&r.path)),
            "A registered working tree cannot be cleaned up."
        );
        job.result_availability = A::CleanupPending;
        if let Err(error) = self.save(state) {
            if let Some(job) = state
                .runs
                .iter_mut()
                .find(|r| r.id == run)
                .and_then(|r| r.jobs.get_mut(index))
            {
                job.result_availability = previous;
            }
            return Err(error).context("Could not save cleanup intent; no cleanup was started.");
        }
        let outcome = manager.lifecycle().cleanup(&metadata).await;
        let report = match &outcome {
            Ok(report) => report.clone(),
            Err(e) => crate::worktrees::recovery::Report::unavailable(
                A::CleanupFailed,
                format!("Could not clean isolated result. {e:#}"),
            ),
        };
        if let Some(job) = state
            .runs
            .iter_mut()
            .find(|r| r.id == run)
            .and_then(|r| r.jobs.get_mut(index))
        {
            report.apply(job);
        }
        self.save(state).context("Cleanup outcome could not be saved. The durable intent and ownership record will be checked on restart.")?;
        outcome.map(|_| ())
    }
    pub fn save(&self, state: &AppState) -> Result<()> {
        let mut file = tempfile::NamedTempFile::new_in(&self.directory)
            .context("Cannot create temporary state file.")?;
        serde_json::to_writer_pretty(&mut file, state)
            .context("Cannot encode application state.")?;
        file.write_all(b"\n")?;
        file.as_file().sync_all()?;
        file.persist(self.directory.join("state.json"))
            .context("Cannot replace application state; previous state is intact.")?;
        #[cfg(unix)]
        File::open(&self.directory)?.sync_all()?;
        Ok(())
    }
}
