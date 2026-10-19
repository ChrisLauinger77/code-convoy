//! About's update check states. The real worker is never dispatched here, so
//! these tests make no network request.
use super::*;
use crate::updates::{Failure, FailureKind, InstallSource, Outcome, Release};

const RELEASE_URL: &str = "https://github.com/ChrisLauinger77/code-convoy/releases/tag/v1.3.0";

fn about() -> Keyboard {
    Keyboard::new(|app, ui, ctx| {
        app.handle_shortcuts(ctx);
        if ui.button("Open About").clicked() {
            app.about_open = true;
        }
        app.about_window(ctx);
    })
}

/// Settles the modal's sizing pass, then keeps only the current frame's nodes.
fn render(keys: &mut Keyboard, app: &mut App) {
    keys.frame(app, vec![]);
    keys.nodes.clear();
    keys.frame(app, vec![]);
}

fn node<'a>(keys: &'a Keyboard, label: &str) -> Option<&'a Node> {
    keys.nodes.values().find(|node| keys.label(node) == label)
}

fn button_enabled(keys: &Keyboard) -> bool {
    !node(keys, "Check for updates on GitHub")
        .expect("Check for updates button is rendered")
        .is_disabled()
}

fn available(source: InstallSource) -> about::UpdateCheck {
    about::UpdateCheck::Finished(Ok(Outcome::Available {
        release: Release {
            version: semver::Version::new(1, 3, 0),
            url: RELEASE_URL.into(),
        },
        source,
    }))
}

#[test]
fn about_offers_an_accessible_check_without_starting_one() {
    let (_temp, mut app) = app();
    let mut keys = about();
    keys.activate(&mut app, "Open About");
    keys.focus(&mut app, "Check for updates on GitHub");
    assert!(button_enabled(&keys));
    // Merely opening, navigating and closing About never contacts GitHub.
    keys.key(&mut app, egui::Key::Escape);
    assert!(!app.about_open);
    assert!(matches!(app.update_check, about::UpdateCheck::Idle));
}

#[test]
fn checking_disables_the_button_until_a_result_allows_rechecking() {
    let (_temp, mut app) = app();
    app.about_open = true;
    assert!(app.begin_update_check());
    assert!(!app.begin_update_check());
    let mut keys = about();
    render(&mut keys, &mut app);
    assert!(!button_enabled(&keys));
    assert!(node(&keys, "Checking…").is_some());

    assert!(app.tx.send(Message::UpdateChecked(Ok(Outcome::UpToDate {
        latest: semver::Version::new(1, 2, 0),
    }))));
    app.poll();
    render(&mut keys, &mut app);
    assert!(button_enabled(&keys));
    assert!(node(&keys, "Checking…").is_none());
    assert!(node(&keys, "Up to date").is_some());
    assert!(node(&keys, "Latest release: 1.2.0").is_some());
    assert!(app.begin_update_check());

    app.update_check = about::UpdateCheck::Idle;
    app.closing = true;
    assert!(!app.begin_update_check());
    assert!(matches!(app.update_check, about::UpdateCheck::Idle));
}

#[test]
fn available_release_opens_its_page_and_shows_only_detected_package_hints() {
    for (source, command) in [
        (
            InstallSource::Homebrew,
            Some(crate::updates::HOMEBREW_COMMAND),
        ),
        (InstallSource::Scoop, Some(crate::updates::SCOOP_COMMAND)),
        (
            InstallSource::ScoopGlobal,
            Some(crate::updates::SCOOP_GLOBAL_COMMAND),
        ),
        (InstallSource::Unknown, None),
    ] {
        let (_temp, mut app) = app();
        app.about_open = true;
        app.update_check = available(source);
        let mut keys = about();
        render(&mut keys, &mut app);
        assert!(node(&keys, "Version 1.3.0 available").is_some());
        assert!(button_enabled(&keys));
        assert!(node(&keys, "CodeConvoy does not download or install updates.").is_some());
        let lag = keys
            .nodes
            .values()
            .any(|node| keys.label(node).contains("can lag a GitHub release"));
        assert_eq!(lag, command.is_some(), "{source:?}");
        for known in [
            crate::updates::HOMEBREW_COMMAND,
            crate::updates::SCOOP_COMMAND,
            crate::updates::SCOOP_GLOBAL_COMMAND,
        ] {
            assert_eq!(node(&keys, known).is_some(), command == Some(known));
        }
        let administrator = "Installed globally with Scoop. Update from an administrator terminal:";
        assert_eq!(
            node(&keys, administrator).is_some(),
            source == InstallSource::ScoopGlobal
        );

        keys.activate(&mut app, "Open release page");
        assert!(keys.commands.iter().any(|command| matches!(
            command,
            egui::OutputCommand::OpenUrl(open) if open.url == RELEASE_URL
        )));
        if let Some(command) = command {
            keys.commands.clear();
            keys.activate(&mut app, "Copy update command");
            assert!(matches!(
                keys.commands.as_slice(),
                [egui::OutputCommand::CopyText(text)] if text == command
            ));
        }
    }
}

#[test]
fn failed_check_keeps_copyable_diagnostics_and_allows_retry() {
    let (_temp, mut app) = app();
    app.about_open = true;
    let failure = Failure::new(
        FailureKind::RateLimited,
        "HTTP status: 403 (rate limited)\nx-ratelimit-remaining: 0",
    );
    app.update_check = about::UpdateCheck::Finished(Err(failure.clone()));
    let mut keys = about();
    render(&mut keys, &mut app);
    assert!(node(&keys, "Update check failed").is_some());
    assert!(node(&keys, failure.summary()).is_some());
    assert!(button_enabled(&keys));
    keys.activate(&mut app, "Diagnostics");
    keys.commands.clear();
    keys.activate(&mut app, "Copy diagnostics");
    assert!(matches!(
        keys.commands.as_slice(),
        [egui::OutputCommand::CopyText(text)] if text == &failure.detail
    ));
    assert!(app.begin_update_check());
}
