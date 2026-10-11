pub mod bulk_discard;
pub mod history;
pub mod results;
#[cfg(test)]
mod tests;
mod validation;
pub mod worktree_cleanup;
use crate::domain::{AppState, MAX_CONCURRENCY, MAX_REPOSITORIES};
use anyhow::{Context, Result};
use directories::ProjectDirs;
use fs2::FileExt;
use std::{
    fs::{self, File, OpenOptions},
    io::{BufWriter, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
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
    history_write: std::sync::Mutex<bool>,
    load_failed: AtomicBool,
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
            history_write: std::sync::Mutex::new(false),
            load_failed: AtomicBool::new(false),
        })
    }
    pub fn directory(&self) -> &Path {
        &self.directory
    }
    /// The sole supported history layout is shared application state, including
    /// version-1 legacy saves. Exports and temporary save files are not history.
    pub fn history_path(&self) -> PathBuf {
        self.directory.join("state.json")
    }
    pub fn load(&self) -> Result<AppState> {
        let result = self.load_state();
        // A failed read must never become a later empty-state save. Reopening
        // after the user has preserved/recovered the file is the recovery boundary.
        if result.is_err() {
            self.load_failed.store(true, Ordering::Release);
        }
        result
    }
    fn load_state(&self) -> Result<AppState> {
        let path = self.history_path();
        let mut state: AppState = match fs::read(&path) {
            Ok(bytes) => decode_state(&bytes)
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
        anyhow::ensure!(
            state.repository_validation.len() <= MAX_REPOSITORIES,
            "Too many validation commands in saved state."
        );
        for command in state.repository_validation.values() {
            command.validate()?;
        }
        state.recover_interrupted();
        state.global_concurrency = state.global_concurrency.clamp(1, MAX_CONCURRENCY);
        state.draft.concurrency = state.draft.concurrency.clamp(1, 16);
        Ok(state)
    }
    /// Internal cleanup uses the same Store transaction as explicit result actions.
    /// The caller owns application state and the Store lock for this transaction.
    pub async fn cleanup_result(
        &self,
        state: &mut AppState,
        manager: &crate::runner::RunManager,
        run: u64,
        index: usize,
    ) -> Result<()> {
        anyhow::ensure!(!manager.is_active(run), "Active work cannot be cleaned up.");
        let op =
            self.begin_result_operation(state, run, index, results::Action::InternalCleanup)?;
        let completion = op.execute(&manager.lifecycle()).await;
        let error = match &completion {
            results::Completion::Cleanup(Err(e)) => Some(e.clone()),
            _ => None,
        };
        self.finish_result_operation(state, &op, completion)?;
        match error {
            Some(e) => anyhow::bail!(e),
            None => Ok(()),
        }
    }

    pub fn save(&self, state: &AppState) -> Result<()> {
        let pending = self
            .history_write
            .lock()
            .map_err(|_| anyhow::anyhow!("History save lock failed."))?;
        anyhow::ensure!(
            !*pending,
            "Application state is being saved; try again when it finishes."
        );
        self.save_atomic(state)
    }
    fn save_atomic(&self, state: &AppState) -> Result<()> {
        anyhow::ensure!(
            !self.load_failed.load(Ordering::Acquire),
            "Saving is disabled because application state could not be loaded. Preserve state.json and restart after recovery."
        );
        self.replace_state(|writer| {
            serde_json::to_writer_pretty(&mut *writer, state)
                .context("Cannot encode application state.")?;
            writer.write_all(b"\n")?;
            Ok(())
        })
    }
    fn replace_state(&self, encode: impl FnOnce(&mut dyn Write) -> Result<()>) -> Result<()> {
        let mut file = tempfile::NamedTempFile::new_in(&self.directory)
            .context("Cannot create temporary state file.")?;
        {
            let mut writer = BufWriter::new(file.as_file_mut());
            encode(&mut writer)?;
            writer
                .flush()
                .context("Cannot finish writing application state.")?;
        }
        file.as_file()
            .sync_all()
            .context("Cannot sync application state; previous state is intact.")?;
        file.persist(self.history_path())
            .context("Cannot replace application state; previous state is intact.")?;
        #[cfg(unix)]
        File::open(&self.directory).and_then(|file| file.sync_all()).context(
            "Application state was replaced, but directory sync failed; durability is uncertain.",
        )?;
        Ok(())
    }
}

/// Require the core envelope that every supported on-disk version wrote.
/// Optional/additive fields still use AppState's backwards-compatible defaults.
/// The first pass skips entries without allocating another history tree.
pub(crate) fn decode_state(bytes: &[u8]) -> Result<AppState> {
    #[derive(serde::Deserialize)]
    struct Envelope {
        version: u32,
        next_run: u64,
        runs: serde::de::IgnoredAny,
    }
    let envelope: Envelope = serde_json::from_slice(bytes)?;
    anyhow::ensure!(
        envelope.version == 1,
        "Unsupported state version {}; use a compatible CodeConvoy version.",
        envelope.version
    );
    let _ = envelope.runs;
    let mut state: AppState = serde_json::from_slice(bytes)?;
    let mut ids = std::collections::HashSet::new();
    for run in &state.runs {
        anyhow::ensure!(
            run.id != 0 && ids.insert(run.id),
            "Invalid or duplicate convoy ID {}; history identity is ambiguous.",
            run.id
        );
    }
    // Older saves may have a lagging counter; never recycle a retained ID.
    let next = ids
        .into_iter()
        .max()
        .unwrap_or(0)
        .checked_add(1)
        .context("Convoy ID space is exhausted; history was preserved.")?;
    state.next_run = envelope.next_run.max(next);
    Ok(state)
}
