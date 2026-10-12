#![allow(clippy::unwrap_used)]
use super::*;
use std::sync::Arc;

#[test]
fn damaged_or_partial_state_is_preserved_and_all_writes_fail_closed() {
    for bytes in [
        b"".as_slice(),
        b"{\"version\":1,",
        b"\xff",
        b"{}",
        b"{\"version\":1,\"next_run\":3}",
        b"{\"version\":1,\"runs\":[]}",
        b"{\"version\":99,\"next_run\":1,\"runs\":[]}",
        b"{\"version\":1,\"next_run\":1,\"runs\":null}",
    ] {
        let dir = tempfile::tempdir().unwrap();
        let store = Arc::new(Store::open(dir.path()).unwrap());
        fs::write(store.history_path(), bytes).unwrap();
        assert!(store.load().is_err());
        let state = AppState::default();
        assert!(store.save(&state).is_err());
        assert!(store.begin_save(&state).is_err());
        assert!(
            store
                .begin_history_write(&state, &[1].into(), &Default::default())
                .is_err()
        );
        assert_eq!(fs::read(store.history_path()).unwrap(), bytes);
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 2);
    }
}

#[test]
fn absent_and_explicitly_empty_history_are_valid_and_legacy_fields_default() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path()).unwrap();
    assert!(store.load().unwrap().runs.is_empty());
    fs::write(
        store.history_path(),
        b"{\"version\":1,\"next_run\":42,\"runs\":[]}",
    )
    .unwrap();
    let state = store.load().unwrap();
    assert_eq!(state.next_run, 42);
    assert!(state.runs.is_empty());
    let legacy = include_bytes!("../../tests/fixtures/state-v0.1.json");
    fs::write(store.history_path(), legacy).unwrap();
    let state = store.load().unwrap();
    assert_eq!(state.runs[0].id, 41);
    assert_eq!(state.next_run, 42);
    assert_eq!(fs::read(store.history_path()).unwrap(), legacy);
}

#[test]
fn duplicate_and_exhausted_ids_fail_and_lagging_counter_is_not_reused() {
    let mut value: serde_json::Value =
        serde_json::from_slice(include_bytes!("../../tests/fixtures/state-v0.1.json")).unwrap();
    value["next_run"] = 1.into();
    assert_eq!(
        decode_state(&serde_json::to_vec(&value).unwrap())
            .unwrap()
            .next_run,
        42
    );
    let run = value["runs"][0].clone();
    value["runs"].as_array_mut().unwrap().push(run);
    assert!(decode_state(&serde_json::to_vec(&value).unwrap()).is_err());
    value["runs"].as_array_mut().unwrap().pop();
    value["runs"][0]["id"] = u64::MAX.into();
    assert!(decode_state(&serde_json::to_vec(&value).unwrap()).is_err());
}

#[test]
fn write_failures_preserve_original_and_remove_temporary_files() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path()).unwrap();
    store.save(&AppState::default()).unwrap();
    let original = fs::read(store.history_path()).unwrap();
    for kind in [
        std::io::ErrorKind::StorageFull,
        std::io::ErrorKind::PermissionDenied,
    ] {
        let error = store
            .replace_state(|writer| {
                // Exceed the buffer so a partial temporary file really exists.
                writer.write_all(&vec![b'x'; 32 * 1024])?;
                Err(std::io::Error::from(kind).into())
            })
            .unwrap_err();
        assert_eq!(error.downcast_ref::<std::io::Error>().unwrap().kind(), kind);
        assert_eq!(fs::read(store.history_path()).unwrap(), original);
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 2);
    }
    fs::rename(store.history_path(), dir.path().join("original.json")).unwrap();
    fs::create_dir(store.history_path()).unwrap();
    assert!(store.save(&AppState::default()).is_err());
    assert_eq!(
        fs::read(dir.path().join("original.json")).unwrap(),
        original
    );
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 3);
}

#[test]
#[cfg(unix)]
fn read_only_directory_preserves_original_state() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path()).unwrap();
    store.save(&AppState::default()).unwrap();
    let original = fs::read(store.history_path()).unwrap();
    fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o500)).unwrap();
    let result = store.save(&AppState {
        next_run: 9,
        ..Default::default()
    });
    fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o700)).unwrap();
    if result.is_err() {
        // root can bypass Unix mode bits
        assert_eq!(fs::read(store.history_path()).unwrap(), original);
    }
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 2);
}

#[test]
#[cfg(windows)]
fn windows_locked_state_preserves_history_and_can_be_saved_after_unlock() {
    use std::os::windows::fs::OpenOptionsExt;
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(&dir.path().join("state ü 日本語")).unwrap();
    let mut state = AppState::default();
    state.draft.prompt = "original".into();
    store.save(&state).unwrap();
    let original = fs::read(store.history_path()).unwrap();
    // Permit reads/writes, but refuse FILE_SHARE_DELETE: another application's
    // open handle prevents atomic replacement even though the file is writable.
    let lock = fs::OpenOptions::new()
        .read(true)
        .share_mode(0x1 | 0x2)
        .open(store.history_path())
        .unwrap();
    state.draft.prompt = "new draft".into();
    let error = store.save(&state).unwrap_err();
    // This message is part of the user-facing failure contract: failed atomic
    // replacement must say the previous state is intact, also verified below.
    assert!(error.to_string().contains("previous state is intact"));
    drop(error);
    assert_eq!(fs::read(store.history_path()).unwrap(), original);
    assert_eq!(fs::read_dir(store.directory()).unwrap().count(), 2);
    drop(lock);
    store.save(&state).unwrap();
    assert_eq!(store.load().unwrap().draft.prompt, "new draft");
}
