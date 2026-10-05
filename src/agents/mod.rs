//! Backends own their options and CLI protocol; process machinery is reusable.
pub mod codex;
pub mod copilot;
pub mod opencode;
use crate::{
    domain::{AgentId, AgentOptions, JobStatus, Repository, TaskConfig},
    process::{self, CommandSpec, ManagedChild, Stream},
};
use anyhow::{Context, Result};
use std::{path::Path, process::ExitStatus};

pub enum OptionKind {
    Text { hint: &'static str },
    Choice(&'static [(&'static str, &'static str)]),
}
pub struct OptionSpec {
    pub key: &'static str,
    pub label: &'static str,
    pub default: &'static str,
    pub help: &'static str,
    pub kind: OptionKind,
}
/// Per-job output state. Framing and success criteria belong to the backend.
pub trait AgentOutput: Send {
    fn push(&mut self, stream: Stream, bytes: &[u8]) -> String;
    fn finish(&mut self) -> String;
    fn interpret(&self, status: ExitStatus) -> (JobStatus, String);
}
pub trait AgentBackend: Send + Sync {
    fn id(&self) -> AgentId;
    fn options(&self) -> &'static [OptionSpec];
    fn detection(&self, options: &AgentOptions, directory: &Path) -> Result<CommandSpec>;
    fn version_command(
        &self,
        _options: &AgentOptions,
        _directory: &Path,
    ) -> Result<Option<CommandSpec>> {
        Ok(None)
    }
    fn check_detection(&self, help: &str) -> Result<()>;
    fn build(&self, task: &TaskConfig, repository: &Repository) -> Result<CommandSpec>;
    fn output(&self) -> Box<dyn AgentOutput>;
    fn execution_summary(&self, options: &AgentOptions) -> String;
    fn spawn(&self, spec: &CommandSpec) -> Result<ManagedChild> {
        process::spawn(spec)
    }
    fn cancel(&self, child: &mut ManagedChild) -> std::io::Result<()> {
        process::cancel(child)
    }
    fn validate(&self, options: &AgentOptions) -> Result<()> {
        for (key, value) in options {
            let spec = self
                .options()
                .iter()
                .find(|s| s.key == key)
                .with_context(|| format!("Unsupported {} option: {key}", self.id().label()))?;
            anyhow::ensure!(
                value.len() <= 4096 && !value.contains(['\0', '\n', '\r']),
                "Invalid value for {}.",
                spec.label
            );
            if let OptionKind::Choice(choices) = &spec.kind {
                anyhow::ensure!(
                    choices.iter().any(|(v, _)| v == value),
                    "Unsupported value for {}: {value}",
                    spec.label
                );
            }
        }
        Ok(())
    }
}
pub fn backend(id: AgentId) -> Result<std::sync::Arc<dyn AgentBackend>> {
    match id {
        AgentId::Codex => Ok(std::sync::Arc::new(codex::Codex)),
        AgentId::Copilot => Ok(std::sync::Arc::new(copilot::Copilot)),
        AgentId::OpenCode => Ok(std::sync::Arc::new(opencode::OpenCode)),
        AgentId::Claude => {
            anyhow::bail!("{} is not implemented yet.", id.label())
        }
    }
}
pub fn value<'a>(options: &'a AgentOptions, spec: &'a OptionSpec) -> &'a str {
    options
        .get(spec.key)
        .map(String::as_str)
        .unwrap_or(spec.default)
}
pub async fn detect(
    backend: &dyn AgentBackend,
    options: &AgentOptions,
    directory: &Path,
) -> Result<String> {
    backend.validate(options)?;
    let result = process::capture(backend.detection(options, directory)?, 128 * 1024).await?;
    anyhow::ensure!(
        result.status.success(),
        "CLI detection failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    anyhow::ensure!(
        !result.truncated,
        "CLI help output was truncated; compatibility cannot be checked."
    );
    let help = String::from_utf8_lossy(&result.stdout);
    backend.check_detection(&help)?;
    let version = if let Some(command) = backend.version_command(options, directory)? {
        match process::capture(command, 8192).await {
            Ok(result) if result.status.success() && !result.truncated => {
                String::from_utf8_lossy(&result.stdout)
                    .lines()
                    .next()
                    .unwrap_or("Version unavailable")
                    .to_owned()
            }
            _ => "Version unavailable (help check passed)".into(),
        }
    } else {
        backend.id().label().to_owned()
    };
    Ok(format!(
        "{version} detected. CLI compatibility checked; authentication is used when a job runs."
    ))
}
