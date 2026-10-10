#![allow(clippy::unwrap_used)]
use super::*;

fn initialization_failure_remains_discoverable(conflict: &str) {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    let storage = root.join("storage");
    let custom = root.join("custom");
    let metadata = allocate(
        &custom,
        1,
        0,
        Repository {
            path: root.join("repository"),
            name: "repository".into(),
        },
        root.join("repository/.git"),
        "a".repeat(40),
    )
    .unwrap();
    let attempt = metadata.path.parent().unwrap().to_owned();
    let conflict_path = attempt.join(conflict);
    // Fail real filesystem operations deterministically, including on Windows
    // and privileged CI runners, without permission changes or timing races.
    fs::write(&conflict_path, "keep conflicting resource").unwrap();
    let reserved = finish_reservation(&storage, metadata.clone());
    assert_eq!(reserved.metadata, metadata);
    let error = reserved.preparation.unwrap_err();
    assert!(
        error
            .to_string()
            .contains("Could not initialize worktree attempt")
    );
    assert!(!metadata.path.exists());
    assert_eq!(
        fs::read_to_string(&conflict_path).unwrap(),
        "keep conflicting resource"
    );
    if conflict == "hooks" {
        verify_manifest(&metadata).unwrap();
    } else {
        assert!(verify_manifest(&metadata).is_err());
        assert!(!attempt.join("hooks").exists());
    }

    // Simulate startup without a persisted run: the Store binding must recover
    // the custom attempt even if its sibling manifest is missing or invalid.
    assert_eq!(
        location::recorded_root(&storage, &metadata).unwrap(),
        custom
    );
    assert_eq!(
        recovery::orphans(&storage, &Default::default()).unwrap(),
        vec![attempt.clone()]
    );
    assert!(attempt.exists(), "Discovery must retain partial attempts");
    assert!(
        recovery::orphans(&storage, &[metadata.path].into())
            .unwrap()
            .is_empty(),
        "Persisted attempts must not be reported again as orphans"
    );
}

#[test]
fn owner_write_failure_preserves_custom_attempt_binding_and_metadata() {
    initialization_failure_remains_discoverable("owner.json");
}

#[test]
fn hooks_creation_failure_preserves_custom_attempt_binding_and_metadata() {
    initialization_failure_remains_discoverable("hooks");
}
