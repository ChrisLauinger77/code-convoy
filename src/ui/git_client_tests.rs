//! Git client settings without a window, native dialog or Git client process.
use super::*;
use crate::git_client::{Client, PLACEHOLDER, Preferences};

fn settle(app: &mut App) -> Result<String, String> {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(status) = &app.git_client.status {
            return status.clone();
        }
        assert!(Instant::now() < deadline, "Git client check did not finish");
        app.runtime().block_on(async {
            tokio::time::sleep(Duration::from_millis(5)).await;
        });
        app.poll();
    }
}

fn launcher(directory: &std::path::Path) -> PathBuf {
    std::fs::create_dir_all(directory).unwrap();
    let path = directory.join(if cfg!(windows) {
        "client ü.exe"
    } else {
        "client ü"
    });
    // Checks only inspect metadata; this file is never started.
    std::fs::write(&path, "not executable code").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    path
}

fn settings(app: &mut App, ui: &mut egui::Ui, _: &egui::Context) {
    app.git_client_settings(ui);
}

#[test]
fn choosing_a_client_starts_clean_and_unconfigured_state_is_not_saved() {
    let (_temp, mut app) = app();
    app.state.git_client = Preferences {
        client: Some(Client::Custom),
        executable: "/opt/custom".into(),
        arguments: vec!["--path={repo}".into()],
    };
    app.dirty = false;
    app.select_git_client(Some(Client::Custom));
    assert!(
        !app.dirty,
        "choosing the current client keeps its configuration"
    );
    assert_eq!(app.state.git_client.executable, "/opt/custom");

    app.select_git_client(Some(Client::SublimeMerge));
    assert!(app.dirty);
    assert_eq!(
        app.state.git_client,
        Preferences::select(Some(Client::SublimeMerge))
    );
    app.select_git_client(Some(Client::Custom));
    assert_eq!(app.state.git_client.arguments, [PLACEHOLDER]);
    assert_eq!(app.git_client.arguments, PLACEHOLDER);

    app.select_git_client(None);
    assert!(app.state.git_client.is_default());
    let saved = serde_json::to_value(app.state.persistence_snapshot()).unwrap();
    assert!(saved.get("git_client").is_none());
}

#[test]
fn custom_fields_are_keyboard_reachable_and_arguments_stay_structured() {
    let (_temp, mut app) = app();
    app.select_git_client(Some(Client::Custom));
    let mut keys = Keyboard::new(settings);
    keys.frame(&mut app, vec![]);
    keys.focus(&mut app, "Git client");
    keys.replace_text(&mut app, "Executable", "/opt/My Client ü/bin/client");
    keys.replace_text(
        &mut app,
        "Arguments, one per line",
        "--path={repo}\r\n\nliteral with spaces ü\n{repo}",
    );
    keys.frame(&mut app, vec![]);
    assert_eq!(
        app.state.git_client.executable,
        "/opt/My Client ü/bin/client"
    );
    assert_eq!(
        app.state.git_client.arguments,
        ["--path={repo}", "literal with spaces ü", "{repo}"]
    );
    assert!(app.dirty);
    let saved = serde_json::to_value(app.state.persistence_snapshot()).unwrap();
    assert_eq!(
        saved["git_client"],
        serde_json::json!({
            "client": "custom",
            "executable": "/opt/My Client ü/bin/client",
            "arguments": ["--path={repo}", "literal with spaces ü", "{repo}"],
        })
    );
}

#[test]
fn checks_run_off_the_ui_thread_once_per_change_and_ignore_stale_results() {
    let (temp, mut app) = app();
    let mut keys = Keyboard::new(settings);
    keys.frame(&mut app, vec![]);
    assert!(app.git_client.checked.is_none(), "nothing to check yet");

    app.select_git_client(Some(Client::Custom));
    let missing = temp.path().join("missing client");
    app.state.git_client.executable = missing.display().to_string();
    keys.frame(&mut app, vec![]);
    let generation = app.git_client.generation;
    keys.frame(&mut app, vec![]);
    assert_eq!(app.git_client.generation, generation, "no check per frame");
    let error = settle(&mut app).unwrap_err();
    assert!(error.contains("does not exist."), "{error}");
    assert!(error.contains(&missing.display().to_string()), "{error}");

    let found = launcher(&temp.path().join("Client tools"));
    app.state.git_client.executable = found.display().to_string();
    keys.frame(&mut app, vec![]);
    assert_eq!(app.git_client.generation, generation + 1);
    app.git_client_checked(generation, Err("stale".into()));
    assert!(app.git_client.status.is_none(), "stale result ignored");
    let ready = settle(&mut app).unwrap();
    assert!(ready.contains(&found.display().to_string()) && ready.contains(PLACEHOLDER));
}

#[test]
fn browsed_executables_are_kept_exactly_and_cancellation_changes_nothing() {
    let (_temp, mut app) = app();
    app.select_git_client(Some(Client::SublimeMerge));
    app.dirty = false;
    app.git_client.browsing = true;
    app.git_client_executable_selected(None);
    assert!(!app.git_client.browsing && !app.dirty);
    assert!(app.state.git_client.executable.is_empty());

    let chosen = PathBuf::from("/Applications/Sublime Merge ü.app ");
    app.git_client_executable_selected(Some(chosen.clone()));
    assert_eq!(
        app.state.git_client.executable,
        chosen.to_str().unwrap(),
        "picked paths keep surrounding whitespace"
    );
    assert!(app.dirty);

    app.select_git_client(None);
    app.git_client_executable_selected(Some(chosen));
    assert!(
        app.state.git_client.is_default(),
        "no client, nothing stored"
    );

    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStringExt;
        app.select_git_client(Some(Client::SublimeMerge));
        let invalid = PathBuf::from(std::ffi::OsString::from_vec(b"/opt/\xff".to_vec()));
        app.git_client_executable_selected(Some(invalid));
        assert!(app.state.git_client.executable.is_empty());
        assert_eq!(
            app.git_client.status,
            Some(Err("Choose an executable with a Unicode path.".into()))
        );
    }
}

#[test]
fn saved_configuration_is_restored_into_the_editor() {
    let state = AppState {
        git_client: Preferences {
            client: Some(Client::Custom),
            executable: "smerge".into(),
            arguments: vec!["--new-window".into(), PLACEHOLDER.into()],
        },
        ..Default::default()
    };
    let editor = git_client_settings::GitClientEditor::new(&state.git_client);
    assert_eq!(editor.arguments, "--new-window\n{repo}");
    assert_eq!(
        git_client_settings::parse_arguments(&editor.arguments),
        state.git_client.arguments
    );
}
