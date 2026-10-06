//! Filesystem-only CLI discovery. Candidates are not executed or trusted here;
//! the selected backend's normal compatibility check remains authoritative.
use crate::domain::AgentId;
use std::{
    collections::HashSet,
    env,
    path::{Path, PathBuf},
};

/// Run on a blocking worker, never while rendering the UI.
pub fn find(agent: AgentId, configured: &str) -> Vec<PathBuf> {
    search(default_name(agent), configured, directories(agent))
}

/// Resolve the requested launcher name in the same GUI-safe locations as Find
/// CLI. A custom name is never replaced with the backend's default launcher.
pub fn resolve(agent: AgentId, name: &str) -> Option<PathBuf> {
    let path = Path::new(name);
    if path.is_absolute()
        || path.components().count() != 1
        || name.trim().is_empty()
        || name.len() > 4096
        || name.contains(['\0', '\n', '\r'])
    {
        return None;
    }
    search(name, "", directories(agent)).into_iter().next()
}

pub fn default_name(agent: AgentId) -> &'static str {
    match agent {
        AgentId::Codex => "codex",
        AgentId::Copilot => "copilot",
        AgentId::OpenCode => "opencode",
        AgentId::Claude => "claude",
    }
}

fn directories(agent: AgentId) -> Vec<PathBuf> {
    let mut directories: Vec<_> = env::var_os("PATH")
        .map(|path| env::split_paths(&path).collect())
        .unwrap_or_default();
    // These are directory overrides, not shell commands or profile files.
    for key in ["XDG_BIN_DIR", "NVM_BIN", "PNPM_HOME"] {
        if let Some(path) = env::var_os(key) {
            directories.push(path.into());
        }
    }
    for key in ["HOMEBREW_PREFIX", "VOLTA_HOME"] {
        if let Some(path) = env::var_os(key) {
            directories.push(PathBuf::from(path).join("bin"));
        }
    }
    if let Some(home) = directories::BaseDirs::new().map(|dirs| dirs.home_dir().to_owned()) {
        for directory in [
            ".local/bin",
            "bin",
            ".npm-global/bin",
            ".bun/bin",
            ".volta/bin",
        ] {
            directories.push(home.join(directory));
        }
        if agent == AgentId::OpenCode {
            directories.push(home.join(".opencode/bin"));
        }
        #[cfg(windows)]
        directories.push(home.join("scoop/shims"));
    }
    #[cfg(unix)]
    directories.extend(
        [
            "/opt/homebrew/bin",
            "/usr/local/bin",
            "/home/linuxbrew/.linuxbrew/bin",
            "/usr/bin",
            "/bin",
        ]
        .map(PathBuf::from),
    );
    #[cfg(windows)]
    if let Some(local) = env::var_os("LOCALAPPDATA") {
        directories.push(PathBuf::from(local).join("Microsoft/WinGet/Links"));
    }
    directories
}

fn search(name: &str, configured: &str, directories: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    let configured = Path::new(configured);
    if configured.is_absolute() {
        candidates.push(configured.to_owned());
    }
    let mut seen_directories = HashSet::new();
    for directory in directories {
        // Never implicitly discover executables in the current repository via
        // empty or relative PATH entries. Explicit absolute paths still work.
        if !directory.is_absolute() || !seen_directories.insert(directory.clone()) {
            continue;
        }
        #[cfg(windows)]
        candidates.push(
            directory.join(
                if Path::new(name)
                    .extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("exe"))
                {
                    name.to_owned()
                } else {
                    format!("{name}.exe")
                },
            ),
        );
        #[cfg(not(windows))]
        candidates.push(directory.join(name));
    }
    let mut seen_files = HashSet::new();
    candidates.retain(|path| {
        if !path
            .to_str()
            .is_some_and(|text| text.len() <= 4096 && !text.contains(['\0', '\n', '\r']))
            || !is_executable(path)
        {
            return false;
        }
        path.canonicalize()
            .is_ok_and(|canonical| seen_files.insert(canonical))
    });
    // Keep the stable launcher/symlink spelling, not a versioned install target.
    candidates
}

fn is_executable(path: &Path) -> bool {
    let Ok(metadata) = path.metadata() else {
        return false;
    };
    if !metadata.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        metadata.permissions().mode() & 0o111 != 0
    }
    #[cfg(windows)]
    {
        // Do not introduce cmd.exe / PowerShell execution to discover npm shims.
        path.extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("exe"))
    }
    #[cfg(not(any(unix, windows)))]
    false
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    fn executable(directory: &Path, name: &str) -> PathBuf {
        std::fs::create_dir_all(directory).unwrap();
        let path = directory.join(if cfg!(windows) {
            format!("{name}.exe")
        } else {
            name.into()
        });
        // Discovery must never run this file (it is deliberately not a program).
        std::fs::write(&path, "not executable code").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        path
    }

    #[test]
    fn finds_all_four_agents_in_search_order_without_running_them() {
        let temp = tempfile::tempdir().unwrap();
        let path_dir = temp.path().join("path with spaces ü");
        let known_dir = temp.path().join("known install");
        for name in ["codex", "copilot", "opencode", "claude"] {
            let first = executable(&path_dir, name);
            let second = executable(&known_dir, name);
            assert_eq!(
                search(
                    name,
                    name,
                    vec![path_dir.clone(), known_dir.clone(), path_dir.clone()]
                ),
                [first, second.clone()]
            );
            assert_eq!(search(name, name, vec![known_dir.clone()]), [second]);
        }
        assert!(search("missing", "", vec![path_dir, known_dir]).is_empty());
    }

    #[test]
    fn includes_custom_absolute_path_and_ignores_missing_and_relative_entries() {
        let temp = tempfile::tempdir().unwrap();
        let custom = executable(temp.path(), "custom launcher");
        assert_eq!(
            search(
                "codex",
                custom.to_str().unwrap(),
                vec!["".into(), ".".into(), "relative/bin".into()]
            ),
            [custom]
        );
        assert!(
            search(
                "codex",
                temp.path().join("missing").to_str().unwrap(),
                vec![]
            )
            .is_empty()
        );
        assert!(search("codex", "./codex", vec![]).is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn rejects_non_executables_broken_links_and_unrepresentable_paths_and_deduplicates_symlinks() {
        use std::os::unix::{
            ffi::OsStringExt,
            fs::{PermissionsExt, symlink},
        };
        let temp = tempfile::tempdir().unwrap();
        let original = executable(&temp.path().join("original"), "codex");
        let alias_dir = temp.path().join("stable");
        std::fs::create_dir(&alias_dir).unwrap();
        let alias = alias_dir.join("codex");
        symlink(&original, &alias).unwrap();
        assert_eq!(
            search(
                "codex",
                "",
                vec![alias_dir.clone(), original.parent().unwrap().into()]
            ),
            [alias]
        );
        std::fs::set_permissions(&original, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert!(search("codex", "", vec![alias_dir.clone()]).is_empty());
        std::fs::remove_file(&original).unwrap();
        assert!(search("codex", "", vec![alias_dir]).is_empty());
        let invalid = temp
            .path()
            .join(std::ffi::OsString::from_vec(b"invalid-\xff".to_vec()));
        // Linux permits byte names that APFS cannot represent.
        #[cfg(target_os = "linux")]
        executable(&invalid, "codex");
        assert!(search("codex", "", vec![invalid]).is_empty());
        std::fs::create_dir(temp.path().join("codex")).unwrap();
        assert!(search("codex", "", vec![temp.path().into()]).is_empty());
    }

    #[cfg(windows)]
    #[test]
    fn does_not_offer_shell_wrappers() {
        let temp = tempfile::tempdir().unwrap();
        for extension in ["cmd", "bat", "ps1"] {
            let wrapper = temp.path().join(format!("claude.{extension}"));
            std::fs::write(&wrapper, "shell wrapper").unwrap();
            assert!(
                search(
                    "claude",
                    wrapper.to_str().unwrap(),
                    vec![temp.path().into()]
                )
                .is_empty()
            );
        }
    }
}
