//! Verified launch methods. Every entry cites its source in docs/git-clients.md;
//! a platform without a verified method is unavailable rather than guessed.
use super::{Argument, Client, Command, Environment, Error, PLACEHOLDER, Platform};
use std::path::{Path, PathBuf};

/// LaunchServices' documented command for opening files with an application.
const MACOS_OPEN: &str = "/usr/bin/open";

#[derive(Debug, PartialEq, Eq)]
pub(super) enum Method {
    /// Run the client's own launcher with these argument templates.
    Executable {
        names: &'static [&'static str],
        locations: &'static [Location],
        /// macOS: launcher inside a bundle the user may choose instead.
        bundle_executable: Option<&'static str>,
        arguments: &'static [&'static str],
    },
    /// macOS: `open -a <bundle> <directory>` hands the directory to the app.
    Application { bundle: &'static str },
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum Location {
    Absolute(&'static str),
    /// Below /Applications and ~/Applications.
    Applications(&'static str),
    LocalAppData(&'static str),
    ProgramFiles(&'static str),
}
impl Location {
    fn candidates(&self, environment: &Environment) -> Vec<PathBuf> {
        match self {
            Self::Absolute(path) => vec![PathBuf::from(path)],
            Self::Applications(relative) => application_folders(environment)
                .iter()
                .map(|folder| folder.join(relative))
                .collect(),
            Self::LocalAppData(relative) => environment
                .local_app_data
                .iter()
                .map(|base| base.join(relative))
                .collect(),
            Self::ProgramFiles(relative) => environment
                .program_files
                .iter()
                .map(|base| base.join(relative))
                .collect(),
        }
    }
}

fn application_folders(environment: &Environment) -> &[PathBuf] {
    &environment.applications
}

const REPOSITORY: &[&str] = &[PLACEHOLDER];

pub(super) fn method(client: Client, platform: Platform) -> Result<&'static Method, &'static str> {
    use Location::*;
    use Platform::*;
    match (client, platform) {
        // `gitkraken /path/to/repo` (GitKraken Desktop 12.1.0+). The install
        // guide documents the `gitkraken` terminal command on Linux.
        (Client::GitKraken, Linux) => Ok(&Method::Executable {
            names: &["gitkraken"],
            locations: &[Absolute("/snap/bin/gitkraken")],
            bundle_executable: None,
            arguments: REPOSITORY,
        }),
        (Client::GitKraken, Windows) => Ok(&Method::Executable {
            names: &["gitkraken"],
            locations: &[],
            bundle_executable: None,
            arguments: REPOSITORY,
        }),
        (Client::GitKraken, MacOs) => Err(
            "GitKraken documents no macOS command or application behavior for opening a repository path.",
        ),
        // README: `<SOURCEGIT_EXEC> <DIR>`; Linux packages install
        // /opt/sourcegit/sourcegit. macOS also handles LaunchServices opens.
        (Client::SourceGit, Linux) => Ok(&Method::Executable {
            names: &["sourcegit"],
            locations: &[Absolute("/opt/sourcegit/sourcegit")],
            bundle_executable: None,
            arguments: REPOSITORY,
        }),
        (Client::SourceGit, Windows) => Ok(&Method::Executable {
            names: &["sourcegit"],
            locations: &[],
            bundle_executable: None,
            arguments: REPOSITORY,
        }),
        (Client::SourceGit, MacOs) => Ok(&Method::Application {
            bundle: "SourceGit.app",
        }),
        // Shipped help: `angkorgit open [path]`, accepted by both the CLI shim
        // and the application binary. Its macOS shim uses `open -a`.
        (Client::AngKorGit, Linux) => Ok(&Method::Executable {
            names: &["angkorgit"],
            locations: &[],
            bundle_executable: None,
            arguments: &["open", PLACEHOLDER],
        }),
        (Client::AngKorGit, Windows) => Ok(&Method::Executable {
            names: &["angkorgit"],
            // Tauri installer defaults: per-user NSIS and per-machine MSI.
            locations: &[
                LocalAppData("AngKorGit/angkorgit.exe"),
                ProgramFiles("AngKorGit/angkorgit.exe"),
            ],
            bundle_executable: None,
            arguments: &["open", PLACEHOLDER],
        }),
        (Client::AngKorGit, MacOs) => Ok(&Method::Application {
            bundle: "AngKorGit.app",
        }),
        // Documented `smerge path` and per-platform install locations.
        (Client::SublimeMerge, Linux) => Ok(&Method::Executable {
            names: &["smerge"],
            locations: &[Absolute("/opt/sublime_merge/sublime_merge")],
            bundle_executable: None,
            arguments: REPOSITORY,
        }),
        (Client::SublimeMerge, MacOs) => Ok(&Method::Executable {
            names: &["smerge"],
            locations: &[Applications(
                "Sublime Merge.app/Contents/SharedSupport/bin/smerge",
            )],
            bundle_executable: Some("Contents/SharedSupport/bin/smerge"),
            arguments: REPOSITORY,
        }),
        (Client::SublimeMerge, Windows) => Ok(&Method::Executable {
            names: &["smerge"],
            locations: &[ProgramFiles("Sublime Merge/smerge.exe")],
            bundle_executable: None,
            arguments: REPOSITORY,
        }),
        // Release notes: `open /path/to/repository -a /Applications/Fork.app`
        // on macOS and `fork.exe path/to/repo` on Windows, where the maintainer
        // documents %LOCALAPPDATA%\Fork\Fork.exe with an absolute path.
        (Client::Fork, MacOs) => Ok(&Method::Application { bundle: "Fork.app" }),
        (Client::Fork, Windows) => Ok(&Method::Executable {
            names: &["fork"],
            locations: &[LocalAppData("Fork/Fork.exe")],
            bundle_executable: None,
            arguments: REPOSITORY,
        }),
        (Client::Fork, Linux) => Err("Fork is available for macOS and Windows only."),
        (Client::Custom, _) => Err("Custom has no built-in launch method."),
    }
}

pub(super) fn locate(
    client: Client,
    method: &Method,
    configured: &str,
    environment: &Environment,
) -> Result<Command, Error> {
    match method {
        Method::Executable {
            names,
            locations,
            bundle_executable,
            arguments,
        } => {
            let program = if configured.is_empty() {
                discover(names, locations, environment).ok_or(Error::NotFound {
                    client,
                    application: false,
                })?
            } else {
                super::executable(configured, *bundle_executable, environment)?
            };
            Ok(Command {
                client,
                program,
                arguments: arguments
                    .iter()
                    .map(|argument| Argument::Template((*argument).into()))
                    .collect(),
            })
        }
        Method::Application { bundle } => {
            let application = if configured.is_empty() {
                application_folders(environment)
                    .iter()
                    .map(|folder| folder.join(bundle))
                    .find(|path| super::is_bundle(path))
                    .ok_or(Error::NotFound {
                        client,
                        application: true,
                    })?
            } else {
                application(configured)?
            };
            Ok(Command {
                client,
                program: PathBuf::from(MACOS_OPEN),
                arguments: vec![
                    Argument::Template("-a".into()),
                    // A bundle path is literal even if it contains "{repo}".
                    Argument::Literal(application.into_os_string()),
                    Argument::Template(PLACEHOLDER.into()),
                ],
            })
        }
    }
}

fn discover(names: &[&str], locations: &[Location], environment: &Environment) -> Option<PathBuf> {
    names
        .iter()
        .find_map(|name| {
            crate::agents::discovery::search(name, "", environment.path.clone())
                .into_iter()
                .next()
        })
        .or_else(|| {
            locations
                .iter()
                .flat_map(|location| location.candidates(environment))
                .find(|path| super::check_executable(path).is_ok())
        })
}

fn application(configured: &str) -> Result<PathBuf, Error> {
    let path = Path::new(configured);
    let invalid = |problem| Error::Application {
        path: configured.to_owned(),
        problem,
    };
    if configured.contains(['\0', '\n', '\r']) || !path.is_absolute() {
        return Err(invalid(
            "must be an absolute path to an application bundle.",
        ));
    }
    if !path.exists() {
        return Err(invalid("does not exist."));
    }
    if !super::is_bundle(path) {
        return Err(invalid("is not an application bundle (.app)."));
    }
    Ok(path.to_owned())
}
