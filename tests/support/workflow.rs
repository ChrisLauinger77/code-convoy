//! Opt-in fixture mode for disposable end-to-end workflows and native UI checks.
//! Its marker lives in a temporary repository's .git directory, never user state.
use base64::Engine;
use sha2::{Digest, Sha256};
use std::{io::Write, path::Path};

pub fn run(
    input: &str,
    args: &[String],
    copilot: bool,
    opencode: bool,
    claude: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let cwd = std::env::current_dir()?;
    let config: serde_json::Value =
        serde_json::from_slice(&std::fs::read(".git/codeconvoy-workflow.json")?)?;
    let mut images = Vec::new();
    let text = if claude
        && args
            .windows(2)
            .any(|pair| pair == ["--input-format", "stream-json"])
    {
        let message: serde_json::Value = serde_json::from_str(input)?;
        let blocks = message["message"]["content"]
            .as_array()
            .ok_or("missing content blocks")?;
        let mut text = String::new();
        for block in blocks {
            match block["type"].as_str() {
                Some("text") => text.push_str(block["text"].as_str().ok_or("missing text")?),
                Some("image") => {
                    let bytes = base64::engine::general_purpose::STANDARD
                        .decode(block["source"]["data"].as_str().ok_or("missing image")?)?;
                    images.push(format!("{:x}", Sha256::digest(&bytes)));
                }
                _ => return Err("unexpected input block".into()),
            }
        }
        text
    } else {
        for (index, arg) in args.iter().enumerate() {
            let path = if arg == "--image" {
                args.get(index + 1).map(String::as_str)
            } else {
                arg.strip_prefix("--attachment=")
                    .or_else(|| arg.strip_prefix("--file="))
            };
            if let Some(path) = path {
                if !Path::new(path).is_absolute() {
                    return Err("relative image path".into());
                }
                images.push(format!("{:x}", Sha256::digest(std::fs::read(path)?)));
            }
        }
        input.to_owned()
    };
    let name = cwd
        .file_name()
        .ok_or("missing repository name")?
        .to_string_lossy();
    let receipt = serde_json::json!({"repository": name, "text": text, "image_sha256": images});
    std::fs::write(
        ".git/codeconvoy-received.json",
        serde_json::to_vec_pretty(&receipt)?,
    )?;
    let progress = format!(
        "Fixture received context for {name}: {} image(s)",
        images.len()
    );
    if claude {
        println!(
            "{}",
            serde_json::json!({"type":"assistant","message":{"content":[{"type":"text","text":progress}]}})
        );
    } else if opencode {
        println!("{}", serde_json::json!({"type":"step_start","part":{}}));
        println!(
            "{}",
            serde_json::json!({"type":"text","part":{"text":progress}})
        );
    } else if copilot {
        println!("{progress}");
    } else {
        println!("{}", serde_json::json!({"type":"turn.started"}));
        println!(
            "{}",
            serde_json::json!({"type":"item.completed","item":{"type":"agent_message","text":progress}})
        );
    }
    std::io::stdout().flush()?;
    std::thread::sleep(std::time::Duration::from_millis(
        config["delay_ms"].as_u64().unwrap_or(0).min(30_000),
    ));
    let result = format!("Fixture result for {name}: independent repository review complete");
    if claude {
        println!(
            "{}",
            serde_json::json!({"type":"result","subtype":"success","is_error":false,"result":result,"duration_ms":1,"duration_api_ms":1,"num_turns":1,"session_id":format!("fixture-{name}")})
        );
    } else if opencode {
        println!(
            "{}",
            serde_json::json!({"type":"text","part":{"text":result}})
        );
        super::opencode_complete();
    } else if copilot {
        println!("{result}");
    } else {
        println!(
            "{}",
            serde_json::json!({"type":"item.completed","item":{"type":"agent_message","text":result}})
        );
        println!(
            "{}",
            serde_json::json!({"type":"turn.completed","usage":{}})
        );
    }
    Ok(())
}
