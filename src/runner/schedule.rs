//! Deterministic admission policy. No I/O or tasks: all slots and path leases are
//! reserved together, so a repository waiter never consumes a concurrency slot.
use crate::domain::{ExecutionMode, QueueReason};
use std::{
    collections::{BTreeMap, VecDeque},
    path::PathBuf,
};

pub(super) type Key = (u64, usize);
#[derive(Clone)]
pub(super) struct Lease {
    pub path: PathBuf,
    pub common_dir: Option<PathBuf>,
    pub mode: ExecutionMode,
}
impl Lease {
    fn conflicts(&self, other: &Self) -> bool {
        let same_git = self.common_dir.is_some() && self.common_dir == other.common_dir;
        let overlap = self.path.starts_with(&other.path) || other.path.starts_with(&self.path);
        if self.mode == ExecutionMode::Direct || other.mode == ExecutionMode::Direct {
            same_git || overlap
        } else {
            // Separate isolated checkouts of one Git repository may execute together.
            // Real nested repositories still conflict, even with different Git dirs.
            overlap && !same_git
        }
    }
}

struct Convoy {
    id: u64,
    limit: usize,
    running: usize,
    pending: VecDeque<(usize, Lease)>,
}
pub(super) struct Schedule {
    pub limit: usize,
    convoys: VecDeque<Convoy>,
    active: BTreeMap<Key, Lease>,
    quarantined: Vec<(u64, Lease)>,
    maintenance: BTreeMap<u64, Lease>,
}
impl Schedule {
    pub fn new(limit: usize) -> Self {
        Self {
            limit,
            convoys: VecDeque::new(),
            active: BTreeMap::new(),
            quarantined: Vec::new(),
            maintenance: BTreeMap::new(),
        }
    }
    pub fn insert(&mut self, id: u64, limit: usize, paths: impl Iterator<Item = Lease>) {
        self.convoys.push_back(Convoy {
            id,
            limit,
            running: 0,
            pending: paths.enumerate().collect(),
        });
    }
    fn repository_wait(&self, path: &Lease) -> Option<QueueReason> {
        let overlaps = |other: &Lease| path.conflicts(other);
        self.quarantined
            .iter()
            .find(|(_, p)| {
                let mut exclusive = p.clone();
                exclusive.mode = ExecutionMode::Direct;
                overlaps(&exclusive)
            })
            .map(|(run, _)| QueueReason::CleanupFailed(*run))
            .or_else(|| {
                self.maintenance
                    .values()
                    .any(overlaps)
                    .then_some(QueueReason::Maintenance)
            })
            .or_else(|| {
                self.active
                    .iter()
                    .find(|(_, p)| overlaps(p))
                    .map(|((run, _), _)| QueueReason::Repository(*run))
            })
    }
    pub fn begin_maintenance(&mut self, id: u64, key: Key, lease: Lease, cleanup: bool) -> bool {
        if self.maintenance.values().any(|p| lease.conflicts(p))
            || self.quarantined.iter().any(|(_, p)| lease.conflicts(p))
            || self
                .active
                .values()
                .any(|p| lease.conflicts(p) && (cleanup || p.mode == ExecutionMode::Direct))
            || (cleanup
                && self
                    .convoys
                    .iter()
                    .any(|c| c.id == key.0 && c.pending.iter().any(|(j, _)| *j == key.1)))
        {
            return false;
        }
        self.maintenance.insert(id, lease);
        true
    }
    pub fn end_maintenance(&mut self, id: u64, safe: bool) {
        if let Some(lease) = self.maintenance.remove(&id)
            && !safe
        {
            self.quarantined.push((0, lease));
        }
    }
    pub fn next(&mut self) -> Option<Key> {
        if self.active.len() + self.quarantined.len() >= self.limit {
            return None;
        }
        // One admission per turn, then rotate. Skip blocked repositories within
        // each convoy so an unrelated job can use otherwise idle capacity.
        for _ in 0..self.convoys.len() {
            let mut convoy = self.convoys.pop_front()?;
            let unknown = self
                .quarantined
                .iter()
                .filter(|(run, _)| *run == convoy.id)
                .count();
            let candidate = (convoy.running + unknown < convoy.limit)
                .then(|| {
                    convoy
                        .pending
                        .iter()
                        .position(|(_, p)| self.repository_wait(p).is_none())
                })
                .flatten();
            let key = candidate
                .and_then(|i| convoy.pending.remove(i))
                .map(|(job, path)| {
                    convoy.running += 1;
                    let key = (convoy.id, job);
                    self.active.insert(key, path);
                    key
                });
            self.convoys.push_back(convoy);
            if key.is_some() {
                return key;
            }
        }
        None
    }
    pub fn waiting(&self) -> Vec<(Key, QueueReason)> {
        self.convoys
            .iter()
            .flat_map(|c| {
                c.pending.iter().map(|(job, path)| {
                    let reason = self.repository_wait(path).unwrap_or({
                        if c.running
                            + self
                                .quarantined
                                .iter()
                                .filter(|(run, _)| *run == c.id)
                                .count()
                            >= c.limit
                        {
                            QueueReason::ConvoyLimit
                        } else {
                            QueueReason::GlobalLimit
                        }
                    });
                    ((c.id, *job), reason)
                })
            })
            .collect()
    }
    pub fn cancel_pending(&mut self, key: Key) {
        if let Some(c) = self.convoys.iter_mut().find(|c| c.id == key.0) {
            c.pending.retain(|(job, _)| *job != key.1);
        }
        self.prune();
    }
    pub fn finish(&mut self, key: Key, repository_safe: bool) {
        if let Some(path) = self.active.remove(&key) {
            if !repository_safe {
                self.quarantined.push((key.0, path));
            }
            if let Some(c) = self.convoys.iter_mut().find(|c| c.id == key.0) {
                c.running -= 1;
            }
        }
        self.prune();
    }
    fn prune(&mut self) {
        self.convoys
            .retain(|c| c.running > 0 || !c.pending.is_empty());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn add(s: &mut Schedule, run: u64, limit: usize, paths: &[&str]) {
        s.insert(
            run,
            limit,
            paths.iter().map(|p| Lease {
                path: PathBuf::from(p),
                common_dir: None,
                mode: ExecutionMode::Direct,
            }),
        );
    }
    #[test]
    fn maintenance_is_exclusive_per_repository_without_consuming_agent_slots() {
        let mut s = Schedule::new(1);
        let lease = Lease {
            path: "/source".into(),
            common_dir: Some("/source/.git".into()),
            mode: ExecutionMode::Direct,
        };
        assert!(s.begin_maintenance(1, (100, 0), lease.clone(), true));
        assert!(!s.begin_maintenance(2, (101, 0), lease.clone(), false));
        s.insert(1, 1, [lease.clone()].into_iter());
        add(&mut s, 2, 1, &["/independent"]);
        assert!(s.waiting().contains(&((1, 0), QueueReason::Maintenance)));
        assert_eq!(s.next(), Some((2, 0)));
        s.finish((2, 0), true);
        s.end_maintenance(1, true);
        assert_eq!(s.next(), Some((1, 0)));
        assert!(!s.begin_maintenance(3, (100, 0), lease.clone(), true));
        s.finish((1, 0), true);
        assert!(s.begin_maintenance(4, (100, 0), lease.clone(), true));
        s.end_maintenance(4, false);
        assert!(!s.begin_maintenance(5, (100, 0), lease, true));
    }
    #[test]
    fn isolated_leases_share_git_but_preserve_nested_and_exclusive_access() {
        let lease = |path: &str, common: &str, mode| Lease {
            path: path.into(),
            common_dir: Some(common.into()),
            mode,
        };
        let a = lease("/source", "/source/.git", ExecutionMode::IsolatedWorktree);
        let b = lease(
            "/retained/tree",
            "/source/.git",
            ExecutionMode::IsolatedWorktree,
        );
        assert!(!a.conflicts(&b));
        assert!(!a.conflicts(&a));
        assert!(a.conflicts(&lease(
            "/source/nested",
            "/source/nested/.git",
            ExecutionMode::IsolatedWorktree
        )));
        assert!(a.conflicts(&lease(
            "/retained/tree",
            "/source/.git",
            ExecutionMode::Direct
        )));
        let mut s = Schedule::new(3);
        s.insert(1, 1, [a.clone()].into_iter());
        s.insert(2, 1, [b.clone()].into_iter());
        assert_eq!(s.next(), Some((1, 0)));
        assert_eq!(s.next(), Some((2, 0)));
        s.finish((1, 0), false);
        s.insert(3, 1, [b].into_iter());
        assert!(
            s.waiting()
                .contains(&((3, 0), QueueReason::CleanupFailed(1)))
        );
        assert_eq!(s.next(), None);
    }

    #[test]
    fn round_robin_enforces_both_limits_and_wakes_queued_jobs() {
        let mut s = Schedule::new(3);
        add(&mut s, 1, 2, &["/a", "/b", "/c", "/d"]);
        add(&mut s, 2, 2, &["/e", "/f", "/g"]);
        assert_eq!(s.next(), Some((1, 0)));
        assert_eq!(s.next(), Some((2, 0)));
        assert_eq!(s.next(), Some((1, 1)));
        assert_eq!(s.next(), None);
        s.finish((1, 0), true);
        assert_eq!(s.next(), Some((2, 1)));
        assert_eq!(s.next(), None);
        s.limit = 1; // Existing jobs finish naturally; no new admission yet.
        s.finish((1, 1), true);
        assert_eq!(s.next(), None);
        s.finish((2, 0), true);
        assert_eq!(s.next(), None);
        s.finish((2, 1), true);
        assert_eq!(s.next(), Some((1, 2)));
        s.limit = 4;
        assert_eq!(s.next(), Some((2, 2)));
    }
    #[test]
    fn repository_waiters_do_not_block_other_work_and_leases_are_released() {
        let mut s = Schedule::new(4);
        add(&mut s, 1, 1, &["/repo"]);
        add(&mut s, 2, 2, &["/repo", "/other"]);
        add(&mut s, 3, 1, &["/repo/nested"]);
        assert_eq!(s.next(), Some((1, 0)));
        assert_eq!(s.next(), Some((2, 1)));
        assert_eq!(s.next(), None);
        assert!(s.waiting().contains(&((2, 0), QueueReason::Repository(1))));
        s.finish((1, 0), true);
        assert_eq!(s.next(), Some((3, 0)));
        s.finish((3, 0), true);
        assert_eq!(s.next(), Some((2, 0)));
    }
    #[test]
    fn queued_cancellation_and_unconfirmed_cleanup_are_safe() {
        let mut s = Schedule::new(2);
        add(&mut s, 1, 1, &["/repo", "/unused"]);
        add(&mut s, 2, 1, &["/repo", "/other"]);
        assert_eq!(s.next(), Some((1, 0)));
        s.cancel_pending((1, 1));
        s.finish((1, 0), false);
        assert!(
            s.waiting()
                .contains(&((2, 0), QueueReason::CleanupFailed(1)))
        );
        assert_eq!(s.next(), Some((2, 1)));
        s.finish((2, 1), true);
        assert_eq!(s.next(), None);
        s.cancel_pending((2, 0));
        assert!(s.convoys.is_empty());
    }
}
