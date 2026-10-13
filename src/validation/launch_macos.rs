//! Finder/Dock launches need toolchain locations absent from the GUI PATH.
//! Configure only this validation child; never load profiles or change global PATH.
use crate::process::CommandSpec;
use anyhow::{Context, Result};
use std::{
    collections::HashSet,
    env,
    ffi::OsStr,
    path::{Path, PathBuf},
};

pub(super) fn configure(spec: &mut CommandSpec) -> Result<()> {
    let mut additional = Vec::new();
    // Absolute custom launchers can use /usr/bin/env to find sibling runtimes.
    let program = Path::new(&spec.program);
    if program.is_absolute()
        && let Some(parent) = program.parent()
    {
        additional.push(parent.to_owned());
    }
    for key in ["XDG_BIN_DIR", "NVM_BIN", "PNPM_HOME"] {
        if let Some(path) = env::var_os(key) {
            additional.push(PathBuf::from(path));
        }
    }
    for key in ["HOMEBREW_PREFIX", "VOLTA_HOME"] {
        if let Some(path) = env::var_os(key) {
            additional.push(PathBuf::from(path).join("bin"));
        }
    }
    if let Some(home) = directories::BaseDirs::new().map(|dirs| dirs.home_dir().to_owned()) {
        for directory in [
            ".local/bin",
            "bin",
            ".cargo/bin",
            ".npm-global/bin",
            ".volta/bin",
        ] {
            additional.push(home.join(directory));
        }
    }
    additional.extend(
        [
            "/opt/homebrew/bin",
            "/usr/local/bin",
            "/opt/local/bin",
            "/usr/bin",
            "/bin",
            "/usr/sbin",
            "/sbin",
        ]
        .map(PathBuf::from),
    );
    let inherited = env::var_os("PATH");
    spec.env.push((
        "PATH".into(),
        search_path(inherited.as_deref(), additional)?,
    ));
    Ok(())
}

fn search_path(inherited: Option<&OsStr>, additional: Vec<PathBuf>) -> Result<String> {
    let mut paths: Vec<_> = inherited
        .map(env::split_paths)
        .into_iter()
        .flatten()
        .collect();
    paths.extend(additional);
    let mut seen = HashSet::new();
    // Never implicitly execute files from the repository via empty/relative entries.
    paths.retain(|path| path.is_absolute() && seen.insert(path.clone()));
    env::join_paths(paths)
        .context("Cannot configure validation PATH. Use toolchain directories without ':' in their names.")?
        .into_string()
        .map_err(|_| anyhow::anyhow!("Cannot configure validation PATH: a toolchain directory is not valid Unicode."))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserves_inherited_precedence_and_appends_deduplicated_fallbacks() {
        let result = search_path(
            Some(OsStr::new("/custom/bin:/usr/bin:/custom/bin")),
            vec![
                "/opt/homebrew/bin".into(),
                "/usr/bin".into(),
                "/usr/local/bin".into(),
            ],
        )
        .expect("valid search path");
        assert_eq!(
            result,
            "/custom/bin:/usr/bin:/opt/homebrew/bin:/usr/local/bin"
        );
    }

    #[test]
    fn missing_path_and_relative_entries_never_search_the_repository() {
        for inherited in [None, Some(OsStr::new(":.:relative/bin::"))] {
            assert_eq!(
                search_path(
                    inherited,
                    vec!["relative/install".into(), "/opt/homebrew/bin".into()]
                )
                .expect("valid search path"),
                "/opt/homebrew/bin"
            );
        }
    }

    #[test]
    fn rejects_unrepresentable_directories_instead_of_searching_different_paths() {
        assert!(search_path(None, vec!["/toolchain:other/bin".into()]).is_err());
    }
}
