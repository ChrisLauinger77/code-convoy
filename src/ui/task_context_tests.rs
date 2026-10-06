//! Keyboard and draft lifecycle checks without a window, native dialog or agent.
use super::*;
use egui::accesskit::{Node, NodeId};

struct Keyboard {
    ctx: egui::Context,
    nodes: HashMap<NodeId, Node>,
    surface: fn(&mut App, &mut egui::Ui, &egui::Context),
}

impl Keyboard {
    fn label(&self, node: &Node) -> String {
        node.label()
            .filter(|label| !label.is_empty())
            .or_else(|| {
                (node.role() == egui::accesskit::Role::Label)
                    .then(|| node.value())
                    .flatten()
            })
            .map(str::to_owned)
            .unwrap_or_else(|| {
                let label = node
                    .labelled_by()
                    .iter()
                    .filter_map(|id| {
                        self.nodes
                            .get(id)
                            .and_then(|node| node.label().or_else(|| node.value()))
                    })
                    .collect::<Vec<_>>()
                    .join(" ");
                if label.is_empty() && node.role() == egui::accesskit::Role::ComboBox {
                    node.value().unwrap_or_default().to_owned()
                } else {
                    label
                }
            })
    }
    fn new(surface: fn(&mut App, &mut egui::Ui, &egui::Context)) -> Self {
        let ctx = egui::Context::default();
        theme::install(&ctx);
        ctx.enable_accesskit();
        Self {
            ctx,
            nodes: HashMap::new(),
            surface,
        }
    }

    fn frame(&mut self, app: &mut App, events: Vec<egui::Event>) {
        self.input(
            app,
            egui::RawInput {
                events,
                ..Default::default()
            },
        );
    }

    fn input(&mut self, app: &mut App, input: egui::RawInput) {
        let mut output = self.ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(700.0, 1600.0),
                )),
                ..input
            },
            |ui| {
                egui::CentralPanel::default().show(ui, |ui| (self.surface)(app, ui, &self.ctx));
                app.library_window(&self.ctx);
            },
        );
        output.textures_delta.clear();
        if let Some(update) = output.platform_output.accesskit_update {
            self.nodes.extend(update.nodes);
        }
    }

    fn key(&mut self, app: &mut App, key: egui::Key) {
        self.frame(
            app,
            [true, false]
                .into_iter()
                .map(|pressed| egui::Event::Key {
                    key,
                    physical_key: None,
                    pressed,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                })
                .collect(),
        );
        // egui applies wrapped Tab/Shift-Tab focus on the following frame.
        self.frame(app, vec![]);
    }

    fn focus(&mut self, app: &mut App, label: &str) {
        let mut visited = Vec::new();
        for _ in 0..120 {
            let focused = self.ctx.memory(|m| m.focused());
            if let Some(node) = focused.and_then(|id| self.nodes.get(&id.accesskit_id())) {
                let name = self.label(node);
                if name == label {
                    return;
                }
                visited.push(name);
            }
            self.key(app, egui::Key::Tab);
        }
        panic!("Tab could not reach {label:?}; visited {visited:?}");
    }

    fn activate(&mut self, app: &mut App, label: &str) {
        self.focus(app, label);
        self.key(app, egui::Key::Enter);
        self.frame(app, vec![]);
    }

    fn replace_text(&mut self, app: &mut App, label: &str, text: &str) {
        self.focus(app, label);
        self.frame(
            app,
            vec![
                egui::Event::Key {
                    key: egui::Key::A,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::COMMAND,
                },
                egui::Event::Key {
                    key: egui::Key::A,
                    physical_key: None,
                    pressed: false,
                    repeat: false,
                    modifiers: egui::Modifiers::COMMAND,
                },
                egui::Event::Text(text.into()),
            ],
        );
    }
}

#[test]
fn templates_and_groups_are_operable_with_tab_and_enter() {
    let (_temp, mut app) = app();
    app.state.repositories = vec![repository("alpha"), repository("beta")];
    app.state.templates.push(domain::TaskTemplate {
        name: "Review".into(),
        prompt: "Review README".into(),
    });
    app.state.groups.push(domain::RepositoryGroup {
        name: "Both".into(),
        repositories: app
            .state
            .repositories
            .iter()
            .map(|r| r.path.clone())
            .collect(),
    });
    let mut keys = Keyboard::new(|app, ui, _| {
        app.template_menu(ui);
        app.groups_section(ui);
    });
    keys.frame(&mut app, vec![]);
    keys.activate(&mut app, "Templates");
    keys.activate(&mut app, "Load template Review");
    assert_eq!(app.state.draft.prompt, "Review README");
    assert!(app.manager.is_idle());
    keys.activate(&mut app, "Both · 2 repos");
    assert_eq!(app.selected.len(), 2);
    keys.activate(&mut app, "Manage groups…");
    keys.focus(&mut app, "Save group");
    keys.key(&mut app, egui::Key::Enter);
    assert!(
        matches!(&app.library_editor, Some(library::LibraryEditor::Group {error, ..}) if !error.is_empty())
    );
    keys.replace_text(&mut app, "Name", "Created group");
    keys.activate(&mut app, "alpha");
    keys.activate(&mut app, "Save group");
    assert_eq!(app.state.groups[1].name, "Created group");
    assert_eq!(
        app.state.groups[1].repositories,
        [app.state.repositories[0].path.clone()]
    );
    assert!(app.library_editor.is_none());
    keys.activate(&mut app, "Manage groups…");
    keys.activate(&mut app, "Group");
    keys.activate(&mut app, "Created group");
    keys.replace_text(&mut app, "Name", "Renamed group");
    keys.activate(&mut app, "beta");
    keys.activate(&mut app, "Save group");
    assert_eq!(app.state.groups[1].name, "Renamed group");
    assert_eq!(app.state.groups[1].repositories.len(), 2);
    keys.activate(&mut app, "Manage groups…");
    keys.activate(&mut app, "Group");
    keys.activate(&mut app, "Renamed group");
    keys.activate(&mut app, "Delete group");
    assert_eq!(app.state.groups.len(), 1);
    assert_eq!(app.selected.len(), 2);
    keys.activate(&mut app, "Templates");
    keys.activate(&mut app, "Edit template Review");
    keys.replace_text(&mut app, "Name", "Renamed review");
    keys.replace_text(&mut app, "Task text", "Updated template text");
    keys.activate(&mut app, "Save template");
    assert_eq!(app.state.templates[0].name, "Renamed review");
    assert_eq!(app.state.templates[0].prompt, "Updated template text");
    assert_eq!(app.state.draft.prompt, "Review README");
    keys.activate(&mut app, "Templates");
    keys.activate(&mut app, "Edit template Renamed review");
    keys.activate(&mut app, "Delete template");
    assert!(app.state.templates.is_empty());
    assert_eq!(app.state.draft.prompt, "Review README");
    assert_eq!(app.state.repositories.len(), 2);
    keys.activate(&mut app, "Templates");
    keys.activate(&mut app, "Save current task as template…");
    keys.replace_text(&mut app, "Name", "Saved task");
    keys.activate(&mut app, "Save template");
    assert_eq!(app.state.templates[0].name, "Saved task");
    assert_eq!(app.state.templates[0].prompt, "Review README");
}

#[test]
fn attachment_removal_and_result_tabs_are_keyboard_accessible() {
    let (temp, mut app) = app();
    let path = temp.path().join("context Grüße.md");
    std::fs::write(&path, "context").unwrap();
    app.state
        .draft
        .attachments
        .push(crate::attachments::Attachment::inspect(&path).unwrap());
    let mut keys = Keyboard::new(|app, ui, ctx| app.attachments_section(ui, ctx));
    keys.frame(&mut app, vec![]);
    keys.focus(&mut app, "Add files…"); // Do not open a real OS picker in a headless test.
    keys.activate(&mut app, "Remove attachment context Grüße.md");
    assert!(app.state.draft.attachments.is_empty());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "context");
    app.state.runs.push(run(20, &[JobStatus::Succeeded]));
    app.state.runs.push(run(21, &[JobStatus::Succeeded]));
    app.select_run(Some(20));
    let mut keys = Keyboard::new(|app, ui, ctx| app.results(ui, ctx));
    keys.frame(&mut app, vec![]);
    keys.activate(&mut app, "Raw output");
    assert!(app.tab == Tab::Raw);
    keys.activate(&mut app, "Activity");
    assert!(app.tab == Tab::Activity);
    keys.activate(
        &mut app,
        "History · #20 · Codex · 1 repos · Succeeded · 1/1",
    );
    keys.activate(
        &mut app,
        "#21 · Codex · 1 repos · Succeeded · 1/1 · 0:00\nTask 21",
    );
    assert_eq!(app.selected_run, Some(21));
    keys.activate(&mut app, "Reuse convoy");
    assert_eq!(app.state.draft.prompt, app.state.runs[1].task.prompt);
    assert!(app.manager.is_idle());
}

#[test]
fn picker_cancellation_preserves_the_draft_and_returns_keyboard_focus() {
    let (temp, mut app) = app();
    let path = temp.path().join("existing.txt");
    std::fs::write(&path, "existing context").unwrap();
    app.state
        .draft
        .attachments
        .push(crate::attachments::Attachment::inspect(&path).unwrap());
    app.state.draft.prompt = "Keep my task".into();
    let before = serde_json::to_value(&app.state).unwrap();
    // Native cancellation resolves to the same empty completion as rfd's None.
    let request = app.attachment_work.begin_picker();
    app.attachments_added(request, vec![], vec![]);
    assert!(!app.attachment_work.pending);
    assert_eq!(serde_json::to_value(&app.state).unwrap(), before);
    assert!(app.notice.is_empty());
    let mut keys = Keyboard::new(|app, ui, ctx| app.attachments_section(ui, ctx));
    keys.frame(&mut app, vec![]);
    let focused = keys.ctx.memory(|m| m.focused()).unwrap();
    assert_eq!(
        keys.label(&keys.nodes[&focused.accesskit_id()]),
        "Add files…"
    );
    assert!(app.manager.is_idle());
}

#[derive(Debug)]
struct DroppedPath(PathBuf);
impl egui::DroppedFile for DroppedPath {
    fn path(&self) -> &std::path::Path {
        &self.0
    }
    fn bytes(&self) -> Result<Vec<u8>, String> {
        panic!("The UI must not read file contents")
    }
}

fn drop_input(paths: &[PathBuf], position: egui::Pos2) -> egui::RawInput {
    egui::RawInput {
        events: vec![egui::Event::PointerMoved(position)],
        dropped_files: paths
            .iter()
            .map(|path| std::sync::Arc::new(DroppedPath(path.clone())) as egui::DroppedFileHandle)
            .collect(),
        ..Default::default()
    }
}

fn finish_attachment_work(app: &mut App) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while app.attachment_work.pending {
        assert!(
            Instant::now() < deadline,
            "Attachment inspection did not finish"
        );
        app.runtime.as_ref().unwrap().block_on(async {
            tokio::time::sleep(Duration::from_millis(2)).await;
        });
        app.poll();
    }
}

#[test]
fn reused_missing_context_survives_save_and_unrelated_add_until_explicit_removal() {
    let (temp, mut app) = app();
    let missing = temp.path().join("requirements.md");
    let other = temp.path().join("other.md");
    std::fs::write(&missing, "original context").unwrap();
    std::fs::write(&other, "additional context").unwrap();
    let attachment = crate::attachments::Attachment::inspect(&missing).unwrap();
    let mut history = run(20, &[JobStatus::Succeeded]);
    history.task.attachments.push(attachment.clone());
    app.state.runs.push(history);
    std::fs::remove_file(&missing).unwrap();
    app.reuse_convoy(20);
    finish_attachment_work(&mut app);
    assert!(app.attachment_error().is_some());
    app.store.save(&app.state).unwrap();
    let saved = app.store.load().unwrap();
    assert_eq!(saved.draft.attachments, std::slice::from_ref(&attachment));
    // Adding valid context cannot silently resolve or replace a missing reference.
    let mut keys = Keyboard::new(|app, ui, ctx| app.attachments_section(ui, ctx));
    keys.frame(&mut app, vec![]);
    keys.input(&mut app, drop_input(&[other], egui::pos2(30.0, 20.0)));
    finish_attachment_work(&mut app);
    assert_eq!(app.state.draft.attachments.len(), 2);
    assert!(app.attachment_error().is_some());
    keys.activate(&mut app, "Remove attachment requirements.md");
    assert!(app.attachment_error().is_none());
    assert!(app.notice.is_empty());
    assert!(app.draft_message.contains("Review the remaining context"));
    assert_eq!(app.state.draft.attachments.len(), 1);
    assert_eq!(app.state.runs[0].task.attachments, [attachment]);
    assert!(app.manager.is_idle());
}

#[test]
fn dropped_paths_are_inspected_off_thread_deduplicated_and_errors_are_explicit() {
    let (temp, mut app) = app();
    let text = temp.path().join("external Grüße specification.md");
    let image = temp.path().join("image with spaces.png");
    let unsupported = temp.path().join("unsupported.pdf");
    std::fs::write(&text, "external context").unwrap();
    std::fs::write(&image, include_bytes!("../../assets/codeconvoy-256.png")).unwrap();
    std::fs::write(&unsupported, "unsupported").unwrap();
    let mut keys = Keyboard::new(|app, ui, ctx| app.attachments_section(ui, ctx));
    keys.frame(&mut app, vec![]);
    // A drop outside the attachment area must not change the task.
    keys.input(
        &mut app,
        drop_input(std::slice::from_ref(&text), egui::pos2(600.0, 1000.0)),
    );
    assert!(!app.attachment_work.pending);
    assert!(app.state.draft.attachments.is_empty());
    keys.input(
        &mut app,
        drop_input(std::slice::from_ref(&text), egui::pos2(30.0, 20.0)),
    );
    assert!(app.attachment_work.pending);
    assert!(app.manager.is_idle()); // File inspection holds no execution capacity.
    finish_attachment_work(&mut app);
    assert_eq!(app.state.draft.attachments.len(), 1);
    keys.frame(&mut app, vec![]);
    assert!(keys.ctx.memory(|memory| memory.focused().is_none())); // A background drop must not steal keyboard focus.
    keys.input(
        &mut app,
        drop_input(
            &[text.clone(), image.clone(), unsupported],
            egui::pos2(30.0, 20.0),
        ),
    );
    finish_attachment_work(&mut app);
    assert_eq!(app.state.draft.attachments.len(), 2);
    assert!(
        app.notice.contains("Unsupported attachment type")
            && app.notice.contains("unsupported.pdf")
    );
    assert_eq!(
        app.state.draft.attachments[0].path,
        text.canonicalize().unwrap()
    );
    assert_eq!(
        app.state.draft.attachments[1].path,
        image.canonicalize().unwrap()
    );
    app.closing = true;
    keys.input(&mut app, drop_input(&[text], egui::pos2(30.0, 20.0)));
    assert!(!app.attachment_work.pending);
}

#[test]
fn history_renders_missing_attachment_metadata_and_durations_without_serialization_details() {
    let (temp, mut app) = app();
    let path = temp.path().join("historical.md");
    std::fs::write(&path, "private file contents").unwrap();
    let mut history = run(20, &[JobStatus::Succeeded]);
    history.created_at = 1700000000;
    history.jobs[0].started_at = Some(1700000001);
    history.jobs[0].finished_at = Some(1700000061);
    history
        .task
        .attachments
        .push(crate::attachments::Attachment::inspect(&path).unwrap());
    history.task.options.insert(
        "unrecognized_serialization_key".into(),
        "hidden_value".into(),
    );
    app.state.runs.push(history);
    std::fs::remove_file(path).unwrap();
    let mut keys = Keyboard::new(|app, ui, _| {
        snapshot::show(ui, &app.state.runs[0], &app.state.runs[0].jobs[0])
    });
    keys.frame(&mut app, vec![]);
    let text = keys
        .nodes
        .values()
        .map(|node| keys.label(node))
        .collect::<Vec<_>>()
        .join("\n");
    for expected in [
        "historical.md",
        "Markdown",
        "21 B",
        "Files may have changed or disappeared",
        "Convoy duration: 1:01",
        "Selected job duration: 1:00",
        "Repositories (1)",
    ] {
        assert!(text.contains(expected), "Missing {expected}: {text}");
    }
    for hidden in [
        "private file contents",
        "sha256",
        "unrecognized_serialization_key",
        "hidden_value",
    ] {
        assert!(!text.contains(hidden));
    }
}

#[test]
fn individual_repository_selection_and_launch_review_are_keyboard_accessible() {
    let (_temp, mut app) = app();
    complete_cli_checks(&mut app);
    app.state.repositories = vec![repository("alpha")];
    app.state.draft.prompt = "Review README".into();
    let mut keys = Keyboard::new(|app, ui, ctx| app.editor_pane(ui, ctx));
    keys.frame(&mut app, vec![]);
    keys.activate(&mut app, "alpha");
    assert_eq!(
        app.selected,
        [app.state.repositories[0].path.clone()].into()
    );
    keys.activate(&mut app, "Run Convoy");
    assert!(app.busy); // Preflight requested; no direct execution or safety bypass.
    assert!(app.prepared.is_none());
    assert!(app.manager.is_idle());
}
