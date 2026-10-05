//! Deterministic admission policy. No I/O or tasks: all slots and path leases are
//! reserved together, so a repository waiter never consumes a concurrency slot.
use crate::domain::QueueReason;
use std::{
    collections::{BTreeMap, VecDeque},
    path::{Path, PathBuf},
};

pub(super) type Key = (u64, usize);
struct Convoy {
    id: u64,
    limit: usize,
    running: usize,
    pending: VecDeque<(usize, PathBuf)>,
}
pub(super) struct Schedule {
    pub limit: usize,
    convoys: VecDeque<Convoy>,
    active: BTreeMap<Key, PathBuf>,
    quarantined: Vec<(u64, PathBuf)>,
}
impl Schedule {
    pub fn new(limit: usize) -> Self {
        Self {
            limit,
            convoys: VecDeque::new(),
            active: BTreeMap::new(),
            quarantined: Vec::new(),
        }
    }
    pub fn insert(&mut self, id: u64, limit: usize, paths: impl Iterator<Item = PathBuf>) {
        self.convoys.push_back(Convoy {
            id,
            limit,
            running: 0,
            pending: paths.enumerate().collect(),
        });
    }
    fn repository_wait(&self, path: &Path) -> Option<QueueReason> {
        let overlaps = |other: &Path| path.starts_with(other) || other.starts_with(path);
        self.quarantined
            .iter()
            .find(|(_, p)| overlaps(p))
            .map(|(run, _)| QueueReason::CleanupFailed(*run))
            .or_else(|| {
                self.active
                    .iter()
                    .find(|(_, p)| overlaps(p))
                    .map(|((run, _), _)| QueueReason::Repository(*run))
            })
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
        s.insert(run, limit, paths.iter().map(PathBuf::from));
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
