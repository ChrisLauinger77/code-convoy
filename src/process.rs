//! Shell-free process execution with bounded output and process-tree cancellation.
use anyhow::{Context, Result};
use process_wrap::tokio::{ChildWrapper, CommandWrap, KillOnDrop};
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
    reaped: bool,
}
impl Drop for ManagedChild {
    fn drop(&mut self) {
        if !self.reaped {
            let _ = self.child.start_kill();
        }
    }
}
pub fn spawn(spec: &CommandSpec) -> Result<ManagedChild> {
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
    command.wrap(KillOnDrop);
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
        child,
        reaped: false,
    })
}
pub fn cancel(child: &mut ManagedChild) -> std::io::Result<()> {
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
                    return Err(e);
                }
            }
            Ok::<_, std::io::Error>(())
        };
        let (status, out, err, written) = tokio::join!(
            child.child.wait(),
            pump(stdout, Stream::Stdout, &output),
            pump(stderr, Stream::Stderr, &output),
            writer
        );
        Ok::<_, anyhow::Error>((status?, out?, err?, written?).0)
    };
    let result = tokio::select! {
        biased;
        _ = cancellation.cancelled() => None,
        result = work => Some(result),
    };
    match result {
        Some(result) => {
            let status = result?;
            child.reaped = true;
            Ok(ProcessResult {
                status: Some(status),
                cancelled: false,
            })
        }
        None => {
            cancel_process(&mut child).context("Could not terminate the agent process tree.")?;
            let status = tokio::time::timeout(Duration::from_secs(5), child.child.wait())
                .await
                .context("Agent termination timed out; inspect remaining processes.")??;
            child.reaped = true;
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
) -> std::io::Result<()> {
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
    anyhow::ensure!(!cancellation.is_cancelled(), "Command cancelled.");
    let command_cancellation = Cancellation::default();
    let data = std::sync::Mutex::new((Vec::new(), Vec::new(), false));
    let child = spawn(&spec)?;
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
            anyhow::bail!("Command cancelled.")
        }
        result = &mut execution => result?,
        _ = tokio::time::sleep(Duration::from_secs(20)) => {
            command_cancellation.cancel();
            let _ = execution.await;
            anyhow::bail!("Command timed out after 20 seconds.")
        }
    };
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
