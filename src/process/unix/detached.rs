//! Records the process groups that a child's descendants move into. Some CLIs
//! (OpenCode 2) start helpers with `setsid`, outside the child's own group,
//! which `Group` signals and probes. Those groups are only ever probed.
use std::{
    collections::BTreeSet,
    io,
    path::{Path, PathBuf},
    time::Duration,
};
use tokio::time::Instant;

const SAMPLE: Duration = Duration::from_millis(200);
/// Beyond this many groups or descendants, tracking is reported as incomplete.
const LIMIT: usize = 1024;

/// A point on the clock that process start times use: microseconds since the
/// Unix epoch on macOS, clock ticks since boot on Linux.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Moment(u64);

#[derive(Default)]
pub(super) struct Groups {
    recorded: BTreeSet<libc::pid_t>,
    sampled: Option<Instant>,
    failure: Option<String>,
    /// The job's working directory, resolved like the paths processes report.
    directory: PathBuf,
    /// When the most recent sample began (initially, when tracking began).
    since: Option<Moment>,
}

impl Groups {
    pub fn new(directory: &Path) -> Self {
        let mut groups = Self::default();
        match std::fs::canonicalize(directory) {
            Ok(directory) => groups.directory = directory,
            Err(error) => {
                groups.failure = Some(format!("cannot resolve {}: {error}", directory.display()));
            }
        }
        match now() {
            Ok(moment) => groups.since = Some(moment),
            Err(error) => groups.failure = Some(error.to_string()),
        }
        groups
    }

    /// Records the groups of `root`'s current descendants, at most every
    /// `SAMPLE`. The caller samples only while `root` is unreaped, so a
    /// descendant's parent chain still leads to it.
    pub fn sample(&mut self, root: libc::pid_t) {
        if self.failure.is_some() || self.sampled.is_some_and(|at| at.elapsed() < SAMPLE) {
            return;
        }
        self.sampled = Some(Instant::now());
        let began = match now() {
            Ok(moment) => moment,
            Err(error) => {
                self.failure = Some(error.to_string());
                return;
            }
        };
        match descendants(root) {
            Ok(found) => {
                self.record(root, found.into_iter().map(|(_, group)| group));
                self.since = Some(began);
            }
            Err(error) => self.failure = Some(error.to_string()),
        }
    }

    /// Runs once `root` has exited. A helper started after the last sample
    /// began and orphaned by root's exit has been reparented, so `sample`
    /// cannot find it. Records the groups of this user's processes that
    /// started since then and work inside the job's directory, except
    /// CodeConvoy's own children (its Git commands).
    pub fn sweep(&mut self, root: libc::pid_t) {
        if self.failure.is_some() {
            return;
        }
        let Some(since) = self.since else {
            return;
        };
        match strays(root, &self.directory, since) {
            Ok(groups) => self.record(root, groups.into_iter()),
            Err(error) => self.failure = Some(error.to_string()),
        }
    }

    fn record(&mut self, root: libc::pid_t, groups: impl Iterator<Item = libc::pid_t>) {
        // Members of root's own group are covered by Group itself.
        self.recorded
            .extend(groups.filter(|&group| group > 1 && group != root));
        if self.recorded.len() > LIMIT {
            self.failure = Some(format!("more than {LIMIT} process groups"));
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
    match bsd_info(pid)? {
        Some(info) => libc::pid_t::try_from(info.pbi_pgid)
            .map(Some)
            .map_err(|_| io::Error::other("invalid process group ID")),
        None => Ok(None),
    }
}

/// `None` once `pid` has exited (or become a zombie).
#[cfg(target_os = "macos")]
fn bsd_info(pid: libc::pid_t) -> io::Result<Option<libc::proc_bsdinfo>> {
    // SAFETY: zero is a valid proc_bsdinfo initialization.
    let mut info: libc::proc_bsdinfo = unsafe { std::mem::zeroed() };
    pid_info(pid, libc::PROC_PIDTBSDINFO, &mut info).map(|found| found.then_some(info))
}

/// Fills `info` with `flavor` for `pid`; false once `pid` has exited.
#[cfg(target_os = "macos")]
fn pid_info<T>(pid: libc::pid_t, flavor: libc::c_int, info: &mut T) -> io::Result<bool> {
    let size = libc::c_int::try_from(std::mem::size_of::<T>())
        .map_err(|_| io::Error::other("process information is too large"))?;
    // SAFETY: valid output buffer of exactly `size` bytes for this flavor.
    let written = unsafe { libc::proc_pidinfo(pid, flavor, 0, (info as *mut T).cast(), size) };
    if written == size {
        return Ok(true);
    }
    let error = io::Error::last_os_error();
    match error.raw_os_error() {
        Some(libc::ESRCH) | Some(0) | None => Ok(false),
        _ => Err(error),
    }
}

#[cfg(target_os = "macos")]
fn now() -> io::Result<Moment> {
    let since_epoch = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| io::Error::other("system clock is before 1970"))?;
    u64::try_from(since_epoch.as_micros())
        .map(Moment)
        .map_err(|_| io::Error::other("system clock is out of range"))
}

/// Allowance for clock granularity when comparing start times.
#[cfg(target_os = "macos")]
fn slack() -> u64 {
    100_000
}

/// Groups of this user's processes, other than CodeConvoy's own children, that
/// started at or after `since` and work inside `directory`.
#[cfg(target_os = "macos")]
fn strays(root: libc::pid_t, directory: &Path, since: Moment) -> io::Result<Vec<libc::pid_t>> {
    let own = libc::pid_t::try_from(std::process::id())
        .map_err(|_| io::Error::other("invalid own process ID"))?;
    // SAFETY: geteuid has no preconditions.
    let uid = unsafe { libc::geteuid() };
    let mut groups = Vec::new();
    for pid in all_pids()? {
        if pid <= 1 || pid == own || pid == root {
            continue;
        }
        let info = match bsd_info(pid) {
            Ok(Some(info)) => info,
            // Exited, or another user's process this user may not inspect.
            Ok(None) => continue,
            Err(error) if error.raw_os_error() == Some(libc::EPERM) => continue,
            Err(error) => return Err(error),
        };
        let started = info
            .pbi_start_tvsec
            .saturating_mul(1_000_000)
            .saturating_add(info.pbi_start_tvusec);
        let parent = libc::pid_t::try_from(info.pbi_ppid).unwrap_or(0);
        if !candidate(
            info.pbi_uid == uid,
            parent == own,
            Moment(started),
            since,
            slack(),
        ) {
            continue;
        }
        if let Some(cwd) = cwd_of(pid)?
            && cwd.starts_with(directory)
            && let Ok(group) = libc::pid_t::try_from(info.pbi_pgid)
        {
            groups.push(group);
        }
    }
    Ok(groups)
}

#[cfg(target_os = "macos")]
fn all_pids() -> io::Result<Vec<libc::pid_t>> {
    let mut buffer: Vec<libc::pid_t> = vec![0; 4096];
    loop {
        let capacity = buffer.len();
        let bytes = libc::c_int::try_from(capacity * std::mem::size_of::<libc::pid_t>())
            .map_err(|_| io::Error::other("process list is too large"))?;
        // SAFETY: the buffer holds `capacity` PIDs and its size is passed in
        // bytes. The call returns the number of PIDs written.
        let count = unsafe { libc::proc_listallpids(buffer.as_mut_ptr().cast(), bytes) };
        if count < 0 {
            return Err(io::Error::last_os_error());
        }
        let count = usize::try_from(count).unwrap_or(capacity);
        if count < capacity {
            buffer.truncate(count);
            return Ok(buffer);
        }
        if capacity >= 1 << 20 {
            return Err(io::Error::other("process list is too large"));
        }
        buffer.resize(capacity * 2, 0);
    }
}

/// `None` once `pid` has exited or when its directory cannot be read.
#[cfg(target_os = "macos")]
fn cwd_of(pid: libc::pid_t) -> io::Result<Option<PathBuf>> {
    use std::os::unix::ffi::OsStrExt;
    // SAFETY: zero is a valid proc_vnodepathinfo initialization.
    let mut info: libc::proc_vnodepathinfo = unsafe { std::mem::zeroed() };
    match pid_info(pid, libc::PROC_PIDVNODEPATHINFO, &mut info) {
        Ok(true) => {}
        Ok(false) => return Ok(None),
        Err(error) if error.raw_os_error() == Some(libc::EPERM) => return Ok(None),
        Err(error) => return Err(error),
    }
    // SAFETY: vip_path is a contiguous MAXPATHLEN byte array split in rows.
    let bytes: &[u8] = unsafe {
        std::slice::from_raw_parts(
            info.pvi_cdir.vip_path.as_ptr().cast(),
            std::mem::size_of_val(&info.pvi_cdir.vip_path),
        )
    };
    let path = bytes.split(|&byte| byte == 0).next().unwrap_or_default();
    Ok((!path.is_empty()).then(|| PathBuf::from(std::ffi::OsStr::from_bytes(path))))
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
            let (_, group, _) = parse_stat(&stat).ok_or_else(|| unreadable(child))?;
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
        let (parent, group, _) = parse_stat(&stat).ok_or_else(|| unreadable(pid))?;
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
fn now() -> io::Result<Moment> {
    // SAFETY: zero is a valid timespec initialization and CLOCK_BOOTTIME is
    // the clock /proc/<pid>/stat start times are measured on.
    let mut time: libc::timespec = unsafe { std::mem::zeroed() };
    if unsafe { libc::clock_gettime(libc::CLOCK_BOOTTIME, &mut time) } != 0 {
        return Err(io::Error::last_os_error());
    }
    let hz = ticks_per_second()?;
    let seconds = u64::try_from(time.tv_sec).unwrap_or(0);
    let nanoseconds = u64::try_from(time.tv_nsec).unwrap_or(0);
    Ok(Moment(
        seconds.saturating_mul(hz) + nanoseconds.saturating_mul(hz) / 1_000_000_000,
    ))
}

#[cfg(target_os = "linux")]
fn ticks_per_second() -> io::Result<u64> {
    // SAFETY: sysconf has no preconditions.
    let hz = unsafe { libc::sysconf(libc::_SC_CLK_TCK) };
    u64::try_from(hz)
        .ok()
        .filter(|&hz| hz > 0)
        .ok_or_else(|| io::Error::other("unknown clock tick rate"))
}

/// Allowance for clock granularity when comparing start times (100 ms).
#[cfg(target_os = "linux")]
fn slack() -> u64 {
    ticks_per_second().map_or(10, |hz| hz.div_ceil(10))
}

/// Groups of this user's processes, other than CodeConvoy's own children, that
/// started at or after `since` and work inside `directory`.
#[cfg(target_os = "linux")]
fn strays(root: libc::pid_t, directory: &Path, since: Moment) -> io::Result<Vec<libc::pid_t>> {
    use std::os::unix::fs::MetadataExt;
    let own = libc::pid_t::try_from(std::process::id())
        .map_err(|_| io::Error::other("invalid own process ID"))?;
    // SAFETY: geteuid has no preconditions.
    let uid = unsafe { libc::geteuid() };
    let slack = slack();
    let mut groups = Vec::new();
    for entry in std::fs::read_dir("/proc")? {
        let entry = entry?;
        let Some(pid) = entry
            .file_name()
            .to_str()
            .and_then(|name| name.parse::<libc::pid_t>().ok())
        else {
            continue;
        };
        if pid <= 1 || pid == own || pid == root {
            continue;
        }
        // A process that exited meanwhile has no entry left.
        let Ok(owner) = entry.metadata().map(|metadata| metadata.uid()) else {
            continue;
        };
        let Ok(stat) = std::fs::read_to_string(entry.path().join("stat")) else {
            continue;
        };
        let (parent, group, started) = parse_stat(&stat).ok_or_else(|| unreadable(pid))?;
        if !candidate(owner == uid, parent == own, Moment(started), since, slack) {
            continue;
        }
        if std::fs::read_link(entry.path().join("cwd")).is_ok_and(|cwd| cwd.starts_with(directory))
        {
            groups.push(group);
        }
    }
    Ok(groups)
}

#[cfg(target_os = "linux")]
fn unreadable(pid: libc::pid_t) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        format!("unreadable /proc/{pid}/stat"),
    )
}

/// Parent, process group and start time (clock ticks since boot) from
/// `/proc/<pid>/stat`: `pid (comm) state ppid pgrp ... starttime ...`, where the
/// command name may itself contain spaces and `)`.
#[cfg(any(target_os = "linux", test))]
fn parse_stat(stat: &str) -> Option<(libc::pid_t, libc::pid_t, u64)> {
    let fields: Vec<&str> = stat
        .get(stat.rfind(')')? + 1..)?
        .split_whitespace()
        .collect();
    // fields[0] is the state; starttime is the 22nd stat field.
    Some((
        fields.get(1)?.parse().ok()?,
        fields.get(2)?.parse().ok()?,
        fields.get(19)?.parse().ok()?,
    ))
}

/// Whether a process may be a helper that the last sample missed, before its
/// working directory is checked: owned by this user, not one of CodeConvoy's
/// own children, and started no earlier than the sample (less `slack`).
fn candidate(
    same_user: bool,
    codeconvoy_child: bool,
    started: Moment,
    since: Moment,
    slack: u64,
) -> bool {
    same_user && !codeconvoy_child && started.0 >= since.0.saturating_sub(slack)
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn descendants(_: libc::pid_t) -> io::Result<Vec<(libc::pid_t, libc::pid_t)>> {
    Err(unsupported())
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn now() -> io::Result<Moment> {
    Err(unsupported())
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn strays(_: libc::pid_t, _: &Path, _: Moment) -> io::Result<Vec<libc::pid_t>> {
    Err(unsupported())
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn unsupported() -> io::Error {
    io::Error::new(
        io::ErrorKind::Unsupported,
        "listing descendant processes is unsupported on this platform",
    )
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    #[test]
    fn stat_fields_follow_the_last_parenthesis_of_the_command_name() {
        // session, tty_nr, tpgid, flags, 4 fault counts, 4 times, priority,
        // nice, num_threads and itrealvalue precede starttime (98765).
        let tail = "42 0 -1 4194560 100 0 0 0 1 2 0 0 20 0 1 0 98765 1000 10";
        assert_eq!(
            parse_stat(&format!("42 (opencode) S 7 42 {tail}")),
            Some((7, 42, 98765))
        );
        assert_eq!(
            parse_stat(&format!("43 (a) b (c) d) R 42 99 {tail}")),
            Some((42, 99, 98765))
        );
        assert_eq!(parse_stat("44 (truncated"), None);
        assert_eq!(parse_stat(&format!("45 (x) S nan 1 {tail}")), None);
        assert_eq!(parse_stat("46 (short) S 1 46 46 0"), None);
    }

    #[test]
    fn sweep_candidates_are_recent_processes_of_this_user_not_codeconvoys_own() {
        let since = Moment(1_000);
        assert!(candidate(true, false, Moment(1_000), since, 0));
        assert!(candidate(true, false, Moment(950), since, 100));
        // Older processes, such as the user's open terminals, are not strays.
        assert!(!candidate(true, false, Moment(899), since, 100));
        assert!(!candidate(false, false, Moment(2_000), since, 100));
        // CodeConvoy's own children are its Git commands.
        assert!(!candidate(true, true, Moment(2_000), since, 100));
    }

    #[cfg(any(target_os = "macos", target_os = "linux"))]
    #[test]
    fn strays_are_recent_orphans_working_inside_the_directory() {
        use std::os::unix::process::CommandExt;
        let inside = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let since = now().unwrap();
        // The shell exits at once, orphaning its background sleep, which keeps
        // the shell's process group and working directory.
        let output = std::process::Command::new("/bin/sh")
            .args(["-c", "sleep 30 >/dev/null 2>&1 & echo $!"])
            .current_dir(inside.path())
            .process_group(0)
            .output()
            .unwrap();
        let orphan: libc::pid_t = String::from_utf8(output.stdout)
            .unwrap()
            .trim()
            .parse()
            .unwrap();
        let groups = |directory: &Path| {
            strays(0, &std::fs::canonicalize(directory).unwrap(), since).unwrap()
        };
        let found = groups(inside.path());
        let missed_elsewhere = groups(outside.path());
        let later = groups_since(inside.path(), Moment(now().unwrap().0 + 10 * slack()));
        // SAFETY: the orphan was started by this test and sleeps for 30 s.
        unsafe { libc::kill(orphan, libc::SIGKILL) };
        assert!(!found.is_empty(), "orphan {orphan} not found");
        assert!(missed_elsewhere.is_empty(), "{missed_elsewhere:?}");
        assert!(later.is_empty(), "{later:?}");
    }

    #[cfg(any(target_os = "macos", target_os = "linux"))]
    fn groups_since(directory: &Path, since: Moment) -> Vec<libc::pid_t> {
        strays(0, &std::fs::canonicalize(directory).unwrap(), since).unwrap()
    }

    #[test]
    fn an_unresolvable_directory_cannot_be_confirmed() {
        let groups = Groups::new(Path::new("/nonexistent/codeconvoy/job directory"));
        assert!(groups.recorded().is_err());
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
