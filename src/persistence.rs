use crate::domain::{AppState, MAX_CONCURRENCY, MAX_REPOSITORIES};
use anyhow::{Context, Result};
use directories::ProjectDirs;
use fs2::FileExt;
use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

pub struct Store {
    directory: PathBuf,
    _lock: File,
}
impl Store {
    pub fn open_default() -> Result<Self> {
        let dirs = ProjectDirs::from("", "", "CodeConvoy")
            .context("Cannot locate your application data directory.")?;
        Self::open(dirs.data_local_dir())
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
            directory: directory.to_owned(),
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
