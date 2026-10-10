//! Exercise real egui controls as well as the session-only membership cache.
use super::*;
use crate::ui::repository_sections::{Counts, RepositorySections};
use egui::collapsing_header::CollapsingState;

#[path = "repository_filter_tests.rs"]
mod filter;

fn grouped_app() -> (tempfile::TempDir, App) {
    let (temp, mut app) = app();
    app.state.repositories = vec![repository("alpha"), repository("beta"), repository("gamma")];
    app.state.groups = vec![
        domain::RepositoryGroup {
            name: "First".into(),
            repositories: vec![repository("alpha").path, repository("beta").path],
        },
        domain::RepositoryGroup {
            name: "Overlap".into(),
            repositories: vec![repository("beta").path],
        },
    ];
    (temp, app)
}

fn picker() -> Keyboard {
    let keys = Keyboard::new(|app, ui, ctx| app.repositories_section(ui, ctx));
    keys.ctx.all_styles_mut(|s| s.animation_time = 0.0);
    keys
}

fn open(keys: &Keyboard, app: &App, section: usize) -> bool {
    CollapsingState::load(&keys.ctx, app.repository_sections.sections[section].id)
        .unwrap()
        .is_open()
}

#[test]
fn ungrouped_excludes_every_assigned_repository_and_memberships_are_deduplicated() {
    let (_temp, mut app) = grouped_app();
    app.state.groups[0]
        .repositories
        .push(repository("beta").path);
    app.state.groups[0]
        .repositories
        .push(repository("missing").path);
    let mut cache = RepositorySections::default();
    cache.sync(&app.state.repositories, &app.state.groups);
    assert_eq!(cache.sections[0].name, "Ungrouped");
    assert_eq!(cache.sections[0].repositories, [2]);
    assert_eq!(cache.sections[1].repositories, [0, 1]);
    assert_eq!(cache.sections[2].repositories, [1]);
    assert_eq!(cache.sections[1].unregistered, [repository("missing").path]);
    assert_eq!(cache.sections[1].counts(&app).total, 3);
    assert_ne!(cache.sections[0].id, cache.sections[1].id);
    // A user-created group named Ungrouped is distinct from the built-in section.
    app.state.groups[0].name = "Ungrouped".into();
    cache.sync(&app.state.repositories, &app.state.groups);
    assert_ne!(cache.sections[0].id, cache.sections[1].id);
}

#[test]
fn section_counts_track_selection_and_availability_without_rebuilding_membership() {
    let (_temp, mut app) = grouped_app();
    app.state.groups[0]
        .repositories
        .push(repository("missing").path);
    let mut cache = RepositorySections::default();
    cache.sync(&app.state.repositories, &app.state.groups);
    let sections = cache.sections.as_ptr();
    let members = cache.sections[1].repositories.as_ptr();
    app.select_repositories(true);
    app.repository_states
        .insert(repository("beta").path, Err("missing directory".into()));
    cache.sync(&app.state.repositories, &app.state.groups);
    assert_eq!(cache.sections.as_ptr(), sections);
    assert_eq!(cache.sections[1].repositories.as_ptr(), members);
    assert_eq!(
        cache.sections[1].counts(&app),
        Counts {
            total: 3,
            selected: 2,
            available: 1,
            selected_available: 1,
            unavailable: 2,
        }
    );
    assert_eq!(cache.sections[2].counts(&app).selected, 1);
    assert_eq!(cache.sections[0].counts(&app).selected, 1);
    app.select_repositories(false);
    assert_eq!(cache.sections[1].counts(&app).selected, 0);
    assert_eq!(cache.sections[2].counts(&app).selected, 0);
}

#[test]
fn collapsed_groups_select_and_deselect_shared_paths_without_duplicate_jobs() {
    let (_temp, mut app) = grouped_app();
    let mut keys = picker();
    keys.frame(&mut app, vec![]);
    assert!(!open(&keys, &app, 1) && !open(&keys, &app, 2));
    keys.activate(&mut app, "Select group First");
    assert_eq!(app.selected.len(), 2);
    assert_eq!(app.repository_sections.sections[2].counts(&app).selected, 1);
    // Deselecting overlapping members removes the same explicit path.
    keys.activate(&mut app, "Select group Overlap");
    assert_eq!(app.selected, [repository("alpha").path].into());
    keys.activate(&mut app, "Select group Overlap");
    app.select_group(0, true);
    assert_eq!(app.selected.len(), 2);
    let prepared = PreparedRun {
        task: app.state.draft.clone(),
        repositories: app
            .state
            .repositories
            .iter()
            .filter(|r| app.selected.contains(&r.path))
            .map(|repository| PreparedRepository {
                repository: repository.clone(),
                state: WorkingTree {
                    summary: domain::GitSummary::default(),
                    entries: vec![],
                },
            })
            .collect(),
    };
    let snapshot = prepared.snapshot(1);
    assert_eq!(snapshot.jobs.len(), 2);
    assert_eq!(
        snapshot
            .jobs
            .iter()
            .map(|j| &j.repository.path)
            .collect::<HashSet<_>>()
            .len(),
        2
    );
    assert!(!open(&keys, &app, 1) && !open(&keys, &app, 2));
    assert!(app.manager.is_idle());
}

#[test]
fn individual_checkboxes_share_selection_in_every_expanded_occurrence() {
    let (_temp, mut app) = grouped_app();
    let mut keys = picker();
    keys.frame(&mut app, vec![]);
    keys.activate(&mut app, "First (2)");
    keys.activate(&mut app, "Overlap (1)");
    keys.activate(&mut app, "beta");
    assert_eq!(app.selected, [repository("beta").path].into());
    let occurrences: Vec<_> = keys
        .nodes
        .iter()
        .filter(|(_, node)| node.label() == Some("beta"))
        .collect();
    assert_eq!(occurrences.len(), 2);
    assert!(
        occurrences
            .iter()
            .all(|(_, node)| node.toggled() == Some(egui::accesskit::Toggled::True))
    );
    assert_eq!(app.repository_sections.sections[1].counts(&app).selected, 1);
    assert_eq!(app.repository_sections.sections[2].counts(&app).selected, 1);
    // Tab from the first occurrence to the second and toggle with Space.
    let first = keys.ctx.memory(|m| m.focused()).unwrap();
    loop {
        keys.key(&mut app, egui::Key::Tab);
        let id = keys.ctx.memory(|m| m.focused()).unwrap();
        if id != first && keys.nodes[&id.accesskit_id()].label() == Some("beta") {
            break;
        }
        assert_ne!(
            id, first,
            "Second checkbox should be independently keyboard reachable"
        );
    }
    keys.key(&mut app, egui::Key::Space);
    assert!(app.selected.is_empty());
    assert!(
        keys.nodes
            .values()
            .filter(|node| node.label() == Some("beta"))
            .all(|node| node.toggled() == Some(egui::accesskit::Toggled::False))
    );
}

#[test]
fn expansion_is_independent_and_survives_navigation_counts_and_group_reordering() {
    let (_temp, mut app) = grouped_app();
    let mut keys = picker();
    keys.frame(&mut app, vec![]);
    assert!(open(&keys, &app, 0));
    keys.activate(&mut app, "Ungrouped (1)");
    keys.activate(&mut app, "First (2)");
    assert!(!open(&keys, &app, 0));
    assert!(open(&keys, &app, 1));
    assert!(!open(&keys, &app, 2));
    assert!(app.selected.is_empty());
    let first_id = app.repository_sections.sections[1].id;
    let overlap_id = app.repository_sections.sections[2].id;
    keys.surface = |_, ui, _| {
        ui.label("Another view");
    };
    keys.frame(&mut app, vec![]);
    app.select_repositories(true);
    app.state.groups.swap(0, 1);
    keys.surface = |app, ui, ctx| app.repositories_section(ui, ctx);
    keys.frame(&mut app, vec![]);
    assert_eq!(app.repository_sections.sections[1].id, overlap_id);
    assert_eq!(app.repository_sections.sections[2].id, first_id);
    assert!(!open(&keys, &app, 1));
    assert!(open(&keys, &app, 2));
    assert_eq!(app.selected.len(), 3);
    keys.activate(&mut app, "Toggle First");
    assert!(!open(&keys, &app, 2));
    assert_eq!(app.selected.len(), 3);
}

#[test]
fn select_all_and_none_work_with_every_section_collapsed() {
    let (_temp, mut app) = grouped_app();
    let mut keys = picker();
    keys.frame(&mut app, vec![]);
    keys.activate(&mut app, "Ungrouped (1)");
    keys.activate(&mut app, "Select all");
    assert_eq!(app.selected.len(), 3);
    assert!(
        keys.nodes
            .values()
            .any(|node| keys.label(node) == "3 / 3 selected")
    );
    for section in &app.repository_sections.sections {
        assert_eq!(section.counts(&app).selected, section.counts(&app).total);
        assert!(
            !CollapsingState::load(&keys.ctx, section.id)
                .unwrap()
                .is_open()
        );
    }
    keys.activate(&mut app, "Select none");
    assert!(app.selected.is_empty());
    for section in &app.repository_sections.sections {
        assert_eq!(section.counts(&app).selected, 0);
        assert!(
            !CollapsingState::load(&keys.ctx, section.id)
                .unwrap()
                .is_open()
        );
    }
}

#[test]
fn editing_and_deleting_groups_preserves_selection_and_other_expansion_states() {
    let (_temp, mut app) = grouped_app();
    let mut keys = picker();
    keys.frame(&mut app, vec![]);
    keys.activate(&mut app, "First (2)");
    let first_id = app.repository_sections.sections[1].id;
    let overlap_id = app.repository_sections.sections[2].id;
    app.select_repositories(true);
    let selection = app.selected.clone();
    keys.activate(&mut app, "Manage groups…");
    keys.activate(&mut app, "Group");
    keys.activate(&mut app, "First");
    keys.replace_text(&mut app, "Name", "Renamed");
    keys.activate(&mut app, "alpha"); // Remove alpha; it becomes Ungrouped.
    keys.activate(&mut app, "Save group");
    assert_eq!(app.repository_sections.sections[1].id, first_id);
    assert_eq!(app.repository_sections.sections[1].repositories, [1]);
    assert_eq!(app.repository_sections.sections[0].repositories, [0, 2]);
    assert!(open(&keys, &app, 1));
    assert_eq!(app.selected, selection);
    keys.activate(&mut app, "Manage groups…");
    keys.activate(&mut app, "Group");
    keys.activate(&mut app, "Renamed");
    keys.activate(&mut app, "Delete group");
    assert_eq!(app.repository_sections.sections.len(), 2);
    assert_eq!(app.repository_sections.sections[1].id, overlap_id);
    assert!(!open(&keys, &app, 1));
    assert_eq!(app.selected, selection);
    assert_eq!(app.repository_sections.sections[0].repositories, [0, 2]);
    // Recreating the old name gets a fresh identity, without inheriting expansion.
    app.state
        .save_group(
            None,
            domain::RepositoryGroup {
                name: "Renamed".into(),
                repositories: vec![repository("alpha").path],
            },
        )
        .unwrap();
    keys.frame(&mut app, vec![]);
    assert_ne!(app.repository_sections.sections[2].id, first_id);
    assert!(!open(&keys, &app, 2));
}

#[test]
fn registration_changes_reclassify_members_without_losing_repairable_membership() {
    let (_temp, mut app) = grouped_app();
    let mut cache = RepositorySections::default();
    cache.sync(&app.state.repositories, &app.state.groups);
    let id = cache.sections[1].id;
    let beta = app.state.repositories.remove(1);
    cache.sync(&app.state.repositories, &app.state.groups);
    assert_eq!(cache.sections[0].repositories, [1]);
    assert_eq!(cache.sections[1].repositories, [0]);
    assert_eq!(
        cache.sections[1].unregistered,
        std::slice::from_ref(&beta.path)
    );
    assert_eq!(
        cache.sections[2].unregistered,
        std::slice::from_ref(&beta.path)
    );
    app.state.repositories.push(beta);
    cache.sync(&app.state.repositories, &app.state.groups);
    assert_eq!(cache.sections[1].id, id);
    assert_eq!(cache.sections[1].repositories, [0, 2]);
    assert!(cache.sections[1].unregistered.is_empty());
    app.state.groups.clear();
    cache.sync(&app.state.repositories, &app.state.groups);
    assert_eq!(cache.sections.len(), 1);
    assert_eq!(cache.sections[0].repositories, [0, 1, 2]);
}

#[test]
fn unavailable_selected_members_can_be_deselected_from_a_collapsed_header() {
    let (_temp, mut app) = grouped_app();
    let mut keys = picker();
    app.select_repositories(true);
    app.repository_states
        .insert(repository("beta").path, Err("offline".into()));
    keys.frame(&mut app, vec![]);
    keys.activate(&mut app, "Select group Overlap");
    assert!(!app.selected.contains(&repository("beta").path));
    assert!(!open(&keys, &app, 2));
    keys.activate(&mut app, "Select group First");
    assert_eq!(app.selected, [repository("gamma").path].into());
    keys.activate(&mut app, "Select group First");
    assert_eq!(
        app.selected,
        [repository("alpha").path, repository("gamma").path].into()
    );
    assert!(app.notice.contains("Membership is retained"));
}

#[test]
fn group_headers_fit_narrow_views_in_all_themes_and_respond_to_pointer_clicks() {
    for theme in [
        egui::ThemePreference::Dark,
        egui::ThemePreference::Light,
        egui::ThemePreference::System,
    ] {
        for width in [280.0, 360.0] {
            let (_temp, mut app) = grouped_app();
            app.state.groups[0].name =
                "A very long repository group name that needs truncation".into();
            let mut keys = picker();
            keys.ctx.set_theme(theme);
            let screen_rect = Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(width, 600.0),
            ));
            keys.input(
                &mut app,
                egui::RawInput {
                    screen_rect,
                    ..Default::default()
                },
            );
            let label = format!("Select group {}", app.state.groups[0].name);
            let check = keys
                .nodes
                .values()
                .find(|node| node.label() == Some(&label))
                .unwrap();
            let bounds = check.bounds().unwrap();
            assert!(
                bounds.x0 >= 0.0 && bounds.x1 <= f64::from(width),
                "{theme:?} at {width}: {bounds:?}"
            );
            let title = format!("{} (2)", app.state.groups[0].name);
            let title_bounds = keys
                .nodes
                .values()
                .filter(|node| node.label() == Some(&title))
                .filter_map(|node| node.bounds())
                .max_by(|a, b| (a.x1 - a.x0).total_cmp(&(b.x1 - b.x0)))
                .unwrap();
            assert!(title_bounds.x1 < bounds.x0);
            let position = egui::pos2(
                ((title_bounds.x0 + title_bounds.x1) / 2.0) as f32,
                ((title_bounds.y0 + title_bounds.y1) / 2.0) as f32,
            );
            for pressed in [true, false] {
                keys.input(
                    &mut app,
                    egui::RawInput {
                        screen_rect,
                        events: vec![
                            egui::Event::PointerMoved(position),
                            egui::Event::PointerButton {
                                pos: position,
                                button: egui::PointerButton::Primary,
                                pressed,
                                modifiers: egui::Modifiers::NONE,
                            },
                        ],
                        ..Default::default()
                    },
                );
            }
            assert!(open(&keys, &app, 1));
            assert!(app.selected.is_empty());
        }
    }
}
