//! GitHub Copilot CLI prompt mode. This is not a Codex sandbox or JSONL protocol.
use super::{AgentBackend, AgentOutput, OptionKind, OptionSpec, value};
use crate::{
    domain::{AgentId, AgentOptions, JobStatus, Repository, TaskConfig},
    process::{CommandSpec, Stream},
};
use anyhow::Result;
use std::{path::Path, process::ExitStatus};

pub struct Copilot;
static OPTIONS: &[OptionSpec] = &[
    OptionSpec {
        key: "executable",
        label: "Copilot executable",
        default: "copilot",
        help: "Executable name on PATH or an absolute path. Detection and jobs disable auto-update and use that executable's bundled version.",
        kind: OptionKind::Text { hint: "copilot" },
    },
    OptionSpec {
        key: "model",
        label: "Model",
        default: "",
        help: "Blank uses Copilot's configured model. Enter a model supported by your account, or auto for Copilot routing.",
        kind: OptionKind::Text {
            hint: "CLI default",
        },
    },
    OptionSpec {
        key: "reasoning_effort",
        label: "Reasoning effort",
        default: "",
        help: "Copilot --reasoning-effort, as supported by the inspected CLI. Availability depends on the selected model. No Codex setting is translated.",
        kind: OptionKind::Choice(&[
            ("", "CLI default"),
            ("none", "None"),
            ("low", "Low"),
            ("medium", "Medium"),
            ("high", "High"),
            ("xhigh", "Extra high"),
            ("max", "Max"),
        ]),
    },
    OptionSpec {
        key: "tool_approvals",
        label: "Tool approvals",
        default: "file-edits",
        help: "File edits grants write tools and denies shell tools. Existing approvals uses the CLI's permissions. All tools grants automatic tool approval, including shell execution. These are Copilot permissions, not an OS sandbox.",
        kind: OptionKind::Choice(&[
            ("file-edits", "File edits; shell denied"),
            ("existing", "Existing CLI approvals"),
            ("all-tools", "Allow all tools, including shell"),
        ]),
    },
    OptionSpec {
        key: "temp_access",
        label: "Temporary directory",
        default: "default",
        help: "Keep Copilot's default temporary-directory access, or pass --disallow-temp-dir. Existing configured path permissions still apply. CodeConvoy never enables all-path access.",
        kind: OptionKind::Choice(&[
            ("default", "CLI default access"),
            ("disallow", "Disallow temp directory"),
        ]),
    },
];
// All of these were checked against the installed bundled CLI 1.0.65.
const REQUIRED_FLAGS: &[&str] = &[
    "--prompt",
    "--model",
    "--reasoning-effort",
    "--allow-tool",
    "--deny-tool",
    "--allow-all-tools",
    "--disallow-temp-dir",
    "--stream",
    "--output-format",
    "--no-ask-user",
    "--no-auto-update",
    "--no-color",
    "--plain-diff",
    "--no-remote",
    "--no-remote-export",
];
impl AgentBackend for Copilot {
    fn id(&self) -> AgentId {
        AgentId::Copilot
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
            "Enter the Copilot executable name or absolute path."
        );
        anyhow::ensure!(
            path.is_absolute() || path.components().count() == 1,
            "Use an absolute Copilot executable path or a name on PATH, not a relative path."
        );
        Ok(CommandSpec::new(executable, directory).args(&["--no-auto-update", "--help"]))
    }
    fn version_command(
        &self,
        options: &AgentOptions,
        directory: &Path,
    ) -> Result<Option<CommandSpec>> {
        let mut spec = self.detection(options, directory)?;
        spec.args.pop();
        Ok(Some(spec.args(&["--version"])))
    }
    fn check_detection(&self, help: &str) -> Result<()> {
        anyhow::ensure!(
            help.contains("GitHub Copilot CLI"),
            "This executable is not the standalone GitHub Copilot CLI. Choose copilot, not gh copilot."
        );
        for flag in REQUIRED_FLAGS {
            let supported = help.split_whitespace().any(|word| {
                word.strip_prefix(flag)
                    .is_some_and(|suffix| suffix.is_empty() || suffix.starts_with(['[', '=', ',']))
            });
            anyhow::ensure!(
                supported,
                "Copilot CLI is missing {flag}. Install a compatible standalone CLI (checked with bundled 1.0.65); --no-auto-update uses the bundled executable, not a cached update."
            );
        }
        Ok(())
    }
    fn build(&self, task: &TaskConfig, repository: &Repository) -> Result<CommandSpec> {
        anyhow::ensure!(
            task.agent == AgentId::Copilot,
            "Copilot requires a Copilot task configuration."
        );
        task.validate(std::slice::from_ref(repository))?;
        let mut spec = self.detection(&task.options, &repository.path)?;
        spec.args.pop(); // retain --no-auto-update, remove --help
        spec = spec.args(&[
            "--no-ask-user",
            "--no-color",
            "--plain-diff",
            "--output-format",
            "text",
            "--stream",
            "on",
            "--no-remote",
            "--no-remote-export",
        ]);
        match value(&task.options, &OPTIONS[3]) {
            "file-edits" => spec = spec.args(&["--allow-tool=write", "--deny-tool=shell"]),
            "all-tools" => spec = spec.args(&["--allow-all-tools"]),
            _ => {} // Existing CLI approvals; validate rejects unknown values.
        }
        if value(&task.options, &OPTIONS[4]) == "disallow" {
            spec = spec.args(&["--disallow-temp-dir"]);
        }
        for (index, flag) in [(1, "--model"), (2, "--reasoning-effort")] {
            let option = value(&task.options, &OPTIONS[index]).trim();
            if !option.is_empty() {
                spec = spec.args(&[flag, option]);
            }
        }
        // Copilot documents piped stdin as non-interactive prompt mode. Do not
        // pass -p "-": Copilot would treat that as a literal prompt and ignore stdin.
        spec.input = Some(task.prompt.as_bytes().to_vec());
        spec.remove_env = std::env::vars_os()
            .filter_map(|(key, _)| {
                key.to_str()
                    .filter(|key| key.starts_with("GIT_"))
                    .map(str::to_owned)
            })
            .collect();
        // Prevent an inherited blanket grant from overriding the selected UI mode.
        // Authentication variables and COPILOT_HOME are deliberately untouched.
        spec.remove_env.push("COPILOT_ALLOW_ALL".into());
        Ok(spec)
    }
    fn output(&self) -> Box<dyn AgentOutput> {
        Box::<CopilotOutput>::default()
    }
    fn execution_summary(&self, options: &AgentOptions) -> String {
        let tools = match value(options, &OPTIONS[3]) {
            "file-edits" => "Write tools approved; shell tools denied",
            "all-tools" => "All tools approved, including shell commands",
            _ => "Existing CLI approvals; unapproved actions may fail",
        };
        let temp = if value(options, &OPTIONS[4]) == "disallow" {
            "temp directory access disabled"
        } else {
            "CLI default temp directory access"
        };
        format!(
            "{tools}; {temp}. Copilot path checks and configured permissions apply. This is not an OS sandbox. Existing Copilot authentication is used; user questions are disabled."
        )
    }
}

#[derive(Default)]
struct CopilotOutput {
    pending: [Vec<u8>; 2],
}
impl AgentOutput for CopilotOutput {
    fn push(&mut self, stream: Stream, bytes: &[u8]) -> String {
        let pending = &mut self.pending[usize::from(stream == Stream::Stderr)];
        pending.extend_from_slice(bytes);
        let mut text = String::new();
        let mut consumed = 0;
        while consumed < pending.len() {
            match std::str::from_utf8(&pending[consumed..]) {
                Ok(valid) => {
                    text.push_str(valid);
                    consumed = pending.len();
                }
                Err(error) => {
                    let end = consumed + error.valid_up_to();
                    text.push_str(&String::from_utf8_lossy(&pending[consumed..end]));
                    consumed = end;
                    match error.error_len() {
                        Some(count) => {
                            text.push('\u{fffd}');
                            consumed += count;
                        }
                        None => break, // At most 3 bytes, completed by the next chunk.
                    }
                }
            }
        }
        pending.drain(..consumed);
        if stream == Stream::Stderr && !text.is_empty() {
            format!("[stderr] {text}")
        } else {
            text
        }
    }
    fn finish(&mut self) -> String {
        let mut text = String::new();
        for (index, pending) in self.pending.iter_mut().enumerate() {
            if !pending.is_empty() {
                if index == 1 {
                    text.push_str("[stderr] ");
                }
                text.push_str(&String::from_utf8_lossy(pending));
                pending.clear();
            }
        }
        text
    }
    fn interpret(&self, status: ExitStatus) -> (JobStatus, String) {
        if status.success() {
            (
                JobStatus::Succeeded,
                "Copilot exited successfully. Review the output and working-tree diff.".into(),
            )
        } else {
            (
                JobStatus::Failed,
                format!(
                    "Copilot failed ({status}). Inspect output for authentication, model, tool/path permission, or CLI errors. Configure authentication using the Copilot CLI outside CodeConvoy if needed."
                ),
            )
        }
    }
}
