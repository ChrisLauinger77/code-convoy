//! Records the process groups that a child's descendants move into. Some CLIs
//! (OpenCode 2) start helpers with `setsid`, outside the child's own group,
//! which `Group` signals and probes. Those groups are only ever probed.
use std::{collections::BTreeSet, io, time::Duration};
use tokio::time::Instant;

const SAMPLE: Duration = Duration::from_millis(200);
/// Beyond this many groups or descendants, tracking is reported as incomplete.
const LIMIT: usize = 1024;

#[derive(Default)]
pub(super) struct Groups {
    recorded: BTreeSet<libc::pid_t>,
    sampled: Option<Instant>,
    failure: Option<String>,
}

impl Groups {
    /// Records the groups of `root`'s current descendants, at most every
    /// `SAMPLE`. The caller samples only while `root` is unreaped, so a
    /// descendant's parent chain still leads to it.
    pub fn sample(&mut self, root: libc::pid_t) {
        if self.failure.is_some() || self.sampled.is_some_and(|at| at.elapsed() < SAMPLE) {
            return;
        }
        self.sampled = Some(Instant::now());
        match descendants(root) {
            Ok(found) => {
                // Members of root's own group are covered by Group itself.
                self.recorded.extend(
                    found
                        .into_iter()
                        .map(|(_, group)| group)
                        .filter(|&group| group > 1 && group != root),
                );
                if self.recorded.len() > LIMIT {
                    self.failure = Some(format!("more than {LIMIT} process groups"));
                }
            }
            Err(error) => self.failure = Some(error.to_string()),
        }
    }

    /// The groups to confirm gone, or an error when the record is incomplete.
    pub fn recorded(&self) -> io::Result<&BTreeSet<libc::pid_t>> {
        match &self.failure {
            Some(failure) => Err(io::Error::other(format!(
                "Cannot track the agent's detached processes: {failure}"
            ))),
            None => Ok(&self.recorded),
        }
    }
}

/// `(pid, process group)` of every current descendant of `root`.
#[cfg(target_os = "macos")]
fn descendants(root: libc::pid_t) -> io::Result<Vec<(libc::pid_t, libc::pid_t)>> {
    let mut found = Vec::new();
    let mut parents = vec![root];
    while let Some(parent) = parents.pop() {
        for child in children(parent)? {
            // A child that exited meanwhile has no group left to record.
            if let Some(group) = group_of(child)? {
                found.push((child, group));
                parents.push(child);
            }
            if found.len() > LIMIT {
                return Err(io::Error::other(format!("more than {LIMIT} descendants")));
            }
        }
    }
    Ok(found)
}

#[cfg(target_os = "macos")]
fn children(parent: libc::pid_t) -> io::Result<Vec<libc::pid_t>> {
    let mut buffer: Vec<libc::pid_t> = vec![0; 64];
    loop {
        let capacity = buffer.len();
        let bytes = libc::c_int::try_from(capacity * std::mem::size_of::<libc::pid_t>())
            .map_err(|_| io::Error::other("child process list is too large"))?;
        // SAFETY: the buffer holds `capacity` PIDs and its size is passed in
        // bytes. The call returns the number of PIDs written.
        let count = unsafe { libc::proc_listchildpids(parent, buffer.as_mut_ptr().cast(), bytes) };
        if count < 0 {
            let error = io::Error::last_os_error();
            return match error.raw_os_error() {
                Some(libc::ESRCH) => Ok(Vec::new()),
                _ => Err(error),
            };
        }
        let count = usize::try_from(count).unwrap_or(capacity);
        if count < capacity {
            buffer.truncate(count);
            buffer.retain(|&pid| pid > 0);
            return Ok(buffer);
        }
        // A full buffer may be truncated; list again with room to spare.
        if capacity > LIMIT {
            return Err(io::Error::other(format!(
                "more than {LIMIT} child processes"
            )));
        }
        buffer.resize(capacity * 2, 0);
    }
}

#[cfg(target_os = "macos")]
fn group_of(pid: libc::pid_t) -> io::Result<Option<libc::pid_t>> {
    // SAFETY: zero is a valid proc_bsdinfo initialization.
    let mut info: libc::proc_bsdinfo = unsafe { std::mem::zeroed() };
    let size = libc::c_int::try_from(std::mem::size_of::<libc::proc_bsdinfo>())
        .map_err(|_| io::Error::other("process information is too large"))?;
    // SAFETY: valid output buffer of exactly `size` bytes.
    let written = unsafe {
        libc::proc_pidinfo(
            pid,
            libc::PROC_PIDTBSDINFO,
            0,
            (&mut info as *mut libc::proc_bsdinfo).cast(),
            size,
        )
    };
    if written == size {
        return libc::pid_t::try_from(info.pbi_pgid)
            .map(Some)
            .map_err(|_| io::Error::other("invalid process group ID"));
    }
    let error = io::Error::last_os_error();
    match error.raw_os_error() {
        // Exited (or became a zombie) between listing and inspection.
        Some(libc::ESRCH) | Some(0) | None => Ok(None),
        _ => Err(error),
    }
}

/// `(pid, process group)` of every current descendant of `root`. Walks only
/// the tree below `root` through `/proc/<pid>/task/<tid>/children`; kernels
/// without those files fall back to one scan of all of `/proc`.
#[cfg(target_os = "linux")]
fn descendants(root: libc::pid_t) -> io::Result<Vec<(libc::pid_t, libc::pid_t)>> {
    static CHILDREN_FILES: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    let own = std::process::id();
    if !CHILDREN_FILES
        .get_or_init(|| std::path::Path::new(&format!("/proc/{own}/task/{own}/children")).exists())
    {
        return scan(root);
    }
    let mut found = Vec::new();
    let mut parents = vec![root];
    while let Some(parent) = parents.pop() {
        for child in children(parent)? {
            // A child that exited meanwhile has no group left to record.
            let stat = match std::fs::read_to_string(format!("/proc/{child}/stat")) {
                Ok(stat) => stat,
                Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
                Err(error) => return Err(error),
            };
            let (_, group) = parse_stat(&stat).ok_or_else(|| unreadable(child))?;
            found.push((child, group));
            parents.push(child);
            if found.len() > LIMIT {
                return Err(io::Error::other(format!("more than {LIMIT} descendants")));
            }
        }
    }
    Ok(found)
}

/// Children of every thread of `parent`; none once `parent` has exited.
#[cfg(target_os = "linux")]
fn children(parent: libc::pid_t) -> io::Result<Vec<libc::pid_t>> {
    let tasks = match std::fs::read_dir(format!("/proc/{parent}/task")) {
        Ok(tasks) => tasks,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error),
    };
    let mut children = Vec::new();
    for task in tasks {
        let task = match task {
            Ok(task) => task,
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error),
        };
        match std::fs::read_to_string(task.path().join("children")) {
            Ok(list) => children.extend(
                list.split_whitespace()
                    .filter_map(|pid| pid.parse::<libc::pid_t>().ok()),
            ),
            // The thread exited between listing and reading.
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
    }
    Ok(children)
}

/// The fallback: one pass over all of `/proc`.
#[cfg(target_os = "linux")]
fn scan(root: libc::pid_t) -> io::Result<Vec<(libc::pid_t, libc::pid_t)>> {
    let mut children: std::collections::HashMap<libc::pid_t, Vec<(libc::pid_t, libc::pid_t)>> =
        std::collections::HashMap::new();
    for entry in std::fs::read_dir("/proc")? {
        let entry = entry?;
        let Some(pid) = entry
            .file_name()
            .to_str()
            .and_then(|name| name.parse::<libc::pid_t>().ok())
        else {
            continue;
        };
        // A process that exited meanwhile has no stat file left.
        let Ok(stat) = std::fs::read_to_string(entry.path().join("stat")) else {
            continue;
        };
        let (parent, group) = parse_stat(&stat).ok_or_else(|| unreadable(pid))?;
        children.entry(parent).or_default().push((pid, group));
    }
    let mut found = Vec::new();
    let mut parents = vec![root];
    while let Some(parent) = parents.pop() {
        for &(child, group) in children.get(&parent).into_iter().flatten() {
            found.push((child, group));
            parents.push(child);
            if found.len() > LIMIT {
                return Err(io::Error::other(format!("more than {LIMIT} descendants")));
            }
        }
    }
    Ok(found)
}

#[cfg(target_os = "linux")]
fn unreadable(pid: libc::pid_t) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        format!("unreadable /proc/{pid}/stat"),
    )
}

/// Parent and process group from `/proc/<pid>/stat`: `pid (comm) state ppid
/// pgrp ...`, where the command name may itself contain spaces and `)`.
#[cfg(any(target_os = "linux", test))]
fn parse_stat(stat: &str) -> Option<(libc::pid_t, libc::pid_t)> {
    let mut fields = stat.get(stat.rfind(')')? + 1..)?.split_whitespace();
    let _state = fields.next()?;
    Some((fields.next()?.parse().ok()?, fields.next()?.parse().ok()?))
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn descendants(_: libc::pid_t) -> io::Result<Vec<(libc::pid_t, libc::pid_t)>> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "listing descendant processes is unsupported on this platform",
    ))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    #[test]
    fn stat_fields_follow_the_last_parenthesis_of_the_command_name() {
        assert_eq!(parse_stat("42 (opencode) S 7 42 42 0"), Some((7, 42)));
        assert_eq!(
            parse_stat("43 (a) b (c) d) R 42 99 99 0 -1"),
            Some((42, 99))
        );
        assert_eq!(parse_stat("44 (truncated"), None);
        assert_eq!(parse_stat("45 (x) S nan 1"), None);
    }

    #[test]
    fn an_incomplete_record_cannot_be_confirmed() {
        let groups = Groups {
            failure: Some("listing failed".into()),
            ..Default::default()
        };
        assert!(
            groups
                .recorded()
                .unwrap_err()
                .to_string()
                .contains("listing failed")
        );
        assert!(Groups::default().recorded().unwrap().is_empty());
    }

    #[cfg(any(target_os = "macos", target_os = "linux"))]
    #[test]
    fn records_children_that_moved_into_their_own_groups() {
        use std::os::unix::process::CommandExt;
        // Children of this test process in new process groups, as OpenCode 2's
        // spawner starts its standalone server below `run`.
        let mut helpers: Vec<_> = (0..2)
            .map(|_| {
                std::process::Command::new("/bin/sh")
                    .args(["-c", "read value"])
                    .stdin(std::process::Stdio::piped())
                    .process_group(0)
                    .spawn()
                    .unwrap()
            })
            .collect();
        let own = libc::pid_t::try_from(std::process::id()).unwrap();
        let found = descendants(own).unwrap();
        let mut groups = Groups::default();
        groups.sample(own);
        for helper in &helpers {
            let pid = libc::pid_t::try_from(helper.id()).unwrap();
            assert!(found.contains(&(pid, pid)), "{found:?}");
            assert!(groups.recorded().unwrap().contains(&pid));
        }
        for helper in &mut helpers {
            drop(helper.stdin.take());
            helper.wait().unwrap();
        }
    }
}
