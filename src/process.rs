//! Shell-free process execution with bounded output and process-tree cancellation.
use anyhow::{Context, Result};
use process_wrap::tokio::{ChildWrapper, CommandWrap};
#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;
#[cfg(any(windows, test))]
mod windows_list;
use std::{
    ffi::OsString,
    path::PathBuf,
    process::{ExitStatus, Stdio},
    time::Duration,
};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWriteExt},
    sync::watch,
};

/// Windows environment names ignore ASCII case. Repository overrides must be
/// removed with the same rule before Git or an agent enters a selected checkout.
pub(crate) fn git_environment_override(key: &str, windows: bool) -> bool {
    if windows {
        key.get(..4)
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case("GIT_"))
    } else {
        key.starts_with("GIT_")
    }
}

/// How CodeConvoy confirms that an agent's processes have stopped.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Containment {
    /// The Unix process group or the Windows Job Object holds every process.
    #[default]
    Group,
    /// On macOS and Linux the CLI moves helpers into their own process groups
    /// and stops them itself on SIGTERM. Stopping sends SIGTERM and waits up to
    /// `grace` before the process group is killed. The repository is released
    /// only once every process group recorded under the CLI is gone. The
    /// Windows Job Object already holds such helpers.
    DetachedGroups { grace: Duration },
}

#[derive(Debug, Clone)]
pub struct CommandSpec {
    pub program: OsString,
    pub args: Vec<OsString>,
    pub directory: PathBuf,
    pub input: Option<Vec<u8>>,
    pub remove_env: Vec<String>,
    pub env: Vec<(String, String)>,
    pub containment: Containment,
}
impl CommandSpec {
    pub fn new(program: impl Into<OsString>, directory: impl Into<PathBuf>) -> Self {
        Self {
            program: program.into(),
            args: Vec::new(),
            directory: directory.into(),
            input: None,
            remove_env: Vec::new(),
            env: Vec::new(),
            containment: Containment::Group,
        }
    }
    pub fn args(mut self, args: &[&str]) -> Self {
        self.args.extend(args.iter().map(OsString::from));
        self
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stream {
    Stdout,
    Stderr,
}
#[derive(Debug, Clone)]
pub struct Cancellation(watch::Sender<bool>);
impl Default for Cancellation {
    fn default() -> Self {
        Self(watch::channel(false).0)
    }
}
impl Cancellation {
    pub fn cancel(&self) {
        self.0.send_replace(true);
    }
    pub fn is_cancelled(&self) -> bool {
        *self.0.borrow()
    }
    pub async fn cancelled(&self) {
        let mut rx = self.0.subscribe();
        let _ = rx.wait_for(|v| *v).await;
    }
}
pub struct ManagedChild {
    child: Box<dyn ChildWrapper>,
    #[cfg(unix)]
    group: std::io::Result<unix::Group>,
    /// SIGTERM grace period for `Containment::DetachedGroups`.
    #[cfg(unix)]
    grace: Option<Duration>,
    #[cfg(windows)]
    job: std::sync::Arc<windows::Job>,
    #[cfg(not(unix))]
    reaped: bool,
}
impl Drop for ManagedChild {
    fn drop(&mut self) {
        #[cfg(unix)]
        if let Ok(group) = &mut self.group {
            let _ = group.terminate();
        }
        #[cfg(windows)]
        if !self.reaped {
            let _ = self.job.terminate();
        }
        #[cfg(not(any(unix, windows)))]
        if !self.reaped {
            let _ = self.child.start_kill();
        }
    }
}
impl ManagedChild {
    /// Asks the CLI to stop itself before its process group is killed.
    #[cfg(unix)]
    fn interrupt(&mut self) -> std::io::Result<()> {
        self.group
            .as_mut()
            .map_err(|error| std::io::Error::new(error.kind(), error.to_string()))?
            .interrupt()
    }
    /// Resolves once the CLI has exited and the rest of its group was killed,
    /// without reaping it.
    #[cfg(unix)]
    async fn exited(&mut self) -> std::io::Result<()> {
        self.group
            .as_mut()
            .map_err(|error| std::io::Error::new(error.kind(), error.to_string()))?
            .before_reap()
            .await
            .map(|_| ())
    }
    async fn wait(&mut self) -> Result<ExitStatus> {
        #[cfg(unix)]
        {
            let group = self
                .group
                .as_mut()
                .map_err(|error| std::io::Error::new(error.kind(), error.to_string()))
                .context("Cannot supervise the Unix process group.")?;
            let deadline = group
                .before_reap()
                .await
                .context("Cannot supervise the Unix process group.")?;
            tokio::time::timeout_at(deadline, async {
                // Reap only our direct child. The wrapper's group-wide waitpid
                // could reap an unrelated child if the old PGID were reused.
                // Unix spawn installs only ProcessGroup; its inner is the native
                // child. Our identity guard owns the group lifecycle instead.
                let status = self.child.inner_mut().wait().await?;
                group.after_reap().await?;
                group.after_detached().await?;
                Ok(status)
            })
            .await
            .context(
                "Process group cleanup timed out; repository protection must remain reserved.",
            )?
        }
        #[cfg(windows)]
        {
            // Bypass process-wrap's cached leader/notification-based wait. Our
            // retained private job verifies actual descendant termination.
            // SAFETY: spawn installs only JobObject as a child wrapper on
            // Windows. Its wait/status cache is never used here; our retained
            // private job owns descendant cleanup before reporting completion.
            let status = unsafe { self.child.try_inner_child_mut() }
                .context("Missing native Windows child for supervision.")?
                .wait()
                .await?;
            self.job
                .finish()
                .await
                .context("Cannot confirm Windows process-tree cleanup.")?;
            Ok(status)
        }
        #[cfg(not(any(unix, windows)))]
        {
            Ok(self.child.wait().await?)
        }
    }
}
pub fn spawn(spec: &CommandSpec) -> Result<ManagedChild> {
    #[cfg(unix)]
    unix::check_reaping()?;
    let mut command = CommandWrap::with_new(&spec.program, |c| {
        c.args(&spec.args)
            .current_dir(&spec.directory)
            .stdin(if spec.input.is_some() {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        for key in &spec.remove_env {
            c.env_remove(key);
        }
        for (key, value) in &spec.env {
            c.env(key, value);
        }
    });
    // Unix Drop uses the unreaped identity guard, never a cached group ID or
    // Tokio's independent kill-on-drop after supervision has lost identity.
    #[cfg(not(unix))]
    command.wrap(process_wrap::tokio::KillOnDrop);
    #[cfg(unix)]
    command.wrap(process_wrap::tokio::ProcessGroup::leader());
    #[cfg(windows)]
    let job = {
        let job = windows::Job::new().context("Cannot create an owned Windows Job Object.")?;
        command.wrap(windows::Assign(job.clone()));
        command.wrap(process_wrap::tokio::JobObject);
        // Use the wrapper so JobObject preserves this flag when adding
        // CREATE_SUSPENDED for race-free descendant containment.
        command.wrap(process_wrap::tokio::CreationFlags(
            ::windows::Win32::System::Threading::CREATE_NO_WINDOW,
        ));
        job
    };
    let child = command.spawn().with_context(|| {
        format!(
            "Cannot start {:?} in {}. Check the executable path and installation.",
            spec.program,
            spec.directory.display()
        )
    })?;
    #[cfg(unix)]
    let grace = match spec.containment {
        Containment::Group => None,
        Containment::DetachedGroups { grace } => Some(grace),
    };
    Ok(ManagedChild {
        // Retain any identity error in the child: returning a spawn error after
        // launch would bypass the caller's unsafe-until-supervised protection.
        // wait/cancel report it and Drop cannot signal an unconfirmed identity.
        #[cfg(unix)]
        group: unix::Group::new(child.id()).map(|group| {
            if grace.is_some() {
                group.track_detached(&spec.directory)
            } else {
                group
            }
        }),
        #[cfg(unix)]
        grace,
        #[cfg(windows)]
        job,
        child,
        #[cfg(not(unix))]
        reaped: false,
    })
}
pub fn cancel(child: &mut ManagedChild) -> std::io::Result<()> {
    #[cfg(unix)]
    return child
        .group
        .as_mut()
        .map_err(|error| std::io::Error::new(error.kind(), error.to_string()))?
        .terminate();
    #[cfg(windows)]
    return child.job.terminate();
    #[cfg(not(any(unix, windows)))]
    child.child.start_kill()
}

#[derive(Debug)]
pub struct ProcessResult {
    pub status: Option<ExitStatus>,
    pub cancelled: bool,
    pub timed_out: bool,
}

pub async fn execute(
    child: ManagedChild,
    input: Option<Vec<u8>>,
    cancellation: &Cancellation,
    cancel_process: impl Fn(&mut ManagedChild) -> std::io::Result<()>,
    output: impl Fn(Stream, &[u8]) + Sync,
) -> Result<ProcessResult> {
    execute_until(child, input, cancellation, None, cancel_process, output).await
}

/// A deadline uses the same owned process-tree cleanup as explicit cancellation.
/// Never drop a running execution future merely because its time limit elapsed.
pub async fn execute_until(
    mut child: ManagedChild,
    input: Option<Vec<u8>>,
    cancellation: &Cancellation,
    deadline: Option<tokio::time::Instant>,
    cancel_process: impl Fn(&mut ManagedChild) -> std::io::Result<()>,
    output: impl Fn(Stream, &[u8]) + Sync,
) -> Result<ProcessResult> {
    let stdout = child
        .child
        .stdout()
        .take()
        .context("Missing stdout pipe.")?;
    let stderr = child
        .child
        .stderr()
        .take()
        .context("Missing stderr pipe.")?;
    let stdin = child.child.stdin().take();
    let work = async {
        let writer = async {
            if let (Some(mut stdin), Some(input)) = (stdin, input) {
                // A CLI can reject its arguments before consuming stdin. Its exit/output
                // are more useful than a secondary broken-pipe error in that case.
                if let Err(e) = stdin.write_all(&input).await
                    && e.kind() != std::io::ErrorKind::BrokenPipe
                {
                    return Err(e.into());
                }
            }
            Ok::<_, anyhow::Error>(())
        };
        // Propagate supervision errors even if an unconfirmed descendant still
        // holds an output pipe. The owner must quarantine, not wait for EOF.
        let (status, _, _, _) = tokio::try_join!(
            child.wait(),
            pump(stdout, Stream::Stdout, &output),
            pump(stderr, Stream::Stderr, &output),
            writer
        )?;
        Ok::<_, anyhow::Error>(status)
    };
    let mut timed_out = false;
    let result = tokio::select! {
        biased;
        _ = cancellation.cancelled() => None,
        _ = deadline_elapsed(deadline) => { timed_out = true; None },
        result = work => Some(result),
    };
    match result {
        Some(result) => {
            let status = result?;
            #[cfg(not(unix))]
            {
                child.reaped = true;
            }
            Ok(ProcessResult {
                status: Some(status),
                cancelled: false,
                timed_out: false,
            })
        }
        None => {
            // A CLI that stops its own detached helpers gets a bounded chance to
            // do so. Process-group termination and confirmation follow either way.
            // Its output pipes are already closed: a write during shutdown fails
            // with EPIPE instead of blocking.
            #[cfg(unix)]
            if let Some(grace) = child.grace
                && child.interrupt().is_ok()
            {
                let _ = tokio::time::timeout(grace, child.exited()).await;
            }
            cancel_process(&mut child).context("Could not terminate the agent process tree.")?;
            let status = tokio::time::timeout(Duration::from_secs(5), child.wait())
                .await
                .context("Agent termination timed out; inspect remaining processes.")??;
            #[cfg(not(unix))]
            {
                child.reaped = true;
            }
            Ok(ProcessResult {
                status: Some(status),
                cancelled: !timed_out,
                timed_out,
            })
        }
    }
}

pub(crate) async fn deadline_elapsed(deadline: Option<tokio::time::Instant>) {
    match deadline {
        Some(deadline) => tokio::time::sleep_until(deadline).await,
        None => std::future::pending().await,
    }
}
async fn pump(
    mut reader: impl AsyncRead + Unpin,
    stream: Stream,
    output: &impl Fn(Stream, &[u8]),
) -> Result<()> {
    let mut bytes = [0; 8192];
    loop {
        let count = reader.read(&mut bytes).await?;
        if count == 0 {
            return Ok(());
        }
        output(stream, &bytes[..count]);
    }
}

pub struct Captured {
    pub status: ExitStatus,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub truncated: bool,
}
/// Used for short Git/detection commands, never for an agent task.
pub async fn capture(spec: CommandSpec, limit: usize) -> Result<Captured> {
    let cancellation = Cancellation::default();
    capture_cancellable(spec, limit, &cancellation).await
}

/// A short command that can also be cancelled by its lifecycle owner.
/// Cancellation waits for process-tree cleanup before returning.
pub async fn capture_cancellable(
    spec: CommandSpec,
    limit: usize,
    cancellation: &Cancellation,
) -> Result<Captured> {
    capture_owned(
        spec,
        limit,
        cancellation,
        Duration::from_secs(20),
        &std::sync::atomic::AtomicBool::new(true),
    )
    .await
}

/// Reports confirmed cleanup to the scheduler for mutating Git preparation too.
pub(crate) async fn capture_owned(
    spec: CommandSpec,
    limit: usize,
    cancellation: &Cancellation,
    timeout: Duration,
    safe: &std::sync::atomic::AtomicBool,
) -> Result<Captured> {
    use std::sync::atomic::Ordering;
    anyhow::ensure!(!cancellation.is_cancelled(), "Command cancelled.");
    let command_cancellation = Cancellation::default();
    let data = std::sync::Mutex::new((Vec::new(), Vec::new(), false));
    let child = spawn(&spec)?;
    safe.store(false, Ordering::Release);
    let execution = execute(
        child,
        spec.input,
        &command_cancellation,
        cancel,
        |stream, bytes| {
            if let Ok(mut data) = data.lock() {
                let target = if stream == Stream::Stdout {
                    &mut data.0
                } else {
                    &mut data.1
                };
                let remaining = limit.saturating_sub(target.len());
                target.extend_from_slice(&bytes[..bytes.len().min(remaining)]);
                data.2 |= bytes.len() > remaining;
            }
        },
    );
    tokio::pin!(execution);
    let result = tokio::select! {
        biased;
        _ = cancellation.cancelled() => {
            command_cancellation.cancel();
            execution.await?;
            safe.store(true, Ordering::Release);
            anyhow::bail!("Command cancelled.")
        }
        result = &mut execution => result?,
        _ = tokio::time::sleep(timeout) => {
            command_cancellation.cancel();
            execution.await?;
            safe.store(true, Ordering::Release);
            anyhow::bail!("Command timed out after {} seconds.", timeout.as_secs())
        }
    };
    safe.store(true, Ordering::Release);
    let (stdout, stderr, truncated) = data
        .lock()
        .map_err(|_| anyhow::anyhow!("Output lock failed."))?
        .clone();
    anyhow::ensure!(!result.cancelled, "Command cancelled.");
    Ok(Captured {
        status: result.status.context("Command cancelled.")?,
        stdout,
        stderr,
        truncated,
    })
}
