//! Google Antigravity CLI (`agy`) headless stream-json protocol, independent of
//! other backends. Sources, decisions and unverified behavior are recorded in
//! docs/antigravity-validation.md.
use super::{AgentBackend, AgentOutput, OptionKind, OptionSpec, value};
use crate::{
    domain::{AgentId, AgentOptions, Repository, TaskConfig},
    process::{CommandSpec, Stream},
};
use anyhow::Result;
use std::{path::Path, time::Duration};

mod output;

pub struct Antigravity;
// Presentation guidance only; CodeConvoy never signs in or installs Antigravity.
pub const SETUP_HINT: &str = "Check CLI verifies agy help/version, not sign-in. Sign in once in an interactive agy session in your terminal; headless runs reuse that cached sign-in.";
pub const INSTALLATION_URL: &str = "https://antigravity.google/docs/install?tab=cli";
pub const HEADLESS_URL: &str = "https://antigravity.google/docs/cli/headless/";

/// Oldest release whose documented headless fixes CodeConvoy depends on:
/// stream-json output (1.1.8) and input (1.1.15), soft-denied tools reported as
/// `denied_actions` (1.1.27), streamed errors exiting 3 (1.2.10) and no early
/// exit before background commands are answered (1.2.16).
pub const MINIMUM_VERSION: (u32, u32, u32) = (1, 2, 16);
/// Antigravity's terminal sandbox needs Windows 11 24H2 (build 26100) or newer.
pub const WINDOWS_SANDBOX_BUILD: u32 = 26100;

static OPTIONS: &[OptionSpec] = &[
    OptionSpec {
        key: "executable",
        label: "Antigravity executable",
        default: "agy",
        help: "Antigravity CLI agy on PATH or an absolute path. Install and sign in separately with Antigravity. Windows requires a native .exe; shell wrappers are unsupported. The Antigravity desktop app launcher is not the CLI.",
        kind: OptionKind::Text { hint: "agy" },
    },
    OptionSpec {
        key: "model",
        label: "Model",
        default: "",
        help: "Blank keeps Antigravity's configured default. Otherwise use a model slug listed by agy models for your account; an unknown slug fails instead of falling back.",
        kind: OptionKind::Text {
            hint: "Antigravity default",
        },
    },
    OptionSpec {
        key: "effort",
        label: "Effort",
        default: "",
        help: "Antigravity reasoning effort (--effort). Blank preserves Antigravity's configuration. Support depends on the selected model.",
        kind: OptionKind::Choice(&[
            ("", "Antigravity default"),
            ("low", "Low"),
            ("medium", "Medium"),
            ("high", "High"),
        ]),
    },
    OptionSpec {
        key: "permissions",
        label: "Permissions",
        default: "configured",
        help: "Antigravity configuration applies your settings.json permission rules. Headless runs have no approval prompt, so tools that need approval (shell commands by default) are soft-denied while the run continues. Approve all tool requests explicitly passes --dangerously-skip-permissions, including file writes and commands; this is not an OS sandbox. CodeConvoy never enables it automatically.",
        kind: OptionKind::Choice(&[
            ("configured", "Antigravity configuration"),
            ("skip", "Approve all tool requests"),
        ]),
    },
    OptionSpec {
        key: "sandbox",
        label: "Terminal sandbox",
        default: "configured",
        help: "Antigravity configuration keeps your terminal sandbox setting. Enable restricts agent-launched commands with Antigravity's OS containment (--sandbox) for this job. On Windows it requires Windows 11 24H2 or newer; older Windows is rejected because Antigravity turns the sandbox off there. CodeConvoy does not provide a sandbox itself.",
        kind: OptionKind::Choice(&[
            ("configured", "Antigravity configuration"),
            ("enabled", "Enable for this job"),
        ]),
    },
    OptionSpec {
        key: "timeout_seconds",
        label: "Time limit (seconds)",
        default: "",
        help: "Optional CodeConvoy execution time limit, 1–86400 seconds. Blank means no time limit. Queueing and worktree preparation are excluded. Expiry stops the owned process tree, fails this job and retains edits/results. This does not change Antigravity CLI flags or permissions.",
        kind: OptionKind::Text {
            hint: "No time limit",
        },
    },
];

// agy's help has no product-name banner (Google's captured help starts with
// "Usage of agy:"), so identify it by flags no other supported CLI advertises.
const IDENTITY_FLAGS: &[&str] = &["--print-timeout", "--prompt-interactive"];
// Documented flags the invocation relies on; older releases lack the first three.
const REQUIRED_FLAGS: &[&str] = &[
    "--input-format",
    "--output-format",
    "--effort",
    "--model",
    "--dangerously-skip-permissions",
    "--sandbox",
];

impl AgentBackend for Antigravity {
    fn id(&self) -> AgentId {
        AgentId::Antigravity
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
            "Enter the Antigravity CLI executable name or absolute path."
        );
        anyhow::ensure!(
            path.is_absolute() || path.components().count() == 1,
            "Use an absolute Antigravity executable path or a name on PATH, not a relative path."
        );
        #[cfg(windows)]
        anyhow::ensure!(
            path.extension()
                .is_none_or(|ext| ext.eq_ignore_ascii_case("exe")),
            "Use the native Antigravity agy.exe on Windows; .cmd/.bat/.ps1 wrappers are unsupported."
        );
        if value(options, &OPTIONS[4]) == "enabled" {
            #[cfg(windows)]
            windows_sandbox(windows_build())?;
        }
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
        let advertises = |flag: &str| {
            help.split_whitespace().any(|word| {
                word.strip_prefix(flag)
                    .is_some_and(|suffix| suffix.is_empty() || suffix.starts_with(['[', '=', ',']))
            })
        };
        // Claude Code shares several flag names, so identity is checked first.
        anyhow::ensure!(
            IDENTITY_FLAGS.iter().all(|flag| advertises(flag)),
            "Expected Antigravity CLI (agy) help with --print-timeout and --prompt-interactive. Install the Antigravity CLI separately or choose its agy executable, then Check CLI again."
        );
        for flag in REQUIRED_FLAGS {
            anyhow::ensure!(
                advertises(flag),
                "Antigravity CLI help is missing {flag}. Update the Antigravity CLI and Check CLI again."
            );
        }
        Ok(())
    }
    /// agy 1.4.2 prints `--help`, `-h` and `help` output only to stderr.
    fn help_stream(&self) -> Stream {
        Stream::Stderr
    }
    fn check_version(&self, version: Option<&str>) -> Result<()> {
        let (major, minor, patch) = MINIMUM_VERSION;
        let minimum = format!("{major}.{minor}.{patch}");
        let reported = version.unwrap_or_default().trim();
        let parsed = parse_version(reported);
        anyhow::ensure!(
            parsed.is_some(),
            "Antigravity CLI version could not be determined from agy --version{}. CodeConvoy requires {minimum} or newer; update the Antigravity CLI and Check CLI again.",
            if reported.is_empty() {
                String::new()
            } else {
                format!(" ({reported})")
            }
        );
        anyhow::ensure!(
            parsed >= Some(MINIMUM_VERSION),
            "Antigravity CLI {reported} is unsupported. CodeConvoy requires {minimum} or newer for its headless protocol; update the Antigravity CLI and Check CLI again."
        );
        Ok(())
    }
    fn build(&self, task: &TaskConfig, repository: &Repository) -> Result<CommandSpec> {
        anyhow::ensure!(
            task.agent == AgentId::Antigravity,
            "Antigravity requires an Antigravity task configuration."
        );
        task.validate(std::slice::from_ref(repository))?;
        self.validate_attachments(task)?;
        let mut spec = self.detection(&task.options, &repository.path)?;
        spec.args.clear();
        // The documented stdin transport takes no -p prompt; one user message is
        // written and stdin closed, so the session ends after that turn. No
        // --print-timeout: headless runs are unlimited by default since 1.2.6,
        // and an explicit expiry returns partial output with exit 0 (1.1.28).
        spec = spec.args(&[
            "--input-format",
            "stream-json",
            "--output-format",
            "stream-json",
        ]);
        for (index, flag) in [(1, "--model"), (2, "--effort")] {
            let selected = value(&task.options, &OPTIONS[index]).trim();
            if !selected.is_empty() {
                spec.args.push(format!("{flag}={selected}").into());
            }
        }
        if value(&task.options, &OPTIONS[3]) == "skip" {
            spec.args.push("--dangerously-skip-permissions".into());
        }
        if value(&task.options, &OPTIONS[4]) == "enabled" {
            spec.args.push("--sandbox".into());
        }
        let prompt = crate::attachments::text_prompt(task)?;
        let mut input = serde_json::to_vec(
            &serde_json::json!({"event": "user", "message": {"content": prompt}}),
        )?;
        input.push(b'\n');
        spec.input = Some(input);
        spec.remove_env = std::env::vars_os()
            .filter_map(|(key, _)| {
                key.to_str()
                    .filter(|key| crate::process::git_environment_override(key, cfg!(windows)))
                    .map(str::to_owned)
            })
            .collect();
        // Antigravity scopes conversations to the process directory; do not let
        // it or its tools consult another invocation's logical directory.
        spec.remove_env.push("PWD".into());
        Ok(spec)
    }
    fn output(&self) -> Box<dyn AgentOutput> {
        Box::<output::AntigravityOutput>::default()
    }
    fn output_for(&self, _options: &AgentOptions, directory: &Path) -> Box<dyn AgentOutput> {
        Box::new(output::AntigravityOutput::for_workspace(directory))
    }
    fn execution_timeout(&self, options: &AgentOptions) -> Result<Option<Duration>> {
        let timeout = value(options, &OPTIONS[5]).trim();
        if timeout.is_empty() {
            return Ok(None);
        }
        let seconds = timeout.parse::<u32>().ok().filter(|seconds| {
            timeout.bytes().all(|byte| byte.is_ascii_digit()) && (1..=86400).contains(seconds)
        });
        anyhow::ensure!(
            seconds.is_some(),
            "Antigravity time limit must be a whole number from 1 to 86400 seconds, or blank for no time limit."
        );
        Ok(seconds.map(|seconds| Duration::from_secs(u64::from(seconds))))
    }
    fn attachment_summary(&self) -> &'static str {
        "UTF-8 text context via stdin in one stream-json user message. Image attachments are unsupported."
    }
    fn execution_summary(&self, options: &AgentOptions) -> String {
        let permissions = if value(options, &OPTIONS[3]) == "skip" {
            "Explicit --dangerously-skip-permissions: Antigravity approves every tool request, including file writes and commands."
        } else {
            "Antigravity's configured permission rules apply; tools that need approval are soft-denied in headless runs."
        };
        let sandbox = if value(options, &OPTIONS[4]) == "enabled" {
            "Explicit --sandbox: agent-launched commands use Antigravity's terminal sandbox."
        } else {
            "Antigravity's configured terminal sandbox setting applies."
        };
        let timeout = match self.execution_timeout(options) {
            Ok(Some(timeout)) => format!(
                "CodeConvoy time limit: {} seconds after spawn; expiry stops the process tree and retains edits.",
                timeout.as_secs()
            ),
            Ok(None) => "No CodeConvoy execution time limit; CodeConvoy passes no Antigravity --print-timeout.".into(),
            Err(_) => "Invalid CodeConvoy time limit; review will reject it.".into(),
        };
        format!(
            "{permissions} {sandbox} {timeout} No interactive approval host. Existing Antigravity sign-in, settings, rules, hooks and MCP configuration apply. Process directory selection is not an OS sandbox. The task and text context are sent on stdin as one stream-json message. Success requires a SUCCESS result; it confirms CLI completion, not that files changed. Sign in with agy separately; CodeConvoy does not manage credentials."
        )
    }
}

/// Antigravity 1.4.1+ turns its terminal sandbox off and asks before commands
/// on Windows 10 and Windows 11 before 24H2; earlier releases fail to start
/// them. Reject the explicit choice there rather than run without it.
#[cfg_attr(not(windows), allow(dead_code))]
fn windows_sandbox(build: Option<u32>) -> Result<()> {
    anyhow::ensure!(
        build.is_some_and(|build| build >= WINDOWS_SANDBOX_BUILD),
        "Antigravity's terminal sandbox requires Windows 11 24H2 (build {WINDOWS_SANDBOX_BUILD}) or newer; {}. Antigravity turns the sandbox off on older Windows, so choose Antigravity configuration for Terminal sandbox.",
        build.map_or_else(
            || "this Windows build could not be determined".to_owned(),
            |build| format!("this system reports build {build}")
        )
    );
    Ok(())
}

#[cfg(windows)]
fn windows_build() -> Option<u32> {
    windows_registry::LOCAL_MACHINE
        .open(r"SOFTWARE\Microsoft\Windows NT\CurrentVersion")
        .and_then(|key| key.get_string("CurrentBuildNumber"))
        .ok()
        .and_then(|build| build.trim().parse().ok())
}

/// First `MAJOR.MINOR.PATCH` sequence in the reported line, ignoring prefixes
/// such as `agy version v` and pre-release or build suffixes.
fn parse_version(line: &str) -> Option<(u32, u32, u32)> {
    line.split(|c: char| !c.is_ascii_digit() && c != '.')
        .find_map(|candidate| {
            let mut parts = candidate.split('.');
            let version = (
                parts.next()?.parse().ok()?,
                parts.next()?.parse().ok()?,
                parts.next()?.parse().ok()?,
            );
            parts.next().is_none().then_some(version)
        })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::{WINDOWS_SANDBOX_BUILD, parse_version, windows_sandbox};

    #[test]
    fn windows_sandbox_requires_a_known_24h2_or_newer_build() {
        for build in [WINDOWS_SANDBOX_BUILD, 26200, 27000] {
            windows_sandbox(Some(build)).unwrap();
        }
        // Windows 10 22H2, Windows 11 21H2/23H2 and unknown builds are rejected.
        for build in [Some(19045), Some(22000), Some(22631), Some(26099), None] {
            let error = windows_sandbox(build).unwrap_err().to_string();
            assert!(error.contains("Windows 11 24H2"), "{error}");
            assert!(error.contains("Antigravity configuration"), "{error}");
        }
    }

    #[test]
    fn parses_the_first_semantic_version_in_common_spellings() {
        for (line, expected) in [
            ("1.4.0", Some((1, 4, 0))),
            ("agy version 1.2.16", Some((1, 2, 16))),
            ("agy v1.3.6 (abc123)", Some((1, 3, 6))),
            ("Antigravity CLI 2.0.1-beta.3", Some((2, 0, 1))),
            ("1.18.34 (CodeConvoy fixture)", Some((1, 18, 34))),
            ("Version unavailable", None),
            ("1.4", None),
            ("1.4.0.7", None),
            ("", None),
            ("99999999999.0.0", None),
        ] {
            assert_eq!(parse_version(line), expected, "{line}");
        }
    }
}
