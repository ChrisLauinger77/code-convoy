//! Session-only identities and cached membership for the repository picker.
use super::*;
use domain::RepositoryGroup;

#[derive(Default)]
pub(super) struct RepositorySections {
    repositories: Vec<Repository>,
    groups: Vec<RepositoryGroup>,
    next_id: u64,
    pub sections: Vec<Section>,
}

pub(super) struct Section {
    pub id: egui::Id,
    pub name: String,
    pub group: Option<usize>,
    pub repositories: Vec<usize>,
    pub unregistered: Vec<PathBuf>,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct Counts {
    pub total: usize,
    pub selected: usize,
    pub available: usize,
    pub selected_available: usize,
    pub unavailable: usize,
}

impl Section {
    pub fn counts(&self, app: &App) -> Counts {
        let mut counts = Counts {
            total: self.repositories.len() + self.unregistered.len(),
            unavailable: self.unregistered.len(),
            ..Default::default()
        };
        for &index in &self.repositories {
            let path = &app.state.repositories[index].path;
            let selected = app.selected.contains(path);
            counts.selected += usize::from(selected);
            if matches!(app.repository_states.get(path), Some(Err(_))) {
                counts.unavailable += 1;
            } else {
                counts.available += 1;
                counts.selected_available += usize::from(selected);
            }
        }
        counts
    }
}

impl RepositorySections {
    pub fn sync(&mut self, repositories: &[Repository], groups: &[RepositoryGroup]) {
        // Selection, Git refreshes and expansion never rebuild membership. The
        // borrowed equality check also catches registration/reuse/library edits.
        if !self.sections.is_empty() && self.repositories == repositories && self.groups == groups {
            return;
        }
        let registered: HashMap<_, _> = repositories
            .iter()
            .enumerate()
            .map(|(index, repository)| (&repository.path, index))
            .collect();
        let mut assigned = HashSet::new();
        let mut sections = vec![Section {
            id: egui::Id::new("ungrouped_repositories"),
            name: "Ungrouped".into(),
            group: None,
            repositories: Vec::new(),
            unregistered: Vec::new(),
        }];
        for (index, group) in groups.iter().enumerate() {
            // Names are unique in the existing library. The editor carries the
            // session ID across renames; vector positions and counts are not IDs.
            let id = self
                .sections
                .iter()
                .find(|section| section.group.is_some() && section.name == group.name)
                .map(|section| section.id)
                .unwrap_or_else(|| {
                    self.next_id += 1;
                    egui::Id::new(("repository_group", self.next_id))
                });
            let mut section = Section {
                id,
                name: group.name.clone(),
                group: Some(index),
                repositories: Vec::new(),
                unregistered: Vec::new(),
            };
            let mut seen = HashSet::new();
            for path in &group.repositories {
                if !seen.insert(path) {
                    continue;
                }
                if let Some(&index) = registered.get(path) {
                    assigned.insert(index);
                    section.repositories.push(index);
                } else {
                    section.unregistered.push(path.clone());
                }
            }
            sections.push(section);
        }
        sections[0].repositories = (0..repositories.len())
            .filter(|index| !assigned.contains(index))
            .collect();
        self.repositories = repositories.to_vec();
        self.groups = groups.to_vec();
        self.sections = sections;
    }

    pub fn rename(&mut self, index: usize, name: &str) {
        if let Some(section) = self.sections.iter_mut().find(|s| s.group == Some(index)) {
            section.name = name.to_owned();
        }
    }
}
