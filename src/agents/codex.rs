use super::{AgentBackend, AgentOutput, OptionKind, OptionSpec, value};
use crate::{
    domain::{AgentId, AgentOptions, JobStatus, Repository, TaskConfig},
    process::{CommandSpec, Stream},
};
use anyhow::Result;
use std::{path::Path, process::ExitStatus};

pub struct Codex;
static OPTIONS: &[OptionSpec] = &[
    OptionSpec {
        key: "executable",
        label: "Codex executable",
        default: "codex",
        help: "Executable name on PATH or a full path. No shell commands or extra arguments.",
        kind: OptionKind::Text { hint: "codex" },
    },
    OptionSpec {
        key: "model",
        label: "Model",
        default: "",
        help: "Blank uses your CLI configuration. Available models depend on your Codex account.",
        kind: OptionKind::Text {
            hint: "CLI default",
        },
    },
    OptionSpec {
        key: "model_reasoning_effort",
        label: "Reasoning effort",
        default: "",
        help: "Codex model_reasoning_effort; support depends on your model and CLI version.",
        kind: OptionKind::Choice(&[
            ("", "CLI default"),
            ("minimal", "Minimal"),
            ("low", "Low"),
            ("medium", "Medium"),
            ("high", "High"),
            ("xhigh", "Extra high"),
        ]),
    },
    OptionSpec {
        key: "sandbox",
        label: "Sandbox",
        default: "read-only",
        help: "Read-only inspects. Workspace-write lets Codex edit the selected repository. Approval requests are disabled for unattended execution.",
        kind: OptionKind::Choice(&[
            ("read-only", "Read-only"),
            ("workspace-write", "Workspace-write"),
        ]),
    },
];
impl AgentBackend for Codex {
    fn id(&self) -> AgentId {
        AgentId::Codex
    }
    fn options(&self) -> &'static [OptionSpec] {
        OPTIONS
    }
    fn detection(&self, options: &AgentOptions, directory: &Path) -> Result<CommandSpec> {
        let executable = value(options, &OPTIONS[0]);
        anyhow::ensure!(
            !executable.trim().is_empty(),
            "Enter the Codex executable name or path."
        );
        let path = Path::new(executable);
        anyhow::ensure!(
            path.is_absolute() || path.components().count() == 1,
            "Use an absolute Codex executable path or a name on PATH, not a relative path."
        );
        Ok(CommandSpec::new(executable, directory).args(&["--help"]))
    }
    fn check_detection(&self, help: &str) -> Result<()> {
        anyhow::ensure!(
            help.contains("--no-daemon")
                && help.contains("--ask-for-approval")
                && help.contains("exec"),
            "This Codex CLI is unsupported. Install a version supporting --no-daemon and codex exec (validated with 0.160.0). Dedicated processes are required for cancellation."
        );
        Ok(())
    }
    fn build(&self, task: &TaskConfig, repository: &Repository) -> Result<CommandSpec> {
        self.validate(&task.options)?;
        self.validate_attachments(task)?;
        let mut spec = self.detection(&task.options, &repository.path)?;
        spec.args.clear();
        spec = spec.args(&[
            "--no-daemon",
            "--ask-for-approval",
            "never",
            "exec",
            "--json",
            "--color",
            "never",
            "--ephemeral",
            "--sandbox",
            value(&task.options, &OPTIONS[3]),
        ]);
        let model = value(&task.options, &OPTIONS[1]).trim();
        if !model.is_empty() {
            spec = spec.args(&["--model", model]);
        }
        let effort = value(&task.options, &OPTIONS[2]);
        if !effort.is_empty() {
            spec = spec.args(&["--config", &format!("model_reasoning_effort=\"{effort}\"")]);
        }
        for attachment in task.attachments.iter().filter(|a| a.kind.is_image()) {
            spec.args.push("--image".into());
            spec.args.push(attachment.path.as_os_str().to_owned());
        }
        spec = spec.args(&["-"]);
        spec.input = Some(crate::attachments::text_prompt(task)?.into_bytes());
        // Do not inherit another invocation's repository overrides.
        spec.remove_env = std::env::vars_os()
            .filter_map(|(key, _)| {
                key.to_str()
                    .filter(|k| k.starts_with("GIT_"))
                    .map(str::to_owned)
            })
            .collect();
        Ok(spec)
    }
    fn validate_attachments(&self, task: &TaskConfig) -> Result<()> {
        crate::attachments::validate_limits(&task.attachments)?;
        for attachment in task.attachments.iter().filter(|a| a.kind.is_image()) {
            anyhow::ensure!(
                !attachment.path.as_os_str().to_string_lossy().contains(','),
                "Codex's image flag treats commas as separators: {}. Move or rename this image before attaching it.",
                attachment.filename
            );
        }
        Ok(())
    }
    fn attachment_help(
        &self,
        task: &TaskConfig,
        directory: &Path,
    ) -> Result<Option<(CommandSpec, &'static str)>> {
        if !task.attachments.iter().any(|a| a.kind.is_image()) {
            return Ok(None);
        }
        let mut spec = self.detection(&task.options, directory)?;
        spec.args.clear();
        Ok(Some((spec.args(&["exec", "--help"]), "--image")))
    }
    fn attachment_summary(&self) -> &'static str {
        "Text context via stdin; images via Codex's --image flag. The selected model must accept images."
    }
    fn output(&self) -> Box<dyn AgentOutput> {
        Box::<CodexOutput>::default()
    }
    fn execution_summary(&self, options: &AgentOptions) -> String {
        format!(
            "Sandbox: {}. Approval policy: never. Existing Codex login/configuration is used.",
            value(options, &OPTIONS[3])
        )
    }
}

#[derive(Default)]
struct Observation {
    failed: bool,
    completed: bool,
}

/// Codex alone requires line framing and turn completion events.
#[derive(Default)]
struct CodexOutput {
    pending: [Vec<u8>; 2],
    observation: Observation,
    activity: String,
}
impl CodexOutput {
    fn format_output(
        stream: Stream,
        line: &str,
        observation: &mut Observation,
        activity: &mut String,
    ) -> String {
        if stream == Stream::Stderr {
            let text = format!("[stderr] {}\n", line.trim_end());
            activity.push_str(&text);
            return text;
        }
        let Ok(event) = serde_json::from_str::<serde_json::Value>(line) else {
            let text = format!("{}\n", line.trim_end());
            activity.push_str(&text);
            return text;
        };
        let kind = event["type"].as_str().unwrap_or("unknown");
        match kind {
            "thread.started" => activity.push_str(&format!(
                "Codex thread started: {}\n",
                event["thread_id"]
                    .as_str()
                    .unwrap_or("identifier unavailable")
            )),
            "turn.started" => activity.push_str("Codex turn started\n"),
            "turn.completed" => activity.push_str(&format!(
                "Codex turn completed\nUsage: {}\n",
                event["usage"]
            )),
            "turn.failed" => activity.push_str(&format!("Codex turn failed: {}\n", event["error"])),
            "error" => activity.push_str(&format!("Error: {}\n", event["message"])),
            "item.started" | "item.updated" | "item.completed" => {
                let item = &event["item"];
                match item["type"].as_str().unwrap_or("unknown") {
                    "agent_message" | "reasoning" => {
                        if let Some(text) = item["text"].as_str() {
                            activity.push_str(text);
                            activity.push('\n');
                        }
                    }
                    "command_execution" => activity.push_str(&format!(
                        "Command ({}): {}\n",
                        item["status"].as_str().unwrap_or(kind),
                        item["command"].as_str().unwrap_or("see raw output")
                    )),
                    "file_change" => {
                        if let Some(changes) = item["changes"].as_array() {
                            for change in changes {
                                activity.push_str(&format!(
                                    "File change ({}): {}\n",
                                    change["kind"].as_str().unwrap_or("reported"),
                                    change["path"].as_str().unwrap_or("see raw output")
                                ));
                            }
                        }
                    }
                    "mcp_tool_call" => activity.push_str(&format!(
                        "Tool ({}): {} / {}\n",
                        item["status"].as_str().unwrap_or(kind),
                        item["server"].as_str().unwrap_or("MCP"),
                        item["tool"].as_str().unwrap_or("see raw output")
                    )),
                    "web_search" => activity.push_str(&format!(
                        "Web search: {}\n",
                        item["query"].as_str().unwrap_or("see raw output")
                    )),
                    other => activity
                        .push_str(&format!("Codex {other} · {kind} (details in Raw output)\n")),
                }
            }
            other => activity.push_str(&format!("Codex {other} (details in Raw output)\n")),
        }
        match event["type"].as_str().unwrap_or("") {
            "turn.failed" => {
                observation.failed = true;
                format!("[turn failed] {}\n", event["error"])
            }
            "error" => format!("[error] {}\n", event["message"]),
            "turn.completed" => {
                observation.completed = true;
                format!("[turn completed] {}\n", event["usage"])
            }
            "item.completed" | "item.started" => {
                let item = &event["item"];
                let kind = item["type"].as_str().unwrap_or("event");
                let mut text = format!("[{kind}] ");
                for field in ["text", "command", "aggregated_output"] {
                    if let Some(value) = item[field].as_str() {
                        text.push_str(value);
                        text.push('\n');
                    }
                }
                if text.ends_with(' ') {
                    text.push_str(&item.to_string());
                    text.push('\n');
                }
                text
            }
            _ => format!("{}\n", line.trim_end()),
        }
    }
}
impl AgentOutput for CodexOutput {
    fn push(&mut self, stream: Stream, bytes: &[u8]) -> String {
        let index = usize::from(stream == Stream::Stderr);
        let mut text = String::new();
        for byte in bytes {
            self.pending[index].push(*byte);
            if *byte == b'\n' || self.pending[index].len() >= 256 * 1024 {
                text.push_str(&Self::format_output(
                    stream,
                    &String::from_utf8_lossy(&self.pending[index]),
                    &mut self.observation,
                    &mut self.activity,
                ));
                self.pending[index].clear();
            }
        }
        text
    }
    fn finish(&mut self) -> String {
        let mut text = String::new();
        for (index, stream) in [Stream::Stdout, Stream::Stderr].into_iter().enumerate() {
            if !self.pending[index].is_empty() {
                text.push_str(&Self::format_output(
                    stream,
                    &String::from_utf8_lossy(&self.pending[index]),
                    &mut self.observation,
                    &mut self.activity,
                ));
                self.pending[index].clear();
            }
        }
        text
    }
    fn interpret(&self, status: ExitStatus) -> (JobStatus, String) {
        let observation = &self.observation;
        if status.success() && !observation.failed && observation.completed {
            (
                JobStatus::Succeeded,
                "Codex completed. Review the working-tree diff.".into(),
            )
        } else {
            (
                JobStatus::Failed,
                format!(
                    "Codex did not complete successfully ({status}). Inspect output for authentication, model, permission, or CLI errors."
                ),
            )
        }
    }
    fn take_activity(&mut self) -> Option<String> {
        Some(std::mem::take(&mut self.activity))
    }
}
