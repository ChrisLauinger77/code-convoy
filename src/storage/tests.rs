#![allow(clippy::unwrap_used)]
use super::*;
use crate::{domain::*, persistence::Store};
use std::{fs, path::Path};

fn fixture() -> (tempfile::TempDir, Store, Request) {
    let temp = tempfile::tempdir_in(std::env::temp_dir().canonicalize().unwrap()).unwrap();
    let store = Store::open(&temp.path().join("state")).unwrap();
    store.save(&AppState::default()).unwrap();
    let request = Request {
        history_path: store.history_path(),
        worktree_root: store.directory().join("worktrees"),
        references: vec![],
    };
    (temp, store, request)
}
fn scan(request: Request) -> Snapshot {
    measure(request, &Cancellation::default()).unwrap()
}
fn attempt(root: &Path, custom: Option<&Path>, id: u64) -> WorktreeMetadata {
    let source = root.parent().unwrap().join("source");
    let reservation = crate::worktrees::reserve_at(
        root,
        custom,
        id,
        0,
        Repository {
            name: "Source".into(),
            path: source.clone(),
        },
        source.join(".git"),
        "a".repeat(40),
    )
    .unwrap();
    reservation.preparation.unwrap();
    let m = reservation.metadata;
    fs::create_dir(&m.path).unwrap();
    m
}
fn historical(id: u64) -> Run {
    let mut job = Job::queued(Repository {
        name: "Repo".into(),
        path: "/fixture/repo".into(),
    });
    job.status = JobStatus::Succeeded;
    job.finished_at = Some(1000);
    Run {
        id,
        provenance: None,
        created_at: 900,
        task: TaskConfig::default(),
        jobs: vec![job],
    }
}
#[test]
fn history_uses_actual_shared_file_excludes_exports_and_refreshes_after_cleanup() {
    let (_temp, store, request) = fixture();
    let empty = scan(request.clone());
    assert_eq!(empty.historical_convoys, Some(0));
    assert_eq!(
        empty.history.bytes,
        Some(fs::metadata(store.history_path()).unwrap().len())
    );
    assert_eq!(empty.worktrees.bytes, Some(0));
    let mut state = AppState {
        runs: vec![historical(1), historical(2)],
        ..Default::default()
    };
    store.save(&state).unwrap();
    fs::write(store.directory().join("export.json"), vec![0; 8000]).unwrap();
    fs::write(store.directory().join("export.md"), vec![0; 9000]).unwrap();
    fs::write(store.directory().join(".temporary-save"), vec![0; 10000]).unwrap();
    let before = scan(request.clone());
    assert_eq!(before.historical_convoys, Some(2));
    assert_eq!(
        before.history.bytes,
        Some(fs::metadata(store.history_path()).unwrap().len())
    );
    state.remove_from_history(1);
    store.save(&state).unwrap();
    let after = scan(request);
    assert_eq!(after.historical_convoys, Some(1));
    assert!(after.history.bytes < before.history.bytes);
    assert_eq!(
        fs::metadata(store.directory().join("export.md"))
            .unwrap()
            .len(),
        9000
    );
}
#[test]
fn missing_corrupt_and_legacy_history_are_explicit() {
    let (_temp, store, request) = fixture();
    fs::remove_file(store.history_path()).unwrap();
    let missing = scan(request.clone());
    assert_eq!(missing.historical_convoys, None);
    assert_eq!(missing.history.bytes, None);
    assert_eq!(missing.total.status, Status::Unavailable);
    fs::write(store.history_path(), b"broken JSON").unwrap();
    let corrupt = scan(request.clone());
    assert_eq!(corrupt.history.bytes, Some(11));
    assert_eq!(corrupt.history.status, Status::Partial);
    assert_eq!(corrupt.total.status, Status::Partial);
    fs::write(
        store.history_path(),
        include_bytes!("../../tests/fixtures/state-v0.1.json"),
    )
    .unwrap();
    let legacy = scan(request);
    assert_eq!(legacy.history.status, Status::Complete);
    assert!(legacy.historical_convoys.is_some());
}

#[test]
fn incomplete_history_envelope_is_not_reported_as_empty_complete_history() {
    let (_temp, store, request) = fixture();
    for bytes in [b"{}".as_slice(), b"{\"version\":1,\"next_run\":3}", b"\xff"] {
        fs::write(store.history_path(), bytes).unwrap();
        let result = scan(request.clone());
        assert_eq!(result.historical_convoys, None);
        assert_eq!(result.history.status, Status::Partial);
        assert_eq!(fs::read(store.history_path()).unwrap(), bytes);
    }
}

#[test]
#[ignore = "repeatable filesystem inventory measurement; no timing threshold"]
fn inventory_scale_performance() {
    for count in [100, 1000, 5000] {
        let (_temp, _, request) = fixture();
        let mut references = Vec::new();
        for id in 1..=count {
            let metadata = attempt(&request.worktree_root, None, id);
            fs::create_dir_all(metadata.path.join("build")).unwrap();
            fs::write(metadata.path.join("build/output"), [b'x'; 128]).unwrap();
            references.push(metadata);
        }
        // Duplicate live/saved references exercise the same inventory dedup path.
        references.extend(references.clone());
        let mut samples = Vec::new();
        for _ in 0..5 {
            let start = std::time::Instant::now();
            let result = crate::worktrees::inventory::discover(
                &request.worktree_root,
                references.clone(),
                &Cancellation::default(),
            );
            assert_eq!(result.entries.len(), count as usize);
            assert_eq!(result.unknown, 0);
            samples.push(start.elapsed().as_secs_f64() * 1000.0);
        }
        samples.sort_by(f64::total_cmp);
        let mut measured_request = request;
        measured_request.references = references;
        let start = std::time::Instant::now();
        let snapshot = scan(measured_request);
        assert_eq!(snapshot.worktrees.bytes, Some(count * 128));
        assert_eq!(snapshot.worktrees.status, Status::Complete);
        println!(
            "inventory {count}: median {:.3} ms; full scan {:.3} ms",
            samples[2],
            start.elapsed().as_secs_f64() * 1000.0
        );
    }
}
#[test]
fn inventory_includes_old_bases_orphans_deduplicates_and_excludes_unrelated() {
    let (temp, _store, mut request) = fixture();
    let default = attempt(&request.worktree_root, None, 1);
    let old = attempt(
        &request.worktree_root,
        Some(&temp.path().join("old base")),
        2,
    );
    let newer = attempt(
        &request.worktree_root,
        Some(&temp.path().join("new base")),
        3,
    );
    for m in [&default, &old, &newer] {
        fs::write(m.path.join("data"), b"12345").unwrap();
    }
    request.references = vec![old.clone(), old];
    fs::create_dir(temp.path().join("unrelated.git")).unwrap();
    fs::write(temp.path().join("unrelated.git/blob"), b"do not count").unwrap();
    let snapshot = scan(request);
    assert_eq!(snapshot.entries.len(), 3);
    assert_eq!(snapshot.locations.len(), 3);
    assert_eq!(snapshot.worktrees.bytes, Some(15));
    assert_eq!(snapshot.total.bytes, snapshot.history.bytes.map(|n| n + 15));
    assert_eq!(snapshot.worktrees.status, Status::Complete);
}
#[test]
fn unknown_ownership_missing_and_bad_records_preserve_known_subset() {
    let (temp, _store, mut request) = fixture();
    let good = attempt(&request.worktree_root, None, 1);
    fs::write(good.path.join("bytes"), b"known").unwrap();
    let missing = attempt(&request.worktree_root, None, 2);
    fs::remove_dir(&missing.path).unwrap();
    let partial = attempt(&request.worktree_root, Some(&temp.path().join("old")), 3);
    fs::remove_file(partial.path.parent().unwrap().join("owner.json")).unwrap();
    fs::create_dir(request.worktree_root.join("unowned.git")).unwrap();
    fs::write(request.worktree_root.join("unowned.git/data"), b"not owned").unwrap();
    fs::write(request.worktree_root.join(".locations/broken.json"), b"bad").unwrap();
    let mut unknown = good.clone();
    unknown.owner = "not-codeconvoy".into();
    request.references = vec![missing, partial, unknown];
    let snapshot = scan(request);
    assert_eq!(snapshot.entries.len(), 3);
    assert_eq!(snapshot.worktrees.bytes, Some(5));
    assert_eq!(snapshot.unavailable, 2);
    assert!(snapshot.unknown_ownership >= 3);
    assert_eq!(snapshot.worktrees.status, Status::Partial);
    assert_eq!(snapshot.total.status, Status::Partial);
}
#[test]
fn tombstones_exclude_removed_but_not_reappeared_worktrees() {
    let (_temp, _store, request) = fixture();
    let m = attempt(&request.worktree_root, None, 1);
    fs::write(
        m.path.parent().unwrap().join("cleanup.json"),
        serde_json::to_vec(&serde_json::json!({"metadata": m, "completed": true})).unwrap(),
    )
    .unwrap();
    fs::remove_dir(&m.path).unwrap();
    assert!(scan(request.clone()).entries.is_empty());
    fs::create_dir(&m.path).unwrap();
    assert_eq!(scan(request).entries.len(), 1);
}
#[test]
fn logical_sizes_include_nested_builds_large_sparse_files_and_empty_directories() {
    let (temp, _, _) = fixture();
    let tree = temp.path().join("tree");
    fs::create_dir(&tree).unwrap();
    assert_eq!(
        filesystem::measure(&tree, &Cancellation::default()).bytes,
        Some(0)
    );
    fs::create_dir_all(tree.join("target/a/node_modules/dist")).unwrap();
    fs::write(tree.join("target/a/node_modules/dist/file"), b"seven77").unwrap();
    fs::File::create(tree.join("sparse"))
        .unwrap()
        .set_len(5 * 1024 * 1024 * 1024)
        .unwrap();
    let m = filesystem::measure(&tree, &Cancellation::default());
    assert_eq!(m.bytes, Some(5 * 1024 * 1024 * 1024 + 7));
    assert_eq!(m.status, Status::Complete);
}
#[cfg(unix)]
#[test]
fn symlinks_hardlinks_and_special_files_are_safe() {
    use std::os::unix::{fs::symlink, net::UnixListener};
    let (temp, _, _) = fixture();
    let tree = temp.path().join("tree");
    fs::create_dir(&tree).unwrap();
    let outside = temp.path().join("outside");
    fs::create_dir(&outside).unwrap();
    fs::write(outside.join("data"), b"excluded").unwrap();
    symlink(&outside, tree.join("external")).unwrap();
    symlink(outside.join("data"), tree.join("file_link")).unwrap();
    symlink(tree.join("missing"), tree.join("broken")).unwrap();
    symlink(&tree, tree.join("cycle")).unwrap();
    fs::write(tree.join("regular"), b"123").unwrap();
    fs::hard_link(tree.join("regular"), tree.join("hardlink")).unwrap();
    let _socket = UnixListener::bind(tree.join("socket")).unwrap();
    let m = filesystem::measure(&tree, &Cancellation::default());
    assert_eq!(m.bytes, Some(3));
    assert_eq!(m.status, Status::Complete);
    let alias = temp.path().join("alias");
    symlink(&tree, &alias).unwrap();
    assert_eq!(
        filesystem::measure(&alias, &Cancellation::default()).bytes,
        None
    );
    assert_eq!(fs::read(outside.join("data")).unwrap(), b"excluded");
}
#[test]
fn vanished_subdirectory_is_partial_not_zero_and_cancellation_interrupts() {
    let (temp, _, _) = fixture();
    let tree = temp.path().join("tree");
    fs::create_dir(&tree).unwrap();
    let gone = tree.join("gone");
    fs::create_dir(&gone).unwrap();
    fs::write(tree.join("keep"), b"1234").unwrap();
    let m = filesystem::walk(&tree, &Cancellation::default(), |path| {
        if path == gone {
            fs::remove_dir(&gone).unwrap();
        }
    });
    assert_eq!(m.bytes, Some(4));
    assert_eq!(m.status, Status::Partial);
    let cancel = Cancellation::default();
    let m = filesystem::walk(&tree, &cancel, |_| cancel.cancel());
    assert!(cancel.is_cancelled());
    assert!(m.bytes.is_none_or(|n| n <= 4));
}
#[cfg(unix)]
#[test]
fn permission_errors_are_reported_for_history_and_directories() {
    use std::os::unix::fs::PermissionsExt;
    let (temp, store, request) = fixture();
    fs::set_permissions(store.history_path(), fs::Permissions::from_mode(0o0)).unwrap();
    // Root can bypass mode bits. The pure error-state assertion below still runs.
    if fs::File::open(store.history_path()).is_err() {
        let s = scan(request);
        assert_eq!(s.history.bytes, None);
        assert_eq!(s.history.status, Status::Unavailable);
    }
    fs::set_permissions(store.history_path(), fs::Permissions::from_mode(0o600)).unwrap();
    let tree = temp.path().join("tree");
    fs::create_dir(&tree).unwrap();
    fs::set_permissions(&tree, fs::Permissions::from_mode(0o0)).unwrap();
    if fs::read_dir(&tree).is_err() {
        let m = filesystem::measure(&tree, &Cancellation::default());
        assert_eq!(m.bytes, None);
        assert_eq!(m.status, Status::Unavailable);
    }
    fs::set_permissions(&tree, fs::Permissions::from_mode(0o700)).unwrap();
    let mut partial = Measurement::complete(42);
    partial.issue("Permission denied");
    assert_eq!(partial.status, Status::Partial);
    assert_eq!(partial.bytes, Some(42));
}
#[test]
fn aggregate_known_components_and_format_iec_units() {
    let good = Measurement::complete(100);
    let mut bad = Measurement::default();
    bad.issue("missing");
    assert_eq!(aggregate([&good, &bad]).bytes, Some(100));
    assert_eq!(aggregate([&good, &bad]).status, Status::Partial);
    assert_eq!(aggregate([&bad]).bytes, None);
    assert_eq!(aggregate([&good, &good]).bytes, Some(200));
    assert_eq!(format_bytes(0), "0 B");
    assert_eq!(format_bytes(1023), "1023 B");
    assert_eq!(format_bytes(1024), "1.00 KiB");
    assert_eq!(format_bytes(1024 * 1024), "1.00 MiB");
    assert_eq!(format_bytes(3 * 1024 * 1024 * 1024), "3.00 GiB");
}
#[test]
fn refresh_coalesces_cancels_rejects_stale_results_and_recovers_worker_failure() {
    let (_temp, _store, request) = fixture();
    let mut service = service::Service::default();
    assert_eq!(service.status, Status::NotMeasured);
    let first = service.refresh(request.clone()).unwrap();
    assert!(service.refresh(request.clone()).is_none());
    assert!(service.refresh(request.clone()).is_none());
    assert!(first.cancel.is_cancelled());
    let latest = service
        .finish(first.generation, Ok(Some(scan(request.clone()))))
        .unwrap();
    assert!(service.snapshot.is_none());
    assert!(
        service
            .finish(first.generation, Err("late failure".into()))
            .is_none()
    );
    assert_eq!(service.status, Status::Measuring);
    service.finish(latest.generation, Ok(Some(scan(request.clone()))));
    assert_eq!(service.status, Status::Complete);
    let last_success = service.history_success;
    let failing = service.refresh(request.clone()).unwrap();
    service.finish(failing.generation, Err("worker panic".into()));
    assert_eq!(service.status, Status::Error);
    assert_eq!(service.history_success, last_success);
    let closing = service.refresh(request.clone()).unwrap();
    service.refresh(request);
    service.cancel();
    assert!(closing.cancel.is_cancelled());
    assert!(service.finish(closing.generation, Ok(None)).is_none());
    assert!(!service.running());
}
#[test]
fn service_drop_cancels_worker_without_waiting_or_writing() {
    let (_temp, store, request) = fixture();
    let before = fs::read(store.history_path()).unwrap();
    let mut service = service::Service::default();
    let work = service.refresh(request).unwrap();
    drop(service);
    assert!(work.cancel.is_cancelled());
    assert!(measure(work.request, &work.cancel).is_none());
    assert_eq!(fs::read(store.history_path()).unwrap(), before);
}

#[test]
fn replacing_child_with_external_link_and_removing_root_never_invent_zero() {
    let (temp, _, _) = fixture();
    let tree = temp.path().join("tree");
    fs::create_dir(&tree).unwrap();
    let m = filesystem::walk(&tree, &Cancellation::default(), |path| {
        fs::remove_dir(path).unwrap();
    });
    assert_eq!(m.bytes, None);
    assert_eq!(m.status, Status::Unavailable);
    #[cfg(unix)]
    {
        let external = temp.path().join("external");
        fs::create_dir(&external).unwrap();
        fs::write(external.join("secret"), b"excluded").unwrap();
        fs::create_dir(&tree).unwrap();
        let child = tree.join("child");
        fs::create_dir(&child).unwrap();
        let m = filesystem::walk(&tree, &Cancellation::default(), |path| {
            if path == child {
                fs::remove_dir(&child).unwrap();
                std::os::unix::fs::symlink(&external, &child).unwrap();
            }
        });
        assert_eq!(m.bytes, Some(0)); // Only the accessible, empty parent was measured.
        assert_eq!(m.status, Status::Partial);
        assert_eq!(fs::read(external.join("secret")).unwrap(), b"excluded");
    }
}

#[test]
fn thousands_of_generated_files_preserve_contents_and_all_ownership_records() {
    let (_temp, store, request) = fixture();
    let m = attempt(&request.worktree_root, None, 1);
    let build = m.path.join("target/node_modules/dist");
    fs::create_dir_all(&build).unwrap();
    for i in 0..4096 {
        fs::write(build.join(i.to_string()), b"output").unwrap();
    }
    let owner = m.path.parent().unwrap().join("owner.json");
    let before_owner = fs::read(&owner).unwrap();
    let before_history = fs::read(store.history_path()).unwrap();
    let snapshot = scan(request);
    assert_eq!(snapshot.worktrees.bytes, Some(4096 * 6));
    assert_eq!(snapshot.worktrees.status, Status::Complete);
    assert_eq!(fs::read(owner).unwrap(), before_owner);
    assert_eq!(fs::read(store.history_path()).unwrap(), before_history);
    assert_eq!(fs::read_dir(&build).unwrap().count(), 4096);
    for i in 0..4096 {
        assert_eq!(fs::read(build.join(i.to_string())).unwrap(), b"output");
    }
}

#[test]
fn conflicting_attempt_identity_is_not_measured_and_errors_are_bounded() {
    let (_temp, _store, mut request) = fixture();
    let m = attempt(&request.worktree_root, None, 1);
    fs::write(m.path.join("data"), b"must not count").unwrap();
    let mut conflicting = m.clone();
    conflicting.base_commit = "b".repeat(40);
    request.references = vec![m, conflicting];
    for i in 0..150 {
        fs::create_dir(request.worktree_root.join(format!("unknown-{i}"))).unwrap();
    }
    let snapshot = scan(request);
    assert!(snapshot.entries.is_empty());
    assert_eq!(snapshot.worktrees.bytes, None);
    assert_eq!(snapshot.unknown_ownership, 151);
    assert_eq!(snapshot.worktrees.issues.len(), 100);
    assert!(snapshot.worktrees.issue_count >= 151);
}

#[cfg(unix)]
#[test]
fn partial_and_unavailable_worktrees_have_separate_counts() {
    use std::os::unix::fs::PermissionsExt;
    let (_temp, _store, request) = fixture();
    let partial = attempt(&request.worktree_root, None, 1);
    fs::write(partial.path.join("measurable"), b"123").unwrap();
    let denied = partial.path.join("denied");
    fs::create_dir(&denied).unwrap();
    fs::set_permissions(&denied, fs::Permissions::from_mode(0o0)).unwrap();
    let inaccessible = fs::read_dir(&denied).is_err();
    let missing = attempt(&request.worktree_root, None, 2);
    fs::remove_dir(&missing.path).unwrap();
    let snapshot = scan(request);
    fs::set_permissions(&denied, fs::Permissions::from_mode(0o700)).unwrap();
    assert_eq!(snapshot.entries.len(), 2);
    assert_eq!(snapshot.unavailable, 1);
    assert_eq!(snapshot.partial, usize::from(inaccessible));
    assert_eq!(snapshot.worktrees.bytes, Some(3));
    assert_eq!(snapshot.worktrees.status, Status::Partial);
}
