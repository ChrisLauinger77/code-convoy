#![allow(clippy::unwrap_used)]
use super::*;
use crate::{
    domain::{AgentId, JobStatus, Repository, TaskConfig, WorktreeResult},
    validation::Record,
    visibility::Changes,
};

fn job(name: &str, status: JobStatus, changed: Option<bool>, files: Option<usize>) -> Job {
    let mut job = Job::queued(Repository {
        name: name.into(),
        path: "/private/source/repository".into(),
    });
    job.status = status;
    job.started_at = Some(1_700_000_010);
    job.finished_at = Some(1_700_000_094);
    job.completion_changes = Some(Changes { changed, files });
    job
}

fn run(jobs: Vec<Job>) -> Run {
    Run {
        id: 42,
        provenance: None,
        created_at: 1_700_000_000,
        task: TaskConfig {
            prompt: "Improve result inspection\nPrivate second line".into(),
            ..Default::default()
        },
        jobs,
    }
}

fn validated(job: &mut Job, status: Status) {
    job.validation = Some(Record {
        command: Command {
            executable: "cargo".into(),
            arguments: vec!["test".into()],
        },
        directory: "/private/validation/directory".into(),
        status,
        started_at: 1_700_000_100,
        finished_at: Some(1_700_000_120),
        duration_ms: 20_000,
        exit_code: Some(if status == Status::Passed { 0 } else { 1 }),
        output: "SECRET_VALIDATION_LOG".into(),
        detail: "SECRET_VALIDATION_DETAIL".into(),
        truncated: false,
    });
}

#[test]
fn successful_convoy_has_stable_complete_markdown() {
    let mut alpha = job("alpha", JobStatus::Succeeded, Some(true), Some(8));
    validated(&mut alpha, Status::Passed);
    let run = run(vec![
        alpha,
        job("beta", JobStatus::Succeeded, Some(false), Some(0)),
    ]);
    let expected = r#"# Convoy #42 — Succeeded

Backend: OpenAI Codex CLI

Task summary: Improve result inspection

First recorded start: 2023-11-14 22:13:30 UTC

Created: 2023-11-14 22:13:20 UTC

Completed: 2023-11-14 22:14:54 UTC

Duration: 1:34

Repository jobs: 2 recorded

## Summary

- Succeeded: 2
- Failed: 0
- Cancelled: 0
- Changed: 1
- Unchanged: 1
- Unknown changes: 0
- Needs review: 1

## Validation

- Passed: 1
- Not run / not recorded: 1

## Repository Results

| Repository | Result | Changes | Review | Validation |
|---|---|---|---|---|
| alpha | Succeeded | Changed · 8 files | Required | Passed |
| beta | Succeeded | Unchanged | Not required | Not run / not recorded |

Changes are saved completion observations; validation is the latest recorded result.
"#;
    assert_eq!(convoy(&run), expected);
    assert_eq!(convoy(&run), convoy(&run));
    // The persisted round-trip, without session state, produces the same report.
    let historical: Run = serde_json::from_str(&serde_json::to_string(&run).unwrap()).unwrap();
    assert_eq!(convoy(&historical), expected);
}

#[test]
fn mixed_outcomes_preserve_launch_order_and_validation_independence() {
    let mut zeta = job("zeta", JobStatus::Succeeded, Some(false), None);
    validated(&mut zeta, Status::Failed);
    let mut alpha = job("alpha", JobStatus::Failed, Some(true), Some(2));
    validated(&mut alpha, Status::Passed);
    let run = run(vec![
        zeta,
        alpha,
        job("middle", JobStatus::Cancelled, None, None),
    ]);
    let text = convoy(&run);
    assert!(text.starts_with("# Convoy #42 — Failed"));
    for line in [
        "- Succeeded: 1",
        "- Failed: 1",
        "- Cancelled: 1",
        "- Changed: 1 known",
        "- Unchanged: 1 known",
        "- Unknown changes: 1",
        "- Needs review: 2",
        "| zeta | Succeeded | Unchanged | Not required | Failed |",
        "| alpha | Failed | Changed · 2 files | Required | Passed |",
        "| middle | Cancelled | Unknown | Required | Not run / not recorded |",
    ] {
        assert!(text.contains(line), "missing {line:?}: {text}");
    }
    assert!(text.find("| zeta |").unwrap() < text.find("| alpha |").unwrap());
    assert!(text.find("| alpha |").unwrap() < text.find("| middle |").unwrap());
    let result = repository(&run, &run.jobs[0]);
    assert!(result.contains("Result: Succeeded"));
    assert!(result.contains("## Validation\n\nResult: Failed"));
    assert!(result.contains("Command: \"cargo\" \"test\""));
    assert!(result.contains("Exit code: 1"));
    assert!(repository(&run, &run.jobs[1]).contains("Changed files: 2"));
}

#[test]
fn cancellation_interruption_and_resolution_remain_distinct() {
    let mut run = run(vec![job("repo", JobStatus::Cancelled, Some(true), None)]);
    assert!(convoy(&run).starts_with("# Convoy #42 — Cancelled"));
    assert!(repository(&run, &run.jobs[0]).contains("Result: Cancelled"));
    run.jobs[0].interrupted = true;
    assert!(convoy(&run).starts_with("# Convoy #42 — Interrupted"));
    assert!(convoy(&run).contains("Interrupted (included in cancelled): 1"));
    assert!(repository(&run, &run.jobs[0]).contains("Result: Interrupted"));
    for (resolution, expected) in [
        (ResultResolution::Applied, "Applied"),
        (ResultResolution::Discarded, "Discarded"),
        (ResultResolution::ApplyPending, "Apply pending / uncertain"),
        (ResultResolution::DiscardPending, "Discard pending"),
    ] {
        run.jobs[0].resolution = resolution;
        assert!(repository(&run, &run.jobs[0]).contains(&format!("Review: {expected}")));
    }
}

#[test]
fn partial_and_empty_history_never_invent_measurements() {
    let mut run = run(vec![job("", JobStatus::Succeeded, None, None)]);
    run.jobs[0].completion_changes = None;
    run.jobs[0].started_at = None;
    run.jobs[0].finished_at = None;
    let text = convoy(&run);
    assert!(text.contains("Duration: Unavailable"));
    assert!(text.contains("Completed: Unavailable"));
    assert!(text.contains("- Changed: 0 known\n"));
    assert!(text.contains("- Unknown changes: 1\n"));
    assert!(text.contains(
        "| Unnamed repository | Succeeded | Unknown | Required | Not run / not recorded |"
    ));
    let text = repository(&run, &run.jobs[0]);
    assert!(text.contains("Duration: Unavailable"));
    assert!(text.contains("Completion observation: Unknown"));
    assert!(!text.contains("Changed files:"));
    assert!(!text.contains("Command:"));
    assert!(!text.contains("Exit code:"));
    run.jobs
        .push(job("known", JobStatus::Succeeded, Some(true), None));
    assert!(convoy(&run).contains("Completed: Unavailable"));
    run.jobs.clear();
    run.task.prompt.clear();
    let text = convoy(&run);
    assert!(text.starts_with("# Convoy #42 — No jobs"));
    assert!(text.contains("No repository results recorded."));
    assert!(!text.contains("## Summary"));
    assert!(!text.contains("## Validation"));
    assert!(!text.contains("Task summary:"));
    assert!(!text.contains("Succeeded: 0"));
}

#[test]
fn durations_use_only_complete_ordered_recorded_timestamps() {
    for (start, end, expected) in [
        (None, Some(20), "Unavailable"),
        (Some(10), None, "Unavailable"),
        (Some(20), Some(10), "Unavailable"),
        (Some(10), Some(10), "0:00"),
        (Some(10), Some(3671), "1:01:01"),
    ] {
        assert_eq!(elapsed(start, end), expected);
    }
    let mut run = run(vec![job("repo", JobStatus::Succeeded, None, None)]);
    run.jobs[0].finished_at = Some(run.created_at - 1);
    assert!(convoy(&run).contains("Duration: Unavailable"));
}

#[test]
fn live_statistics_and_mutable_worktree_observations_are_never_exported() {
    let mut run = run(vec![job("repo", JobStatus::Failed, None, None)]);
    let before = convoy(&run);
    run.jobs[0].review = Some(Ok(crate::review::Statistics {
        files: 999,
        ..Default::default()
    }));
    run.jobs[0].worktree_result = Some(WorktreeResult {
        observed_this_session: true,
        exists: true,
        changed: Some(true),
    });
    run.jobs[0].before = Some(crate::domain::GitSummary {
        changed: 888,
        ..Default::default()
    });
    assert_eq!(convoy(&run), before);
    assert!(!repository(&run, &run.jobs[0]).contains("Changed files:"));
}

#[test]
fn tables_escape_markdown_html_pipes_and_multiline_unicode_names() {
    let name = "Übersicht|日本語\r\n*bold* _em_ `code` [link](url) <tag> & &#124; a\\b";
    let run = run(vec![job(name, JobStatus::Succeeded, Some(false), None)]);
    let text = convoy(&run);
    assert!(text.contains("| Übersicht\\|日本語 \\*bold\\* \\_em\\_ \\`code\\` \\[link\\](url) &lt;tag&gt; &amp; &amp;\\#124; a\\\\b |"));
    assert_eq!(text.lines().filter(|line| line.starts_with('|')).count(), 3);
    assert!(!text.contains("<tag>"));
    assert_eq!(
        markdown("one\ntwo\tthree\u{2028}four\0"),
        "one two three four"
    );
}

#[test]
fn summaries_and_commands_are_bounded_without_slicing_unicode() {
    let mut run = run(vec![job(
        &"界".repeat(1000),
        JobStatus::Succeeded,
        None,
        None,
    )]);
    run.task.prompt = format!("{}\nPRIVATE_TAIL", "語".repeat(500));
    validated(&mut run.jobs[0], Status::Passed);
    run.jobs[0].validation.as_mut().unwrap().command.arguments = vec!["arg".repeat(1000); 128];
    let text = convoy(&run);
    assert!(text.contains(&format!("Task summary: {}…", "語".repeat(160))));
    assert!(text.contains(&format!("| {}… |", "界".repeat(80))));
    assert!(!text.contains("PRIVATE_TAIL"));
    assert!(text.len() < 2200);
    assert!(repository(&run, &run.jobs[0]).len() < 1500);
}

#[test]
fn private_sources_and_local_paths_are_omitted_on_every_host() {
    let mut run = run(vec![job("repo", JobStatus::Failed, Some(true), Some(2))]);
    run.task.attachments.push(crate::attachments::Attachment {
        path: "/private/SECRET_ATTACHMENT.txt".into(),
        filename: "SECRET_ATTACHMENT.txt".into(),
        kind: crate::attachments::AttachmentKind::Text,
        size: 100,
        sha256: "SECRET_ATTACHMENT_HASH".into(),
    });
    run.task
        .options
        .insert("environment".into(), "SECRET_ENVIRONMENT".into());
    run.jobs[0].raw_log.append("SECRET_STDOUT_STDERR");
    run.jobs[0].log.append("SECRET_ACTIVITY");
    run.jobs[0].detail = "SECRET_DIAGNOSTICS".into();
    run.jobs[0].worktree_detail = "SECRET_DIFF".into();
    validated(&mut run.jobs[0], Status::Failed);
    for path in [
        "/private/person/project",
        "C:\\Users\\person\\project",
        "D:/work/project",
        "\\\\server\\share\\project",
        "~/work/project",
    ] {
        run.task.prompt = format!("Inspect ({path})");
        run.jobs[0].repository.name = path.into();
        let record = run.jobs[0].validation.as_mut().unwrap();
        record.command.executable = format!("{path}/cargo");
        record.command.arguments = vec!["test".into(), format!("--directory={path}")];
        for text in [convoy(&run), repository(&run, &run.jobs[0])] {
            assert!(!text.contains(path));
            assert!(!text.contains("SECRET_"));
            assert!(!text.contains("/private/"));
            assert!(text.contains("[local path omitted]"));
        }
        assert!(repository(&run, &run.jobs[0]).contains("\"cargo\" \"test\""));
    }
}

#[test]
fn absolute_paths_after_list_delimiters_are_omitted_from_all_copied_fields() {
    let mut run = run(vec![job("repo", JobStatus::Succeeded, Some(false), None)]);
    validated(&mut run.jobs[0], Status::Passed);
    for delimiter in [',', ';', '|', '&'] {
        let value = format!("--inputs=src{delimiter}/home/alice/private");
        run.task.prompt = value.clone();
        run.jobs[0].repository.name = value.clone();
        run.jobs[0].validation.as_mut().unwrap().command.arguments = vec![value.clone()];
        assert_eq!(field(&value, 160), "[local path omitted]", "{value}");
        for text in [convoy(&run), repository(&run, &run.jobs[0])] {
            assert!(!text.contains("/home/alice/private"), "{value}: {text}");
            assert!(text.contains("[local path omitted]"));
        }
        assert!(
            repository(&run, &run.jobs[0])
                .contains("Command: \"cargo\" \"\\[local path omitted\\]\"")
        );

        // A slash within a relative list item is not an absolute-path boundary.
        let relative = format!("--inputs=src{delimiter}tests/unit");
        assert_eq!(field(&relative, 160), markdown(&relative));
    }
}

#[test]
fn named_user_home_paths_are_omitted_only_at_token_boundaries() {
    let mut run = run(vec![job("repo", JobStatus::Succeeded, Some(false), None)]);
    validated(&mut run.jobs[0], Status::Passed);
    for path in [
        "~alice/private",
        "~alice\\private",
        "~service.account-2/private",
        "~álîce/private",
        "~domain-user$/private",
        "~user@domain/private",
        "~/private",
        "~\\private",
    ] {
        for prefix in [
            "",
            "Inspect ",
            "--config=",
            "'",
            "\"",
            "`",
            "(",
            "[",
            "{",
            "src,",
            "src;",
            "src|",
            "src&",
            "config:",
        ] {
            let value = format!("{prefix}{path}");
            run.task.prompt = value.clone();
            run.jobs[0].repository.name = value.clone();
            run.jobs[0].validation.as_mut().unwrap().command.arguments = vec![value.clone()];
            assert_eq!(field(&value, 160), "[local path omitted]", "{value}");
            for text in [convoy(&run), repository(&run, &run.jobs[0])] {
                assert!(!text.contains("private"), "{value}: {text}");
                assert!(text.contains("[local path omitted]"));
            }
        }
    }
    for literal in [
        "repository~alice/docs",
        "--tag=release~alice/docs",
        "~alice",
        "~alice notes/docs",
        "~alice,notes/docs",
    ] {
        assert_eq!(field(literal, 160), markdown(literal), "{literal}");
    }
}

#[test]
fn all_backends_and_validation_states_use_their_recorded_labels() {
    let mut run = run(vec![job("repo", JobStatus::Succeeded, Some(false), None)]);
    for agent in AgentId::ALL {
        run.task.agent = agent;
        assert!(convoy(&run).contains(agent.label()));
        assert!(repository(&run, &run.jobs[0]).contains(agent.label()));
    }
    for status in [
        Status::Passed,
        Status::Failed,
        Status::Cancelled,
        Status::Unavailable,
        Status::Running,
    ] {
        validated(&mut run.jobs[0], status);
        run.jobs[0].validation.as_mut().unwrap().exit_code = None;
        assert!(convoy(&run).contains(&format!("- {}: 1", status.label())));
        let text = repository(&run, &run.jobs[0]);
        assert!(text.contains(&format!("## Validation\n\nResult: {}", status.label())));
        assert!(!text.contains("Exit code:"));
    }
}
