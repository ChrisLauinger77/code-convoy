use super::{Measurement, filesystem::ordinary_path};
use crate::{
    domain::{AppState, WorktreeMetadata},
    process::Cancellation,
};
use std::{
    fs::File,
    io::{self, Read},
    path::Path,
};

pub(super) struct History {
    pub measurement: Measurement,
    pub convoys: Option<usize>,
    pub references: Vec<WorktreeMetadata>,
}
struct Reader<'a, R> {
    inner: R,
    cancel: &'a Cancellation,
}
impl<R: Read> Read for Reader<'_, R> {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        if self.cancel.is_cancelled() {
            return Err(io::Error::other("Measurement cancelled."));
        }
        self.inner.read(bytes)
    }
}

pub(super) fn measure(path: &Path, cancel: &Cancellation) -> History {
    let mut result = History {
        measurement: Measurement::default(),
        convoys: None,
        references: Vec::new(),
    };
    let file = ordinary_path(path).and_then(|()| {
        if !std::fs::symlink_metadata(path)?.is_file() {
            return Err(io::Error::other("History storage is not an ordinary file."));
        }
        File::open(path)
    });
    let file = match file {
        Ok(file) => file,
        Err(e) => {
            result.measurement.issue(format!("{}: {e}", path.display()));
            return result;
        }
    };
    match file.metadata() {
        Ok(metadata) => result.measurement = Measurement::complete(metadata.len()),
        Err(e) => result
            .measurement
            .issue(format!("Cannot measure history: {e}")),
    }
    let reader = io::BufReader::new(Reader {
        inner: file,
        cancel,
    });
    match serde_json::from_reader::<_, AppState>(reader) {
        Ok(state) if state.version == 1 => {
            result.convoys = Some(state.runs.iter().filter(|r| !r.active()).count());
            result.references = state
                .runs
                .into_iter()
                .flat_map(|r| r.jobs)
                .filter_map(|j| j.worktree)
                .collect();
        }
        Ok(_) => result
            .measurement
            .issue("Unsupported persisted history version; count and references are unknown."),
        Err(e) => result
            .measurement
            .issue(format!("Cannot read persisted history: {e}")),
    }
    result
}
