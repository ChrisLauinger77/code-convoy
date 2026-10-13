//! Shell-free process execution with bounded output and process-tree cancellation.
use anyhow::{Context, Result};
use process_wrap::tokio::{ChildWrapper, CommandWrap};
#[cfg(unix)]
mod unix;
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

#[derive(Debug, Clone)]
pub struct CommandSpec {
    pub program: OsString,
    pub args: Vec<OsString>,
    pub directory: PathBuf,
    pub input: Option<Vec<u8>>,
    pub remove_env: Vec<String>,
    pub env: Vec<(String, String)>,
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
    group: unix::Group,
    #[cfg(not(unix))]
    reaped: bool,
}
impl Drop for ManagedChild {
    fn drop(&mut self) {
        #[cfg(unix)]
        let _ = self.group.terminate();
        #[cfg(not(unix))]
        if !self.reaped {
            let _ = self.child.start_kill();
        }
    }
}
impl ManagedChild {
    async fn wait(&mut self) -> Result<ExitStatus> {
        #[cfg(unix)]
        {
            let deadline = self
                .group
                .before_reap()
                .await
                .context("Cannot supervise the Unix process group.")?;
            tokio::time::timeout_at(deadline, async {
                // Reap only our direct child. The wrapper's group-wide waitpid
                // could reap an unrelated child if the old PGID were reused.
                // Unix spawn installs only ProcessGroup; its inner is the native
                // child. Our identity guard owns the group lifecycle instead.
                let status = self.child.inner_mut().wait().await?;
                self.group.after_reap().await?;
                Ok(status)
            })
            .await
            .context(
                "Process group cleanup timed out; repository protection must remain reserved.",
            )?
        }
        #[cfg(not(unix))]
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
    {
        command.wrap(process_wrap::tokio::JobObject);
        // Use the wrapper so JobObject preserves this flag when adding
        // CREATE_SUSPENDED for race-free descendant containment.
        command.wrap(process_wrap::tokio::CreationFlags(
            windows::Win32::System::Threading::CREATE_NO_WINDOW,
        ));
    }
    let child = command.spawn().with_context(|| {
        format!(
            "Cannot start {:?} in {}. Check the executable path and installation.",
            spec.program,
            spec.directory.display()
        )
    })?;
    Ok(ManagedChild {
        // No wait has been polled on this freshly spawned child, so its ID exists.
        #[cfg(unix)]
        group: unix::Group::new(child.id().expect("freshly spawned child has an ID")),
        child,
        #[cfg(not(unix))]
        reaped: false,
    })
}
pub fn cancel(child: &mut ManagedChild) -> std::io::Result<()> {
    #[cfg(unix)]
    return child.group.terminate();
    #[cfg(not(unix))]
    child.child.start_kill()
}

#[derive(Debug)]
pub struct ProcessResult {
    pub status: Option<ExitStatus>,
    pub cancelled: bool,
}

pub async fn execute(
    mut child: ManagedChild,
    input: Option<Vec<u8>>,
    cancellation: &Cancellation,
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
    let result = tokio::select! {
        biased;
        _ = cancellation.cancelled() => None,
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
            })
        }
        None => {
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
                cancelled: true,
            })
        }
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
