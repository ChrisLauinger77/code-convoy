use super::{Measurement, filesystem::ordinary_path};
use crate::{
    domain::{Run, WorktreeMetadata},
    process::Cancellation,
};

// Require the same core envelope as Store, but do not perform recovery. Keep
// the existing Run parser so malformed historical fields never look complete.
// Reading references is not recovery validation and grants no cleanup authority.
#[derive(serde::Deserialize)]
struct StoredHistory {
    version: u32,
    #[serde(rename = "next_run")]
    _next_run: u64,
    runs: Vec<Run>,
}
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
    match serde_json::from_reader::<_, StoredHistory>(reader) {
        Ok(state) if state.version == 1 => {
            let mut ids = std::collections::HashSet::new();
            if state.runs.iter().any(|r| r.id == 0 || !ids.insert(r.id)) {
                result
                    .measurement
                    .issue("Ambiguous convoy IDs; count and history references are unknown.");
                return result;
            }
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
