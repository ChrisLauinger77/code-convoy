//! Claude Code print mode, not a translation of another agent's protocol.
//! Contract evidence and limits: docs/claude-validation.md.
use super::{AgentBackend, AgentOutput, OptionKind, OptionSpec, value};
use crate::{
    domain::{AgentId, AgentOptions, JobStatus, Repository, TaskConfig},
    process::{CommandSpec, Stream},
};
use anyhow::Result;
use serde_json::Value;
use std::{path::Path, process::ExitStatus};

pub struct Claude;
static OPTIONS: &[OptionSpec] = &[
    OptionSpec {
        key: "executable",
        label: "Claude executable",
        default: "claude",
        help: "Claude Code executable on PATH or an absolute path. Installation and authentication are managed separately. On Windows use the native claude.exe, not a .cmd/.bat wrapper.",
        kind: OptionKind::Text { hint: "claude" },
    },
    OptionSpec {
        key: "model",
        label: "Model",
        default: "",
        help: "Blank uses Claude's configured default. Otherwise enter a Claude model alias or full model identifier. Availability is determined by Claude Code and your account.",
        kind: OptionKind::Text {
            hint: "Claude default",
        },
    },
    OptionSpec {
        key: "effort",
        label: "Effort",
        default: "",
        help: "Claude's session effort override. Supported levels depend on your model and CLI. Blank preserves existing configuration.",
        kind: OptionKind::Choice(&[
            ("", "Claude default"),
            ("low", "Low"),
            ("medium", "Medium"),
            ("high", "High"),
            ("xhigh", "Extra high"),
            ("max", "Max"),
        ]),
    },
    OptionSpec {
        key: "permission_mode",
        label: "Permissions",
        default: "dontAsk",
        help: "Existing approvals denies calls that need new approval; existing allow rules still apply. Accept edits also approves file edits and common filesystem commands, including mkdir, touch, mv and cp. Print mode has no permission host, so unresolved requests are denied. Neither choice is an OS sandbox.",
        kind: OptionKind::Choice(&[
            ("dontAsk", "Existing approvals; deny asks"),
            ("acceptEdits", "Accept edits / file commands"),
        ]),
    },
    OptionSpec {
        key: "max_turns",
        label: "Max turns",
        default: "",
        help: "Optional positive integer limiting Claude's agentic turns. Reaching the limit is a failed run, not successful completion. Blank leaves Claude's default (no turn limit). This is not a time limit.",
        kind: OptionKind::Text {
            hint: "Claude default (unlimited)",
        },
    },
];
// Claude documents that --help omits some supported flags. Check the core
// visible interface, not every optional/hidden flag. Jobs still fail closed
// when the installed CLI rejects a documented flag; no authenticated probes.
const CORE_FLAGS: &[&str] = &[
    "--print",
    "--output-format",
    "--permission-mode",
    "--verbose",
];

impl AgentBackend for Claude {
    fn id(&self) -> AgentId {
        AgentId::Claude
    }
    fn options(&self) -> &'static [OptionSpec] {
        OPTIONS
    }
    fn detection(&self, options: &AgentOptions, directory: &Path) -> Result<CommandSpec> {
        self.validate(options)?;
        let executable = value(options, &OPTIONS[0]);
        let path = Path::new(executable);
        anyhow::ensure!(
            !executable.trim().is_empty(),
            "Enter the Claude Code executable name or absolute path."
        );
        anyhow::ensure!(
            path.is_absolute() || path.components().count() == 1,
            "Use an absolute Claude Code executable path or a name on PATH, not a relative path."
        );
        #[cfg(windows)]
        anyhow::ensure!(
            !path
                .extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| e.eq_ignore_ascii_case("cmd") || e.eq_ignore_ascii_case("bat")),
            "Use the native Claude Code claude.exe on Windows, not a shell wrapper."
        );
        let turns = value(options, &OPTIONS[4]).trim();
        anyhow::ensure!(
            turns.is_empty()
                || (turns.bytes().all(|b| b.is_ascii_digit())
                    && turns.parse::<u32>().is_ok_and(|n| n > 0)),
            "Claude max turns must be a positive integer (at most 4294967295), or blank for no limit."
        );
        Ok(CommandSpec::new(executable, directory).args(&["--help"]))
    }
    fn version_command(
        &self,
        options: &AgentOptions,
        directory: &Path,
    ) -> Result<Option<CommandSpec>> {
        let mut spec = self.detection(options, directory)?;
        spec.args.clear();
        Ok(Some(spec.args(&["--version"])))
    }
    fn check_detection(&self, help: &str) -> Result<()> {
        anyhow::ensure!(
            help.contains("Claude Code") && help.contains("stream-json"),
            "Expected Claude Code help with stream-json output. Install Claude Code separately or choose its executable path, then Check CLI again."
        );
        for flag in CORE_FLAGS {
            anyhow::ensure!(
                help.split_whitespace().any(|word| {
                    word.strip_prefix(flag).is_some_and(|suffix| {
                        suffix.is_empty() || suffix.starts_with(['[', '=', ','])
                    })
                }),
                "Claude Code help is missing the core {flag} interface. Choose a current Claude Code executable and Check CLI again."
            );
        }
        Ok(())
    }
    fn build(&self, task: &TaskConfig, repository: &Repository) -> Result<CommandSpec> {
        anyhow::ensure!(
            task.agent == AgentId::Claude,
            "Claude requires a Claude Code task configuration."
        );
        task.validate(std::slice::from_ref(repository))?;
        let mut spec = self.detection(&task.options, &repository.path)?;
        spec.args.clear();
        spec = spec.args(&[
            "--print",
            "--input-format",
            "text",
            "--output-format",
            "stream-json",
            "--verbose",
            "--no-session-persistence",
            "--permission-mode",
            value(&task.options, &OPTIONS[3]),
        ]);
        for (index, flag) in [(1, "--model"), (2, "--effort"), (4, "--max-turns")] {
            let selected = value(&task.options, &OPTIONS[index]).trim();
            if !selected.is_empty() {
                spec.args.push(format!("{flag}={selected}").into());
            }
        }
        spec.input = Some(task.prompt.as_bytes().to_vec());
        spec.remove_env = std::env::vars_os()
            .filter_map(|(key, _)| {
                key.to_str()
                    .filter(|k| k.starts_with("GIT_"))
                    .map(str::to_owned)
            })
            .collect();
        // Do not let tools consult another invocation's logical working directory.
        // Claude's auth, configuration and provider variables are inherited.
        spec.remove_env.push("PWD".into());
        Ok(spec)
    }
    fn output(&self) -> Box<dyn AgentOutput> {
        Box::<ClaudeOutput>::default()
    }
    fn execution_summary(&self, options: &AgentOptions) -> String {
        let mode = if value(options, &OPTIONS[3]) == "acceptEdits" {
            "File edits and common filesystem commands are automatically approved."
        } else {
            "Existing approvals apply; calls needing new approval are denied."
        };
        format!(
            "{mode} No interactive permission host. These are Claude tool permissions, not an OS sandbox; configured access and hooks still apply. Existing Claude authentication/configuration is used. Print mode skips workspace trust prompts and can run repository hooks/MCP servers. Only use trusted repositories. Session persistence is disabled; other Claude logs may remain."
        )
    }
}

// Matches the official Python Agent SDK's default stdout buffer limit, allowing
// larger records than the older backends' 256 KiB. Existing UI/log caps still apply.
const RECORD_LIMIT: usize = 1024 * 1024;
#[derive(Default)]
struct ClaudeOutput {
    pending: [Vec<u8>; 2],
    oversized: [bool; 2],
    protocol_error: bool,
    explicit_error: bool,
    // Keep the parsed Claude result (including usage/denials/metadata) for outcome
    // interpretation, without introducing a shared activity/event abstraction.
    result: Option<Value>,
}
impl ClaudeOutput {
    fn record(&mut self, bytes: &[u8]) -> String {
        if bytes.iter().all(u8::is_ascii_whitespace) {
            return String::new();
        }
        let event = match serde_json::from_slice::<Value>(bytes) {
            Ok(event) if event.is_object() && event["type"].as_str().is_some() => event,
            _ => {
                self.protocol_error = true;
                return format!(
                    "[Invalid Claude JSON record]\n{}\n",
                    String::from_utf8_lossy(bytes).trim_end()
                );
            }
        };
        // A print invocation has a single final result, documented as the last
        // record. A duplicate result or later activity is not a confirmed finish.
        self.protocol_error |= self.result.is_some();
        let kind = event["type"].as_str().unwrap_or("");
        self.explicit_error |= kind == "error"
            || (kind == "assistant" && !event["error"].is_null())
            || (kind == "system" && event["subtype"].as_str() == Some("error"));
        if kind == "result" {
            let mut display = event.clone();
            let text = event["result"].as_str().unwrap_or("");
            if event["result"].is_string()
                && let Some(object) = display.as_object_mut()
            {
                object.remove("result");
            }
            let rendered = format!("[Claude result]\n{text}\n[Claude result metadata] {display}\n");
            self.result = Some(event);
            return rendered;
        }
        // Preserve complete backend-specific records (including tool payloads,
        // assistant messages, retries, usage and unknown future types) in Output.
        format!("{}\n", String::from_utf8_lossy(bytes).trim_end())
    }
    fn flush(&mut self, index: usize) -> String {
        let bytes = std::mem::take(&mut self.pending[index]);
        if index == 1 {
            format!("[stderr] {}", String::from_utf8_lossy(&bytes))
        } else if self.oversized[index] {
            String::from_utf8_lossy(&bytes).into_owned()
        } else {
            self.record(&bytes)
        }
    }
    fn confirmed_result(&self) -> bool {
        let Some(result) = &self.result else {
            return false;
        };
        result["subtype"].as_str() == Some("success")
            && result["is_error"].as_bool() == Some(false)
            && result["result"].is_string()
            && result["session_id"]
                .as_str()
                .is_some_and(|id| !id.is_empty())
            && ["duration_ms", "duration_api_ms", "num_turns"]
                .iter()
                .all(|key| result[key].as_u64().is_some())
            && (result["terminal_reason"].is_null()
                || result["terminal_reason"].as_str() == Some("completed"))
            // A final response explicitly marked truncated/paused is not a
            // completed answer, even on older CLIs without terminal_reason.
            && (result["stop_reason"].is_null() || result["stop_reason"].is_string())
            && !matches!(
                result["stop_reason"].as_str(),
                Some("max_tokens" | "pause_turn" | "model_context_window_exceeded")
            )
            && result["deferred_tool_use"].is_null()
            && result["api_error_status"].is_null()
            && (result["errors"].is_null()
                || result["errors"].as_array().is_some_and(Vec::is_empty))
    }
}
impl AgentOutput for ClaudeOutput {
    fn push(&mut self, stream: Stream, bytes: &[u8]) -> String {
        let index = usize::from(stream == Stream::Stderr);
        let mut text = String::new();
        for byte in bytes {
            self.pending[index].push(*byte);
            if *byte == b'\n' {
                text.push_str(&self.flush(index));
                self.oversized[index] = false;
            } else if self.pending[index].len() > RECORD_LIMIT {
                if index == 0 && !self.oversized[index] {
                    self.protocol_error = true;
                    text.push_str(
                        "[Claude JSON record exceeded 1 MiB; completion cannot be verified.]\n",
                    );
                }
                self.oversized[index] = true;
                text.push_str(&self.flush(index));
            }
        }
        text
    }
    fn finish(&mut self) -> String {
        let mut text = String::new();
        for index in 0..2 {
            if !self.pending[index].is_empty() {
                text.push_str(&self.flush(index));
            }
        }
        text
    }
    fn interpret(&self, status: ExitStatus) -> (JobStatus, String) {
        if status.success()
            && self.confirmed_result()
            && !self.protocol_error
            && !self.explicit_error
        {
            (JobStatus::Succeeded, "Claude Code completed. Review its response, permission denials and working-tree diff; CLI completion does not prove task fulfillment.".into())
        } else {
            (
                JobStatus::Failed,
                format!(
                    "Claude Code completion was not confirmed ({status}). Expected one valid final success result with is_error=false and no execution/protocol errors. Inspect output for interrupted execution, turn limits, authentication, model, permission or CLI errors."
                ),
            )
        }
    }
}
