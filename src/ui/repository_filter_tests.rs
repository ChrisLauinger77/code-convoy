//! Visibility must never become selection, membership, or expansion state.
use super::*;

fn visible(cache: &RepositorySections) -> Vec<(&str, Vec<usize>)> {
    cache
        .sections
        .iter()
        .filter(|section| cache.visible(section))
        .map(|section| {
            (
                section.name.as_str(),
                section
                    .repositories
                    .iter()
                    .copied()
                    .filter(|&index| cache.matches(index))
                    .collect(),
            )
        })
        .collect()
}

#[test]
fn name_path_case_whitespace_and_empty_queries_match_in_memory() {
    let repositories = vec![
        Repository {
            name: "My SERVICE".into(),
            path: PathBuf::from("/Work/Backend/api"),
        },
        Repository {
            name: "Übersicht".into(),
            path: PathBuf::from("/Work/Frontend/web"),
        },
    ];
    let mut cache = RepositorySections::default();
    for (query, expected) in [
        ("service", vec![0]),
        ("  My sErViCe\t", vec![0]),
        ("bACKend/AP", vec![0]),
        ("ÜBER", vec![1]),
        ("FRONTEND", vec![1]),
        ("", vec![0, 1]),
        (" \t\n ", vec![0, 1]),
        ("absent", vec![]),
    ] {
        cache.query = query.into();
        cache.sync(&repositories, &[]);
        assert_eq!(
            (0..repositories.len())
                .filter(|&index| cache.matches(index))
                .collect::<Vec<_>>(),
            expected,
            "{query:?}"
        );
        assert_eq!(cache.filtering(), !query.trim().is_empty());
    }
}

#[test]
fn full_unicode_case_folding_matches_names_and_paths_in_both_directions() {
    // Exercise expansions as well as characters with distinct lowercase forms.
    for (text, query) in [
        ("Straße", "STRASSE"),
        ("STRAẞE", "strasse"),
        ("ΟΣ", "οσ"),
        ("ος", "οσ"),
        ("oﬃce", "OFFICE"),
    ] {
        for (text, query) in [(text, query), (query, text)] {
            let repositories = vec![
                Repository {
                    name: format!("prefix-{text}-suffix"),
                    path: PathBuf::from("/work/plain"),
                },
                Repository {
                    name: "plain".into(),
                    path: PathBuf::from(format!("/work/{text}/project")),
                },
                repository("unrelated"),
            ];
            let mut cache = RepositorySections::default();
            cache.query = format!("  {query}\t");
            cache.sync(&repositories, &[]);
            assert_eq!(
                visible(&cache),
                [("Ungrouped", vec![0, 1])],
                "{text:?} / {query:?}"
            );
        }
    }
}

#[test]
fn matching_sections_keep_order_overlap_and_original_ungrouped_classification() {
    let (_temp, mut app) = grouped_app();
    app.state.groups.push(domain::RepositoryGroup {
        name: "Empty".into(),
        repositories: vec![repository("missing").path],
    });
    let mut cache = RepositorySections::default();
    cache.query = "a".into();
    cache.sync(&app.state.repositories, &app.state.groups);
    assert_eq!(
        visible(&cache),
        [
            ("Ungrouped", vec![2]),
            ("First", vec![0, 1]),
            ("Overlap", vec![1])
        ]
    );
    let sections = cache.sections.as_ptr();
    let ids: Vec<_> = cache.sections.iter().map(|s| s.id).collect();
    cache.query = "beta".into();
    cache.sync(&app.state.repositories, &app.state.groups);
    assert_eq!(visible(&cache), [("First", vec![1]), ("Overlap", vec![1])]);
    assert_eq!(cache.sections.as_ptr(), sections);
    assert_eq!(cache.sections.iter().map(|s| s.id).collect::<Vec<_>>(), ids);
    // Group names and unregistered members are not repository search results.
    for query in ["First", "missing", "no-match"] {
        cache.query = query.into();
        cache.sync(&app.state.repositories, &app.state.groups);
        assert!(visible(&cache).is_empty());
    }
    cache.query.clear();
    cache.sync(&app.state.repositories, &app.state.groups);
    assert_eq!(visible(&cache).last(), Some(&("Empty", vec![])));
}

#[test]
fn unchanged_query_tracks_registration_name_path_and_group_edits() {
    let (_temp, mut app) = grouped_app();
    let mut cache = RepositorySections::default();
    cache.query = "beta".into();
    cache.sync(&app.state.repositories, &app.state.groups);
    app.state.repositories.remove(1);
    cache.sync(&app.state.repositories, &app.state.groups);
    assert!(visible(&cache).is_empty());
    app.state.repositories.push(repository("beta"));
    cache.sync(&app.state.repositories, &app.state.groups);
    assert_eq!(visible(&cache), [("First", vec![2]), ("Overlap", vec![2])]);
    app.state.groups.swap(0, 1);
    cache.sync(&app.state.repositories, &app.state.groups);
    assert_eq!(visible(&cache), [("Overlap", vec![2]), ("First", vec![2])]);
    app.state.groups.clear();
    cache.sync(&app.state.repositories, &app.state.groups);
    assert_eq!(visible(&cache), [("Ungrouped", vec![2])]);
    app.state.repositories[0].name = "Beta display name".into();
    app.state.repositories[1].path = PathBuf::from("/some/beta/path");
    cache.sync(&app.state.repositories, &app.state.groups);
    assert_eq!(visible(&cache), [("Ungrouped", vec![0, 1, 2])]);
}

#[test]
fn typing_reveals_collapsed_matches_and_clear_restores_manual_expansion() {
    let (_temp, mut app) = grouped_app();
    let mut keys = picker();
    keys.frame(&mut app, vec![]);
    keys.activate(&mut app, "Ungrouped (1)");
    keys.activate(&mut app, "First (2)");
    let before = serde_json::to_value(&app.state).unwrap();
    app.dirty = false;
    keys.replace_text(&mut app, "Filter repositories", " BeTa ");
    assert_eq!(
        visible(&app.repository_sections),
        [("First", vec![1]), ("Overlap", vec![1])]
    );
    assert!(!open(&keys, &app, 0));
    assert!(open(&keys, &app, 1));
    assert!(!open(&keys, &app, 2));
    assert_eq!(serde_json::to_value(&app.state).unwrap(), before);
    assert!(!app.dirty && app.selected.is_empty());
    keys.activate(&mut app, "beta");
    assert_eq!(app.selected, [repository("beta").path].into());
    assert_eq!(
        keys.nodes
            .values()
            .filter(|n| n.label() == Some("beta")
                && n.toggled() == Some(egui::accesskit::Toggled::True))
            .count(),
        2
    );
    keys.activate(&mut app, "Clear");
    assert!(app.repository_sections.query.is_empty());
    assert!(!open(&keys, &app, 0));
    assert!(open(&keys, &app, 1));
    assert!(!open(&keys, &app, 2));
    let focus = keys.ctx.memory(|m| m.focused()).unwrap();
    assert_eq!(
        keys.label(&keys.nodes[&focus.accesskit_id()]),
        "Filter repositories"
    );
    assert_eq!(app.selected, [repository("beta").path].into());
}

#[test]
fn filtering_before_first_render_creates_no_manual_expansion_preferences() {
    let (_temp, mut app) = grouped_app();
    let mut keys = picker();
    app.repository_sections.query = "a".into();
    keys.frame(&mut app, vec![]);
    for section in &app.repository_sections.sections {
        assert!(CollapsingState::load(&keys.ctx, section.id).is_none());
    }
    keys.activate(&mut app, "Clear");
    assert!(open(&keys, &app, 0));
    assert!(!open(&keys, &app, 1) && !open(&keys, &app, 2));
}

#[test]
fn filtered_group_actions_include_hidden_members_and_launch_selection_is_unique() {
    let (_temp, mut app) = grouped_app();
    let mut keys = picker();
    app.selected.insert(repository("gamma").path);
    app.repository_sections.query = "beta".into();
    keys.frame(&mut app, vec![]);
    keys.activate(&mut app, "Select group First");
    assert_eq!(app.selected.len(), 3); // alpha and gamma are hidden.
    keys.activate(&mut app, "Select group Overlap");
    assert_eq!(
        app.selected,
        [repository("alpha").path, repository("gamma").path].into()
    );
    keys.activate(&mut app, "beta");
    let prepared = PreparedRun {
        task: app.state.draft.clone(),
        repositories: app
            .state
            .repositories
            .iter()
            .filter(|r| app.selected.contains(&r.path))
            .map(|r| PreparedRepository {
                repository: r.clone(),
                state: WorkingTree {
                    summary: domain::GitSummary::default(),
                    entries: vec![],
                },
            })
            .collect(),
    };
    let snapshot = prepared.snapshot(1);
    assert_eq!(snapshot.jobs.len(), 3);
    assert_eq!(
        snapshot
            .jobs
            .iter()
            .map(|j| &j.repository.path)
            .collect::<HashSet<_>>()
            .len(),
        3
    );
    keys.activate(&mut app, "Select group First");
    assert_eq!(app.selected, [repository("gamma").path].into());
    assert!(app.manager.is_idle());
}

#[test]
fn filtered_ungrouped_selection_and_global_controls_keep_full_membership_semantics() {
    let (_temp, mut app) = grouped_app();
    app.state.repositories.push(repository("delta"));
    let mut keys = picker();
    app.repository_sections.query = "gamma".into();
    keys.frame(&mut app, vec![]);
    keys.activate(&mut app, "Select group Ungrouped");
    assert_eq!(
        app.selected,
        [repository("gamma").path, repository("delta").path].into()
    );
    keys.activate(&mut app, "Select group Ungrouped");
    assert!(app.selected.is_empty());
    keys.replace_text(&mut app, "Filter repositories", "nothing matches");
    keys.activate(&mut app, "Select all");
    assert_eq!(app.selected.len(), 4);
    assert!(
        keys.nodes
            .values()
            .any(|n| keys.label(n) == "4 / 4 selected")
    );
    assert!(
        keys.nodes
            .values()
            .any(|n| keys.label(n) == "No repositories match. Clear the filter to see all.")
    );
    keys.activate(&mut app, "Select none");
    assert!(app.selected.is_empty());
}

#[test]
fn filter_and_matching_groups_fit_target_sizes_and_themes() {
    for appearance in [
        domain::Appearance::System,
        domain::Appearance::Dark,
        domain::Appearance::Light,
    ] {
        for (width, height) in [(780.0, 560.0), (1180.0, 820.0), (1600.0, 1000.0)] {
            let (_temp, mut app) = grouped_app();
            app.state.groups[0].name = "A long group name that must truncate".into();
            app.select_repositories(true);
            let ctx = egui::Context::default();
            theme::install(&ctx);
            ctx.set_theme(theme::preference(appearance));
            ctx.enable_accesskit();
            for query in ["", "beta", "no matching repository"] {
                app.repository_sections.query = query.into();
                let mut output = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::vec2(width, height),
                        )),
                        ..Default::default()
                    },
                    |ui| {
                        egui::Panel::left("editor")
                            .exact_size(310.0)
                            .frame(
                                egui::Frame::side_top_panel(ui.style())
                                    .inner_margin(theme::PANEL_MARGIN),
                            )
                            .show(ui, |ui| {
                                let available = ui.available_width();
                                let response = egui::ScrollArea::vertical()
                                    .show(ui, |ui| app.repositories_section(ui, &ctx));
                                assert!(
                                    response.content_size.x <= available + 1.0,
                                    "{appearance:?} {width}x{height} {query}"
                                );
                            });
                        egui::CentralPanel::default().show(ui, |ui| app.results(ui, &ctx));
                    },
                );
                output.textures_delta.clear();
                let nodes = output.platform_output.accesskit_update.unwrap().nodes;
                for label in ["Filter repositories", "Clear"] {
                    let node = nodes
                        .iter()
                        .find(|(_, n)| n.label() == Some(label))
                        .unwrap();
                    let bounds = node.1.bounds().unwrap();
                    assert!(
                        bounds.x0 >= 0.0 && bounds.x1 <= 310.0 && bounds.y1 <= f64::from(height),
                        "{appearance:?} {width}x{height}: {label} {bounds:?}"
                    );
                }
                let occurrences = nodes
                    .iter()
                    .filter(|(_, n)| n.label() == Some("beta"))
                    .count();
                assert_eq!(occurrences, if query == "beta" { 2 } else { 0 });
                assert_eq!(app.selected.len(), 3);
            }
        }
    }
}
