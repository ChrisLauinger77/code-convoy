#![allow(clippy::unwrap_used)]
use super::presets::{Method, method};
use super::*;
use std::fs;

fn executable(directory: &Path, name: &str) -> PathBuf {
    fs::create_dir_all(directory).unwrap();
    let path = directory.join(if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.into()
    });
    // Resolution must never run this file; it is deliberately not a program.
    fs::write(&path, "not executable code").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    }
    path
}

fn environment(root: &Path) -> Environment {
    Environment {
        path: vec![root.join("bin ü")],
        applications: vec![root.join("Applications"), root.join("home/Applications")],
        local_app_data: Some(root.join("Local AppData")),
        program_files: Some(root.join("Program Files")),
    }
}

fn configured(client: Client, executable: &str) -> Preferences {
    Preferences {
        executable: executable.into(),
        ..Preferences::select(Some(client))
    }
}

fn custom(executable: &Path, arguments: &[&str]) -> Preferences {
    Preferences {
        client: Some(Client::Custom),
        executable: executable.display().to_string(),
        arguments: arguments.iter().map(|a| (*a).to_owned()).collect(),
    }
}

/// A preset whose fallback locations on this host are all below `environment`,
/// so a missing launcher is deterministic regardless of installed software.
fn host_preset() -> (Client, &'static str) {
    match Platform::current() {
        Some(Platform::MacOs) => (Client::SublimeMerge, "smerge"),
        Some(Platform::Windows) => (Client::GitKraken, "gitkraken"),
        _ => (Client::AngKorGit, "angkorgit"),
    }
}

#[test]
fn verified_platform_matrix_is_explicit_and_custom_is_always_available() {
    use Platform::*;
    let expected = [
        (Client::GitKraken, [true, false, true]),
        (Client::SourceGit, [true, true, true]),
        (Client::AngKorGit, [true, true, true]),
        (Client::SublimeMerge, [true, true, true]),
        (Client::Fork, [false, true, true]),
        (Client::Custom, [true, true, true]),
    ];
    for (client, support) in expected {
        for (platform, supported) in [Linux, MacOs, Windows].into_iter().zip(support) {
            let availability = client.availability(Some(platform));
            assert_eq!(
                availability.is_ok(),
                supported,
                "{client:?} on {platform:?}"
            );
            if let Err(reason) = availability {
                assert!(!reason.is_empty());
            }
        }
        assert_eq!(client.availability(None).is_ok(), client == Client::Custom);
    }
    assert_eq!(Client::ALL.len(), expected.len());
}

#[test]
fn preset_arguments_pass_the_path_as_one_separate_argument() {
    use Platform::*;
    for client in Client::ALL {
        for platform in [Linux, MacOs, Windows] {
            match method(client, platform) {
                Ok(Method::Executable { arguments, .. }) => {
                    assert_eq!(
                        arguments
                            .iter()
                            .filter(|argument| argument.contains(PLACEHOLDER))
                            .collect::<Vec<_>>(),
                        [&PLACEHOLDER],
                        "{client:?} on {platform:?}"
                    );
                    let expected: &[&str] = if client == Client::AngKorGit {
                        &["open", PLACEHOLDER]
                    } else {
                        &[PLACEHOLDER]
                    };
                    assert_eq!(*arguments, expected, "{client:?} on {platform:?}");
                }
                Ok(Method::Application { bundle }) => {
                    assert_eq!(platform, MacOs);
                    assert!(bundle.ends_with(".app"));
                }
                Err(_) => {}
            }
        }
    }
    assert!(Client::Fork.opens_application(Some(MacOs)));
    assert!(!Client::Fork.opens_application(Some(Windows)));
    assert!(!Client::SublimeMerge.opens_application(Some(MacOs)));
}

#[test]
fn macos_application_presets_use_launch_services_with_the_located_bundle() {
    let temp = tempfile::tempdir().unwrap();
    let environment = environment(temp.path());
    let repository = Path::new("/Users/me/Repo with spaces ü/{repo}");
    for (client, bundle) in [
        (Client::SourceGit, "SourceGit.app"),
        (Client::AngKorGit, "AngKorGit.app"),
        (Client::Fork, "Fork.app"),
    ] {
        let preferences = Preferences::select(Some(client));
        assert_eq!(
            resolve_in(&preferences, Some(Platform::MacOs), &environment),
            Err(Error::NotFound {
                client,
                application: true
            })
        );
        // The per-user folder is searched after /Applications.
        let installed = environment.applications[1].join(bundle);
        fs::create_dir_all(&installed).unwrap();
        let command = resolve_in(&preferences, Some(Platform::MacOs), &environment).unwrap();
        assert_eq!(command.program, Path::new("/usr/bin/open"));
        assert_eq!(
            command.arguments_for(repository),
            [
                OsString::from("-a"),
                installed.clone().into_os_string(),
                repository.as_os_str().to_owned()
            ]
        );
        let shared = environment.applications[0].join(bundle);
        fs::create_dir_all(&shared).unwrap();
        let command = resolve_in(&preferences, Some(Platform::MacOs), &environment).unwrap();
        assert_eq!(command.arguments_for(repository)[1], shared.as_os_str());
    }
}

#[test]
fn configured_application_must_be_an_existing_absolute_bundle() {
    let temp = tempfile::tempdir().unwrap();
    let environment = environment(temp.path());
    let bundle = temp.path().join("Elsewhere {repo}/Fork.app");
    fs::create_dir_all(&bundle).unwrap();
    let file = temp.path().join("Fork");
    fs::write(&file, "").unwrap();
    let resolve = |path: &str| {
        resolve_in(
            &configured(Client::Fork, path),
            Some(Platform::MacOs),
            &environment,
        )
    };
    // A configured bundle path is literal, even when it contains "{repo}".
    let command = resolve(bundle.to_str().unwrap()).unwrap();
    assert_eq!(
        command.arguments_for(Path::new("/r"))[1],
        bundle.as_os_str()
    );
    for (path, problem) in [
        (
            "Fork.app",
            "must be an absolute path to an application bundle.",
        ),
        (
            temp.path().join("Missing.app").to_str().unwrap(),
            "does not exist.",
        ),
        (
            file.to_str().unwrap(),
            "is not an application bundle (.app).",
        ),
    ] {
        assert_eq!(
            resolve(path),
            Err(Error::Application {
                path: path.into(),
                problem
            })
        );
    }
}

#[test]
fn presets_discover_launchers_in_search_order_and_configured_paths_win() {
    let temp = tempfile::tempdir().unwrap();
    let environment = environment(temp.path());
    let (client, name) = host_preset();
    let preferences = Preferences::select(Some(client));
    let platform = Platform::current();
    assert_eq!(
        resolve_in(&preferences, platform, &environment),
        Err(Error::NotFound {
            client,
            application: false
        })
    );
    let found = executable(&environment.path[0], name);
    let command = resolve_in(&preferences, platform, &environment).unwrap();
    assert_eq!(command.program, found);
    assert_eq!(command.client, client);
    let blank = resolve_in(&configured(client, " \t"), platform, &environment).unwrap();
    assert_eq!(blank.program, found, "blank means automatic detection");
    let chosen = executable(&temp.path().join("chosen dir"), "launcher ü ");
    let command = resolve_in(
        &configured(client, chosen.to_str().unwrap()),
        platform,
        &environment,
    )
    .unwrap();
    assert_eq!(command.program, chosen, "configured paths are used exactly");
}

#[test]
fn preset_fallback_locations_are_found_without_path() {
    let temp = tempfile::tempdir().unwrap();
    let environment = environment(temp.path());
    let (client, location) = match Platform::current() {
        Some(Platform::MacOs) => (
            Client::SublimeMerge,
            environment.applications[1].join("Sublime Merge.app/Contents/SharedSupport/bin"),
        ),
        Some(Platform::Windows) => (
            Client::Fork,
            environment.local_app_data.clone().unwrap().join("Fork"),
        ),
        // Linux fallbacks are fixed system paths; host-specific, not asserted here.
        _ => return,
    };
    let name = if client == Client::Fork {
        "Fork"
    } else {
        "smerge"
    };
    let expected = executable(&location, name);
    let command = resolve_in(
        &Preferences::select(Some(client)),
        Platform::current(),
        &environment,
    )
    .unwrap();
    assert_eq!(command.program, expected);
}

#[cfg(target_os = "macos")]
#[test]
fn a_chosen_sublime_merge_bundle_uses_its_documented_launcher() {
    let temp = tempfile::tempdir().unwrap();
    let bundle = temp.path().join("Sublime Merge.app");
    let launcher = executable(&bundle.join("Contents/SharedSupport/bin"), "smerge");
    let command = resolve_in(
        &configured(Client::SublimeMerge, bundle.to_str().unwrap()),
        Some(Platform::MacOs),
        &environment(temp.path()),
    )
    .unwrap();
    assert_eq!(command.program, launcher);
}

#[test]
fn missing_and_invalid_executables_are_reported_without_falling_back() {
    let temp = tempfile::tempdir().unwrap();
    let environment = environment(temp.path());
    let (client, name) = host_preset();
    // A discoverable launcher must not hide an invalid configured path.
    executable(&environment.path[0], name);
    let directory = temp.path().join("a directory");
    fs::create_dir(&directory).unwrap();
    let missing = temp.path().join("missing launcher");
    let cases = [
        (missing.display().to_string(), "does not exist."),
        (directory.display().to_string(), "is not a file."),
        (
            "relative/launcher".into(),
            "must be an absolute path or a command name without directories.",
        ),
        (
            "no-such-command".into(),
            "was not found in the searched command directories.",
        ),
        ("bad\nname".into(), "is not a valid path."),
    ];
    for (path, problem) in cases {
        assert_eq!(
            resolve_in(
                &configured(client, &path),
                Platform::current(),
                &environment
            ),
            Err(Error::Executable {
                path: path.clone(),
                problem
            }),
            "{path}"
        );
    }
}

#[cfg(unix)]
#[test]
fn non_executable_files_are_rejected() {
    use std::os::unix::fs::PermissionsExt;
    let temp = tempfile::tempdir().unwrap();
    let path = executable(temp.path(), "plain");
    fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
    assert_eq!(check_executable(&path), Err("is not executable."));
}

#[cfg(windows)]
#[test]
fn windows_never_starts_shell_wrappers_or_shortcuts() {
    let temp = tempfile::tempdir().unwrap();
    for extension in ["cmd", "bat", "ps1", "lnk", "com"] {
        let path = temp.path().join(format!("smerge.{extension}"));
        fs::write(&path, "wrapper").unwrap();
        assert!(
            check_executable(&path)
                .unwrap_err()
                .starts_with("is not an .exe file.")
        );
    }
}

#[test]
fn custom_commands_require_an_executable_and_the_placeholder() {
    let temp = tempfile::tempdir().unwrap();
    let environment = environment(temp.path());
    let launcher = executable(&environment.path[0], "my-client");
    let command = resolve_in(
        &custom(&launcher, &["--new-window", "--path={repo}"]),
        None,
        &environment,
    )
    .unwrap();
    assert_eq!(command.client, Client::Custom);
    assert_eq!(command.program, launcher);
    assert_eq!(
        command.arguments_for(Path::new("/r p")),
        [
            OsString::from("--new-window"),
            OsString::from("--path=/r p")
        ]
    );
    // A bare command name is searched like a preset launcher, never in ".".
    let by_name = resolve_in(
        &Preferences {
            executable: "my-client".into(),
            ..custom(&launcher, &["{repo}"])
        },
        Some(Platform::Linux),
        &environment,
    )
    .unwrap();
    assert_eq!(by_name.program, launcher);
    let blank = resolve_in(
        &Preferences {
            executable: " ".into(),
            ..custom(&launcher, &["{repo}"])
        },
        None,
        &environment,
    )
    .unwrap_err();
    assert_eq!(
        blank.to_string(),
        "A Git client executable is required for Custom. Enter an absolute path or a command name."
    );
    let too_long = "x".repeat(4097) + PLACEHOLDER;
    let many: Vec<&str> = std::iter::repeat_n(PLACEHOLDER, 65).collect();
    for (arguments, problem) in [
        (vec![], "must include {repo}"),
        (vec!["--path", "repo"], "must include {repo}"),
        (vec!["{REPO}"], "must include {repo}"),
        (vec!["{repo}\0"], "cannot contain NUL"),
        (vec![too_long.as_str()], "at most 4096 bytes"),
        (many, "at most 64"),
    ] {
        let error = resolve_in(&custom(&launcher, &arguments), None, &environment).unwrap_err();
        assert!(
            matches!(&error, Error::Arguments(text) if text.contains(problem)),
            "{error}"
        );
    }
}

#[test]
fn not_configured_and_unsupported_platforms_explain_the_next_step() {
    let environment = Environment::default();
    let error = resolve_in(&Preferences::default(), Platform::current(), &environment).unwrap_err();
    assert_eq!(error, Error::NotConfigured);
    assert!(error.to_string().contains("Settings › Git client"));
    let fork = resolve_in(
        &Preferences::select(Some(Client::Fork)),
        Some(Platform::Linux),
        &environment,
    )
    .unwrap_err();
    assert!(matches!(
        fork,
        Error::Unavailable {
            platform: Some(Platform::Linux),
            ..
        }
    ));
    assert!(fork.to_string().contains("Fork is not available on Linux."));
    assert!(fork.to_string().contains("Choose Custom"));
    let kraken = resolve_in(
        &Preferences::select(Some(Client::GitKraken)),
        Some(Platform::MacOs),
        &environment,
    )
    .unwrap_err();
    assert!(
        kraken
            .to_string()
            .starts_with("GitKraken is not available on macOS.")
    );
    let other = resolve_in(
        &Preferences::select(Some(Client::SourceGit)),
        None,
        &environment,
    )
    .unwrap_err();
    assert!(other.to_string().contains("this operating system"));
}

#[test]
fn substitution_keeps_argument_boundaries_and_runs_once() {
    let command = Command {
        client: Client::Custom,
        program: "client".into(),
        arguments: vec![
            Argument::Template("{repo}".into()),
            Argument::Template("--path={repo}".into()),
            Argument::Template("{repo}|{repo}".into()),
            Argument::Template("plain".into()),
            Argument::Literal("{repo}".into()),
        ],
    };
    for repository in [
        "/home/me/Repo with spaces/日本語 ü",
        "/tmp/a; rm -rf ~ $(touch x) `id` \"q\" 'p' & | > *",
        // A directory may itself be called {repo}; substitution is single-pass.
        "/srv/{repo}/nested",
        "C:\\Users\\Me\\Repo with spaces\\ü",
    ] {
        let path = Path::new(repository);
        assert_eq!(
            command.arguments_for(path),
            [
                OsString::from(repository),
                OsString::from(format!("--path={repository}")),
                OsString::from(format!("{repository}|{repository}")),
                OsString::from("plain"),
                OsString::from("{repo}"),
            ]
        );
    }
}

#[cfg(unix)]
#[test]
fn substitution_preserves_non_unicode_paths_exactly() {
    use std::os::unix::ffi::{OsStrExt, OsStringExt};
    let path = PathBuf::from(OsString::from_vec(b"/repo-\xff-x".to_vec()));
    let argument = substitute("--path={repo}!", path.as_os_str());
    assert_eq!(argument.as_bytes(), b"--path=/repo-\xff-x!");
}

#[test]
fn configuration_round_trips_and_old_or_newer_saves_still_load() {
    // Old saves have no git_client and must stay unconfigured.
    let state: crate::domain::AppState = serde_json::from_str("{}").unwrap();
    assert!(state.git_client.is_default());
    let saved = serde_json::to_value(&state).unwrap();
    assert!(saved.get("git_client").is_none(), "{saved}");

    for (client, key) in [
        (Client::GitKraken, "gitkraken"),
        (Client::SourceGit, "sourcegit"),
        (Client::AngKorGit, "angkorgit"),
        (Client::SublimeMerge, "sublime_merge"),
        (Client::Fork, "fork"),
        (Client::Custom, "custom"),
    ] {
        let preferences = Preferences {
            client: Some(client),
            executable: "/opt/My Client/bin ü".into(),
            arguments: vec!["--path={repo}".into(), "two words".into()],
        };
        let value = serde_json::to_value(&preferences).unwrap();
        assert_eq!(value["client"], key);
        let loaded: Preferences = serde_json::from_value(value).unwrap();
        assert_eq!(loaded, preferences);
    }
    let selected = serde_json::to_value(Preferences::select(Some(Client::Fork))).unwrap();
    assert_eq!(selected, serde_json::json!({"client": "fork"}));

    // A client known only to a newer release loads as not configured instead
    // of making the whole state unreadable.
    let state: crate::domain::AppState = serde_json::from_value(serde_json::json!({
        "git_client": {"client": "future_client", "executable": "/x"}
    }))
    .unwrap();
    assert_eq!(state.git_client.client, None);
    assert!(
        serde_json::from_value::<Preferences>(serde_json::json!({"client": 7})).is_err(),
        "malformed values are still rejected"
    );
}

#[test]
fn selecting_a_client_starts_from_a_clean_configuration() {
    assert_eq!(Preferences::select(None), Preferences::default());
    assert_eq!(
        Preferences::select(Some(Client::Custom)).arguments,
        [PLACEHOLDER]
    );
    for client in Client::ALL.into_iter().filter(|c| *c != Client::Custom) {
        let preferences = Preferences::select(Some(client));
        assert!(preferences.executable.is_empty() && preferences.arguments.is_empty());
    }
}

#[test]
fn launch_environment_drops_git_redirection_and_keeps_the_desktop_session() {
    let variables = [
        "GIT_DIR",
        "GIT_WORK_TREE",
        "GIT_INDEX_FILE",
        "PATH",
        "HOME",
        "DISPLAY",
        "WAYLAND_DISPLAY",
        "XDG_RUNTIME_DIR",
        "DBUS_SESSION_BUS_ADDRESS",
        "XAUTHORITY",
        "APPIMAGE",
        "APPDIR",
        "ARGV0",
        "OWD",
    ]
    .map(|key| (OsString::from(key), OsString::from("value")));
    let removed = |appimage| {
        launch::removed_environment(variables.clone().into_iter(), appimage)
            .into_iter()
            .map(|key| key.into_string().unwrap())
            .collect::<Vec<_>>()
    };
    assert_eq!(
        removed(false),
        ["GIT_DIR", "GIT_WORK_TREE", "GIT_INDEX_FILE"]
    );
    assert_eq!(
        removed(true),
        [
            "GIT_DIR",
            "GIT_WORK_TREE",
            "GIT_INDEX_FILE",
            "APPIMAGE",
            "APPDIR",
            "ARGV0",
            "OWD"
        ]
    );
}

#[test]
fn only_existing_absolute_directories_can_be_opened() {
    let temp = tempfile::tempdir().unwrap();
    let directory = temp.path().join("Repo with spaces ü");
    fs::create_dir(&directory).unwrap();
    let file = temp.path().join("file");
    fs::write(&file, "").unwrap();
    assert_eq!(check_directory(&directory), Ok(()));
    for (path, problem) in [
        (PathBuf::from("relative/repo"), "the path is not absolute."),
        (
            temp.path().join("deleted"),
            "the directory no longer exists.",
        ),
        (file, "the path is not a directory."),
    ] {
        assert_eq!(
            check_directory(&path),
            Err(Error::Directory {
                path: path.clone(),
                problem
            })
        );
    }
    #[cfg(unix)]
    {
        let link = temp.path().join("link");
        std::os::unix::fs::symlink(&directory, &link).unwrap();
        assert_eq!(check_directory(&link), Ok(()));
        fs::remove_dir(&directory).unwrap();
        assert_eq!(
            check_directory(&link),
            Err(Error::Directory {
                path: link.clone(),
                problem: "the directory no longer exists."
            })
        );
    }
}
