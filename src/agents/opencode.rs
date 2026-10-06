//! OpenCode's local `run` protocol. See docs/opencode-validation.md for sources.
use super::{AgentBackend, AgentOutput, OptionKind, OptionSpec, value};
use crate::{
    domain::{AgentId, AgentOptions, JobStatus, Repository, TaskConfig},
    process::{CommandSpec, Stream},
};
use anyhow::Result;
use std::{path::Path, process::ExitStatus};

pub struct OpenCode;
static OPTIONS: &[OptionSpec] = &[
    OptionSpec {
        key: "executable",
        label: "OpenCode executable",
        default: "opencode",
        help: "Executable name on PATH or an absolute path. Install and authenticate OpenCode separately.",
        kind: OptionKind::Text { hint: "opencode" },
    },
    OptionSpec {
        key: "model",
        label: "Model",
        default: "",
        help: "Blank uses OpenCode's default. Otherwise enter provider/model from opencode models. Provider access is managed by OpenCode.",
        kind: OptionKind::Text {
            hint: "OpenCode default",
        },
    },
    OptionSpec {
        key: "agent",
        label: "Agent",
        default: "",
        help: "Blank uses OpenCode's default agent. Use a primary agent from opencode agent list (for example build or plan). OpenCode may fall back to its default for an unknown name or a subagent; review output.",
        kind: OptionKind::Text {
            hint: "OpenCode default",
        },
    },
    OptionSpec {
        key: "variant",
        label: "Variant",
        default: "",
        help: "Optional OpenCode model variant, including provider-specific reasoning effort. Names and support depend on your model/configuration. This does not request thinking output.",
        kind: OptionKind::Text {
            hint: "OpenCode default",
        },
    },
    OptionSpec {
        key: "permissions",
        label: "Permissions",
        default: "existing",
        help: "Existing rules uses OpenCode's permissions; requests needing approval are rejected in non-interactive run mode. Auto-approve allows requests unless explicitly denied. Existing rules can already allow shell and file access. Neither mode is an OS sandbox.",
        kind: OptionKind::Choice(&[
            ("existing", "Existing rules; reject asks"),
            ("auto", "Auto-approve; keep denies"),
        ]),
    },
];
const REQUIRED_FLAGS: &[&str] = &[
    "--format",
    "--dir",
    "--model",
    "--agent",
    "--variant",
    "--auto",
];

impl AgentBackend for OpenCode {
    fn id(&self) -> AgentId {
        AgentId::OpenCode
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
            "Enter the OpenCode executable name or absolute path."
        );
        anyhow::ensure!(
            path.is_absolute() || path.components().count() == 1,
            "Use an absolute OpenCode executable path or a name on PATH, not a relative path."
        );
        let model = value(options, &OPTIONS[1]).trim();
        anyhow::ensure!(
            model.is_empty()
                || model
                    .split_once('/')
                    .is_some_and(|(provider, model)| !provider.is_empty()
                        && !model.is_empty()
                        && !model.chars().any(char::is_whitespace)
                        && !provider.chars().any(char::is_whitespace)),
            "OpenCode model must use provider/model format, or be blank for OpenCode default."
        );
        Ok(CommandSpec::new(executable, directory).args(&["run", "--help"]))
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
            help.contains("run [message..]") && help.contains("json"),
            "Expected OpenCode run help with JSON output. Choose the OpenCode CLI from opencode.ai (source contract checked against v1.18.34)."
        );
        for flag in REQUIRED_FLAGS {
            anyhow::ensure!(
                help.split_whitespace().any(|word| {
                    word.strip_prefix(flag).is_some_and(|suffix| {
                        suffix.is_empty() || suffix.starts_with(['[', '=', ','])
                    })
                }),
                "OpenCode run is missing {flag}. Install a compatible OpenCode version (source contract checked against v1.18.34), then Check CLI again."
            );
        }
        Ok(())
    }
    fn build(&self, task: &TaskConfig, repository: &Repository) -> Result<CommandSpec> {
        anyhow::ensure!(
            task.agent == AgentId::OpenCode,
            "OpenCode requires an OpenCode task configuration."
        );
        task.validate(std::slice::from_ref(repository))?;
        self.validate_attachments(task)?;
        let mut spec = self.detection(&task.options, &repository.path)?;
        spec.args.clear();
        spec = spec.args(&["run", "--format", "json", "--dir"]);
        spec.args.push(repository.path.as_os_str().to_owned());
        for (option, flag) in [(1, "--model"), (2, "--agent"), (3, "--variant")] {
            let selected = value(&task.options, &OPTIONS[option]).trim();
            if !selected.is_empty() {
                // A single --key=value argument also keeps leading dashes literal.
                spec.args.push(format!("{flag}={selected}").into());
            }
        }
        if value(&task.options, &OPTIONS[4]) == "auto" {
            spec = spec.args(&["--auto"]);
        }
        for attachment in task.attachments.iter().filter(|a| a.kind.is_image()) {
            let mut argument = std::ffi::OsString::from("--file=");
            argument.push(&attachment.path);
            spec.args.push(argument);
        }
        spec.input = Some(crate::attachments::text_prompt(task)?.into_bytes());
        // OpenCode uses PWD as well as cwd. Keep repository discovery local to this
        // job, while leaving provider/auth/config environment variables untouched.
        spec.remove_env = std::env::vars_os()
            .filter_map(|(key, _)| {
                key.to_str()
                    .filter(|k| k.starts_with("GIT_"))
                    .map(str::to_owned)
            })
            .collect();
        spec.remove_env.push("PWD".into());
        Ok(spec)
    }
    fn output(&self) -> Box<dyn AgentOutput> {
        Box::<OpenCodeOutput>::default()
    }
    fn validate_attachments(&self, task: &TaskConfig) -> Result<()> {
        crate::attachments::validate_limits(&task.attachments)
    }
    fn attachment_help(
        &self,
        task: &TaskConfig,
        directory: &Path,
    ) -> Result<Option<(CommandSpec, &'static str)>> {
        if !task.attachments.iter().any(|a| a.kind.is_image()) {
            return Ok(None);
        }
        Ok(Some((self.detection(&task.options, directory)?, "--file")))
    }
    fn attachment_summary(&self) -> &'static str {
        "Text context via stdin; images via OpenCode's --file flag and local file reader. The selected provider/model must accept images."
    }
    fn execution_summary(&self, options: &AgentOptions) -> String {
        let permission = if value(options, &OPTIONS[4]) == "auto" {
            "Auto-approve requests; explicit OpenCode denies still apply."
        } else {
            "Existing OpenCode rules; requests needing approval are rejected."
        };
        format!(
            "{permission} Configured rules may allow shell and file access; this is not an OS sandbox. Existing OpenCode authentication, providers, agents and configuration are used. OpenCode may retain sessions or auto-share them if configured. Review output and diff."
        )
    }
}

const LINE_LIMIT: usize = 256 * 1024;
/// OpenCode owns its JSON events; no Codex observation or normalized activity.
#[derive(Default)]
struct OpenCodeOutput {
    pending: [Vec<u8>; 2],
    oversized: [bool; 2],
    failed: bool,
    protocol_error: bool,
    stopped: bool,
    activity: String,
}
impl OpenCodeOutput {
    fn line(&mut self, stream: Stream, bytes: &[u8]) -> String {
        let line = String::from_utf8_lossy(bytes);
        if stream == Stream::Stderr {
            self.activity.push_str(&format!("[stderr] {line}"));
            return format!("[stderr] {line}");
        }
        let event = match serde_json::from_slice::<serde_json::Value>(bytes) {
            Ok(event) => event,
            Err(_) => {
                // OpenCode also prints plain diagnostics (e.g. rejected approvals).
                // A broken JSON record, however, cannot be trusted for completion.
                self.protocol_error |= line.trim_start().starts_with('{');
                if line.trim_start().starts_with('{') {
                    self.activity
                        .push_str("Invalid OpenCode JSON event (details in Raw output)\n");
                } else {
                    self.activity.push_str(&line);
                }
                return line.into_owned();
            }
        };
        match event["type"].as_str().unwrap_or("unknown") {
            "step_start" => self.activity.push_str("OpenCode step started\n"),
            "step_finish" => {
                self.activity.push_str(&format!(
                    "OpenCode step finished: {}\n",
                    event["part"]["reason"].as_str().unwrap_or("unknown")
                ));
                if !event["part"]["tokens"].is_null() {
                    self.activity
                        .push_str(&format!("Usage: {}\n", event["part"]["tokens"]));
                }
            }
            "text" => {
                if let Some(text) = event["part"]["text"].as_str() {
                    self.activity.push_str(text);
                    self.activity.push('\n');
                }
            }
            "tool_use" => {
                let part = &event["part"];
                let input = &part["state"]["input"];
                let detail = input["filePath"]
                    .as_str()
                    .or_else(|| input["command"].as_str())
                    .unwrap_or("details in Raw output");
                self.activity.push_str(&format!(
                    "Tool ({}): {} · {detail}\n",
                    part["state"]["status"].as_str().unwrap_or("reported"),
                    part["tool"].as_str().unwrap_or("unknown")
                ));
            }
            "error" => self
                .activity
                .push_str(&format!("OpenCode error: {}\n", event["error"])),
            other => self
                .activity
                .push_str(&format!("OpenCode {other} (details in Raw output)\n")),
        }
        match event["type"].as_str().unwrap_or("") {
            "error" => self.failed = true,
            "step_start" => self.stopped = false,
            "step_finish" => self.stopped = event["part"]["reason"].as_str() == Some("stop"),
            "text" => {
                if let Some(text) = event["part"]["text"].as_str() {
                    return format!("{text}\n");
                }
            }
            _ => {}
        }
        // Keep tool state/input/output, errors, step metadata and unknown events
        // intact for diagnostics and a possible later backend-specific view.
        format!("{}\n", line.trim_end())
    }
    fn flush(&mut self, index: usize, stream: Stream) -> String {
        let bytes = std::mem::take(&mut self.pending[index]);
        if self.oversized[index] {
            let prefix = if stream == Stream::Stderr {
                "[stderr] "
            } else {
                ""
            };
            format!("{prefix}{}", String::from_utf8_lossy(&bytes))
        } else {
            self.line(stream, &bytes)
        }
    }
}
impl AgentOutput for OpenCodeOutput {
    fn push(&mut self, stream: Stream, bytes: &[u8]) -> String {
        let index = usize::from(stream == Stream::Stderr);
        let mut text = String::new();
        for byte in bytes {
            self.pending[index].push(*byte);
            if *byte == b'\n' {
                text.push_str(&self.flush(index, stream));
                self.oversized[index] = false;
            } else if self.pending[index].len() >= LINE_LIMIT {
                if !self.oversized[index] && stream == Stream::Stdout {
                    self.protocol_error = true;
                    self.activity.push_str("OpenCode event exceeded 256 KiB; completion cannot be verified. See Raw output.\n");
                    text.push_str(
                        "[OpenCode event exceeded 256 KiB; completion cannot be verified.]\n",
                    );
                }
                self.oversized[index] = true;
                text.push_str(&self.flush(index, stream));
            }
        }
        text
    }
    fn finish(&mut self) -> String {
        let mut text = String::new();
        for (index, stream) in [Stream::Stdout, Stream::Stderr].into_iter().enumerate() {
            if !self.pending[index].is_empty() {
                text.push_str(&self.flush(index, stream));
            }
        }
        text
    }
    fn interpret(&self, status: ExitStatus) -> (JobStatus, String) {
        if status.success() && self.stopped && !self.failed && !self.protocol_error {
            (JobStatus::Succeeded, "OpenCode completed. Review its response and working-tree diff; denied tools do not prove the task was fulfilled.".into())
        } else {
            (
                JobStatus::Failed,
                format!(
                    "OpenCode completion was not confirmed ({status}). Expected a final step_finish reason=stop, no error events, and valid bounded JSON. Inspect output for provider, model, permission or CLI errors."
                ),
            )
        }
    }
    fn take_activity(&mut self) -> Option<String> {
        Some(std::mem::take(&mut self.activity))
    }
}
