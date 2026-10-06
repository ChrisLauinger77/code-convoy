//! Session-local CLI checks. At most one probe per backend owns a process tree;
//! changed selections cancel it and queue only the newest executable.
use super::{backend, detect_cancellable, discovery};
use crate::{domain::AgentId, process::Cancellation};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::{Arc, mpsc},
    time::Duration,
};
use tokio::{runtime::Handle, task::JoinHandle};

#[derive(Debug)]
pub struct CliInfo {
    pub executable: String,
    pub detail: String,
}

#[derive(Debug)]
pub struct CliError {
    pub missing: bool,
    pub detail: String,
}
impl CliError {
    pub fn from_error(error: anyhow::Error) -> Self {
        Self {
            missing: error.chain().any(|cause| {
                cause
                    .downcast_ref::<std::io::Error>()
                    .is_some_and(|error| error.kind() == std::io::ErrorKind::NotFound)
            }),
            detail: format!("{error:#}"),
        }
    }
}

struct Probe {
    request: u64,
    cancellation: Cancellation,
    task: JoinHandle<()>,
}

struct Entry {
    executable: Option<String>,
    request: u64,
    probe: Option<Probe>,
    pending: bool,
    delay: Duration,
    result: Option<Result<CliInfo, CliError>>,
}

struct Completion {
    agent: AgentId,
    request: u64,
    discovered: Option<String>,
    result: Result<CliInfo, CliError>,
}

pub struct ResolvedExecutable {
    pub agent: AgentId,
    pub configured: Option<String>,
    pub path: String,
}

pub struct CliChecks {
    entries: BTreeMap<AgentId, Entry>,
    directory: PathBuf,
    runtime: Handle,
    wake: Arc<dyn Fn() + Send + Sync>,
    tx: mpsc::Sender<Completion>,
    rx: mpsc::Receiver<Completion>,
    stopped: bool,
}

impl CliChecks {
    pub fn new(
        directory: PathBuf,
        runtime: Handle,
        wake: impl Fn() + Send + Sync + 'static,
    ) -> Self {
        let (tx, rx) = mpsc::channel();
        Self {
            entries: BTreeMap::new(),
            directory,
            runtime,
            wake: Arc::new(wake),
            tx,
            rx,
            stopped: false,
        }
    }

    /// Called at startup, backend selection and executable edits. A result or
    /// pending check for the same backend/executable is reused without spawning.
    /// None means automatic discovery; an explicit name/path never falls back.
    pub fn ensure(&mut self, agent: AgentId, executable: Option<String>) {
        if self.stopped
            || self
                .entries
                .get(&agent)
                .is_some_and(|entry| entry.executable == executable)
        {
            return;
        }
        if let Some(entry) = self.entries.get_mut(&agent) {
            entry.executable = executable;
            entry.request += 1;
            entry.pending = true;
            entry.result = None;
            // Coalesce typing without launching a process for each character.
            entry.delay = Duration::from_millis(300);
            if let Some(probe) = &entry.probe {
                probe.cancellation.cancel();
            }
        } else {
            self.entries.insert(
                agent,
                Entry {
                    executable,
                    request: 0,
                    probe: None,
                    pending: true,
                    delay: Duration::ZERO,
                    result: None,
                },
            );
        }
        self.start_pending(agent);
    }

    /// Explicit refresh bypasses the session result. Repeated clicks while this
    /// backend is already checking are ignored to avoid duplicate probes.
    pub fn recheck(&mut self, agent: AgentId) {
        if self.stopped {
            return;
        }
        let Some(entry) = self.entries.get_mut(&agent) else {
            return;
        };
        if entry.probe.is_some() || entry.pending {
            return;
        }
        entry.request += 1;
        entry.result = None;
        entry.pending = true;
        entry.delay = Duration::ZERO;
        self.start_pending(agent);
    }

    fn start_pending(&mut self, agent: AgentId) {
        if self.stopped {
            return;
        }
        let Some(entry) = self.entries.get_mut(&agent) else {
            return;
        };
        if !entry.pending || entry.probe.is_some() {
            return;
        }
        entry.pending = false;
        let request = entry.request;
        let executable = entry.executable.clone();
        let delay = entry.delay;
        let directory = self.directory.clone();
        let cancellation = Cancellation::default();
        let token = cancellation.clone();
        let tx = self.tx.clone();
        let wake = self.wake.clone();
        let task = self.runtime.spawn(async move {
            let (discovered, result) = probe(agent, executable, directory, delay, &token).await;
            let _ = tx.send(Completion {
                agent,
                request,
                discovered,
                result,
            });
            wake();
        });
        entry.probe = Some(Probe {
            request,
            cancellation,
            task,
        });
    }

    /// Returns discovered paths for the owner to adopt into its normal settings.
    /// Only matching generations can publish status or replace automatic settings.
    pub fn poll(&mut self) -> Vec<ResolvedExecutable> {
        let mut discovered = Vec::new();
        while let Ok(completion) = self.rx.try_recv() {
            let Some(entry) = self.entries.get_mut(&completion.agent) else {
                continue;
            };
            if entry
                .probe
                .as_ref()
                .is_none_or(|probe| probe.request != completion.request)
            {
                continue;
            }
            entry.probe = None;
            if !self.stopped && entry.request == completion.request {
                if let Some(path) = completion.discovered {
                    let configured = entry.executable.clone();
                    entry.executable = Some(path.clone());
                    discovered.push(ResolvedExecutable {
                        agent: completion.agent,
                        configured,
                        path,
                    });
                }
                entry.result = Some(completion.result);
            }
            self.start_pending(completion.agent);
        }
        discovered
    }

    pub fn result(
        &self,
        agent: AgentId,
        executable: Option<&str>,
    ) -> Option<&Result<CliInfo, CliError>> {
        self.entries
            .get(&agent)
            .filter(|entry| entry.executable.as_deref() == executable)
            .and_then(|entry| entry.result.as_ref())
    }

    pub fn checking(&self, agent: AgentId) -> bool {
        !self.stopped
            && self
                .entries
                .get(&agent)
                .is_some_and(|entry| entry.probe.is_some() || entry.pending)
    }

    pub fn any_checking(&self) -> bool {
        AgentId::ALL.into_iter().any(|agent| self.checking(agent))
    }

    pub fn stop(&mut self) {
        self.stopped = true;
        for entry in self.entries.values_mut() {
            entry.pending = false;
            if let Some(probe) = &entry.probe {
                probe.cancellation.cancel();
            }
        }
    }
}

impl Drop for CliChecks {
    fn drop(&mut self) {
        self.stop();
        for entry in self.entries.values() {
            if let Some(probe) = &entry.probe {
                probe.task.abort();
            }
        }
    }
}

async fn probe(
    agent: AgentId,
    configured: Option<String>,
    directory: PathBuf,
    delay: Duration,
    cancellation: &Cancellation,
) -> (Option<String>, Result<CliInfo, CliError>) {
    let mut discovered = None;
    let result = async {
        tokio::select! {
            biased;
            _ = cancellation.cancelled() => anyhow::bail!("CLI check cancelled."),
            _ = tokio::time::sleep(delay) => {}
        }
        let backend = backend(agent)?;
        let automatic = configured.is_none();
        let mut executable =
            configured.unwrap_or_else(|| discovery::default_name(agent).to_owned());
        // Absolute and malformed paths go directly through backend validation.
        // Exact launcher names also use common GUI installation directories.
        if !executable.is_empty() && Path::new(&executable).components().count() == 1 {
            let name = executable.clone();
            let search = tokio::task::spawn_blocking(move || discovery::resolve(agent, &name));
            let path = tokio::select! {
                biased;
                _ = cancellation.cancelled() => anyhow::bail!("CLI check cancelled."),
                path = search => path?,
            };
            if let Some(path) = path.as_ref().and_then(|path| path.to_str()) {
                discovered = Some(path.to_owned());
                executable = path.to_owned();
            } else if automatic {
                anyhow::bail!(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    format!(
                        "{} executable not found in PATH or common installation folders.",
                        agent.label()
                    )
                ));
            }
        }
        // Availability belongs to the executable. Job settings are validated at
        // preflight; editing a model/permission never spawns another help probe.
        let options = [("executable".into(), executable.clone())].into();
        let detail =
            detect_cancellable(backend.as_ref(), &options, &directory, cancellation).await?;
        Ok(CliInfo { executable, detail })
    }
    .await
    .map_err(CliError::from_error);
    (discovered, result)
}
