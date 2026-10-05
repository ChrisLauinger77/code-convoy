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
    for name in ["codex", "copilot", "opencode", "claude"] {
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
    println!(
        "AppImage host Git, repository access, all four CLI fixture lookups, and environment preservation passed."
    );
    Ok(())
}
