//! Clipboard reports read saved metadata only: no Git, live review cache, or logs.
use super::format;
use crate::{
    domain::{Job, ResultResolution, Run},
    validation::{Command, Status},
    visibility,
};
use std::fmt::Write;

pub(super) fn convoy(run: &Run) -> String {
    let mut text = format!(
        "# Convoy #{} — {}\n\nBackend: {}\n",
        run.id,
        format::run_state(run),
        run.task.agent.label()
    );
    if let Some(task) = run.task.prompt.lines().find(|line| !line.trim().is_empty()) {
        let _ = writeln!(text, "\nTask summary: {}", field(task, 160));
    }
    if let Some(start) = run.jobs.iter().filter_map(|job| job.started_at).min() {
        let _ = writeln!(text, "\nFirst recorded start: {}", format::timestamp(start));
    }
    let end = completion(run);
    let _ = write!(
        text,
        "\nCreated: {}\n\nCompleted: {}\n\nDuration: {}\n\nRepository jobs: {} recorded\n",
        format::timestamp(run.created_at),
        end.map_or_else(|| "Unavailable".into(), format::timestamp),
        elapsed(Some(run.created_at), end),
        run.jobs.len()
    );
    if run.jobs.is_empty() {
        text.push_str("\nNo repository results recorded.\n");
        return text;
    }
    let s = visibility::summary(run);
    let known = if s.unknown > 0 { " known" } else { "" };
    let _ = write!(
        text,
        "\n## Summary\n\n- Succeeded: {}\n- Failed: {}\n- Cancelled: {}\n- Changed: {}{known}\n- Unchanged: {}{known}\n- Unknown changes: {}\n- Needs review: {}\n",
        s.succeeded, s.failed, s.cancelled, s.changed, s.unchanged, s.unknown, s.review
    );
    let interrupted = run.jobs.iter().filter(|job| job.interrupted).count();
    if interrupted > 0 {
        let _ = writeln!(text, "- Interrupted (included in cancelled): {interrupted}");
    }
    text.push_str("\n## Validation\n\n");
    for status in [
        Status::Passed,
        Status::Failed,
        Status::Cancelled,
        Status::Unavailable,
        Status::Running,
    ] {
        let count = run
            .jobs
            .iter()
            .filter(|job| job.validation.as_ref().is_some_and(|v| v.status == status))
            .count();
        if count > 0 {
            let _ = writeln!(text, "- {}: {count}", status.label());
        }
    }
    let missing = run
        .jobs
        .iter()
        .filter(|job| job.validation.is_none())
        .count();
    if missing > 0 {
        let _ = writeln!(text, "- Not run / not recorded: {missing}");
    }
    text.push_str(
        "\n## Repository Results\n\n| Repository | Result | Changes | Review | Validation |\n|---|---|---|---|---|\n",
    );
    for job in &run.jobs {
        let _ = writeln!(
            text,
            "| {} | {} | {} | {} | {} |",
            repository_name(job),
            outcome(job),
            visibility::changes(job).label(),
            review(job),
            validation(job)
        );
    }
    text.push_str(
        "\nChanges are saved completion observations; validation is the latest recorded result.\n",
    );
    text
}

pub(super) fn repository(run: &Run, job: &Job) -> String {
    let changes = visibility::changes(job);
    let mut text = format!(
        "# CodeConvoy — Repository Result\n\nConvoy: #{}\n\nRepository: {}\n\nBackend: {}\n\nResult: {}\n\nDuration: {}\n\n## Changes\n\nCompletion observation: {}\n",
        run.id,
        repository_name(job),
        run.task.agent.label(),
        outcome(job),
        elapsed(job.started_at, job.finished_at),
        changes.label()
    );
    if let Some(files) = changes.files {
        let _ = write!(text, "\nChanged files: {files}\n");
    }
    let _ = write!(
        text,
        "\nReview: {}\n\n## Validation\n\nResult: {}\n",
        review(job),
        validation(job)
    );
    if let Some(record) = &job.validation {
        let _ = write!(text, "\nCommand: {}\n", command(&record.command));
        if let Some(code) = record.exit_code {
            let _ = write!(text, "\nExit code: {code}\n");
        }
        let _ = write!(
            text,
            "\nStarted: {}\n",
            format::timestamp(record.started_at)
        );
        if let Some(finished) = record.finished_at {
            let _ = write!(text, "\nCompleted: {}\n", format::timestamp(finished));
        }
    }
    text
}

fn completion(run: &Run) -> Option<u64> {
    if run.jobs.is_empty() || run.active() || run.jobs.iter().any(|j| j.finished_at.is_none()) {
        return None;
    }
    run.jobs.iter().filter_map(|j| j.finished_at).max()
}

fn elapsed(start: Option<u64>, end: Option<u64>) -> String {
    start
        .zip(end)
        .and_then(|(start, end)| end.checked_sub(start))
        .map_or_else(|| "Unavailable".into(), format::duration)
}

fn outcome(job: &Job) -> &'static str {
    if job.interrupted {
        "Interrupted"
    } else {
        job.status.label()
    }
}

fn review(job: &Job) -> &'static str {
    match job.resolution {
        ResultResolution::Applied => "Applied",
        ResultResolution::Discarded => "Discarded",
        ResultResolution::ApplyPending => "Apply pending / uncertain",
        ResultResolution::DiscardPending => "Discard pending",
        ResultResolution::Unresolved if visibility::needs_review(job) => "Required",
        ResultResolution::Unresolved if job.status.is_terminal() => "Not required",
        ResultResolution::Unresolved => "Unavailable",
    }
}

fn validation(job: &Job) -> &'static str {
    // Configuration is mutable, and was never snapshotted with an unvalidated job.
    // Today's configuration cannot establish historical "Not configured" vs "Not run".
    job.validation
        .as_ref()
        .map_or("Not run / not recorded", |v| v.status.label())
}

fn repository_name(job: &Job) -> String {
    if job.repository.name.trim().is_empty() {
        "Unnamed repository".into()
    } else {
        field(&job.repository.name, 80)
    }
}

fn command(saved: &Command) -> String {
    // Display-only quoting retains argument boundaries. No directory is included.
    let executable = if local_path(&saved.executable) {
        saved.executable.rsplit(['/', '\\']).next().unwrap_or("")
    } else {
        &saved.executable
    };
    let display = Command {
        executable: bounded(executable, 80),
        arguments: saved
            .arguments
            .iter()
            .take(24)
            .map(|arg| {
                if local_path(arg) {
                    "[local path omitted]".into()
                } else {
                    bounded(arg, 160)
                }
            })
            .collect(),
    };
    let mut label = display.label();
    if saved.arguments.len() > display.arguments.len() {
        label.push_str(" …");
    }
    markdown(&bounded(&label, 320))
}

/// Conservative omission of a field containing an absolute or home-relative path.
/// This is not a general secret scrubber; names, tasks and command arguments must
/// still be reviewed before sharing. Recognize Windows paths on every host too.
fn local_path(value: &str) -> bool {
    if value.contains("\\\\") {
        return true;
    }
    if value
        .as_bytes()
        .windows(3)
        .any(|w| w[0].is_ascii_alphabetic() && w[1] == b':' && matches!(w[2], b'/' | b'\\'))
    {
        return true;
    }
    let mut previous = None;
    let mut home_prefix = false;
    let mut short_option_start = false;
    let mut short_option_value = false;
    for ch in value.chars() {
        let token_boundary = path_boundary(previous);
        let at_boundary = token_boundary || short_option_value;
        if matches!(ch, '/' | '\\') && (at_boundary || home_prefix) {
            return true;
        }
        // A token beginning with ~ may contain a named user before its separator.
        // Do not resolve accounts or restrict names to the current host's syntax.
        // Keep that prefix across controls removed by Markdown, but not whitespace.
        if !ch.is_control() || ch.is_whitespace() {
            home_prefix = (ch == '~' && at_boundary) || (home_prefix && !path_boundary(Some(ch)));
            // Recognize -<letter><value> without interpreting any CLI's flag semantics.
            // Only the first value character is a boundary: -Irelative/path stays relative.
            short_option_value = short_option_start && ch.is_ascii_alphabetic();
            short_option_start = ch == '-' && token_boundary;
        }
        previous = Some(ch);
    }
    false
}

fn path_boundary(previous: Option<char>) -> bool {
    previous.is_none_or(|p| {
        p.is_whitespace()
            || p.is_control()
            || matches!(
                p,
                '\'' | '"' | '`' | '=' | ':' | '(' | '[' | '{' | '<' | '>' | ',' | ';' | '|' | '&'
            )
    })
}

fn field(value: &str, limit: usize) -> String {
    if local_path(value) {
        return "[local path omitted]".into();
    }
    markdown(&bounded(value, limit))
}

fn bounded(value: &str, limit: usize) -> String {
    let mut chars = value.chars();
    let mut result: String = chars.by_ref().take(limit).collect();
    if chars.next().is_some() {
        result.push('…');
    }
    result
}

/// A single plain-text Markdown cell/paragraph, also safe from HTML injection.
fn markdown(value: &str) -> String {
    let mut text = String::new();
    for (index, word) in value.split_whitespace().enumerate() {
        if index > 0 {
            text.push(' ');
        }
        for ch in word.chars().filter(|ch| !ch.is_control()) {
            match ch {
                '&' => text.push_str("&amp;"),
                '<' => text.push_str("&lt;"),
                '>' => text.push_str("&gt;"),
                '\\' | '|' | '`' | '*' | '_' | '[' | ']' | '{' | '}' | '#' | '!' | '~' => {
                    text.push('\\');
                    text.push(ch);
                }
                _ => text.push(ch),
            }
        }
    }
    text
}

#[cfg(test)]
#[path = "result_markdown_tests.rs"]
mod tests;
