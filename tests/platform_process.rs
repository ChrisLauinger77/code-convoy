#![cfg(feature = "test-support")]
#![allow(clippy::unwrap_used)]
//! Real OS processes in disposable paths; the same tests run on all CI hosts.
#[path = "support/executable.rs"]
mod executable;

use codeconvoy::process::{self, Cancellation, CommandSpec};
use std::{fs, path::Path, time::Duration};

fn executable(root: &Path) -> std::path::PathBuf {
    let directory = root.join("CLI tools ü 日本語");
    fs::create_dir(&directory).unwrap();
    let name = if cfg!(windows) {
        "fixture agent.exe"
    } else {
        "fixture agent"
    };
    let path = directory.join(name);
    executable::install(&path);
    path
}

#[cfg(windows)]
fn descendant_stopped(control: &Path) -> bool {
    use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
    use windows::Win32::{
        Foundation::{ERROR_INVALID_PARAMETER, HANDLE, WAIT_OBJECT_0, WAIT_TIMEOUT},
        System::Threading::{OpenProcess, PROCESS_SYNCHRONIZE, WaitForSingleObject},
    };

    // The fixture writes its PID before the ready handshake. This read-only
    // probe never signals a PID. Reuse can only cause a conservative failure:
    // a reused PID means the original fixture has already stopped.
    let pid: u32 = fs::read_to_string(control.join("child.pid"))
        .unwrap()
        .parse()
        .unwrap();
    assert_ne!(pid, 0);
    // Open only while observing, so retained test handles cannot delay job
    // accounting. An absent PID or a signaled process proves termination;
    // access failures are test failures, never evidence of successful cleanup.
    let process = match unsafe { OpenProcess(PROCESS_SYNCHRONIZE, false, pid) } {
        Ok(handle) => unsafe { OwnedHandle::from_raw_handle(handle.0) },
        Err(error) if error.code() == ERROR_INVALID_PARAMETER.to_hresult() => return true,
        Err(error) => panic!("Cannot observe fixture process {pid}: {error}"),
    };
    // SAFETY: fresh owned process handle with synchronization access; no wait.
    let state = unsafe { WaitForSingleObject(HANDLE(process.as_raw_handle()), 0) };
    assert!(state == WAIT_OBJECT_0 || state == WAIT_TIMEOUT);
    state == WAIT_OBJECT_0
}

async fn assert_descendant_cleanup(control: &Path, lock: &fs::File) {
    #[cfg(windows)]
    assert!(
        descendant_stopped(control),
        "execution returned while its native descendant was still running"
    );
    #[cfg(not(windows))]
    let _ = control;
    #[cfg(unix)]
    assert!(fs2::FileExt::try_lock_exclusive(lock).is_ok());
    // Windows may release file locks later than process termination (LockFileEx
    // documentation). Native termination above must already be observable;
    // this bounded wait checks the separate filesystem cleanup consequence.
    tokio::time::timeout(Duration::from_secs(5), async {
        while fs2::FileExt::try_lock_exclusive(lock).is_err() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("descendant retained its OS lock after termination");
}

#[tokio::test]
async fn native_executable_directory_arguments_and_stdin_preserve_unicode_and_spaces() {
    let temp = executable::directory();
    let executable = executable(temp.path());
    let directory = temp.path().join("repository ü 日本語");
    fs::create_dir(&directory).unwrap();
    let arguments = [
        "",
        "two words",
        "日本語",
        "\"quoted\"",
        "trailing\\",
        "& | ; $(literal) %PATH%",
    ];
    let mut spec = CommandSpec::new(executable, &directory).args(&["invocation-probe"]);
    spec.args
        .extend(arguments.iter().map(std::ffi::OsString::from));
    let input = "Task with spaces and 日本語\n$(not a shell)\r\n";
    spec.input = Some(input.as_bytes().to_vec());
    let result = process::capture(spec, 16 * 1024).await.unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(!result.truncated);
    let receipt: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(receipt["arguments"], serde_json::json!(arguments));
    assert_eq!(receipt["input"], input);
    assert_eq!(
        Path::new(receipt["directory"].as_str().unwrap())
            .canonicalize()
            .unwrap(),
        directory.canonicalize().unwrap()
    );
}

#[tokio::test]
async fn expired_deadline_stops_descendants_and_ready_cancellation_takes_precedence() {
    for cancel_first in [false, true] {
        let temp = executable::directory();
        let executable = executable(temp.path());
        let mut spec = CommandSpec::new(executable, temp.path()).args(&["validation-gate"]);
        spec.args.push(temp.path().as_os_str().to_owned());
        let child = process::spawn(&spec).unwrap();
        // Establish a live descendant before expiring the deadline, without a
        // race against fixture startup or an assumption about process speed.
        tokio::time::timeout(Duration::from_secs(10), async {
            while !temp.path().join("child.ready").exists() {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .unwrap();
        let lock = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(temp.path().join("child.lock"))
            .unwrap();
        assert!(fs2::FileExt::try_lock_exclusive(&lock).is_err());
        #[cfg(windows)]
        assert!(!descendant_stopped(temp.path()));
        let cancellation = Cancellation::default();
        if cancel_first {
            cancellation.cancel();
        }
        let result = process::execute_until(
            child,
            None,
            &cancellation,
            Some(tokio::time::Instant::now()),
            process::cancel,
            |_, _| {},
        )
        .await
        .unwrap();
        assert_eq!(result.cancelled, cancel_first);
        assert_eq!(result.timed_out, !cancel_first);
        assert!(result.status.is_some());
        assert_descendant_cleanup(temp.path(), &lock).await;
    }
}

#[tokio::test]
async fn cancellation_and_future_drop_stop_descendants_in_unicode_paths() {
    for abort in [false, true] {
        let temp = executable::directory();
        let executable = executable(temp.path());
        let control = temp.path().join("child control ü 日本語");
        fs::create_dir(&control).unwrap();
        let mut spec = CommandSpec::new(executable, temp.path()).args(&["smudge-gate"]);
        spec.args.push(control.as_os_str().to_owned());
        spec.input = Some(b"fixture input".to_vec());
        let cancellation = Cancellation::default();
        let token = cancellation.clone();
        let task =
            tokio::spawn(async move { process::capture_cancellable(spec, 4096, &token).await });
        let ready = tokio::time::timeout(Duration::from_secs(10), async {
            while !control.join("filter.ready").exists() {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await;
        if ready.is_err() {
            task.abort();
            let _ = task.await;
            panic!("descendant did not reach its lock handshake");
        }
        let lock = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(control.join("filter.lock"))
            .unwrap();
        assert!(fs2::FileExt::try_lock_exclusive(&lock).is_err());
        if abort {
            // Models shutdown dropping a native probe's future, not just Stop.
            task.abort();
            assert!(matches!(task.await, Err(error) if error.is_cancelled()));
        } else {
            cancellation.cancel();
            let Err(error) = task.await.unwrap() else {
                panic!("cancelled tree reported success");
            };
            assert!(error.to_string().contains("cancelled"), "{error:#}");
        }
        // A lock held by the child proves actual OS cleanup without PID reuse
        // races or a timing assertion based only on a delayed output file.
        tokio::time::timeout(Duration::from_secs(5), async {
            while fs2::FileExt::try_lock_exclusive(&lock).is_err() {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("descendant retained its OS lock after cancellation");
        assert!(!control.join("filter.survived").exists());
    }
}

#[tokio::test]
async fn cancellation_racing_leader_failure_stops_descendants_but_preserves_a_peer() {
    let temp = executable::directory();
    let executable = executable(temp.path());
    let control = temp.path().join("failed tree");
    let peer_control = temp.path().join("independent tree");
    fs::create_dir(&control).unwrap();
    fs::create_dir(&peer_control).unwrap();
    let start = |mode, directory: &Path, token: Cancellation| {
        let mut spec = CommandSpec::new(&executable, temp.path()).args(&[mode]);
        spec.args.push(directory.as_os_str().to_owned());
        tokio::spawn(async move { process::capture_cancellable(spec, 4096, &token).await })
    };
    let cancellation = Cancellation::default();
    let failed = start("validation-crash-tree", &control, cancellation.clone());
    let peer_token = Cancellation::default();
    let peer = start("validation-gate", &peer_control, peer_token.clone());
    let ready = tokio::time::timeout(Duration::from_secs(10), async {
        for path in [&control, &peer_control] {
            while !path.join("child.ready").exists() {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        }
        fs::write(control.join("exit-parent"), "exit leader").unwrap();
        // This lock is released by OS process exit, proving that cancellation
        // below runs after the leader has died, not merely after child startup.
        let parent = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(control.join("parent.lock"))
            .unwrap();
        while fs2::FileExt::try_lock_exclusive(&parent).is_err() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await;
    if ready.is_err() {
        cancellation.cancel();
        peer_token.cancel();
        let _ = failed.await;
        let _ = peer.await;
        panic!("fixture leader did not exit after starting its descendant");
    }
    let child_lock = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(control.join("child.lock"))
        .unwrap();
    let peer_lock = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(peer_control.join("child.lock"))
        .unwrap();
    cancellation.cancel();
    cancellation.cancel();
    let result = failed.await.unwrap();
    let stopped = tokio::time::timeout(Duration::from_secs(5), async {
        while fs2::FileExt::try_lock_exclusive(&child_lock).is_err() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await;
    let peer_alive = !peer.is_finished() && fs2::FileExt::try_lock_exclusive(&peer_lock).is_err();
    fs::write(peer_control.join("release"), "finish peer").unwrap();
    // Also release the failed fixture on a regression, without any PID-based kill.
    fs::write(control.join("release"), "finish descendant").unwrap();
    let peer_result = peer.await.unwrap().unwrap();
    match result {
        Err(error) => assert!(error.to_string().contains("cancelled"), "{error:#}"),
        // Unix supervision may already have finished cleaning the failed tree.
        Ok(result) => assert_eq!(result.status.code(), Some(7)),
    }
    assert!(
        stopped.is_ok(),
        "descendant survived cancellation after leader failure"
    );
    assert!(
        peer_alive && peer_result.status.success(),
        "cancellation affected an unrelated process tree"
    );
}

#[tokio::test]
async fn parent_exit_cannot_complete_while_closed_stream_descendant_holds_a_lock() {
    for code in [0, 7] {
        let temp = executable::directory();
        let executable = executable(temp.path());
        let mut spec = CommandSpec::new(executable, temp.path()).args(&["validation-orphan-tree"]);
        spec.args
            .extend([temp.path().as_os_str().to_owned(), code.to_string().into()]);
        let task = tokio::spawn(async move { process::capture(spec, 4096).await });
        tokio::time::timeout(Duration::from_secs(10), async {
            while !temp.path().join("child.ready").exists() {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .unwrap();
        let lock = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(temp.path().join("child.lock"))
            .unwrap();
        assert!(fs2::FileExt::try_lock_exclusive(&lock).is_err());
        #[cfg(windows)]
        assert!(!descendant_stopped(temp.path()));
        fs::write(temp.path().join("exit-parent"), "exit with live descendant").unwrap();
        let result = task.await.unwrap();
        #[cfg(windows)]
        let stopped_before_return = descendant_stopped(temp.path());
        #[cfg(not(windows))]
        let stopped_before_return = fs2::FileExt::try_lock_exclusive(&lock).is_ok();
        // Always release the original unfixed fixture before asserting.
        fs::write(temp.path().join("release"), "finish fixture").unwrap();
        tokio::time::timeout(Duration::from_secs(5), async {
            while fs2::FileExt::try_lock_exclusive(&lock).is_err() {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .unwrap();
        assert!(
            stopped_before_return,
            "parent exit {code} returned while its closed-stream descendant was alive: {:?}",
            result.as_ref().map(|output| output.status)
        );
        assert_eq!(result.unwrap().status.code(), Some(code));
    }
}
