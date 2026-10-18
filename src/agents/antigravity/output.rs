//! Antigravity headless stream-json, not another backend's events. The contract
//! is derived from Google's headless guide; see docs/antigravity-validation.md.
use crate::{agents::AgentOutput, domain::JobStatus, process::Stream};
use serde_json::Value;
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
    process::ExitStatus,
};

const RECORD_LIMIT: usize = 1024 * 1024;
const STEP_LIMIT: usize = 1024;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Diagnostic {
    Agent,
    Authentication,
}

/// Recovery guidance recognized in Antigravity's own error text. It only adds
/// advice to a failure; it never changes classification.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Hint {
    SlashCommand,
    Model,
    Quota,
}
impl Hint {
    fn advice(self) -> &'static str {
        match self {
            Self::SlashCommand => {
                " The task starts with a slash command that agy answers itself, such as /model; start the task with ordinary text."
            }
            Self::Model => {
                " The selected model is not available; choose a slug listed by agy models, or leave Model blank."
            }
            Self::Quota => {
                " Antigravity reports exhausted quota or AI credits; check agy -p /usage in a terminal before retrying."
            }
        }
    }
}

#[derive(Default)]
pub(super) struct AntigravityOutput {
    pending: Vec<u8>,
    oversized: bool,
    stderr_pending: Vec<u8>,
    diagnostic_tail: String,
    diagnostic: Option<Diagnostic>,
    hint: Option<Hint>,
    invalid: Option<&'static str>,
    workspace: Option<PathBuf>,
    conversation: Option<String>,
    initialized: bool,
    results: usize,
    // Step activity after the latest result needs another result to finish.
    unfinished: bool,
    failed: Option<(String, String)>,
    denied: usize,
    denied_unrecognized: bool,
    streamed: bool,
    // Steps reported ACTIVE that have not yet reported DONE, of any type.
    active_steps: BTreeSet<u64>,
}

impl AntigravityOutput {
    pub(super) fn for_workspace(directory: &Path) -> Self {
        Self {
            workspace: Some(directory.to_owned()),
            ..Self::default()
        }
    }
    fn invalid(&mut self, reason: &'static str) {
        self.invalid.get_or_insert(reason);
    }
    fn diagnostic(&mut self, diagnostic: Diagnostic) {
        self.diagnostic = self.diagnostic.max(Some(diagnostic));
    }
    fn observe_diagnostic(&mut self, text: &str) {
        // Keep a small overlap so a diagnostic split across chunks is recognized.
        let text = format!("{}{text}", self.diagnostic_tail);
        let lower = text.to_lowercase();
        if [
            "authentication required",
            "authentication failed",
            "authentication expired",
            "please log back in",
            "not authenticated",
        ]
        .iter()
        .any(|s| lower.contains(s))
        {
            self.diagnostic(Diagnostic::Authentication);
        }
        // Model/agent failures print a structured AGY_ERROR line on stderr.
        if text.contains("AGY_ERROR") {
            self.diagnostic(Diagnostic::Agent);
        }
        for (hint, markers) in [
            (Hint::SlashCommand, &["answered by the cli itself"][..]),
            (
                Hint::Model,
                &["invalid model selection", "not recognized as a known model"][..],
            ),
            (
                Hint::Quota,
                &[
                    "quota",
                    "resource_exhausted",
                    "credits balance",
                    "out of credits",
                ][..],
            ),
        ] {
            if markers.iter().any(|marker| lower.contains(marker)) {
                self.hint = self.hint.max(Some(hint));
            }
        }
        self.diagnostic_tail = text
            .chars()
            .rev()
            .take(256)
            .collect::<String>()
            .chars()
            .rev()
            .collect();
    }
    fn conversation(&mut self, id: &Value) {
        let Some(id) = id.as_str().filter(|id| valid_id(id)) else {
            self.invalid("Missing or invalid Antigravity conversation identifier");
            return;
        };
        match &self.conversation {
            Some(conversation) if conversation != id => {
                self.invalid("Antigravity conversation identifier changed during execution");
            }
            Some(_) => {}
            None => self.conversation = Some(id.to_owned()),
        }
    }
    fn record(&mut self, bytes: &[u8]) -> String {
        if bytes.iter().all(u8::is_ascii_whitespace) {
            return String::new();
        }
        let event = match serde_json::from_slice::<Value>(bytes) {
            Ok(event)
                if event.is_object()
                    && event["event"].as_str().is_some_and(|kind| !kind.is_empty()) =>
            {
                event
            }
            _ => {
                self.invalid("Invalid Antigravity JSON record");
                return "[CodeConvoy] Invalid Antigravity JSON record; see Raw output.\n".into();
            }
        };
        let kind = event["event"].as_str().unwrap_or("");
        let payload = &event[kind];
        if !payload.is_object() {
            self.invalid("Antigravity event is missing its payload");
            return format!(
                "[CodeConvoy] Antigravity {} event without payload; see Raw output.\n",
                short(kind)
            );
        }
        match kind {
            "init" => self.init(&event, payload),
            "step_update" => {
                if !self.initialized {
                    self.invalid("Antigravity event stream is missing its initialization event");
                }
                self.conversation(&payload["conversation_id"]);
                self.unfinished = self.results > 0;
                self.step(payload)
            }
            "result" => self.result(payload),
            _ => {
                self.invalid("Unrecognized Antigravity event; completion cannot be confirmed");
                format!(
                    "[CodeConvoy] Unrecognized Antigravity event: {}; see Raw output.\n",
                    short(kind)
                )
            }
        }
    }
    fn init(&mut self, event: &Value, init: &Value) -> String {
        if self.initialized || self.results > 0 {
            self.invalid("Duplicate or late Antigravity initialization event");
        }
        self.initialized = true;
        self.conversation(&event["conversation_id"]);
        let directory = init["cwd"]
            .as_str()
            .filter(|directory| !directory.is_empty());
        if let (Some(expected), Some(reported)) = (&self.workspace, directory) {
            // One filesystem identity check on the worker, never in UI rendering.
            let expected = expected.canonicalize();
            let reported = if Path::new(reported).is_absolute() {
                Path::new(reported).canonicalize()
            } else {
                Err(std::io::Error::other(
                    "Antigravity working directory must be absolute",
                ))
            };
            if !matches!((expected, reported), (Ok(expected), Ok(reported)) if expected == reported)
            {
                self.invalid("Antigravity reported a different or unavailable working directory");
            }
        } else if directory.is_none() {
            self.invalid("Antigravity initialization is missing its working directory");
        }
        format!(
            "[CodeConvoy] Antigravity conversation: {}; model: {}; reported permissions: {}.\n",
            short(event["conversation_id"].as_str().unwrap_or("unknown")),
            short(init["model"].as_str().unwrap_or("Antigravity default")),
            short(init["permission_mode"].as_str().unwrap_or("not reported"))
        )
    }
    fn step(&mut self, step: &Value) -> String {
        let Some(index) = step["step_index"].as_u64() else {
            self.invalid("Missing or invalid Antigravity step index");
            return "[CodeConvoy] Invalid Antigravity step; see Raw output.\n".into();
        };
        let done = match step["state"].as_str() {
            Some("DONE") => true,
            Some("ACTIVE") => false,
            _ => {
                self.invalid("Unrecognized Antigravity step state");
                return "[CodeConvoy] Invalid Antigravity step state; see Raw output.\n".into();
            }
        };
        // ACTIVE means still running for every step type, including streamed
        // responses; untracked steps could hide one, so overflow fails.
        let started = if done {
            self.active_steps.remove(&index);
            false
        } else if self.active_steps.contains(&index) {
            false
        } else if self.active_steps.len() >= STEP_LIMIT {
            self.invalid("Too many unfinished Antigravity steps");
            false
        } else {
            self.active_steps.insert(index)
        };
        let kind = step["step_type"].as_str().unwrap_or("");
        let mut text = String::new();
        if !step["text_delta"].is_null() {
            match step["text_delta"].as_str() {
                Some(delta) if kind == "agent_response" => {
                    self.streamed |= !delta.is_empty();
                    text.push_str(delta);
                }
                Some(_) => {}
                None => self.invalid("Invalid Antigravity text delta"),
            }
        }
        match kind {
            "agent_response" | "checkpoint" => {}
            "user_input" if done => {
                text.push_str("[CodeConvoy] Antigravity received the task.\n");
            }
            "user_input" => {}
            "tool" => text.push_str(&tool(done, started, step)),
            "" => self.invalid("Missing Antigravity step type"),
            other => text.push_str(&format!(
                "[CodeConvoy] Antigravity step: {}; details in Raw output.\n",
                short(other)
            )),
        }
        if let Some(subagents) = step["subagent_info"]["subagents"].as_array() {
            let roles = subagents
                .iter()
                .map(|agent| {
                    short(
                        agent["role"]
                            .as_str()
                            .or_else(|| agent["type_name"].as_str())
                            .unwrap_or("subagent"),
                    )
                })
                .collect::<Vec<_>>()
                .join(", ");
            text.push_str(&format!(
                "[CodeConvoy] Antigravity subagent activity: {roles}; details in Raw output.\n"
            ));
        }
        text
    }
    fn result(&mut self, result: &Value) -> String {
        self.results += 1;
        self.unfinished = false;
        if !self.initialized {
            self.invalid("Antigravity event stream is missing its initialization event");
        } else {
            self.conversation(&result["conversation_id"]);
        }
        let status = result["status"].as_str().unwrap_or("");
        // The documented error field is a string, present only on failure.
        let error = match &result["error"] {
            Value::Null => "",
            Value::String(error) => error.trim(),
            _ => {
                self.invalid("Antigravity result has a non-string error");
                ""
            }
        };
        self.observe_diagnostic(error);
        if !result["response"].is_string() || result["num_turns"].as_u64().is_none() {
            self.invalid("Antigravity result is missing its response or turn count");
        }
        if status == "SUCCESS" {
            if !error.is_empty() || result["num_turns"].as_u64() == Some(0) {
                self.invalid("Antigravity reported an inconsistent successful result");
            }
            // ACTIVE means still running; a finished turn cannot have one left.
            if !self.active_steps.is_empty() {
                self.invalid("Antigravity reported success while a step was still active");
            }
        } else {
            self.failed.get_or_insert_with(|| {
                (
                    if status.is_empty() {
                        "no status".into()
                    } else {
                        short(status)
                    },
                    short(error),
                )
            });
        }
        let mut text = format!(
            "[CodeConvoy] Antigravity result: {}{}\n",
            short(if status.is_empty() {
                "missing status"
            } else {
                status
            }),
            if error.is_empty() {
                String::new()
            } else {
                format!(" · {}", short(error))
            }
        );
        if !std::mem::take(&mut self.streamed)
            && let Some(response) = result["response"].as_str()
            && !response.is_empty()
        {
            text.push_str(response);
            if !response.ends_with('\n') {
                text.push('\n');
            }
        }
        match &result["denied_actions"] {
            Value::Null => {}
            Value::Array(denied) => {
                self.denied += denied.len();
                if !denied.is_empty() {
                    text.push_str(&format!(
                        "Warning: Antigravity refused {} tool action(s) that needed approval; see Raw output.\n",
                        denied.len()
                    ));
                }
            }
            _ => {
                self.denied_unrecognized = true;
                text.push_str(
                    "Warning: Antigravity reported refused actions in an unrecognized form; see Raw output.\n",
                );
            }
        }
        if let Some(tokens) = result["usage"]["total_tokens"].as_u64() {
            text.push_str(&format!("Usage: {tokens} total tokens.\n"));
        }
        text
    }
    fn stderr(&mut self, bytes: &[u8]) -> String {
        self.stderr_pending.extend_from_slice(bytes);
        let mut text = String::new();
        let mut consumed = 0;
        while consumed < self.stderr_pending.len() {
            match std::str::from_utf8(&self.stderr_pending[consumed..]) {
                Ok(valid) => {
                    text.push_str(valid);
                    consumed = self.stderr_pending.len();
                }
                Err(error) => {
                    let end = consumed + error.valid_up_to();
                    text.push_str(&String::from_utf8_lossy(
                        &self.stderr_pending[consumed..end],
                    ));
                    consumed = end;
                    if let Some(count) = error.error_len() {
                        text.push('\u{fffd}');
                        consumed += count;
                    } else {
                        break;
                    }
                }
            }
        }
        self.stderr_pending.drain(..consumed);
        self.observe_diagnostic(&text);
        if text.is_empty() {
            text
        } else {
            format!("[stderr] {text}")
        }
    }
}

impl AgentOutput for AntigravityOutput {
    fn push(&mut self, stream: Stream, bytes: &[u8]) -> String {
        if stream == Stream::Stderr {
            return self.stderr(bytes);
        }
        let mut text = String::new();
        for &byte in bytes {
            if self.oversized {
                if byte == b'\n' {
                    self.oversized = false;
                }
                continue;
            }
            if byte == b'\n' {
                let record = std::mem::take(&mut self.pending);
                text.push_str(&self.record(&record));
            } else if self.pending.len() == RECORD_LIMIT {
                self.pending.clear();
                self.oversized = true;
                self.invalid("Antigravity stdout record exceeded the 1 MiB limit");
                text.push_str(
                    "[CodeConvoy] Antigravity record exceeded the 1 MiB limit; see Raw output.\n",
                );
            } else {
                self.pending.push(byte);
            }
        }
        text
    }
    fn finish(&mut self) -> String {
        let mut text = String::new();
        if !self.pending.is_empty() {
            let record = std::mem::take(&mut self.pending);
            text.push_str(&self.record(&record));
        }
        if !self.stderr_pending.is_empty() {
            let rest =
                String::from_utf8_lossy(&std::mem::take(&mut self.stderr_pending)).into_owned();
            self.observe_diagnostic(&rest);
            text.push_str(&format!("[stderr] {rest}"));
        }
        if self.results == 0 || self.unfinished {
            text.push_str("[CodeConvoy] Antigravity stream ended without a final result.\n");
        }
        text
    }
    fn interpret(&self, status: ExitStatus) -> (JobStatus, String) {
        // Summaries show the text before the first colon. Recognized advice
        // therefore precedes Antigravity's own error text, which may contain
        // colons, and the outcome suffix is stated without one. Every failure
        // keeps the known conversation ID for Antigravity's local history.
        let advice = self.hint.map(Hint::advice).unwrap_or_default();
        let outcome = format!(
            "({}{}) See Activity/Raw output.",
            exit(status),
            self.conversation
                .as_deref()
                .map(|id| format!("; conversation {}", short(id)))
                .unwrap_or_default()
        );
        let failed = |detail: String| (JobStatus::Failed, detail);
        if self.diagnostic == Some(Diagnostic::Authentication) {
            return failed(format!(
                "Antigravity sign-in is required or has expired. Run agy once in a terminal with the same installation and user account to sign in, then retry; CodeConvoy does not manage credentials. {outcome}"
            ));
        }
        if let Some((reported, error)) = &self.failed {
            let state = match reported.as_str() {
                "WAITING" => " The run ended waiting for input that a headless run cannot provide.",
                "CANCELED" | "INTERRUPTED" => " The run was stopped before it completed.",
                _ => "",
            };
            let error = if error.is_empty() {
                String::new()
            } else {
                format!(" Antigravity error — {error}.")
            };
            return failed(format!(
                "Antigravity reported {reported}.{state}{advice}{error} {outcome}"
            ));
        }
        if self.diagnostic == Some(Diagnostic::Agent) {
            return failed(format!(
                "Antigravity reported a model or agent error (AGY_ERROR), so task completion cannot be confirmed.{advice} {outcome}"
            ));
        }
        if !status.success() {
            return failed(format!("Antigravity process failed.{advice} {outcome}"));
        }
        if let Some(reason) = self.invalid {
            return failed(format!("{reason}. {outcome}"));
        }
        if !self.initialized || self.results == 0 || self.unfinished {
            return failed(format!(
                "Antigravity event stream was incomplete or had no final SUCCESS result. {outcome}"
            ));
        }
        let mut detail = format!(
            "Antigravity reported a SUCCESS result (conversation {}).",
            self.conversation.as_deref().map(short).unwrap_or_default()
        );
        if self.denied > 0 {
            detail.push_str(&format!(
                " It refused {} tool action(s) that needed approval, so the task may be incomplete; review Activity, Raw output and the diff.",
                self.denied
            ));
        } else if self.denied_unrecognized {
            detail.push_str(
                " It reported refused actions in an unrecognized form; review Raw output and the diff.",
            );
        }
        (JobStatus::Succeeded, detail)
    }
}

/// Outcome without a colon (summaries end at the first one), keeping the
/// signal so an external kill stays distinguishable from a crash.
fn exit(status: ExitStatus) -> String {
    if let Some(code) = status.code() {
        return format!("exit code {code}");
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        if let Some(signal) = status.signal() {
            return format!(
                "terminated by signal {signal}{}{}",
                signal_name(signal)
                    .map(|name| format!(" ({name})"))
                    .unwrap_or_default(),
                if status.core_dumped() {
                    ", core dumped"
                } else {
                    ""
                }
            );
        }
    }
    "process ended without an exit code".to_owned()
}
#[cfg(unix)]
fn signal_name(signal: i32) -> Option<&'static str> {
    Some(match signal {
        libc::SIGHUP => "SIGHUP",
        libc::SIGINT => "SIGINT",
        libc::SIGQUIT => "SIGQUIT",
        libc::SIGILL => "SIGILL",
        libc::SIGTRAP => "SIGTRAP",
        libc::SIGABRT => "SIGABRT",
        libc::SIGBUS => "SIGBUS",
        libc::SIGFPE => "SIGFPE",
        libc::SIGKILL => "SIGKILL",
        libc::SIGSEGV => "SIGSEGV",
        libc::SIGPIPE => "SIGPIPE",
        libc::SIGALRM => "SIGALRM",
        libc::SIGTERM => "SIGTERM",
        _ => return None,
    })
}
fn tool(done: bool, started: bool, step: &Value) -> String {
    let info = &step["tool_info"];
    let name = short(
        step["tool_name"]
            .as_str()
            .or_else(|| info["name"].as_str())
            .unwrap_or("tool"),
    );
    if !done {
        return if started {
            format!("[CodeConvoy] Antigravity tool started: {name}.\n")
        } else {
            String::new()
        };
    }
    if info["error"].is_object() {
        // Antigravity reports recoverable tool errors without failing the run.
        return format!(
            "[CodeConvoy] Antigravity tool failed: {name} · {}; details in Raw output.\n",
            short(info["error"]["message"].as_str().unwrap_or("no message"))
        );
    }
    let detail = info["parameters"]["CommandLine"]
        .as_str()
        .map(|command| format!(" · {}", short(command)))
        .unwrap_or_default();
    format!("[CodeConvoy] Antigravity tool: {name}{detail}; details in Raw output.\n")
}
fn valid_id(id: &str) -> bool {
    !id.trim().is_empty() && id.len() <= 256 && !id.chars().any(char::is_control)
}
fn short(text: &str) -> String {
    text.chars()
        .take(256)
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect()
}
