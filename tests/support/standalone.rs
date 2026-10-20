//! Stand-in for OpenCode 2's `run --standalone`: a private server helper, in its
//! own process group on Unix like OpenCode's spawner places it, that the run
//! process stops before exiting and on SIGTERM (OpenCode's scope finalizer).
//! The working directory's name selects a failure mode: `orphan` leaves a server
//! that ignores stops running, `fast-orphan` does so and exits at once, before
//! any sample can list it, and `stubborn` also ignores SIGTERM in the run.
use std::{
    error::Error,
    io::Read,
    process::{Child, Command, Stdio},
    sync::{
        Mutex,
        atomic::{AtomicI32, Ordering},
    },
    time::Duration,
};

static SERVER: Mutex<Option<Child>> = Mutex::new(None);
static SERVER_PID: AtomicI32 = AtomicI32::new(0);

fn mode() -> String {
    std::env::current_dir()
        .ok()
        .and_then(|directory| {
            directory
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
        })
        .unwrap_or_default()
}

fn stubborn() -> bool {
    matches!(mode().as_str(), "orphan" | "fast-orphan" | "stubborn")
}

/// Starts the server and records its PID in `opencode2-server.pid`.
pub fn start() -> Result<(), Box<dyn Error>> {
    let mut command = Command::new(std::env::current_exe()?);
    command.arg("opencode2-server");
    if stubborn() {
        command.arg("stubborn");
    }
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    let child = command.spawn()?;
    std::fs::write("opencode2-server.pid", child.id().to_string())?;
    SERVER_PID.store(i32::try_from(child.id())?, Ordering::SeqCst);
    *SERVER.lock().map_err(|_| "server lock poisoned")? = Some(child);
    if mode() == "fast-orphan" {
        std::process::exit(0);
    }
    #[cfg(unix)]
    {
        let handler = if mode() == "stubborn" {
            libc::SIG_IGN
        } else {
            stop_and_exit as extern "C" fn(libc::c_int) as libc::sighandler_t
        };
        // SAFETY: installs a handler that only calls async-signal-safe functions.
        unsafe { libc::signal(libc::SIGTERM, handler) };
    }
    Ok(())
}

/// Stops a started server as OpenCode's finalizer does. Failure modes leave it.
pub fn stop() {
    if stubborn() {
        return;
    }
    if let Ok(mut server) = SERVER.lock()
        && let Some(mut child) = server.take()
    {
        // EOF on its stdin ends the server's lease.
        drop(child.stdin.take());
        let _ = child.wait();
    }
}

/// Exits like `std::process::exit`, after stopping a started server.
pub fn exit(code: i32) -> ! {
    stop();
    std::process::exit(code)
}

/// Stops a started server when the run returns normally.
pub struct Guard;
impl Drop for Guard {
    fn drop(&mut self) {
        stop();
    }
}

#[cfg(unix)]
extern "C" fn stop_and_exit(_: libc::c_int) {
    let pid = SERVER_PID.load(Ordering::SeqCst);
    // SAFETY: kill, waitpid and _exit are async-signal-safe, and pid is this
    // process's own unreaped child, so neither ID can have been recycled.
    unsafe {
        if pid > 0 {
            libc::kill(-pid, libc::SIGTERM);
            libc::waitpid(pid, std::ptr::null_mut(), 0);
        }
        libc::_exit(143)
    }
}

/// The server helper. A cooperative one ends at stdin EOF or on SIGTERM; a
/// stubborn one ignores both for 30 seconds (tests stop it explicitly).
pub fn serve(args: &[String]) -> Result<(), Box<dyn Error>> {
    if args.get(2).is_some_and(|mode| mode == "stubborn") {
        #[cfg(unix)]
        // SAFETY: ignoring a signal installs no handler code.
        unsafe {
            libc::signal(libc::SIGTERM, libc::SIG_IGN)
        };
        std::thread::sleep(Duration::from_secs(30));
        return Ok(());
    }
    std::io::stdin().read_to_end(&mut Vec::new())?;
    Ok(())
}
