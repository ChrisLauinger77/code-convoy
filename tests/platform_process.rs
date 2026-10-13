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
