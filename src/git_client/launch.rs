//! Detached, shell-free start of a Git client. Unlike agent processes, the
//! client outlives CodeConvoy and is never cancelled, supervised or killed.
use super::{Command, Error, Preferences, Target};
use std::{
    ffi::OsString,
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};

/// Launchers such as `open` or `smerge` hand off and exit. A failure within
/// this window is reported; a client still running afterwards is left alone.
const OBSERVE: Duration = Duration::from_secs(2);

/// Open one existing directory. The caller decides which directory; this only
/// checks that it is an absolute, existing directory before starting the client.
pub async fn open(preferences: Preferences, directory: PathBuf) -> Result<(), Error> {
    let (command, directory) = tokio::task::spawn_blocking(move || {
        super::check_directory(&directory)?;
        Ok::<_, Error>((super::resolve(&preferences)?, directory))
    })
    .await
    .map_err(|error| Error::Internal(error.to_string()))??;
    start(&command, &directory, OBSERVE).await
}

/// Open a recorded repository or worktree after proving, from Git's metadata
/// files, that it is still that working tree. Nothing is substituted for it.
pub async fn open_target(preferences: Preferences, target: Target) -> Result<(), Error> {
    let directory = tokio::task::spawn_blocking(move || {
        super::target::verify(&target)?;
        Ok::<_, Error>(target.path)
    })
    .await
    .map_err(|error| Error::Internal(error.to_string()))??;
    open(preferences, directory).await
}

pub(crate) async fn start(
    command: &Command,
    directory: &Path,
    observe: Duration,
) -> Result<(), Error> {
    let mut process = tokio::process::Command::new(&command.program);
    process
        .args(command.arguments_for(directory))
        // Only the child's directory. A client that ignores its arguments still
        // starts in the requested tree, never in CodeConvoy's own directory.
        .current_dir(directory)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(false);
    for key in removed_environment(std::env::vars_os(), own_appimage()) {
        process.env_remove(key);
    }
    // Keep terminal signals for CodeConvoy away from the independent client.
    #[cfg(unix)]
    process.process_group(0);
    #[cfg(windows)]
    {
        use windows::Win32::System::Threading::{CREATE_NEW_PROCESS_GROUP, CREATE_NO_WINDOW};
        // No console window for console launchers such as smerge.exe.
        process.creation_flags(CREATE_NO_WINDOW.0 | CREATE_NEW_PROCESS_GROUP.0);
    }
    let mut child = process.spawn().map_err(|error| Error::Start {
        program: command.program.clone(),
        detail: error.to_string(),
    })?;
    match tokio::time::timeout(observe, child.wait()).await {
        Ok(Ok(status)) if status.success() => Ok(()),
        Ok(Ok(status)) => Err(Error::Exited {
            client: command.client,
            status: status.to_string(),
        }),
        Ok(Err(error)) => Err(Error::Internal(format!(
            "could not observe {}: {error}",
            command.program.display()
        ))),
        // Dropping the handle neither kills nor waits; Tokio reaps it later.
        Err(_) => Ok(()),
    }
}

/// The client must see the selected tree and the user's desktop session, so
/// the environment is inherited except for variables that would redirect Git
/// or describe CodeConvoy's own AppImage mount.
pub(super) fn removed_environment(
    variables: impl Iterator<Item = (OsString, OsString)>,
    appimage: bool,
) -> Vec<OsString> {
    variables
        .filter_map(|(key, _)| {
            let name = key.to_str()?;
            (crate::process::git_environment_override(name, cfg!(windows))
                || appimage && ["APPIMAGE", "APPDIR", "ARGV0", "OWD"].contains(&name))
            .then_some(key)
        })
        .collect()
}

/// The AppImage runtime exports these for CodeConvoy. A host-installed client
/// that inherits them may treat CodeConvoy's mount as its own resources.
fn own_appimage() -> bool {
    cfg!(target_os = "linux")
        && std::env::var_os("APPDIR").is_some_and(|appdir| {
            std::env::current_exe().is_ok_and(|exe| exe.starts_with(Path::new(&appdir)))
        })
}
