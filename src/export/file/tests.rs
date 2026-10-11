#![allow(clippy::unwrap_used)]
use super::*;
fn contents(dir: &Path) -> Vec<PathBuf> {
    fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect()
}
#[test]
fn atomic_success_and_explicit_overwrite() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("report.json");
    let destination = Destination::inspect(&path).unwrap();
    atomic_write(&destination, false, &Default::default(), |w| {
        w.write_all("日本語\n".as_bytes())?;
        Ok(())
    })
    .unwrap();
    assert_eq!(fs::read_to_string(&path).unwrap(), "日本語\n");
    assert_eq!(contents(dir.path()), std::slice::from_ref(&path));
    assert!(atomic_write(&destination, false, &Default::default(), |_| Ok(())).is_err());
    let destination = Destination::inspect(&path).unwrap();
    assert!(atomic_write(&destination, false, &Default::default(), |_| Ok(())).is_err());
    assert_eq!(fs::read_to_string(&path).unwrap(), "日本語\n");
    atomic_write(&destination, true, &Default::default(), |w| {
        w.write_all(b"new")?;
        Ok(())
    })
    .unwrap();
    assert_eq!(fs::read_to_string(&path).unwrap(), "new");
    assert_eq!(contents(dir.path()), [path]);
}
#[test]
fn failed_render_and_cancel_leave_no_partial_target_or_temporary_file() {
    for existing in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("report.md");
        if existing {
            fs::write(&path, "original").unwrap();
        }
        for cancel_during in [false, true] {
            let destination = Destination::inspect(&path).unwrap();
            let cancel = Cancellation::default();
            let result = atomic_write(&destination, existing, &cancel, |w| {
                w.write_all(b"partial output")?;
                if cancel_during {
                    cancel.cancel();
                    w.write_all(b"more")?;
                }
                Err(io::Error::from(io::ErrorKind::StorageFull).into())
            });
            assert!(result.is_err());
            if existing {
                assert_eq!(fs::read_to_string(&path).unwrap(), "original");
            } else {
                assert!(!path.exists());
            }
            assert_eq!(contents(dir.path()).len(), usize::from(existing));
        }
        let cancel = Cancellation::default();
        cancel.cancel();
        assert!(
            atomic_write(
                &Destination::inspect(&path).unwrap(),
                existing,
                &cancel,
                |_| panic!("cancelled before creation")
            )
            .is_err()
        );
        assert_eq!(contents(dir.path()).len(), usize::from(existing));
    }
}
#[test]
fn invalid_paths_destination_changes_and_links_fail_safely() {
    let dir = tempfile::tempdir().unwrap();
    assert!(Destination::inspect(&dir.path().join("missing/report.md")).is_err());
    assert!(Destination::inspect(dir.path()).is_err());
    let path = dir.path().join("report.md");
    fs::write(&path, "reviewed").unwrap();
    let destination = Destination::inspect(&path).unwrap();
    fs::write(&path, "changed after review").unwrap();
    assert!(
        atomic_write(&destination, true, &Default::default(), |w| {
            w.write_all(b"new")?;
            Ok(())
        })
        .is_err()
    );
    assert_eq!(fs::read_to_string(&path).unwrap(), "changed after review");
    assert_eq!(contents(dir.path()), std::slice::from_ref(&path));
    let new = dir.path().join("new.md");
    let destination = Destination::inspect(&new).unwrap();
    fs::create_dir(&new).unwrap();
    assert!(atomic_write(&destination, false, &Default::default(), |_| Ok(())).is_err());
    #[cfg(unix)]
    {
        let link = dir.path().join("link.md");
        std::os::unix::fs::symlink(&path, &link).unwrap();
        assert!(Destination::inspect(&link).is_err());
    }
}
#[test]
#[cfg(unix)]
fn permission_failure_preserves_target_and_leaves_no_temp() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let parent = dir.path().join("restricted");
    fs::create_dir(&parent).unwrap();
    let path = parent.join("report.md");
    let destination = Destination::inspect(&path).unwrap();
    fs::set_permissions(&parent, fs::Permissions::from_mode(0o500)).unwrap();
    let result = atomic_write(&destination, false, &Default::default(), |_| Ok(()));
    fs::set_permissions(&parent, fs::Permissions::from_mode(0o700)).unwrap();
    // Root can write regardless of permission bits; don't claim that check there.
    if result.is_err() {
        assert!(!path.exists());
        assert!(contents(&parent).is_empty());
    }
}
