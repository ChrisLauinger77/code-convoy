//! Cursor NDJSON, not Claude events. See docs/cursor-validation.md for the
//! documentation-derived contract and the limits of unauthenticated evidence.
use crate::{agents::AgentOutput, domain::JobStatus, process::Stream};
use serde_json::Value;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    process::ExitStatus,
};

const RECORD_LIMIT: usize = 1024 * 1024;
const TOOL_LIMIT: usize = 128;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Diagnostic {
    Tool,
    Agent,
    Trust,
    Permission,
    Authentication,
}

#[derive(Default)]
pub(super) struct CursorOutput {
    pending: Vec<u8>,
    oversized: bool,
    stderr_pending: Vec<u8>,
    diagnostic_tail: String,
    diagnostic: Option<Diagnostic>,
    invalid: Option<&'static str>,
    workspace: Option<PathBuf>,
    session: Option<String>,
    initialized: bool,
    terminal: bool,
    success: bool,
    tools: BTreeMap<String, (String, String)>,
}

impl CursorOutput {
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
        let text = format!("{}{text}", self.diagnostic_tail).to_lowercase();
        if [
            "authentication required",
            "not authenticated",
            "not logged in",
        ]
        .iter()
        .any(|s| text.contains(s))
        {
            self.diagnostic(Diagnostic::Authentication);
        } else if [
            "permission denied",
            "permission_denied",
            "approval required",
            "approval_required",
        ]
        .iter()
        .any(|s| text.contains(s))
        {
            self.diagnostic(Diagnostic::Permission);
        } else if text.contains("workspace is not trusted")
            || text.contains("workspace trust required")
        {
            self.diagnostic(Diagnostic::Trust);
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
    fn session(&mut self, event: &Value) {
        let Some(id) = event["session_id"].as_str().filter(|id| valid_id(id)) else {
            self.invalid("Missing or invalid Cursor session identifier");
            return;
        };
        if let Some(session) = &self.session {
            if session != id {
                self.invalid("Cursor session identifier changed during execution");
            }
        } else {
            self.session = Some(id.to_owned());
        }
    }
    fn record(&mut self, bytes: &[u8]) -> String {
        if bytes.iter().all(u8::is_ascii_whitespace) {
            return String::new();
        }
        let event = match serde_json::from_slice::<Value>(bytes) {
            Ok(event)
                if event.is_object()
                    && event["type"].as_str().is_some_and(|kind| !kind.is_empty()) =>
            {
                event
            }
            _ => {
                self.invalid("Invalid Cursor JSON record");
                return "[CodeConvoy] Invalid Cursor JSON record; see Raw output.\n".into();
            }
        };
        if self.terminal {
            self.invalid("Cursor emitted an event after its terminal result");
        }
        let kind = event["type"].as_str().unwrap_or("");
        if kind == "system" && event["subtype"] == "init" {
            return self.init(&event);
        }
        if kind == "error" {
            // Native failures may precede init; this sticky diagnostic already
            // precludes success without requiring initialization evidence.
            self.diagnostic(Diagnostic::Agent);
            let message = error_text(&event["error"]);
            self.observe_diagnostic(&format!(
                "{} {message}",
                event["error"]["code"].as_str().unwrap_or("")
            ));
            return format!("[CodeConvoy] Cursor error: {message}\n");
        }
        if !self.initialized {
            self.invalid("Cursor event stream is missing its initialization event");
        }
        self.session(&event);
        match kind {
            "user" => {
                if !event["message"]["content"].is_array() {
                    self.invalid("Invalid Cursor user message");
                }
                "[CodeConvoy] Cursor received task context.\n".into()
            }
            "assistant" => self.assistant(&event),
            "tool_call" => self.tool(&event),
            "result" => {
                self.terminal = true;
                self.success = event["subtype"] == "success"
                    && event["is_error"].as_bool() == Some(false)
                    && event["result"].is_string()
                    && ["duration_ms", "duration_api_ms"]
                        .iter()
                        .all(|key| event[key].as_u64().is_some());
                if !self.success {
                    self.invalid("Cursor did not report a valid successful terminal result");
                }
                if !self.tools.is_empty() {
                    self.invalid("Cursor terminal result has unfinished tool calls");
                }
                format!(
                    "[CodeConvoy] Cursor result: {}\n{}\n",
                    short(event["subtype"].as_str().unwrap_or("unknown")),
                    event["result"].as_str().unwrap_or("")
                )
            }
            _ => {
                self.invalid("Unrecognized Cursor event; completion cannot be confirmed");
                format!(
                    "[CodeConvoy] Unrecognized Cursor event: {}; see Raw output.\n",
                    short(kind)
                )
            }
        }
    }
    fn init(&mut self, event: &Value) -> String {
        if self.initialized {
            self.invalid("Duplicate Cursor initialization event");
        }
        self.initialized = true;
        self.session(event);
        let directory = event["cwd"]
            .as_str()
            .filter(|directory| !directory.is_empty());
        if let (Some(expected), Some(reported)) = (&self.workspace, directory) {
            // One filesystem identity check on the worker, never in UI rendering.
            let expected = expected.canonicalize();
            let reported = if Path::new(reported).is_absolute() {
                Path::new(reported).canonicalize()
            } else {
                Err(std::io::Error::other("Cursor workspace must be absolute"))
            };
            if !matches!((expected, reported), (Ok(expected), Ok(reported)) if expected == reported)
            {
                self.invalid("Cursor reported a different or unavailable workspace");
            }
        } else if directory.is_none() {
            self.invalid("Cursor initialization is missing its workspace");
        }
        format!(
            "[CodeConvoy] Cursor session: {}; model: {}; reported permissions: {}.\n",
            short(event["session_id"].as_str().unwrap_or("unknown")),
            short(event["model"].as_str().unwrap_or("not reported")),
            short(event["permissionMode"].as_str().unwrap_or("not reported"))
        )
    }
    fn assistant(&mut self, event: &Value) -> String {
        let Some(content) = event["message"]["content"].as_array() else {
            self.invalid("Invalid Cursor assistant message");
            return "[CodeConvoy] Invalid Cursor assistant message; see Raw output.\n".into();
        };
        let mut text = String::new();
        for block in content {
            if block["type"] == "text" {
                if let Some(value) = block["text"].as_str() {
                    text.push_str(value);
                    text.push('\n');
                } else {
                    self.invalid("Invalid Cursor assistant text");
                }
            } else {
                self.invalid("Unrecognized Cursor assistant content");
                text.push_str("[CodeConvoy] Cursor assistant content; details in Raw output.\n");
            }
        }
        text
    }
    fn tool(&mut self, event: &Value) -> String {
        let Some(id) = event["call_id"].as_str().filter(|id| valid_id(id)) else {
            self.invalid("Missing or invalid Cursor tool-call identifier");
            return "[CodeConvoy] Invalid Cursor tool call; see Raw output.\n".into();
        };
        let Some(tool) = event["tool_call"]
            .as_object()
            .filter(|tool| tool.len() == 1)
        else {
            self.invalid("Invalid Cursor tool-call payload");
            return "[CodeConvoy] Invalid Cursor tool call; see Raw output.\n".into();
        };
        let Some((name, payload)) = tool.iter().next() else {
            return String::new();
        };
        if !valid_id(name) || !payload.is_object() {
            self.invalid("Invalid Cursor tool-call payload");
            return "[CodeConvoy] Invalid Cursor tool call; see Raw output.\n".into();
        }
        let label = if name == "function" {
            let Some(label) = payload["name"].as_str().filter(|name| valid_id(name)) else {
                self.invalid("Missing or invalid Cursor function name");
                return "[CodeConvoy] Invalid Cursor function call; see Raw output.\n".into();
            };
            label
        } else {
            name.as_str()
        };
        let detail = payload["args"]["path"]
            .as_str()
            .map(short)
            .unwrap_or_default();
        let state = event["subtype"].as_str().unwrap_or("unknown");
        match state {
            "started" => {
                if self.tools.len() >= TOOL_LIMIT || self.tools.contains_key(id) {
                    self.invalid("Too many or duplicate unfinished Cursor tool calls");
                } else {
                    self.tools
                        .insert(id.to_owned(), (name.clone(), label.to_owned()));
                }
            }
            "completed" => {
                if !self
                    .tools
                    .remove(id)
                    .is_some_and(|(key, started_label)| key == *name && started_label == label)
                {
                    self.invalid("Uncorrelated Cursor tool completion");
                }
                let result = &payload["result"];
                if !result["error"].is_null() {
                    self.diagnostic(Diagnostic::Tool);
                    let error = &result["error"];
                    self.observe_diagnostic(&format!(
                        "{} {}",
                        error["code"].as_str().unwrap_or(""),
                        error_text(error)
                    ));
                } else if !result["success"].is_object() {
                    self.invalid("Unrecognized Cursor tool result");
                }
            }
            _ => self.invalid("Unrecognized Cursor tool-call state"),
        }
        format!(
            "[CodeConvoy] Cursor tool {state}: {}{}; details in Raw output.\n",
            short(label),
            if detail.is_empty() {
                String::new()
            } else {
                format!(" · {detail}")
            }
        )
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

impl AgentOutput for CursorOutput {
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
                self.invalid("Cursor stdout record exceeded the 1 MiB limit");
                text.push_str(
                    "[CodeConvoy] Cursor record exceeded the 1 MiB limit; see Raw output.\n",
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
            text.push_str(&format!(
                "[stderr] {}",
                String::from_utf8_lossy(&std::mem::take(&mut self.stderr_pending))
            ));
        }
        if !self.terminal {
            text.push_str("[CodeConvoy] Cursor stream ended without a terminal result.\n");
        }
        text
    }
    fn interpret(&self, status: ExitStatus) -> (JobStatus, String) {
        let diagnostic = match self.diagnostic {
            Some(Diagnostic::Authentication) => Some(
                "Cursor authentication is required. Run agent status / agent login separately; CodeConvoy does not manage credentials.",
            ),
            Some(Diagnostic::Permission) => Some(
                "Cursor reported a permission denial or approval requirement. Review Cursor's allow/deny policy and the requested operation. No broader-permission retry was attempted.",
            ),
            Some(Diagnostic::Trust) => Some(
                "Cursor requires workspace trust. Review the assigned checkout and explicitly select its trust setting if appropriate. No automatic trust retry was attempted.",
            ),
            Some(Diagnostic::Tool) => {
                Some("Cursor reported a failed tool call; task completion cannot be confirmed.")
            }
            Some(Diagnostic::Agent) => {
                Some("Cursor reported an explicit error; task completion cannot be confirmed.")
            }
            None => None,
        };
        if let Some(diagnostic) = diagnostic {
            return (
                JobStatus::Failed,
                format!("{diagnostic} Process {status}. See Activity/Raw output."),
            );
        }
        if !status.success() {
            return (
                JobStatus::Failed,
                format!("Cursor process failed ({status}). See Activity/Raw output."),
            );
        }
        if let Some(reason) = self.invalid {
            return (
                JobStatus::Failed,
                format!("{reason}. See Activity/Raw output."),
            );
        }
        if !self.initialized || !self.terminal || !self.success || !self.tools.is_empty() {
            return (JobStatus::Failed, "Cursor event stream was incomplete or had no valid successful terminal result. See Activity/Raw output.".into());
        }
        (
            JobStatus::Succeeded,
            format!(
                "Cursor reported a successful terminal result (session {}).",
                self.session.as_deref().map(short).unwrap_or_default()
            ),
        )
    }
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
fn error_text(error: &Value) -> String {
    short(
        error
            .as_str()
            .or_else(|| error["message"].as_str())
            .unwrap_or("details in Raw output"),
    )
}
