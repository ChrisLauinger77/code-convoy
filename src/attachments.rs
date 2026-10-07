//! Local attachment metadata and bounded file validation, never persisted contents.
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::Read,
    path::{Path, PathBuf},
};

// Product memory/payload guards, not claims about every provider's context limits.
pub const MAX_FILES: usize = 32;
pub const MAX_FILE_BYTES: u64 = 10 * 1024 * 1024;
pub const MAX_TOTAL_BYTES: u64 = 16 * 1024 * 1024;
pub const EXTENSIONS: &[&str] = &[
    "md", "txt", "json", "yaml", "yml", "png", "jpg", "jpeg", "webp",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttachmentKind {
    Markdown,
    Text,
    Json,
    Yaml,
    Png,
    Jpeg,
    Webp,
}
impl AttachmentKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Markdown => "Markdown",
            Self::Text => "Text",
            Self::Json => "JSON",
            Self::Yaml => "YAML",
            Self::Png | Self::Jpeg | Self::Webp => "Image",
        }
    }
    pub fn mime(self) -> &'static str {
        match self {
            Self::Png => "image/png",
            Self::Jpeg => "image/jpeg",
            Self::Webp => "image/webp",
            _ => "text/plain",
        }
    }
    pub fn is_image(self) -> bool {
        matches!(self, Self::Png | Self::Jpeg | Self::Webp)
    }
    fn from_path(path: &Path) -> Result<Self> {
        let extension = path
            .extension()
            .and_then(|v| v.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        match extension.as_str() {
            "md" => Ok(Self::Markdown),
            "txt" => Ok(Self::Text),
            "json" => Ok(Self::Json),
            "yaml" | "yml" => Ok(Self::Yaml),
            "png" => Ok(Self::Png),
            "jpg" | "jpeg" => Ok(Self::Jpeg),
            "webp" => Ok(Self::Webp),
            _ => anyhow::bail!(
                "Unsupported attachment type: {}. Use Markdown, text, JSON, YAML, PNG, JPEG or WebP.",
                path.display()
            ),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Attachment {
    pub path: PathBuf,
    pub filename: String,
    pub kind: AttachmentKind,
    pub size: u64,
    pub sha256: String,
}
impl Attachment {
    pub fn inspect(path: &Path) -> Result<Self> {
        let kind = AttachmentKind::from_path(path)?;
        let filename = path
            .file_name()
            .and_then(|name| name.to_str())
            .context("Attachment filename must be Unicode to save it in local history.")?
            .to_owned();
        let path = path
            .canonicalize()
            .with_context(|| format!("Cannot locate attachment {}.", path.display()))?;
        anyhow::ensure!(
            path.to_str().is_some(),
            "Attachment path must be Unicode to save it in local history: {}",
            path.display()
        );
        let bytes = read_file(&path, kind)?;
        Ok(Self {
            path,
            filename,
            kind,
            size: bytes.len() as u64,
            sha256: sha256_hex(&bytes),
        })
    }

    /// Contents are transient and used only to construct the selected CLI's input.
    pub fn read_validated(&self) -> Result<Vec<u8>> {
        anyhow::ensure!(
            self.path.is_absolute(),
            "Attachment path must be absolute: {}.",
            self.filename
        );
        let bytes = read_file(&self.path, self.kind)?;
        anyhow::ensure!(
            bytes.len() as u64 == self.size && sha256_hex(&bytes) == self.sha256,
            "Attachment changed since it was added: {}. Remove it and add it again before launching.",
            self.path.display()
        );
        Ok(bytes)
    }
}

// Keep the persisted hash format stable across digest output type changes.
fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn read_file(path: &Path, kind: AttachmentKind) -> Result<Vec<u8>> {
    let metadata = std::fs::metadata(path).with_context(|| {
        format!(
            "Attachment is missing or unavailable: {}. Restore it or remove it from the draft.",
            path.display()
        )
    })?;
    anyhow::ensure!(
        metadata.is_file(),
        "Attachment must be a regular file: {}",
        path.display()
    );
    anyhow::ensure!(
        metadata.len() <= MAX_FILE_BYTES,
        "Attachment exceeds CodeConvoy's 10 MiB file limit: {}",
        path.display()
    );
    let file = File::open(path).with_context(|| {
        format!(
            "Cannot read attachment {}. Check access permissions.",
            path.display()
        )
    })?;
    anyhow::ensure!(
        file.metadata()?.is_file(),
        "Attachment must be a regular file: {}",
        path.display()
    );
    let mut bytes = Vec::new();
    file.take(MAX_FILE_BYTES + 1)
        .read_to_end(&mut bytes)
        .with_context(|| format!("Cannot read attachment {}.", path.display()))?;
    anyhow::ensure!(
        bytes.len() as u64 <= MAX_FILE_BYTES,
        "Attachment exceeds CodeConvoy's 10 MiB file limit: {}",
        path.display()
    );
    if !kind.is_image() {
        anyhow::ensure!(
            std::str::from_utf8(&bytes).is_ok() && !bytes.contains(&0),
            "Text attachment must contain UTF-8 text without NUL bytes: {}",
            path.display()
        );
    } else {
        let valid = match kind {
            AttachmentKind::Png => bytes.starts_with(b"\x89PNG\r\n\x1a\n"),
            AttachmentKind::Jpeg => bytes.starts_with(&[0xff, 0xd8, 0xff]),
            AttachmentKind::Webp => bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP"),
            _ => false,
        };
        anyhow::ensure!(
            valid,
            "Image contents do not match the file type: {}",
            path.display()
        );
    }
    Ok(bytes)
}

pub fn validate_limits(attachments: &[Attachment]) -> Result<()> {
    anyhow::ensure!(
        attachments.len() <= MAX_FILES,
        "Attach at most {MAX_FILES} files."
    );
    let mut total = 0u64;
    let mut paths = std::collections::HashSet::new();
    for attachment in attachments {
        anyhow::ensure!(
            attachment.size <= MAX_FILE_BYTES,
            "Attachment {} exceeds the 10 MiB file limit.",
            attachment.filename
        );
        anyhow::ensure!(
            paths.insert(&attachment.path),
            "Attachment supplied twice: {}.",
            attachment.filename
        );
        total = total.saturating_add(attachment.size);
    }
    anyhow::ensure!(
        total <= MAX_TOTAL_BYTES,
        "Attachments exceed CodeConvoy's 16 MiB total limit. Reduce the files before launching."
    );
    Ok(())
}

pub fn revalidate(attachments: &[Attachment]) -> Result<()> {
    validate_limits(attachments)?;
    for attachment in attachments {
        attachment.read_validated()?;
    }
    Ok(())
}

/// Reuse validates references and reports every omission, without reading on egui.
pub fn for_reuse(attachments: &[Attachment]) -> (Vec<Attachment>, Vec<String>) {
    let mut valid = Vec::new();
    let mut unavailable = Vec::new();
    for attachment in attachments {
        match attachment.read_validated() {
            Ok(_) => valid.push(attachment.clone()),
            Err(error) => unavailable.push(format!("{}: {error:#}", attachment.filename)),
        }
    }
    (valid, unavailable)
}

/// Common file reading/escaping only; each backend chooses how to transport it.
pub fn text_prompt(task: &crate::domain::TaskConfig) -> Result<String> {
    validate_limits(&task.attachments)?;
    let mut prompt = task.prompt.clone();
    for attachment in &task.attachments {
        let bytes = attachment.read_validated()?;
        if attachment.kind.is_image() {
            continue;
        }
        let content = std::str::from_utf8(&bytes).context("Text attachment is not UTF-8.")?;
        prompt.push_str("\n\nAttached task context (JSON-escaped file contents):\n");
        prompt.push_str(&serde_json::to_string(
            &serde_json::json!({"filename": attachment.filename, "content": content}),
        )?);
    }
    Ok(prompt)
}
