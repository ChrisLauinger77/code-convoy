//! The unreaped direct child reserves the PGID until the last destructive signal.
//! After reaping starts we ONLY probe with signal 0, including on cancellation,
//! timeout and Drop. A reused ID can delay/refuse cleanup, never receive a kill.
use std::{io, time::Duration};
use tokio::time::Instant;

const POLL: Duration = Duration::from_millis(5);
const CLEANUP: Duration = Duration::from_secs(5);

pub(super) fn check_reaping() -> io::Result<()> {
    // Reject automatic reaping before a process exists. Otherwise a running
    // child's PID could be recycled between the identity check and signalling.
    // CodeConvoy must exclusively own its children and never install an external
    // reaper or change SIGCHLD to auto-reap while jobs are active.
    let mut action: libc::sigaction = unsafe { std::mem::zeroed() };
    // SAFETY: reads the disposition only, using a valid output pointer.
    if unsafe { libc::sigaction(libc::SIGCHLD, std::ptr::null(), &mut action) } != 0 {
        return Err(io::Error::last_os_error());
    }
    if action.sa_sigaction == libc::SIG_IGN || action.sa_flags & libc::SA_NOCLDWAIT != 0 {
        return Err(io::Error::other(
            "Cannot safely supervise children while SIGCHLD automatically reaps them.",
        ));
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum Phase {
    Anchored,
    Draining(Instant),
    Uncertain,
}

pub(super) struct Group {
    pid: libc::pid_t,
    phase: Phase,
}

impl Group {
    pub fn new(pid: Option<u32>) -> io::Result<Self> {
        let pid = pid
            .and_then(|pid| libc::pid_t::try_from(pid).ok())
            .filter(|&pid| pid > 1)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "Spawned child has no valid Unix process identity.",
                )
            })?;
        Ok(Self {
            pid,
            phase: Phase::Anchored,
        })
    }

    fn exited(&mut self) -> io::Result<bool> {
        // SAFETY: zero is a valid siginfo_t initialization. Darwin may leave it
        // unchanged on WNOHANG, so reading si_pid without zeroing is incorrect.
        let mut info: libc::siginfo_t = unsafe { std::mem::zeroed() };
        loop {
            // SAFETY: valid pointer and positive ID of our exclusively owned
            // child. WNOWAIT does not reap it; no other CodeConvoy waiter does so.
            let result = unsafe {
                libc::waitid(
                    libc::P_PID,
                    self.pid as libc::id_t,
                    &mut info,
                    libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
                )
            };
            if result == 0 {
                // SAFETY: waitid initialized the SIGCHLD fields (or left zeros).
                return Ok(unsafe { info.si_pid() } == self.pid);
            }
            let error = io::Error::last_os_error();
            if error.kind() != io::ErrorKind::Interrupted {
                // ECHILD or any failed identity check forbids even a Drop kill.
                self.phase = Phase::Uncertain;
                return Err(error);
            }
        }
    }

    pub fn terminate(&mut self) -> io::Result<()> {
        match self.phase {
            Phase::Draining(_) => return Ok(()), // permanently disarmed
            Phase::Uncertain => {
                return Err(io::Error::other(
                    "Process identity is unconfirmed; refusing to signal its old group ID.",
                ));
            }
            Phase::Anchored => {}
        }
        let exited = self.exited()?;
        // SAFETY: no wait has reaped the child. Its PID cannot be recycled while
        // it is alive or a zombie. This is the only destructive Unix signal site.
        if unsafe { libc::kill(-self.pid, libc::SIGKILL) } == 0 {
            return Ok(());
        }
        let error = io::Error::last_os_error();
        match error.raw_os_error() {
            Some(libc::ESRCH) => Ok(()),
            // Darwin's POSIX group kill can report EPERM for a group containing
            // only zombies. This is NOT proof of cleanup: after reaping, wait
            // still requires ESRCH. A live inaccessible member keeps protection.
            Some(libc::EPERM) if cfg!(target_os = "macos") && exited => Ok(()),
            _ => Err(error),
        }
    }

    pub async fn before_reap(&mut self) -> io::Result<Instant> {
        if let Phase::Draining(deadline) = self.phase {
            return Ok(deadline);
        }
        if matches!(self.phase, Phase::Uncertain) {
            return Err(io::Error::other(
                "Process supervision lost its child identity.",
            ));
        }
        let mut poll = POLL;
        while !self.exited()? {
            tokio::time::sleep(poll).await;
            // Short commands finish promptly; long jobs need at most ~13 idle
            // probes/second. Cancellation drops this sleep and signals directly.
            poll = (poll * 2).min(Duration::from_millis(80));
        }
        // A completed CLI must not leave background writers behind, regardless
        // of exit code or inherited pipe state. Preserve the CLI's exit status.
        // A failed signal intentionally leaves Anchored so Drop may retry while
        // the unreaped child still reserves the identity.
        self.terminate()?;
        let deadline = Instant::now() + CLEANUP;
        // Disarm BEFORE the first poll of the reaping wait. Dropping that future
        // after it reaps cannot bring back permission to signal the numeric ID.
        self.phase = Phase::Draining(deadline);
        Ok(deadline)
    }

    pub async fn after_reap(&self) -> io::Result<()> {
        loop {
            // SAFETY: signal zero never sends a signal, even if the ID is reused.
            let result = unsafe { libc::kill(-self.pid, 0) };
            if result != 0 {
                let error = io::Error::last_os_error();
                match error.raw_os_error() {
                    Some(libc::ESRCH) => return Ok(()),
                    Some(libc::EPERM) | Some(libc::EINTR) => {}
                    _ => return Err(error),
                }
            }
            // Zombies awaiting init/launchd reaping also keep protection. Never
            // infer absence from pipes, parent status, permission errors or time.
            tokio::time::sleep(POLL).await;
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;
    use std::{
        os::unix::process::CommandExt,
        process::{Command, Stdio},
    };

    fn peer() -> std::process::Child {
        Command::new("/bin/sh")
            .args(["-c", "read value"])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .process_group(0)
            .spawn()
            .unwrap()
    }

    #[tokio::test]
    async fn invalid_spawn_identity_fails_supervision_without_signalling() {
        use super::super::{self as process, CommandSpec};
        for pid in [
            None,
            Some(0),
            Some(1),
            Some(i32::MAX as u32 + 1),
            Some(u32::MAX),
        ] {
            assert_eq!(
                Group::new(pid).err().unwrap().kind(),
                io::ErrorKind::InvalidInput
            );
        }
        let directory = tempfile::tempdir().unwrap();
        let mut spec = CommandSpec::new("/bin/sh", directory.path()).args(&["-c", "read value"]);
        spec.input = Some(Vec::new()); // Keep stdin open so the child cannot finish.
        let mut child = process::spawn(&spec).unwrap();
        // Model a wrapper losing its ID immediately after launch. Keep the real
        // guard only for fixture cleanup; the managed child must report failure.
        let identity = std::mem::replace(&mut child.group, Group::new(None));
        let wait = tokio::time::timeout(Duration::from_secs(1), child.wait()).await;
        let cancel = process::cancel(&mut child);
        let survived = child.child.inner_mut().try_wait().unwrap().is_none();
        child.group = identity;
        process::cancel(&mut child).unwrap();
        child.wait().await.unwrap();

        let error = wait
            .expect("identity failure must not await process exit")
            .unwrap_err();
        assert!(format!("{error:#}").contains("no valid Unix process identity"));
        assert_eq!(cancel.unwrap_err().kind(), io::ErrorKind::InvalidInput);
        assert!(survived, "unconfirmed identity must never authorize a kill");
    }

    #[test]
    fn automatic_reaping_is_rejected_before_launch() {
        const CHILD: &str = "CODECONVOY_REAPING_TEST";
        let Ok(mode) = std::env::var(CHILD) else {
            for mode in ["ignore", "no-child-wait"] {
                assert!(
                    Command::new(std::env::current_exe().unwrap())
                        .args([
                            "--exact",
                            "process::unix::tests::automatic_reaping_is_rejected_before_launch"
                        ])
                        .env(CHILD, mode)
                        .status()
                        .unwrap()
                        .success()
                );
            }
            return;
        };
        // Only this isolated single-test subprocess changes process-global state.
        let mut action: libc::sigaction = unsafe { std::mem::zeroed() };
        action.sa_sigaction = if mode == "ignore" {
            libc::SIG_IGN
        } else {
            libc::SIG_DFL
        };
        action.sa_flags = if mode == "no-child-wait" {
            libc::SA_NOCLDWAIT
        } else {
            0
        };
        // SAFETY: valid sigaction with an empty mask, in the dedicated subprocess.
        unsafe {
            assert_eq!(libc::sigemptyset(&mut action.sa_mask), 0);
            assert_eq!(
                libc::sigaction(libc::SIGCHLD, &action, std::ptr::null_mut()),
                0
            );
        }
        let directory = tempfile::tempdir().unwrap();
        let spec = super::super::CommandSpec::new("/bin/sh", directory.path())
            .args(&["-c", ": > unexpected-launch"]);
        // spawn must reject before it could even need an active Tokio runtime.
        let error = super::super::spawn(&spec)
            .err()
            .expect("auto-reaping must prevent launch");
        assert!(
            error.to_string().contains("automatically reaps"),
            "{error:#}"
        );
        assert!(!directory.path().join("unexpected-launch").exists());
    }

    #[tokio::test]
    async fn lost_child_identity_permanently_forbids_signalling_even_if_id_is_reused() {
        let mut child = Command::new("/bin/sh")
            .args(["-c", "exit 0"])
            .process_group(0)
            .spawn()
            .unwrap();
        let mut group = Group::new(Some(child.id())).unwrap();
        child.wait().unwrap(); // Simulate an unexpected external reaper.
        assert_eq!(
            group.terminate().unwrap_err().raw_os_error(),
            Some(libc::ECHILD)
        );
        let mut peer = peer();
        // Model numeric PID reuse deterministically, without exhausting system PIDs.
        group.pid = peer.id() as libc::pid_t;
        assert!(group.terminate().is_err());
        assert!(group.before_reap().await.is_err());
        let survived = peer.try_wait().unwrap().is_none();
        peer.kill().unwrap();
        peer.wait().unwrap();
        assert!(survived);
    }

    #[tokio::test]
    async fn reaping_disarms_repeated_cancellation_and_drop_even_after_timeout() {
        let mut peer = peer();
        let deadline = Instant::now() + Duration::from_millis(20);
        // Model the old group number being reused by an unrelated live group
        // after the original leader was reaped. All subsequent calls are probes.
        let mut group = Group {
            pid: peer.id() as libc::pid_t,
            phase: Phase::Draining(deadline),
        };
        assert!(group.terminate().is_ok());
        assert_eq!(group.before_reap().await.unwrap(), deadline);
        assert!(
            tokio::time::timeout_at(deadline, group.after_reap())
                .await
                .is_err()
        );
        assert!(group.terminate().is_ok()); // ManagedChild::Drop uses this path too.
        let survived = peer.try_wait().unwrap().is_none();
        peer.kill().unwrap();
        peer.wait().unwrap();
        assert!(survived);
    }

    #[tokio::test]
    async fn supervision_failure_returns_error_even_while_a_descendant_holds_output_open() {
        use super::super::{self as process, Cancellation, CommandSpec};
        let directory = tempfile::tempdir().unwrap();
        // Only disposable marker files, no numeric-ID cleanup after the forced
        // identity loss. The descendant stops via its explicit release marker.
        let spec = CommandSpec::new("/bin/sh", directory.path()).args(&[
            "-c",
            "( : > ready; while [ ! -e release ]; do sleep 0.01; done; : > stopped ) & while [ ! -e ready ]; do sleep 0.01; done; exit 0",
        ]);
        let child = process::spawn(&spec).unwrap();
        let pid = child.child.id().unwrap() as libc::pid_t;
        // SAFETY: this is the test's exclusively owned child, deliberately reaped
        // outside the supervisor to exercise its identity-loss path.
        assert_eq!(unsafe { libc::waitpid(pid, std::ptr::null_mut(), 0) }, pid);
        let result = tokio::time::timeout(
            Duration::from_secs(1),
            process::execute(
                child,
                None,
                &Cancellation::default(),
                process::cancel,
                |_, _| {},
            ),
        )
        .await;
        std::fs::write(directory.path().join("release"), "stop fixture").unwrap();
        tokio::time::timeout(Duration::from_secs(5), async {
            while !directory.path().join("stopped").exists() {
                tokio::time::sleep(POLL).await;
            }
        })
        .await
        .unwrap();
        let error = result
            .expect("supervision error must not wait for output EOF")
            .unwrap_err();
        assert!(error.to_string().contains("Cannot supervise"), "{error:#}");
    }
}
