#![allow(clippy::unwrap_used)]
use codeconvoy::{
    domain::{AppState, Repository, RepositoryGroup},
    persistence::Store,
    validation::{
        Command,
        configuration::{Assignment, Existing, Selection},
        presets::BUILT_INS,
    },
};

fn fixture() -> (tempfile::TempDir, Store, AppState) {
    let temp = tempfile::tempdir().unwrap();
    let store = Store::open(&temp.path().join("state")).unwrap();
    // Deliberately absent paths: configuration must not inspect Git or create files.
    let repositories: Vec<_> = ["Extension", "Rust", "Shared"]
        .iter()
        .map(|name| Repository {
            name: (*name).into(),
            path: temp.path().join(name),
        })
        .collect();
    let state = AppState {
        groups: vec![
            RepositoryGroup {
                name: "GNOME Extensions".into(),
                repositories: vec![repositories[0].path.clone(), repositories[2].path.clone()],
            },
            RepositoryGroup {
                name: "Rust projects".into(),
                repositories: vec![repositories[1].path.clone(), repositories[2].path.clone()],
            },
        ],
        repositories,
        ..Default::default()
    };
    (temp, store, state)
}

#[test]
fn validation_presets_have_stable_ids_and_exact_literal_commands() {
    let expected: &[(&str, &str, &[&str])] = &[
        ("gnome-extension-lint", "npm", &["run", "lint"]),
        ("rust-tests", "cargo", &["test"]),
        ("rust-check", "cargo", &["check"]),
        ("rust-clippy", "cargo", &["clippy", "--all-targets"]),
        ("npm-test", "npm", &["test"]),
        ("npm-build", "npm", &["run", "build"]),
        ("python-pytest", "pytest", &[]),
        ("cmake-build", "cmake", &["--build", "build"]),
        ("make-test", "make", &["test"]),
    ];
    assert_eq!(BUILT_INS.len(), expected.len());
    for (preset, (id, executable, arguments)) in BUILT_INS.iter().zip(expected) {
        assert_eq!(preset.id, *id);
        let command = preset.command();
        assert_eq!(command.executable, *executable);
        assert_eq!(&command.arguments, arguments);
        command.validate().unwrap();
        let encoded = serde_json::to_value(&command).unwrap();
        assert_eq!(encoded.as_object().unwrap().len(), 2);
        assert_eq!(serde_json::from_value::<Command>(encoded).unwrap(), command);
    }
}

#[test]
fn validation_preview_preserves_argument_boundaries_without_shell_quoting() {
    assert_eq!(BUILT_INS[0].command().preview(), "npm run lint");
    let command = Command {
        executable: "cargo".into(),
        arguments: vec![
            "test".into(),
            "two words".into(),
            "".into(),
            "a\"b".into(),
            "$VAR && *".into(),
            "line\nnext".into(),
            r"C:\space dir\file".into(),
            "日本語".into(),
        ],
    };
    let before = command.clone();
    assert_eq!(
        command.preview(),
        r#"cargo test "two words" "" "a\"b" "$VAR && *" "line\nnext" "C:\\space dir\\file" "日本語""#
    );
    assert_eq!(command, before);
}

#[test]
fn validation_single_save_edit_remove_and_restart_keep_existing_schema() {
    let (_temp, store, mut state) = fixture();
    let path = state.repositories[0].path.clone();
    let mut command = BUILT_INS[0].command();
    store
        .save_validation_command(&mut state, path.clone(), Some(command.clone()))
        .unwrap();
    assert_eq!(store.load().unwrap().repository_validation[&path], command);
    command.arguments.push("literal argument".into());
    store
        .save_validation_command(&mut state, path.clone(), Some(command.clone()))
        .unwrap();
    assert_eq!(store.load().unwrap().repository_validation[&path], command);
    store
        .save_validation_command(&mut state, path.clone(), None)
        .unwrap();
    assert!(store.load().unwrap().repository_validation.is_empty());
    let old: AppState = serde_json::from_str(include_str!("fixtures/state-v0.1.json")).unwrap();
    assert!(old.repository_validation.is_empty());
    let encoded = serde_json::to_string(&state).unwrap();
    assert!(!encoded.contains("preset"));
    assert_eq!(state.version, 1);
}

#[test]
fn validation_bulk_individual_selection_and_empty_selection() {
    for count in 0..=3 {
        let (_temp, store, mut state) = fixture();
        let selection = Selection {
            paths: state
                .repositories
                .iter()
                .take(count)
                .map(|r| r.path.clone())
                .collect(),
        };
        let review = Assignment::review(&state, &selection, &BUILT_INS[1], Existing::default());
        assert!(state.repository_validation.is_empty());
        let report = store.assign_validation(&mut state, &review);
        assert_eq!(report.updated.len(), count);
        assert!(report.skipped.is_empty() && report.failed.is_empty());
        assert_eq!(state.repository_validation.len(), count);
    }
}

#[test]
fn validation_bulk_groups_overlap_deduplicate_and_do_not_mutate_other_state() {
    let (temp, store, mut state) = fixture();
    let before = serde_json::to_value(&state).unwrap();
    let mut selection = Selection::default();
    selection.select_group(&state.groups[0], true);
    assert_eq!(selection.paths.len(), 2);
    selection.select_group(&state.groups[1], true);
    selection.select_group(&state.groups[0], true);
    selection.paths.insert(state.repositories[2].path.clone());
    assert_eq!(selection.paths.len(), 3);
    let report = store.assign_validation(
        &mut state,
        &Assignment::review(
            &serde_json::from_value(before.clone()).unwrap(),
            &selection,
            &BUILT_INS[0],
            Existing::Preserve,
        ),
    );
    assert_eq!(report.updated.len(), 3);
    assert_eq!(
        report.summary(),
        "3 repositories updated, 0 existing configurations preserved, 0 failed."
    );
    let loaded = store.load().unwrap();
    assert_eq!(loaded.repository_validation.len(), 3);
    for repo in &loaded.repositories {
        assert_eq!(
            loaded.repository_validation[&repo.path],
            BUILT_INS[0].command()
        );
        assert!(!repo.path.exists());
    }
    assert!(!store.directory().join("worktrees").exists());
    assert_eq!(std::fs::read_dir(temp.path()).unwrap().count(), 1);
    state.repository_validation.clear();
    assert_eq!(serde_json::to_value(&state).unwrap(), before);
    selection.select_group(&state.groups[0], false);
    assert_eq!(selection.paths, [state.repositories[1].path.clone()].into());
}

#[test]
fn validation_bulk_preserves_custom_by_default_and_overwrites_only_explicitly() {
    let (_temp, store, mut state) = fixture();
    let custom = Command {
        executable: "local-check".into(),
        arguments: vec!["literal spaces".into()],
    };
    let path = state.repositories[2].path.clone();
    state
        .save_validation(path.clone(), Some(custom.clone()))
        .unwrap();
    let mut selection = Selection::default();
    selection.select_group(&state.groups[1], true);
    let review = Assignment::review(&state, &selection, &BUILT_INS[1], Existing::default());
    assert_eq!(review.existing_count(), 1);
    let report = store.assign_validation(&mut state, &review);
    assert_eq!(
        (
            report.updated.len(),
            report.skipped.len(),
            report.failed.len()
        ),
        (1, 1, 0)
    );
    assert_eq!(state.repository_validation[&path], custom);
    let review = Assignment::review(&state, &selection, &BUILT_INS[1], Existing::Overwrite);
    assert_eq!(review.existing_count(), 2);
    // Reviewing (or dropping the review on Cancel) has no persistence effects.
    assert_eq!(store.load().unwrap().repository_validation[&path], custom);
    let report = store.assign_validation(&mut state, &review);
    assert_eq!(
        (
            report.updated.len(),
            report.skipped.len(),
            report.failed.len()
        ),
        (2, 0, 0)
    );
    assert_eq!(
        store.load().unwrap().repository_validation[&path],
        BUILT_INS[1].command()
    );
}

#[test]
fn validation_bulk_rechecks_removed_targets_and_changed_overwrite_commands() {
    let (_temp, store, mut state) = fixture();
    let selection = Selection {
        paths: state.repositories.iter().map(|r| r.path.clone()).collect(),
    };
    let review = Assignment::review(&state, &selection, &BUILT_INS[0], Existing::Overwrite);
    let removed = state.repositories.remove(0);
    let changed = state.repositories[0].path.clone();
    state
        .save_validation(changed.clone(), Some(BUILT_INS[2].command()))
        .unwrap();
    let report = store.assign_validation(&mut state, &review);
    assert_eq!(
        (
            report.updated.len(),
            report.skipped.len(),
            report.failed.len()
        ),
        (1, 0, 2)
    );
    assert!(
        report
            .failed
            .iter()
            .any(|(r, e)| r == &removed && e.contains("no longer registered"))
    );
    assert_eq!(
        state.repository_validation[&changed],
        BUILT_INS[2].command()
    );
    assert!(!state.repository_validation.contains_key(&removed.path));
    // Stale group references remain visible in review and failed outcomes.
    let mut selection = Selection::default();
    selection.select_group(&state.groups[0], true);
    let review = Assignment::review(&state, &selection, &BUILT_INS[0], Existing::Preserve);
    assert_eq!(review.missing_count(), 1);
    let report = store.assign_validation(&mut state, &review);
    assert_eq!(
        (
            report.updated.len(),
            report.skipped.len(),
            report.failed.len()
        ),
        (0, 1, 1)
    );
}

#[test]
fn validation_persistence_failure_keeps_live_state_and_reports_every_target() {
    let (_temp, store, mut state) = fixture();
    let mut selection = Selection::default();
    selection.select_group(&state.groups[0], true);
    selection.select_group(&state.groups[1], true);
    state
        .save_validation(
            state.repositories[0].path.clone(),
            Some(BUILT_INS[2].command()),
        )
        .unwrap();
    let before = serde_json::to_value(&state).unwrap();
    // Deterministic rename failure on all platforms, without permission assumptions.
    std::fs::create_dir(store.directory().join("state.json")).unwrap();
    let review = Assignment::review(&state, &selection, &BUILT_INS[0], Existing::Preserve);
    let report = store.assign_validation(&mut state, &review);
    assert_eq!(
        (
            report.updated.len(),
            report.skipped.len(),
            report.failed.len()
        ),
        (0, 1, 2)
    );
    assert!(
        report
            .failed
            .iter()
            .all(|(_, e)| e.contains("Could not confirm saving"))
    );
    assert_eq!(serde_json::to_value(&state).unwrap(), before);
    assert!(
        store
            .save_validation_command(&mut state, state_path(&before), None)
            .is_err()
    );
    assert_eq!(serde_json::to_value(&state).unwrap(), before);
}

fn state_path(value: &serde_json::Value) -> std::path::PathBuf {
    serde_json::from_value(value["repositories"][0]["path"].clone()).unwrap()
}
