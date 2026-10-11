//! Export-only atomic output; no Store access and no repository operations.
use super::{Format, Snapshot};
use crate::process::Cancellation;
use anyhow::{Context, Result, ensure};
use std::{
    fs,
    io::{self, BufWriter, Write},
    path::{Path, PathBuf},
    time::SystemTime,
};

/// Reviewed destination. Existing ordinary files require explicit replacement
/// confirmation in the UI, even on desktops without native overwrite prompts.
#[derive(Debug, PartialEq, Eq)]
pub struct Destination {
    pub path: PathBuf,
    existing: Option<Stamp>,
}
#[derive(Debug, PartialEq, Eq)]
struct Stamp {
    len: u64,
    modified: SystemTime,
    created: Option<SystemTime>,
    #[cfg(unix)]
    identity: (u64, u64, i64, i64),
}
impl Destination {
    /// Runs on a worker. Canonicalize only the parent, never follow a target symlink.
    pub fn inspect(path: &Path) -> Result<Self> {
        let name = path
            .file_name()
            .context("Choose a filename for the export.")?;
        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let path = parent
            .canonicalize()
            .context("Cannot access the destination directory")?
            .join(name);
        let existing = stamp(&path)?;
        Ok(Self { path, existing })
    }
    pub fn exists(&self) -> bool {
        self.existing.is_some()
    }
}
fn stamp(path: &Path) -> Result<Option<Stamp>> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e).context("Cannot inspect the export destination"),
    };
    ensure!(
        metadata.is_file() && !metadata.file_type().is_symlink(),
        "Choose an ordinary file; directories and links cannot be replaced."
    );
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        ensure!(
            metadata.file_attributes() & 0x400 == 0,
            "Reparse points cannot be replaced."
        );
    }
    Ok(Some(Stamp {
        len: metadata.len(),
        modified: metadata.modified()?,
        created: metadata.created().ok(),
        #[cfg(unix)]
        identity: {
            use std::os::unix::fs::MetadataExt;
            (
                metadata.dev(),
                metadata.ino(),
                metadata.ctime(),
                metadata.ctime_nsec(),
            )
        },
    }))
}

pub fn save(
    snapshot: &Snapshot,
    format: Format,
    destination: &Destination,
    replace: bool,
    cancel: &Cancellation,
) -> Result<()> {
    atomic_write(destination, replace, cancel, |writer| {
        snapshot.write(format, writer)
    })
}
fn atomic_write(
    destination: &Destination,
    replace: bool,
    cancel: &Cancellation,
    render: impl FnOnce(&mut dyn Write) -> Result<()>,
) -> Result<()> {
    ensure!(
        !destination.exists() || replace,
        "The destination exists. Confirm replacement or choose another file."
    );
    check_cancel(cancel)?;
    let parent = destination
        .path
        .parent()
        .context("Invalid destination directory")?;
    let mut temporary = tempfile::Builder::new()
        .prefix(".codeconvoy-export-")
        .tempfile_in(parent)
        .context("Cannot create export file in the destination directory")?;
    {
        let mut writer = CancelWriter {
            inner: BufWriter::new(temporary.as_file_mut()),
            cancel,
        };
        render(&mut writer).context("Could not write export")?;
        writer.flush().context("Could not finish export")?;
    }
    temporary
        .as_file()
        .sync_all()
        .context("Could not sync export")?;
    check_cancel(cancel)?;
    ensure!(
        stamp(&destination.path)? == destination.existing,
        "The destination changed. Choose the file again and review replacement."
    );
    check_cancel(cancel)?;
    // Never truncate the target. A failed render/sync/rename drops the temporary file.
    if destination.exists() {
        temporary
            .persist(&destination.path)
            .map_err(|e| e.error)
            .context("Could not replace export file")?;
    } else {
        // Also protects a new file created by another process after our last check.
        temporary
            .persist_noclobber(&destination.path)
            .map_err(|e| e.error)
            .context("Could not create export file (the destination may now exist)")?;
    }
    #[cfg(unix)]
    fs::File::open(parent)
        .and_then(|file| file.sync_all())
        .context("Export was published, but directory sync failed; durability is uncertain")?;
    Ok(())
}
fn check_cancel(cancel: &Cancellation) -> io::Result<()> {
    if cancel.is_cancelled() {
        Err(io::Error::other("Export cancelled"))
    } else {
        Ok(())
    }
}
struct CancelWriter<'a, W> {
    inner: W,
    cancel: &'a Cancellation,
}
impl<W: Write> Write for CancelWriter<'_, W> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        check_cancel(self.cancel)?;
        self.inner.write(bytes)
    }
    fn flush(&mut self) -> io::Result<()> {
        check_cancel(self.cancel)?;
        self.inner.flush()
    }
}

#[cfg(test)]
mod tests;
