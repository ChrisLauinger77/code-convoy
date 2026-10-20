//! Cursor's direct print protocol. Evidence and unverified behavior are recorded
//! in docs/cursor-validation.md; this is independent of Claude's protocol.
use super::{AgentBackend, AgentOutput, OptionKind, OptionSpec, value};
use crate::{
    domain::{AgentId, AgentOptions, Repository, TaskConfig},
    process::CommandSpec,
};
use anyhow::Result;
use std::{path::Path, time::Duration};

mod output;

pub struct Cursor;
// Presentation guidance only; CodeConvoy never runs login/status or installs Cursor.
pub const SETUP_HINT: &str = "Check CLI verifies help/version, not sign-in. Use agent status or agent login separately in your terminal.";
pub const INSTALLATION_URL: &str = "https://cursor.com/docs/cli/installation";
pub const AUTHENTICATION_URL: &str = "https://cursor.com/docs/cli/reference/authentication";

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
    OptionSpec {
        key: "timeout_seconds",
        label: "Time limit (seconds)",
        default: "3600",
        help: "CodeConvoy execution time limit, 1–86400 seconds; blank uses 3600 seconds. Bounds unattended waits, including unresolved approval. Queueing and worktree preparation are excluded. Expiry stops the owned process tree, fails this job and retains edits/results. This does not change Cursor CLI flags or permissions.",
        kind: OptionKind::Text { hint: "3600" },
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
        self.execution_timeout(options)?;
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
        anyhow::ensure!(
            help.lines()
                .any(|line| line.trim_start().starts_with("agent [prompt...]")),
            "Cursor CLI help does not advertise the required agent subcommand. Choose a compatible Cursor CLI executable and Check CLI again."
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
        Box::<output::CursorOutput>::default()
    }
    fn output_for(&self, _options: &AgentOptions, directory: &Path) -> Box<dyn AgentOutput> {
        Box::new(output::CursorOutput::for_workspace(directory))
    }
    fn execution_timeout(&self, options: &AgentOptions) -> Result<Option<Duration>> {
        // Older saved drafts may omit this option or contain a blank value.
        // Keep unattended approval/network waits bounded for those jobs too.
        let configured = value(options, &OPTIONS[4]).trim();
        let timeout = if configured.is_empty() {
            OPTIONS[4].default
        } else {
            configured
        };
        let seconds = timeout.parse::<u32>().ok().filter(|seconds| {
            timeout.bytes().all(|byte| byte.is_ascii_digit()) && (1..=86400).contains(seconds)
        });
        anyhow::ensure!(
            seconds.is_some(),
            "Cursor time limit must be a whole number from 1 to 86400 seconds, or blank for the 3600-second default."
        );
        Ok(seconds.map(|seconds| Duration::from_secs(u64::from(seconds))))
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
        let timeout = match self.execution_timeout(options) {
            Ok(Some(timeout)) => format!(
                "CodeConvoy time limit: {} seconds after spawn; expiry stops the process tree and retains edits.",
                timeout.as_secs()
            ),
            Ok(None) => "No CodeConvoy execution time limit.".into(),
            Err(_) => "Invalid CodeConvoy time limit; review will reject it.".into(),
        };
        format!(
            "{permissions} {trust} {timeout} No interactive approval host. Existing Cursor authentication, rules, hooks and MCP configuration apply. Workspace selection is not an OS sandbox. Prompts/text context use a process argument. A successful result confirms CLI completion, not that files changed. Authenticate separately with agent login; CodeConvoy does not manage credentials."
        )
    }
}
