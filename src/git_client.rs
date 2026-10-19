//! Opens one directory in the user's external Git client. Presets contain only
//! launch methods verified from each client's documentation or source; see
//! docs/git-clients.md. Lookups and process start run off the UI thread. Opening
//! never runs Git, never changes a repository and never supervises the client.
mod launch;
mod presets;
#[cfg(test)]
mod tests;

pub use launch::open;

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::{
    ffi::{OsStr, OsString},
    fmt,
    path::{Path, PathBuf},
};

/// Replaced by the absolute repository or worktree path in every argument.
pub const PLACEHOLDER: &str = "{repo}";
const MAX_ARGUMENTS: usize = 64;
const MAX_TEXT: usize = 4096;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Client {
    GitKraken,
    SourceGit,
    AngKorGit,
    SublimeMerge,
    Fork,
    Custom,
}
impl Client {
    pub const ALL: [Self; 6] = [
        Self::GitKraken,
        Self::SourceGit,
        Self::AngKorGit,
        Self::SublimeMerge,
        Self::Fork,
        Self::Custom,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::GitKraken => "GitKraken",
            Self::SourceGit => "SourceGit",
            Self::AngKorGit => "AngKorGit",
            Self::SublimeMerge => "Sublime Merge",
            Self::Fork => "Fork",
            Self::Custom => "Custom",
        }
    }
    /// Stable persisted name. Never rename an existing key.
    fn key(self) -> &'static str {
        match self {
            Self::GitKraken => "gitkraken",
            Self::SourceGit => "sourcegit",
            Self::AngKorGit => "angkorgit",
            Self::SublimeMerge => "sublime_merge",
            Self::Fork => "fork",
            Self::Custom => "custom",
        }
    }
    /// Whether the configured location is a macOS application bundle.
    pub fn opens_application(self, platform: Option<Platform>) -> bool {
        platform.is_some_and(|platform| {
            presets::method(self, platform)
                .is_ok_and(|method| matches!(method, presets::Method::Application { .. }))
        })
    }
    /// Whether this build offers the client, and why not otherwise.
    pub fn availability(self, platform: Option<Platform>) -> Result<(), &'static str> {
        match (self, platform) {
            (Self::Custom, _) => Ok(()),
            (_, None) => Err("Built-in presets support Linux, macOS and Windows only."),
            (client, Some(platform)) => presets::method(client, platform).map(|_| ()),
        }
    }
}
impl Serialize for Client {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.key())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    Linux,
    MacOs,
    Windows,
}
impl Platform {
    pub fn current() -> Option<Self> {
        if cfg!(target_os = "linux") {
            Some(Self::Linux)
        } else if cfg!(target_os = "macos") {
            Some(Self::MacOs)
        } else if cfg!(windows) {
            Some(Self::Windows)
        } else {
            None
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Linux => "Linux",
            Self::MacOs => "macOS",
            Self::Windows => "Windows",
        }
    }
}

/// Global preference. Old saves have no field and load as not configured.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Preferences {
    /// A client saved by a newer CodeConvoy loads as not configured.
    #[serde(
        deserialize_with = "known_client",
        skip_serializing_if = "Option::is_none"
    )]
    pub client: Option<Client>,
    /// Preset: optional override of automatic detection. Custom: required.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub executable: String,
    /// Custom only: one process argument per entry, never parsed by a shell.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub arguments: Vec<String>,
}
impl Preferences {
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
    /// A fresh configuration for a newly selected client.
    pub fn select(client: Option<Client>) -> Self {
        Self {
            client,
            executable: String::new(),
            arguments: if client == Some(Client::Custom) {
                vec![PLACEHOLDER.into()]
            } else {
                Vec::new()
            },
        }
    }
}

fn known_client<'de, D: Deserializer<'de>>(d: D) -> Result<Option<Client>, D::Error> {
    let key = Option::<String>::deserialize(d)?;
    Ok(key.and_then(|key| Client::ALL.into_iter().find(|client| client.key() == key)))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Argument {
    /// Every `{repo}` is replaced by the target path.
    Template(String),
    /// Passed unchanged, for example a located application bundle.
    Literal(OsString),
}

/// A located launcher. Arguments stay separate; no shell ever joins them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Command {
    pub client: Client,
    pub program: PathBuf,
    pub arguments: Vec<Argument>,
}
impl Command {
    pub fn arguments_for(&self, repository: &Path) -> Vec<OsString> {
        self.arguments
            .iter()
            .map(|argument| match argument {
                Argument::Template(template) => substitute(template, repository.as_os_str()),
                Argument::Literal(value) => value.clone(),
            })
            .collect()
    }
    /// Human-readable form for diagnostics. It is not a shell command.
    pub fn describe(&self) -> String {
        std::iter::once(self.program.display().to_string())
            .chain(self.arguments.iter().map(|argument| match argument {
                Argument::Template(template) => template.clone(),
                Argument::Literal(value) => value.to_string_lossy().into_owned(),
            }))
            .collect::<Vec<_>>()
            .join(" · ")
    }
}

fn substitute(template: &str, repository: &OsStr) -> OsString {
    let mut parts = template.split(PLACEHOLDER);
    let mut argument = OsString::from(parts.next().unwrap_or_default());
    for part in parts {
        argument.push(repository);
        argument.push(part);
    }
    argument
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    NotConfigured,
    Unavailable {
        client: Client,
        platform: Option<Platform>,
        reason: &'static str,
    },
    NotFound {
        client: Client,
        application: bool,
    },
    Executable {
        path: String,
        problem: &'static str,
    },
    Application {
        path: String,
        problem: &'static str,
    },
    Arguments(&'static str),
    Directory {
        path: PathBuf,
        problem: &'static str,
    },
    Start {
        program: PathBuf,
        detail: String,
    },
    Exited {
        client: Client,
        status: String,
    },
    Internal(String),
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotConfigured => {
                write!(
                    f,
                    "No Git client is configured. Choose one in Settings › Git client."
                )
            }
            Self::Unavailable {
                client,
                platform,
                reason,
            } => write!(
                f,
                "{} is not available on {}. {reason} Choose Custom in Settings › Git client to configure a command.",
                client.label(),
                platform.map_or("this operating system", Platform::label)
            ),
            Self::NotFound {
                client,
                application: true,
            } => write!(
                f,
                "{} was not found in Applications. Install it, or choose its application bundle in Settings › Git client.",
                client.label()
            ),
            Self::NotFound { client, .. } => write!(
                f,
                "{} was not found. Install it, or enter its executable path in Settings › Git client.",
                client.label()
            ),
            Self::Executable { path, problem } if path.is_empty() => {
                write!(f, "A Git client executable {problem}")
            }
            Self::Executable { path, problem } => {
                write!(f, "Git client executable {path} {problem}")
            }
            Self::Application { path, problem } => {
                write!(f, "Git client application {path} {problem}")
            }
            Self::Arguments(problem) => f.write_str(problem),
            Self::Directory { path, problem } => write!(
                f,
                "Cannot open {} in the Git client: {problem}",
                path.display()
            ),
            Self::Start { program, detail } => write!(
                f,
                "Could not start the Git client {}: {detail}. Check its installation and path in Settings › Git client.",
                program.display()
            ),
            Self::Exited { client, status } => write!(
                f,
                "{} stopped with {status} before opening the directory. Check its installation and arguments in Settings › Git client.",
                client.label()
            ),
            Self::Internal(detail) => write!(f, "Could not open the Git client: {detail}"),
        }
    }
}
impl std::error::Error for Error {}

/// Host directories searched for launcher names. Captured once per lookup.
#[derive(Debug, Clone, Default)]
pub(crate) struct Environment {
    pub path: Vec<PathBuf>,
    /// macOS application folders, searched for preset bundles.
    pub applications: Vec<PathBuf>,
    pub local_app_data: Option<PathBuf>,
    pub program_files: Option<PathBuf>,
}
impl Environment {
    pub fn current() -> Self {
        let home = directories::BaseDirs::new().map(|dirs| dirs.home_dir().to_owned());
        let mut path: Vec<PathBuf> = std::env::var_os("PATH")
            .map(|path| std::env::split_paths(&path).collect())
            .unwrap_or_default();
        // GUI sessions (for example macOS Finder launches) often have a minimal
        // PATH. These are directories, not shell profiles.
        if let Some(home) = &home {
            path.extend([home.join(".local/bin"), home.join("bin")]);
            #[cfg(windows)]
            path.push(home.join("scoop/shims"));
        }
        #[cfg(unix)]
        path.extend(
            [
                "/opt/homebrew/bin",
                "/usr/local/bin",
                "/usr/bin",
                "/bin",
                "/snap/bin",
            ]
            .map(PathBuf::from),
        );
        let local_app_data = std::env::var_os("LOCALAPPDATA").map(PathBuf::from);
        #[cfg(windows)]
        if let Some(local) = &local_app_data {
            path.push(local.join("Microsoft/WinGet/Links"));
        }
        let mut applications = vec![PathBuf::from("/Applications")];
        applications.extend(home.map(|home| home.join("Applications")));
        Self {
            path,
            applications,
            local_app_data,
            program_files: std::env::var_os("ProgramFiles").map(PathBuf::from),
        }
    }
}

/// Filesystem-only lookup; run on a blocking worker, never while rendering.
pub fn resolve(preferences: &Preferences) -> Result<Command, Error> {
    resolve_in(preferences, Platform::current(), &Environment::current())
}

pub(crate) fn resolve_in(
    preferences: &Preferences,
    platform: Option<Platform>,
    environment: &Environment,
) -> Result<Command, Error> {
    let client = preferences.client.ok_or(Error::NotConfigured)?;
    // Paths are used exactly, including surrounding whitespace; a blank
    // value means automatic detection for presets.
    let configured = match preferences.executable.as_str() {
        blank if blank.trim().is_empty() => "",
        exact => exact,
    };
    if client == Client::Custom {
        let arguments = custom_arguments(&preferences.arguments)?;
        if configured.is_empty() {
            return Err(Error::Executable {
                path: String::new(),
                problem: "is required for Custom. Enter an absolute path or a command name.",
            });
        }
        let program = executable(configured, None, environment)?;
        return Ok(Command {
            client,
            program,
            arguments,
        });
    }
    let platform = platform.ok_or(Error::Unavailable {
        client,
        platform,
        reason: "Built-in presets support Linux, macOS and Windows only.",
    })?;
    let method = presets::method(client, platform).map_err(|reason| Error::Unavailable {
        client,
        platform: Some(platform),
        reason,
    })?;
    presets::locate(client, method, configured, environment)
}

fn custom_arguments(arguments: &[String]) -> Result<Vec<Argument>, Error> {
    if arguments.len() > MAX_ARGUMENTS {
        return Err(Error::Arguments("Use at most 64 custom arguments."));
    }
    if arguments.iter().any(|argument| argument.contains('\0')) {
        return Err(Error::Arguments(
            "Custom arguments cannot contain NUL characters.",
        ));
    }
    if arguments.iter().any(|argument| argument.len() > MAX_TEXT) {
        return Err(Error::Arguments(
            "Each custom argument must be at most 4096 bytes.",
        ));
    }
    // Without the placeholder a client could open whatever it last showed.
    if !arguments
        .iter()
        .any(|argument| argument.contains(PLACEHOLDER))
    {
        return Err(Error::Arguments(
            "Custom arguments must include {repo}, which is replaced by the repository or worktree path.",
        ));
    }
    Ok(arguments.iter().cloned().map(Argument::Template).collect())
}

/// An absolute executable, or a bare command name found in `environment.path`.
/// Relative paths are rejected so the current directory is never searched.
fn executable(
    configured: &str,
    bundle_executable: Option<&str>,
    environment: &Environment,
) -> Result<PathBuf, Error> {
    let invalid = |problem| Error::Executable {
        path: configured.to_owned(),
        problem,
    };
    if configured.len() > MAX_TEXT || configured.contains(['\0', '\n', '\r']) {
        return Err(invalid("is not a valid path."));
    }
    let path = Path::new(configured);
    if !path.is_absolute() {
        if path.components().count() != 1 {
            return Err(invalid(
                "must be an absolute path or a command name without directories.",
            ));
        }
        return crate::agents::discovery::search(configured, "", environment.path.clone())
            .into_iter()
            .next()
            .ok_or_else(|| invalid("was not found in the searched command directories."));
    }
    // A macOS bundle may be chosen instead of the launcher inside it.
    let path = match bundle_executable {
        Some(inner) if is_bundle(path) => path.join(inner),
        _ => path.to_owned(),
    };
    check_executable(&path).map_err(invalid)?;
    Ok(path)
}

pub(crate) fn check_executable(path: &Path) -> Result<(), &'static str> {
    let metadata = path.metadata().map_err(|_| "does not exist.")?;
    if !metadata.is_file() {
        return Err("is not a file.");
    }
    if cfg!(windows)
        && !path
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("exe"))
    {
        // Rust would run .bat/.cmd through cmd.exe; never start shell wrappers.
        return Err(
            "is not an .exe file. Scripts and shortcuts such as .cmd, .bat, .ps1 or .lnk are not started.",
        );
    }
    if !crate::agents::discovery::is_executable(path) {
        return Err("is not executable.");
    }
    Ok(())
}

fn is_bundle(path: &Path) -> bool {
    path.extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("app"))
        && path.is_dir()
}

fn check_directory(path: &Path) -> Result<(), Error> {
    let invalid = |problem| Error::Directory {
        path: path.to_owned(),
        problem,
    };
    if !path.is_absolute() {
        return Err(invalid("the path is not absolute."));
    }
    match path.metadata() {
        Ok(metadata) if metadata.is_dir() => Ok(()),
        Ok(_) => Err(invalid("the path is not a directory.")),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Err(invalid("the directory no longer exists."))
        }
        Err(_) => Err(invalid("the directory cannot be accessed.")),
    }
}
