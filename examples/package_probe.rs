//! Packaging-only probe; never distributed or used for authenticated agent tasks.
use anyhow::{Result, ensure};
use codeconvoy::{
    git,
    process::{self, CommandSpec},
};

#[tokio::main]
async fn main() -> Result<()> {
    ensure!(
        std::env::var("PATH")? == std::env::var("CODECONVOY_EXPECT_PATH")?,
        "AppImage changed host PATH"
    );
    ensure!(
        std::env::var("LD_LIBRARY_PATH").unwrap_or_default()
            == std::env::var("CODECONVOY_EXPECT_LD_LIBRARY_PATH")?,
        "AppImage changed host library search path"
    );
    let directory = tempfile::tempdir()?;
    let repository = directory.path().join("host repository ü");
    std::fs::create_dir(&repository)?;
    let init = process::capture(
        CommandSpec::new("git", &repository).args(&["init", "--quiet"]),
        8192,
    )
    .await?;
    ensure!(
        init.status.success(),
        "Host Git could not initialize the disposable repository"
    );
    let registered = git::register(&repository).await?;
    std::fs::write(repository.join("untracked.txt"), "host repository access")?;
    ensure!(
        git::status(&registered.path).await?.summary.changed == 1,
        "Host repository inspection failed"
    );
    // Exercise the distribution's Git, including its normal administration and
    // removal commands, only inside this probe's disposable repository.
    for args in [
        vec!["add", "untracked.txt"],
        vec![
            "-c",
            "user.name=Package probe",
            "-c",
            "user.email=probe@example.invalid",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "--quiet",
            "-m",
            "fixture",
        ],
        vec![
            "worktree",
            "add",
            "--quiet",
            "--detach",
            "../checkout",
            "HEAD",
        ],
        vec!["worktree", "remove", "../checkout"],
    ] {
        let result =
            process::capture(CommandSpec::new("git", &repository).args(&args), 8192).await?;
        ensure!(
            result.status.success(),
            "Disposable Git worktree probe failed"
        );
    }
    ensure!(
        !directory.path().join("checkout").exists(),
        "Worktree was not removed"
    );
    for name in ["codex", "copilot", "opencode", "claude", "agent"] {
        let result = process::capture(
            CommandSpec::new(name, &repository).args(&["--version"]),
            8192,
        )
        .await?;
        ensure!(
            result.status.success()
                && String::from_utf8_lossy(&result.stdout).trim() == format!("package-host-{name}"),
            "Host executable lookup failed for {name}"
        );
    }
    #[cfg(unix)]
    cancellation_probe(&repository).await?;
    println!(
        "AppImage host Git, disposable worktree creation/removal, all five CLI fixture lookups, process cancellation, and environment preservation passed."
    );
    Ok(())
}

#[cfg(unix)]
async fn cancellation_probe(repository: &std::path::Path) -> Result<()> {
    use std::time::Duration;
    let token = process::Cancellation::default();
    // A deterministic fixture, not an agent command: the descendant redirects
    // every standard stream and announces readiness before cancellation.
    let child = process::spawn(&CommandSpec::new("sh", repository).args(&[
        "-c",
        "sleep 60 </dev/null >/dev/null 2>&1 & printf '%s\\n' \"$!\" > descendant.pid; touch ready; wait",
    ]))?;
    let readiness = async {
        let ready = tokio::time::timeout(Duration::from_secs(10), async {
            while !repository.join("ready").exists() {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await;
        token.cancel(); // Also clean up if the readiness assertion fails.
        ready
    };
    let (result, ready) = tokio::join!(
        process::execute(child, None, &token, process::cancel, |_, _| {}),
        readiness
    );
    ready?;
    ensure!(result?.cancelled, "Fixture cancellation was not observed");
    let pid = std::fs::read_to_string(repository.join("descendant.pid"))?;
    let state = process::capture(
        CommandSpec::new("ps", repository).args(&["-o", "stat=", "-p", pid.trim()]),
        8192,
    )
    .await?;
    let state = String::from_utf8_lossy(&state.stdout);
    ensure!(
        state.trim().is_empty() || state.trim().starts_with('Z'),
        "Cancelled fixture descendant remains active: {state}"
    );
    Ok(())
}
