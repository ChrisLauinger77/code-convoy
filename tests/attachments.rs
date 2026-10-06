#![allow(clippy::unwrap_used)]
use codeconvoy::{
    agents,
    attachments::{self, Attachment, MAX_FILE_BYTES, MAX_TOTAL_BYTES},
    domain::{AgentId, Repository, TaskConfig},
};
use std::{ffi::OsString, fs, path::Path};

fn file(directory: &Path, name: &str, bytes: &[u8]) -> Attachment {
    let path = directory.join(name);
    fs::write(&path, bytes).unwrap();
    Attachment::inspect(&path).unwrap()
}

#[test]
fn attachment_validation_checks_type_bytes_size_and_same_size_changes() {
    let temp = tempfile::tempdir().unwrap();
    for name in ["requirements.md", "a.txt", "a.json", "a.yaml", "a.yml"] {
        let attachment = file(temp.path(), name, "Grüße\n\"context\"".as_bytes());
        assert!(!attachment.kind.is_image());
        assert_eq!(
            attachment.read_validated().unwrap(),
            "Grüße\n\"context\"".as_bytes()
        );
        fs::write(&attachment.path, "Grüße\n\"changed\"").unwrap();
        assert!(
            attachment
                .read_validated()
                .unwrap_err()
                .to_string()
                .contains("changed")
        );
    }
    for (name, bytes) in [
        ("unsupported.pdf", &b"pdf"[..]),
        ("invalid.txt", &b"\xff"[..]),
        ("nul.md", &b"text\0binary"[..]),
        ("invalid.png", &b"not an image"[..]),
    ] {
        let path = temp.path().join(name);
        fs::write(&path, bytes).unwrap();
        assert!(Attachment::inspect(&path).is_err(), "{name}");
    }
    let path = temp.path().join("directory.md");
    fs::create_dir(&path).unwrap();
    assert!(
        Attachment::inspect(&path)
            .unwrap_err()
            .to_string()
            .contains("regular file")
    );
    let path = temp.path().join("large.txt");
    fs::File::create(&path)
        .unwrap()
        .set_len(MAX_FILE_BYTES + 1)
        .unwrap();
    assert!(
        Attachment::inspect(&path)
            .unwrap_err()
            .to_string()
            .contains("10 MiB")
    );
    assert!(Attachment::inspect(&temp.path().join("missing.txt")).is_err());
}

#[test]
fn reuse_restores_valid_references_and_reports_every_missing_or_changed_file() {
    let temp = tempfile::tempdir().unwrap();
    let valid = file(temp.path(), "valid.txt", b"context");
    let changed = file(temp.path(), "changed.txt", b"before");
    let missing = file(temp.path(), "missing.md", b"missing");
    fs::write(&changed.path, b"after!").unwrap();
    fs::remove_file(&missing.path).unwrap();
    let (restored, issues) = attachments::for_reuse(&[valid.clone(), changed, missing]);
    assert_eq!(restored, [valid]);
    assert_eq!(issues.len(), 2);
    assert!(
        issues
            .iter()
            .any(|e| e.contains("changed.txt") && e.contains("changed"))
    );
    assert!(
        issues
            .iter()
            .any(|e| e.contains("missing.md") && e.contains("missing"))
    );
}

#[test]
fn duplicate_canonical_files_and_combined_payload_limits_are_guarded() {
    let temp = tempfile::tempdir().unwrap();
    let attachment = file(temp.path(), "a.txt", b"a");
    assert!(attachments::validate_limits(&[attachment.clone(), attachment.clone()]).is_err());
    let mut second = attachment.clone();
    second.path = temp.path().join("b.txt");
    second.size = MAX_FILE_BYTES;
    let mut first = attachment;
    first.size = MAX_TOTAL_BYTES - MAX_FILE_BYTES + 1;
    assert!(
        attachments::validate_limits(&[first, second])
            .unwrap_err()
            .to_string()
            .contains("total")
    );
    #[cfg(unix)]
    {
        let target = file(temp.path(), "target.txt", b"context");
        let alias = temp.path().join("alias.md");
        std::os::unix::fs::symlink(&target.path, &alias).unwrap();
        let alias = Attachment::inspect(&alias).unwrap();
        assert_eq!(alias.path, target.path);
        assert!(attachments::validate_limits(&[target, alias]).is_err());
    }
}

#[test]
fn every_backend_supplies_escaped_text_via_stdin_without_changing_permissions() {
    let temp = tempfile::tempdir().unwrap();
    let attachment = file(
        temp.path(),
        "spécification $(echo nope) with spaces.md",
        b"text\n\"quoted\"\\escaped",
    );
    let repository = Repository {
        name: "repo".into(),
        path: temp.path().join("repo"),
    };
    for agent in AgentId::ALL {
        let backend = agents::backend(agent).unwrap();
        let mut task = TaskConfig {
            agent,
            prompt: "Review attached context".into(),
            ..Default::default()
        };
        let original = backend.build(&task, &repository).unwrap();
        task.attachments = vec![attachment.clone()];
        let supplied = backend.build(&task, &repository).unwrap();
        assert_eq!(original.args, supplied.args, "{agent:?}");
        assert_eq!(original.env, supplied.env);
        assert_eq!(original.remove_env, supplied.remove_env);
        let input = String::from_utf8(supplied.input.unwrap()).unwrap();
        let context: serde_json::Value =
            serde_json::from_str(input.lines().last().unwrap()).unwrap();
        assert_eq!(context["filename"], attachment.filename);
        assert_eq!(context["content"], "text\n\"quoted\"\\escaped");
        assert!(!input.contains(attachment.path.to_str().unwrap()));
        assert_eq!(supplied.directory, repository.path);
        assert!(
            backend
                .attachment_help(&task, &repository.path)
                .unwrap()
                .is_none()
        );
    }
}

#[test]
fn image_transport_preserves_paths_and_uses_backend_owned_native_or_structured_input() {
    use base64::Engine;
    let temp = tempfile::tempdir().unwrap();
    let png = include_bytes!("../assets/codeconvoy-256.png");
    let images = vec![
        file(temp.path(), "screenshot Grüße with spaces.png", png),
        file(temp.path(), "$(touch nope);another.png", png),
    ];
    let repository = Repository {
        name: "repo".into(),
        path: temp.path().join("repo"),
    };
    for agent in AgentId::ALL {
        let backend = agents::backend(agent).unwrap();
        let task = TaskConfig {
            agent,
            prompt: "Inspect screenshots".into(),
            attachments: images.clone(),
            ..Default::default()
        };
        let spec = backend.build(&task, &repository).unwrap();
        match agent {
            AgentId::Codex => {
                let values: Vec<_> = spec
                    .args
                    .windows(2)
                    .filter(|args| args[0] == "--image")
                    .map(|args| args[1].clone())
                    .collect();
                assert_eq!(
                    values,
                    images
                        .iter()
                        .map(|a| a.path.as_os_str().to_owned())
                        .collect::<Vec<_>>()
                );
                assert_eq!(spec.input.unwrap(), task.prompt.as_bytes());
            }
            AgentId::Copilot | AgentId::OpenCode => {
                let flag = if agent == AgentId::Copilot {
                    "--attachment="
                } else {
                    "--file="
                };
                for image in &images {
                    let mut expected = OsString::from(flag);
                    expected.push(&image.path);
                    assert!(spec.args.contains(&expected));
                }
                assert_eq!(spec.input.unwrap(), task.prompt.as_bytes());
            }
            AgentId::Claude => {
                assert!(
                    spec.args
                        .windows(2)
                        .any(|args| args[0] == "--input-format" && args[1] == "stream-json")
                );
                assert!(!spec.args.iter().any(|arg| arg == "--replay-user-messages"));
                let input = spec.input.unwrap();
                assert_eq!(input.last(), Some(&b'\n'));
                let message: serde_json::Value = serde_json::from_slice(&input).unwrap();
                assert_eq!(message["type"], "user");
                assert_eq!(message["message"]["role"], "user");
                let blocks = message["message"]["content"].as_array().unwrap();
                let blocks: Vec<_> = blocks
                    .iter()
                    .filter(|block| block["type"] == "image")
                    .collect();
                assert_eq!(blocks.len(), 2);
                for image in blocks {
                    assert_eq!(image["source"]["media_type"], "image/png");
                    assert_eq!(
                        base64::engine::general_purpose::STANDARD
                            .decode(image["source"]["data"].as_str().unwrap())
                            .unwrap(),
                        png
                    );
                }
            }
        }
    }
}

#[test]
fn backend_specific_image_limits_identify_the_affected_file() {
    let temp = tempfile::tempdir().unwrap();
    let image = file(
        temp.path(),
        "comma,name.png",
        include_bytes!("../assets/codeconvoy-256.png"),
    );
    let mut task = TaskConfig {
        prompt: "Inspect image".into(),
        attachments: vec![image],
        ..Default::default()
    };
    let error = agents::backend(AgentId::Codex)
        .unwrap()
        .validate_attachments(&task)
        .unwrap_err()
        .to_string();
    assert!(error.contains("comma,name.png") && error.contains("commas"));
    task.agent = AgentId::Claude;
    task.attachments[0].size = 7_500_001;
    let error = agents::backend(AgentId::Claude)
        .unwrap()
        .validate_attachments(&task)
        .unwrap_err()
        .to_string();
    assert!(error.contains("comma,name.png") && error.contains("base64"));
}
