//! Cursor's direct print protocol. Evidence and unverified behavior are recorded
//! in docs/cursor-validation.md; this is independent of Claude's protocol.
use super::{AgentBackend, AgentOutput, OptionKind, OptionSpec, value};
use crate::{
    domain::{AgentId, AgentOptions, JobStatus, Repository, TaskConfig},
    process::{CommandSpec, Stream},
};
use anyhow::Result;
use serde_json::Value;
use std::{path::Path, process::ExitStatus};

pub struct Cursor;
static OPTIONS: &[OptionSpec] = &[
    OptionSpec {
        key: "executable",
        label: "Cursor executable",
        default: "agent",
        help: "Cursor CLI agent or cursor-agent, or an absolute launcher path. Install and authenticate separately with Cursor. Windows requires a native .exe; shell wrappers are unsupported.",
        kind: OptionKind::Text { hint: "agent" },
    },
    OptionSpec {
        key: "model",
        label: "Model",
        default: "",
        help: "Blank keeps Cursor's configured default. Use a model identifier supported by your installed Cursor CLI and account (agent models).",
        kind: OptionKind::Text {
            hint: "Cursor default",
        },
    },
    OptionSpec {
        key: "permissions",
        label: "Permissions",
        default: "configured",
        help: "Configured preserves Cursor's permission configuration without --force. Headless tasks may only propose edits or fail when approval is needed. Force explicitly allows commands and edits unless Cursor denies them; this is not an OS sandbox. CodeConvoy never enables force automatically.",
        kind: OptionKind::Choice(&[
            ("configured", "Cursor configuration"),
            ("force", "Force allow unless denied"),
        ]),
    },
    OptionSpec {
        key: "workspace_trust",
        label: "Workspace trust",
        default: "configured",
        help: "Configured preserves Cursor's trust decisions. A fresh isolated worktree may require trust before headless execution. Trust assigned workspace explicitly passes --trust for this job; only choose it for repositories whose rules, hooks and MCP configuration you trust.",
        kind: OptionKind::Choice(&[
            ("configured", "Cursor configuration"),
            ("trust", "Trust assigned workspace"),
        ]),
    },
];

impl AgentBackend for Cursor {
    fn id(&self) -> AgentId {
        AgentId::Cursor
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
            "Enter the Cursor CLI executable name or absolute path."
        );
        anyhow::ensure!(
            path.is_absolute() || path.components().count() == 1,
            "Use an absolute Cursor executable path or a name on PATH, not a relative path."
        );
        #[cfg(windows)]
        anyhow::ensure!(
            path.extension()
                .is_none_or(|ext| ext.eq_ignore_ascii_case("exe")),
            "Use a native Cursor agent.exe or cursor-agent.exe on Windows; .cmd/.bat/.ps1 wrappers are unsupported."
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
            help.contains("Cursor") && help.contains("stream-json"),
            "Expected Cursor CLI help with stream-json output. Install Cursor CLI separately or choose its agent/cursor-agent executable, then Check CLI again."
        );
        for flag in [
            "--print",
            "--output-format",
            "--workspace",
            "--model",
            "--force",
            "--trust",
        ] {
            anyhow::ensure!(
                help.split_whitespace().any(|word| {
                    word.strip_prefix(flag).is_some_and(|suffix| {
                        suffix.is_empty() || suffix.starts_with(['[', '=', ','])
                    })
                }),
                "Cursor CLI help is missing {flag}. Choose a current Cursor CLI executable and Check CLI again."
            );
        }
        Ok(())
    }
    fn build(&self, task: &TaskConfig, repository: &Repository) -> Result<CommandSpec> {
        anyhow::ensure!(
            task.agent == AgentId::Cursor,
            "Cursor requires a Cursor task configuration."
        );
        task.validate(std::slice::from_ref(repository))?;
        self.validate_attachments(task)?;
        let mut spec = self.detection(&task.options, &repository.path)?;
        spec.args.clear();
        // Select the documented agent subcommand explicitly so a task such as
        // "logout" cannot be interpreted as a root CLI management command.
        spec = spec.args(&["agent", "--print", "--output-format", "stream-json"]);
        let mut workspace = std::ffi::OsString::from("--workspace=");
        workspace.push(&repository.path);
        spec.args.push(workspace);
        let model = value(&task.options, &OPTIONS[1]).trim();
        if !model.is_empty() {
            spec.args.push(format!("--model={model}").into());
        }
        if value(&task.options, &OPTIONS[2]) == "force" {
            spec.args.push("--force".into());
        }
        if value(&task.options, &OPTIONS[3]) == "trust" {
            spec.args.push("--trust".into());
        }
        // Cursor documents positional prompts, not another backend's stdin
        // transport. End options so task text cannot become flags/subcommands.
        let prompt = crate::attachments::text_prompt(task)?;
        anyhow::ensure!(
            !prompt.contains('\0') && prompt.len() < 128 * 1024,
            "Cursor's prompt and text attachments must fit in one argument (less than 128 KiB, with no NUL bytes). Reduce the task/context."
        );
        spec.args.push("--".into());
        spec.args.push(prompt.into());
        #[cfg(windows)]
        {
            use std::os::windows::ffi::OsStrExt;
            // Conservative bound includes worst-case Windows argument escaping.
            let units = std::iter::once(&spec.program)
                .chain(&spec.args)
                .map(|arg| {
                    arg.encode_wide()
                        .count()
                        .saturating_mul(2)
                        .saturating_add(3)
                })
                .sum::<usize>();
            anyhow::ensure!(
                units < 32_767,
                "Cursor's Windows command line is too long. Reduce the task/context or executable path."
            );
        }
        spec.remove_env = std::env::vars_os()
            .filter_map(|(key, _)| {
                key.to_str()
                    .filter(|key| crate::process::git_environment_override(key, cfg!(windows)))
                    .map(str::to_owned)
            })
            .collect();
        spec.remove_env.push("PWD".into());
        Ok(spec)
    }
    fn output(&self) -> Box<dyn AgentOutput> {
        Box::<CursorOutput>::default()
    }
    fn attachment_summary(&self) -> &'static str {
        "UTF-8 text context in the prompt argument (visible to local process inspection; platform length limits apply). Image attachments are unsupported."
    }
    fn execution_summary(&self, options: &AgentOptions) -> String {
        let permissions = if value(options, &OPTIONS[2]) == "force" {
            "Explicit --force: Cursor allows commands and edits unless explicitly denied."
        } else {
            "Cursor's configured permissions apply; --force is omitted. Headless tasks may propose edits without applying them or fail when approval is required."
        };
        let trust = if value(options, &OPTIONS[3]) == "trust" {
            "Explicit --trust: the assigned workspace is trusted without a prompt."
        } else {
            "Cursor's saved workspace trust applies; a fresh worktree may be rejected."
        };
        format!(
            "{permissions} {trust} No interactive approval host. Existing Cursor authentication, rules, hooks and MCP configuration apply. Workspace selection is not an OS sandbox. Prompts/text context use a process argument. A successful result confirms CLI completion, not that files changed. Authenticate separately with agent login; CodeConvoy does not manage credentials."
        )
    }
}

// A local framing bound, not a claim about Cursor's maximum event size. Raw
// output remains independently bounded by the runner even for rejected records.
const RECORD_LIMIT: usize = 1024 * 1024;
#[derive(Default)]
struct CursorOutput {
    pending: [Vec<u8>; 2],
    oversized: [bool; 2],
    invalid: bool,
    terminal: bool,
    success: bool,
}
impl CursorOutput {
    fn record(&mut self, bytes: &[u8]) -> String {
        if bytes.iter().all(u8::is_ascii_whitespace) {
            return String::new();
        }
        let event = match serde_json::from_slice::<Value>(bytes) {
            Ok(event) if event.is_object() && event["type"].is_string() => event,
            _ => {
                self.invalid = true;
                return format!(
                    "[CodeConvoy] Invalid Cursor JSON record; see Raw output.\n{}\n",
                    String::from_utf8_lossy(bytes)
                );
            }
        };
        // Initial single-turn contract: a second result or any later event is
        // uncertain, and cannot repair an earlier failure.
        self.invalid |= self.terminal;
        if event["type"] == "error" {
            self.invalid = true;
        }
        if event["type"] == "result" {
            self.terminal = true;
            self.success = event["subtype"] == "success"
                && event["is_error"].as_bool() == Some(false)
                && event["result"].is_string()
                && event["session_id"]
                    .as_str()
                    .is_some_and(|id| !id.trim().is_empty())
                && ["duration_ms", "duration_api_ms"]
                    .iter()
                    .all(|key| event[key].as_u64().is_some());
            return format!(
                "[CodeConvoy] Cursor result: {}\n{}\n",
                event["subtype"].as_str().unwrap_or("unknown"),
                event["result"].as_str().unwrap_or("")
            );
        }
        format!("{}\n", String::from_utf8_lossy(bytes).trim_end())
    }
    fn flush(&mut self, index: usize) -> String {
        let bytes = std::mem::take(&mut self.pending[index]);
        if index == 1 {
            format!("[stderr] {}", String::from_utf8_lossy(&bytes))
        } else {
            self.record(&bytes)
        }
    }
}
impl AgentOutput for CursorOutput {
    fn push(&mut self, stream: Stream, bytes: &[u8]) -> String {
        let index = usize::from(stream == Stream::Stderr);
        let mut rendered = String::new();
        for &byte in bytes {
            if self.oversized[index] {
                if byte == b'\n' {
                    self.oversized[index] = false;
                }
                continue;
            }
            if byte == b'\n' {
                rendered.push_str(&self.flush(index));
            } else if self.pending[index].len() == RECORD_LIMIT {
                self.pending[index].clear();
                self.oversized[index] = true;
                self.invalid |= index == 0;
                rendered.push_str("[CodeConvoy] Cursor record exceeded the 1 MiB display limit; see Raw output.\n");
            } else {
                self.pending[index].push(byte);
            }
        }
        rendered
    }
    fn finish(&mut self) -> String {
        let mut rendered = String::new();
        for index in 0..2 {
            if !self.pending[index].is_empty() {
                rendered.push_str(&self.flush(index));
            }
        }
        rendered
    }
    fn interpret(&self, status: ExitStatus) -> (JobStatus, String) {
        if status.success() && self.terminal && self.success && !self.invalid {
            (
                JobStatus::Succeeded,
                "Cursor reported a successful terminal result.".into(),
            )
        } else {
            (
                JobStatus::Failed,
                format!(
                    "Cursor failed ({status}) or did not provide a valid successful terminal result. See Activity/Raw output. Check Cursor authentication with agent status / agent login, permissions and workspace trust; CodeConvoy does not retry with broader permissions."
                ),
            )
        }
    }
}
