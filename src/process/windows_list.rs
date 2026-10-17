//! Retry changing Windows job membership without accepting a partial snapshot.
use std::io::{Error, Result};

pub(super) const CAPACITY: usize = 4096;
const ATTEMPTS: usize = 100;

pub(super) struct Snapshot {
    pub(super) assigned: u32,
    pub(super) ids: Vec<usize>,
}

pub(super) fn query(
    mut read: impl FnMut() -> Result<Snapshot>,
    mut progress: impl FnMut(),
) -> Result<Vec<usize>> {
    let mut last = (0, 0);
    for attempt in 0..ATTEMPTS {
        let snapshot = read()?;
        let count = snapshot.ids.len();
        if snapshot.assigned as usize > CAPACITY || count > CAPACITY {
            return Err(Error::other(format!(
                "Windows job process list exceeded its {CAPACITY}-member cleanup bound (assigned {}, listed {count}).",
                snapshot.assigned
            )));
        }
        if snapshot.assigned as usize == count {
            return Ok(snapshot.ids);
        }
        last = (snapshot.assigned, count);
        if attempt + 1 < ATTEMPTS {
            progress();
        }
    }
    Err(Error::other(format!(
        "Windows job member list did not stabilize after {ATTEMPTS} queries (assigned {}, listed {}).",
        last.0, last.1
    )))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    #[test]
    fn changing_membership_requeries_and_returns_only_the_complete_snapshot() {
        let mut responses = [
            Snapshot {
                assigned: 2,
                ids: vec![41],
            },
            Snapshot {
                assigned: 1,
                ids: vec![42],
            },
        ]
        .into_iter();
        let mut waits = 0;
        let ids = query(|| Ok(responses.next().unwrap()), || waits += 1).unwrap();
        assert_eq!(ids, [42]);
        assert_eq!(waits, 1);
        assert!(responses.next().is_none());
    }

    #[test]
    fn persistent_incompleteness_fails_closed_after_bounded_retries() {
        let mut reads = 0;
        let mut waits = 0;
        let result = query(
            || {
                reads += 1;
                Ok(Snapshot {
                    assigned: 2,
                    ids: vec![41],
                })
            },
            || waits += 1,
        );
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("did not stabilize")
        );
        assert_eq!(reads, ATTEMPTS);
        assert_eq!(waits, ATTEMPTS - 1);
    }

    #[test]
    fn oversized_lists_and_query_failures_are_not_retried_or_accepted() {
        for (assigned, ids) in [(CAPACITY as u32 + 1, vec![]), (0, vec![41; CAPACITY + 1])] {
            let mut waits = 0;
            let result = query(
                || {
                    Ok(Snapshot {
                        assigned,
                        ids: ids.clone(),
                    })
                },
                || waits += 1,
            );
            assert!(result.unwrap_err().to_string().contains("cleanup bound"));
            assert_eq!(waits, 0);
        }
        let mut waits = 0;
        let result = query(|| Err(Error::other("native query failed")), || waits += 1);
        assert_eq!(result.unwrap_err().to_string(), "native query failed");
        assert_eq!(waits, 0);
    }
}
