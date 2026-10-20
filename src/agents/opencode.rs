//! OpenCode's local `run` protocol. See docs/opencode-validation.md for sources.
use super::{AgentBackend, AgentOutput, OptionKind, OptionSpec, value};
use crate::{
    domain::{AgentId, AgentOptions, JobStatus, Repository, TaskConfig},
    process::{CommandSpec, Containment, Stream},
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
        key: "version",
        label: "OpenCode version",
        default: "1",
        help: if OPENCODE_2_SUPPORTED {
            "Major version of the selected executable; launch verifies it against opencode run --help. OpenCode 1 runs with --dir. OpenCode 2 has no --dir: it runs in the job's directory with a private --standalone server instead of the shared background service."
        } else {
            "Major version of the selected executable; launch verifies it against opencode run --help. OpenCode 2 is supported only on macOS, Linux and Windows."
        },
        kind: OptionKind::Choice(&[
            ("1", "OpenCode 1"),
            (
                "2",
                if OPENCODE_2_SUPPORTED {
                    "OpenCode 2"
                } else {
                    "OpenCode 2 (unsupported here)"
                },
            ),
        ]),
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
        help: "Blank uses OpenCode's default agent. Use a primary agent (for example build or plan; OpenCode 1 lists them with opencode agent list). OpenCode may fall back to its default for an unknown name or a subagent; review output.",
        kind: OptionKind::Text {
            hint: "OpenCode default",
        },
    },
    OptionSpec {
        key: "variant",
        label: "Variant",
        default: "",
        help: "Optional OpenCode model variant, including provider-specific reasoning effort. Names and support depend on your model/configuration. This does not request thinking output. OpenCode 2 requires a model and receives provider/model#variant.",
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
/// OpenCode 2 starts its `--standalone` server through a spawner that detaches it
/// into its own process group on macOS and Linux, and its `run` stops that
/// server only on its way out (SIGTERM or completion). `Containment` asks it to
/// stop and confirms the recorded groups are gone; that tracking exists only on
/// macOS and Linux. A Windows Job Object contains the server and its tools.
const OPENCODE_2_SUPPORTED: bool = cfg!(any(windows, target_os = "macos", target_os = "linux"));
const OPENCODE_2_UNSUPPORTED: &str = "OpenCode 2 is not supported on this platform: CodeConvoy can confirm that its detached server has stopped only on macOS, Linux and Windows. Use an OpenCode 1 executable.";
/// `run` gives its server 3 seconds after SIGTERM before killing it.
const OPENCODE_2_STOP: std::time::Duration = std::time::Duration::from_secs(10);

/// OpenCode's two incompatible `run` interfaces. Each CLI rejects the other's
/// distinguishing argument (`--dir` / `--standalone`), so a binary replaced
/// after Check CLI fails before a session starts instead of running elsewhere.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Cli {
    V1,
    V2,
}
impl Cli {
    fn selected(options: &AgentOptions) -> Self {
        if value(options, &OPTIONS[1]) == "2" {
            Self::V2
        } else {
            Self::V1
        }
    }
    /// The interface whose usage line `help` shows; OpenCode 1 otherwise.
    fn offered(help: &str) -> Self {
        if help.contains(Self::V2.contract().0) {
            Self::V2
        } else {
            Self::V1
        }
    }
    fn other(self) -> Self {
        match self {
            Self::V1 => Self::V2,
            Self::V2 => Self::V1,
        }
    }
    fn label(self) -> &'static str {
        match self {
            Self::V1 => "OpenCode 1",
            Self::V2 => "OpenCode 2",
        }
    }
    /// Release sources the help contract was derived from.
    fn checked(self) -> &'static str {
        match self {
            Self::V1 => "v1.18.34 and v1.19.3",
            Self::V2 => "v2.0.32",
        }
    }
    /// Usage text and options the generated command needs from `run --help`.
    fn contract(self) -> (&'static str, &'static [&'static str]) {
        match self {
            Self::V1 => (
                "run [message..]",
                &[
                    "--format",
                    "--dir",
                    "--model",
                    "--agent",
                    "--variant",
                    "--auto",
                ],
            ),
            Self::V2 => (
                "run [flags]",
                &["--standalone", "--format", "--model", "--agent", "--auto"],
            ),
        }
    }
    /// The first part of this interface absent from `help`.
    fn missing(self, help: &str) -> Option<&'static str> {
        let (usage, flags) = self.contract();
        if !help.contains(usage) || !help.contains("json") {
            return Some(usage);
        }
        flags.iter().copied().find(|flag| {
            !help.split_whitespace().any(|word| {
                word.strip_prefix(flag)
                    .is_some_and(|suffix| suffix.is_empty() || suffix.starts_with(['[', '=', ',']))
            })
        })
    }
}

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
        let model = value(options, &OPTIONS[2]).trim();
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
        anyhow::ensure!(
            OPENCODE_2_SUPPORTED || Cli::selected(options) == Cli::V1,
            OPENCODE_2_UNSUPPORTED
        );
        let variant = value(options, &OPTIONS[4]).trim();
        if Cli::selected(options) == Cli::V2 && !variant.is_empty() {
            // OpenCode 2 has no --variant; it parses provider/model#variant.
            anyhow::ensure!(
                !model.is_empty(),
                "OpenCode 2 selects a variant together with a model (provider/model#variant). Enter a model or clear Variant."
            );
            anyhow::ensure!(
                !model.contains('#')
                    && !variant.contains('#')
                    && !variant.chars().any(char::is_whitespace),
                "OpenCode 2 combines Model and Variant as provider/model#variant. Neither may contain #, and Variant may not contain spaces."
            );
        }
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
        self.check_detection_for(&AgentOptions::default(), help)
    }
    /// OpenCode 1 prints run help to stderr, OpenCode 2 to stdout.
    fn help_text(&self, stdout: &[u8], stderr: &[u8]) -> String {
        format!(
            "{}\n{}",
            String::from_utf8_lossy(stdout),
            String::from_utf8_lossy(stderr)
        )
    }
    fn check_detection_for(&self, options: &AgentOptions, help: &str) -> Result<()> {
        // Check CLI and startup checks belong to the executable and pass no
        // version: they accept whichever supported interface it offers. Launch
        // preflight passes the materialized setting, which must then match.
        let chosen = options.contains_key(OPTIONS[1].key);
        let selected = if chosen {
            Cli::selected(options)
        } else {
            Cli::offered(help)
        };
        anyhow::ensure!(
            OPENCODE_2_SUPPORTED || selected == Cli::V1,
            if chosen {
                OPENCODE_2_UNSUPPORTED.to_owned()
            } else {
                format!(
                    "This OpenCode executable offers the OpenCode 2 run interface. {OPENCODE_2_UNSUPPORTED}"
                )
            }
        );
        let Some(missing) = selected.missing(help) else {
            return Ok(());
        };
        let other = selected.other();
        anyhow::ensure!(
            OPENCODE_2_SUPPORTED || other.missing(help).is_some(),
            "This OpenCode executable offers the OpenCode 2 run interface. {OPENCODE_2_UNSUPPORTED}"
        );
        anyhow::ensure!(
            other.missing(help).is_some(),
            "This OpenCode executable offers the {} run interface, but {} is selected. Choose {} under OpenCode version, then start the convoy again.",
            other.label(),
            selected.label(),
            other.label()
        );
        anyhow::ensure!(
            missing.starts_with("--"),
            "Expected {} run help with JSON output. Choose the OpenCode CLI from opencode.ai (source contract checked against {}).",
            selected.label(),
            selected.checked()
        );
        anyhow::bail!(
            "{} run is missing {missing}. Install a compatible OpenCode version (source contract checked against {}), then Check CLI again.",
            selected.label(),
            selected.checked()
        )
    }
    fn build(&self, task: &TaskConfig, repository: &Repository) -> Result<CommandSpec> {
        anyhow::ensure!(
            task.agent == AgentId::OpenCode,
            "OpenCode requires an OpenCode task configuration."
        );
        task.validate(std::slice::from_ref(repository))?;
        self.validate_attachments(task)?;
        // The process directory is the only workspace signal for OpenCode 2.
        // A relative path would resolve against CodeConvoy's own directory.
        anyhow::ensure!(
            repository.path.is_absolute(),
            "OpenCode needs an absolute repository or worktree path; {} is relative.",
            repository.path.display()
        );
        let cli = Cli::selected(&task.options);
        let mut spec = self.detection(&task.options, &repository.path)?;
        spec.args.clear();
        match cli {
            Cli::V1 => {
                spec = spec.args(&["run", "--format", "json", "--dir"]);
                spec.args.push(repository.path.as_os_str().to_owned());
            }
            // OpenCode 2 resolves its directory from PWD or the process cwd; PWD
            // is removed below. --standalone starts a private server with this
            // job's directory and environment instead of the shared background
            // service, which would outlive cancellation.
            Cli::V2 => {
                spec = spec.args(&["run", "--standalone", "--format", "json"]);
                spec.containment = Containment::DetachedGroups {
                    grace: OPENCODE_2_STOP,
                };
            }
        }
        let model = value(&task.options, &OPTIONS[2]).trim();
        let variant = value(&task.options, &OPTIONS[4]).trim();
        let model = match cli {
            Cli::V2 if !variant.is_empty() => format!("{model}#{variant}"),
            _ => model.to_owned(),
        };
        let agent = value(&task.options, &OPTIONS[3]).trim();
        let variant = if cli == Cli::V1 { variant } else { "" };
        for (flag, selected) in [
            ("--model", model.as_str()),
            ("--agent", agent),
            ("--variant", variant),
        ] {
            if !selected.is_empty() {
                // A single --key=value argument also keeps leading dashes literal.
                spec.args.push(format!("{flag}={selected}").into());
            }
        }
        if value(&task.options, &OPTIONS[5]) == "auto" {
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
                    .filter(|k| crate::process::git_environment_override(k, cfg!(windows)))
                    .map(str::to_owned)
            })
            .collect();
        spec.remove_env.push("PWD".into());
        Ok(spec)
    }
    fn output(&self) -> Box<dyn AgentOutput> {
        Box::<OpenCodeOutput>::default()
    }
    fn output_with_options(&self, options: &AgentOptions) -> Box<dyn AgentOutput> {
        Box::new(OpenCodeOutput {
            v2: Cli::selected(options) == Cli::V2,
            ..Default::default()
        })
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
        let permission = if value(options, &OPTIONS[5]) == "auto" {
            "Auto-approve requests; explicit OpenCode denies still apply."
        } else {
            "Existing OpenCode rules; requests needing approval are rejected."
        };
        let server = match Cli::selected(options) {
            Cli::V1 => "",
            Cli::V2 => {
                " OpenCode 2 starts a private standalone server for each job instead of using the shared background service."
            }
        };
        format!(
            "{permission}{server} Configured rules may allow shell and file access; this is not an OS sandbox. Existing OpenCode authentication, providers, agents and configuration are used. OpenCode may retain sessions or auto-share them if configured. Review output and diff."
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
    /// OpenCode 2 closes its event stream once the session is idle, so it never
    /// prints the final step's step_finish. Its run exits nonzero on every
    /// error path, so completion rests on the exit status plus a final step
    /// that responded and did not end unfinished or for a non-stop reason.
    v2: bool,
    started: bool,
    responded: bool,
    last_finish: Option<String>,
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
                    .or_else(|| input["path"].as_str())
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
            "step_start" => {
                // Completion is about the latest step: it must answer itself.
                self.stopped = false;
                self.started = true;
                self.responded = false;
                self.last_finish = None;
            }
            "step_finish" => {
                let reason = event["part"]["reason"].as_str();
                self.stopped = reason == Some("stop");
                self.last_finish = Some(reason.unwrap_or("unknown").to_owned());
            }
            // Only a structurally complete answer counts as a response.
            "tool_use" => {
                self.responded |= event["part"]["tool"]
                    .as_str()
                    .is_some_and(|tool| !tool.is_empty());
            }
            "text" => {
                if let Some(text) = event["part"]["text"].as_str() {
                    self.responded |= !text.trim().is_empty();
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
        let completed = if self.v2 {
            self.started
                && self.responded
                && self
                    .last_finish
                    .as_deref()
                    .is_none_or(|reason| reason == "stop")
        } else {
            self.stopped
        };
        if status.success() && completed && !self.failed && !self.protocol_error {
            (JobStatus::Succeeded, "OpenCode completed. Review its response and working-tree diff; denied tools do not prove the task was fulfilled.".into())
        } else if self.v2 {
            (
                JobStatus::Failed,
                format!(
                    "OpenCode completion was not confirmed ({status}). OpenCode 2 does not report its final step, so success requires exit 0, a final step with its own response that did not end unfinished or for a non-stop reason, no error events, and valid bounded JSON. Inspect output for provider, model, permission or CLI errors."
                ),
            )
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
