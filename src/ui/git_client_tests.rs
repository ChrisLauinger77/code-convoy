//! Git client settings without a native dialog or Git client process.
use super::accessibility::focused_name;
use super::*;
use crate::git_client::{Client, PLACEHOLDER, Preferences};
use egui::Key;

fn dialog(app: &App) -> &git_client_settings::GitClientDialog {
    app.git_client_dialog.as_ref().unwrap()
}

fn settle(app: &mut App) -> Result<String, String> {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(status) = &dialog(app).status {
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

fn window(app: &mut App, _: &mut egui::Ui, ctx: &egui::Context) {
    app.git_client_settings_window(ctx);
}

#[test]
fn settings_menu_opens_the_dialog_and_ends_with_quit() {
    let (_temp, mut app) = app();
    let mut keys = Keyboard::new(|app, ui, ctx| {
        app.handle_shortcuts(ctx);
        app.header(ui, ctx);
        app.git_client_settings_window(ctx);
    });
    keys.activate(&mut app, "Settings");
    keys.focus(&mut app, "Keyboard Shortcuts…");
    keys.key(&mut app, Key::Tab);
    assert_eq!(focused_name(&keys).as_deref(), Some("Quit CodeConvoy"));
    keys.key(&mut app, Key::Escape);
    keys.activate(&mut app, "Settings");
    keys.activate(&mut app, "Git client…");
    assert!(app.git_client_dialog.is_some());
    assert_eq!(focused_name(&keys).as_deref(), Some("Cancel"));
    keys.key(&mut app, Key::Escape);
    assert!(app.git_client_dialog.is_none());
    assert_eq!(focused_name(&keys).as_deref(), Some("Settings"));
    assert!(!app.explicit_quit);
}

#[test]
fn a_client_can_be_chosen_from_the_dialog_by_keyboard() {
    let (_temp, mut app) = app();
    app.open_git_client_settings();
    let mut keys = Keyboard::new(window);
    keys.activate(&mut app, "Client");
    keys.activate(&mut app, "Custom");
    assert_eq!(
        dialog(&app).draft,
        Preferences::select(Some(Client::Custom))
    );
    assert!(
        app.state.git_client.is_default(),
        "nothing saved before Save"
    );
    keys.activate(&mut app, "Save");
    assert!(app.git_client_dialog.is_none());
    assert_eq!(app.state.git_client.client, Some(Client::Custom));
    assert_eq!(app.store.load().unwrap().git_client, app.state.git_client);
}

#[test]
fn choosing_a_client_starts_clean_and_cancel_keeps_the_saved_configuration() {
    let (_temp, mut app) = app();
    let saved = Preferences {
        client: Some(Client::Custom),
        executable: "/opt/custom".into(),
        arguments: vec!["--path={repo}".into()],
    };
    app.state.git_client = saved.clone();
    app.open_git_client_settings();
    assert_eq!(dialog(&app).arguments, "--path={repo}");
    app.select_git_client(Some(Client::Custom));
    assert_eq!(dialog(&app).draft, saved, "same client keeps its draft");
    app.select_git_client(Some(Client::SublimeMerge));
    assert_eq!(
        dialog(&app).draft,
        Preferences::select(Some(Client::SublimeMerge))
    );
    app.select_git_client(Some(Client::Custom));
    assert_eq!(dialog(&app).draft.arguments, [PLACEHOLDER]);
    assert_eq!(dialog(&app).arguments, PLACEHOLDER);

    let mut keys = Keyboard::new(window);
    keys.activate(&mut app, "Cancel");
    assert!(app.git_client_dialog.is_none());
    assert_eq!(app.state.git_client, saved);

    app.open_git_client_settings();
    app.select_git_client(None);
    app.save_git_client();
    assert!(app.state.git_client.is_default());
    let stored = serde_json::to_value(app.store.load().unwrap()).unwrap();
    assert!(stored.get("git_client").is_none());
}

#[test]
fn custom_fields_are_keyboard_reachable_and_arguments_stay_structured() {
    let (_temp, mut app) = app();
    app.open_git_client_settings();
    app.select_git_client(Some(Client::Custom));
    let mut keys = Keyboard::new(window);
    keys.frame(&mut app, vec![]);
    keys.replace_text(&mut app, "Executable", "/opt/My Client ü/bin/client");
    keys.replace_text(
        &mut app,
        "Arguments, one per line",
        "--path={repo}\r\n\nliteral with spaces ü\n{repo}",
    );
    keys.frame(&mut app, vec![]);
    let draft = &dialog(&app).draft;
    assert_eq!(draft.executable, "/opt/My Client ü/bin/client");
    assert_eq!(
        draft.arguments,
        ["--path={repo}", "literal with spaces ü", "{repo}"]
    );
    keys.activate(&mut app, "Save");
    let saved = serde_json::to_value(app.store.load().unwrap()).unwrap();
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
    app.open_git_client_settings();
    let mut keys = Keyboard::new(window);
    keys.frame(&mut app, vec![]);
    assert!(dialog(&app).checked.is_none(), "nothing to check yet");

    app.select_git_client(Some(Client::Custom));
    let missing = temp.path().join("missing client");
    app.git_client_dialog.as_mut().unwrap().draft.executable = missing.display().to_string();
    keys.frame(&mut app, vec![]);
    let generation = app.git_client_generation;
    keys.frame(&mut app, vec![]);
    assert_eq!(app.git_client_generation, generation, "no check per frame");
    let error = settle(&mut app).unwrap_err();
    assert!(error.contains(&missing.display().to_string()), "{error}");
    assert!(error.contains("does not exist."), "{error}");

    let found = launcher(&temp.path().join("Client tools"));
    app.git_client_dialog.as_mut().unwrap().draft.executable = found.display().to_string();
    keys.frame(&mut app, vec![]);
    assert_eq!(app.git_client_generation, generation + 1);
    app.git_client_checked(generation, Err("stale".into()));
    assert!(dialog(&app).status.is_none(), "stale result ignored");
    let ready = settle(&mut app).unwrap();
    assert!(ready.contains(&found.display().to_string()) && ready.contains(PLACEHOLDER));
    assert!(app.state.git_client.is_default(), "checks never save");

    // A result for a closed dialog is dropped, not shown in a later one.
    app.git_client_generation += 1;
    let late = app.git_client_generation;
    app.git_client_dialog = None;
    app.git_client_checked(late, Ok("late".into()));
    app.open_git_client_settings();
    assert!(dialog(&app).status.is_none());
}

#[test]
fn browsed_executables_are_kept_exactly_and_cancellation_changes_nothing() {
    let (_temp, mut app) = app();
    let chosen = PathBuf::from("/Applications/Sublime Merge ü.app ");
    app.git_client_executable_selected(Some(chosen.clone()));
    assert!(app.git_client_dialog.is_none(), "no dialog, nothing stored");

    app.open_git_client_settings();
    app.select_git_client(Some(Client::SublimeMerge));
    app.git_client_dialog.as_mut().unwrap().browsing = true;
    app.git_client_executable_selected(None);
    assert!(!dialog(&app).browsing);
    assert!(dialog(&app).draft.executable.is_empty());

    app.git_client_executable_selected(Some(chosen.clone()));
    assert_eq!(
        dialog(&app).draft.executable,
        chosen.to_str().unwrap(),
        "picked paths keep surrounding whitespace"
    );

    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStringExt;
        app.select_git_client(Some(Client::Custom));
        let invalid = PathBuf::from(std::ffi::OsString::from_vec(b"/opt/\xff".to_vec()));
        app.git_client_executable_selected(Some(invalid));
        assert!(dialog(&app).draft.executable.is_empty());
        assert_eq!(
            dialog(&app).error,
            "Choose an executable with a Unicode path."
        );
    }
}

#[test]
fn argument_lines_parse_into_separate_arguments() {
    assert_eq!(
        git_client_settings::parse_arguments("--new-window\r\n\n  spaced \n{repo}"),
        ["--new-window", "  spaced ", "{repo}"]
    );
    assert!(git_client_settings::parse_arguments("\n\r\n").is_empty());
}
