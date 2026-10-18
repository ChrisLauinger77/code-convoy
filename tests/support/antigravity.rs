//! Synthetic Antigravity stdin/stream-json fixture; never invokes agy or reads
//! credentials. Event shapes follow tests/fixtures/antigravity-success.jsonl.
use serde_json::json;
use std::io::Read;

const CONVERSATION: &str = "antigravity-fixture";

pub fn run(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    assert!(
        args.windows(2)
            .any(|pair| pair == ["--input-format", "stream-json"])
    );
    assert!(
        args.windows(2)
            .any(|pair| pair == ["--output-format", "stream-json"])
    );
    // A blank CodeConvoy limit must not cap Antigravity's own run.
    assert!(!args.iter().any(|arg| arg.starts_with("--print-timeout")));
    // The streaming transport drops prompts passed as flags.
    assert!(
        !args
            .iter()
            .any(|arg| arg == "-p" || arg == "--print" || arg == "--prompt")
    );
    assert!(std::env::var_os("PWD").is_none());
    assert!(std::env::vars_os().all(|(key, _)| !key.to_string_lossy().starts_with("GIT_")));
    let mut input = String::new();
    std::io::stdin().read_to_string(&mut input)?;
    let line = input
        .strip_suffix('\n')
        .filter(|line| !line.contains('\n'))
        .ok_or("expected exactly one stream-json line")?;
    let message: serde_json::Value = serde_json::from_str(line)?;
    assert_eq!(message["event"], "user");
    let prompt = message["message"]["content"]
        .as_str()
        .ok_or("expected string content")?
        .to_owned();
    if let Some(json) = prompt.strip_prefix("codeconvoy-fixture-gate\n") {
        let config: serde_json::Value = serde_json::from_str(json.lines().next().unwrap_or(json))?;
        // A lock-holding descendant: awaited like a tool process, or left behind
        // like a background "daemon" that outlives the agy process.
        let mut child = if config["spawn_child"].as_bool() == Some(true)
            || config["daemon"].as_bool() == Some(true)
        {
            let control = config["control"].as_str().ok_or("missing child control")?;
            Some(
                std::process::Command::new(std::env::current_exe()?)
                    .args(["validation-child", control])
                    .stdin(std::process::Stdio::null())
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .spawn()?,
            )
        } else {
            None
        };
        let result = super::gate(json, &prompt, args, false, false);
        if config["daemon"].as_bool() != Some(true)
            && let Some(child) = child.as_mut()
        {
            let _ = child.wait()?;
        }
        return result;
    }
    std::fs::write("agent-input", &prompt)?;
    std::fs::write("antigravity-args", serde_json::to_vec(&args[1..])?)?;
    match prompt.as_str() {
        "antigravity-fixture:missing-auth" => {
            eprintln!("Error: authentication required. Run agy in a terminal to sign in.");
            std::process::exit(1);
        }
        "antigravity-fixture:error" => {
            init()?;
            text("Partial answer");
            result("ERROR", "fixture quota exhausted", None);
            eprintln!(
                "AGY_ERROR: {}",
                json!({"status":"RESOURCE_EXHAUSTED","code":429,"retryable":false})
            );
            std::process::exit(3);
        }
        "antigravity-fixture:malformed" => {
            init()?;
            println!("{{broken-json");
            return Ok(());
        }
        "antigravity-fixture:incomplete" => {
            init()?;
            text("Started but never finished");
            return Ok(());
        }
        "antigravity-fixture:interrupted" => {
            init()?;
            result("INTERRUPTED", "", None);
            std::process::exit(130);
        }
        "antigravity-fixture:error-exit-zero" => {
            // Streaming sessions can warn and continue after a turn error.
            init()?;
            result("ERROR", "fixture model failure", None);
            return Ok(());
        }
        "antigravity-fixture:denied" => {
            init()?;
            eprintln!(
                "Antigravity fixture refused run_command; allow command(cargo) to permit it."
            );
            text("Edited files; tests were not run.");
            result("SUCCESS", "", Some(json!(["command(cargo test)"])));
            return Ok(());
        }
        "antigravity-fixture:wrong-cwd" => {
            println!(
                "{}",
                json!({"event":"init","conversation_id":CONVERSATION,"init":{"cwd":std::env::temp_dir(),"tools":[],"permission_mode":"request-review"}})
            );
            result("SUCCESS", "", None);
            return Ok(());
        }
        "antigravity-fixture:edit" => {
            std::fs::write("tracked.txt", "Antigravity fixture edit\n")?;
        }
        _ => {}
    }
    eprintln!("Antigravity fixture diagnostic");
    init()?;
    text("Fixture task complete.");
    result("SUCCESS", "", None);
    Ok(())
}

pub fn init() -> Result<(), Box<dyn std::error::Error>> {
    println!(
        "{}",
        json!({"event":"init","conversation_id":CONVERSATION,"init":{"cwd":std::env::current_dir()?,"tools":["run_command","write_to_file"],"permission_mode":"request-review"}})
    );
    println!(
        "{}",
        json!({"event":"step_update","step_update":{"conversation_id":CONVERSATION,"step_index":0,"state":"DONE","step_type":"user_input"}})
    );
    Ok(())
}

pub fn text(text: &str) {
    println!(
        "{}",
        json!({"event":"step_update","step_update":{"conversation_id":CONVERSATION,"step_index":1,"state":"DONE","step_type":"agent_response","text_delta":format!("{text}\n")}})
    );
}

pub fn active_tool() {
    println!(
        "{}",
        json!({"event":"step_update","step_update":{"conversation_id":CONVERSATION,"step_index":2,"state":"ACTIVE","step_type":"tool","tool_name":"run_command"}})
    );
}

pub fn result(status: &str, error: &str, denied: Option<serde_json::Value>) {
    let mut result = json!({"conversation_id":CONVERSATION,"status":status,"response":"","duration_seconds":0.1,"num_turns":1,"usage":{"total_tokens":1}});
    if !error.is_empty() {
        result["error"] = json!(error);
    }
    if let Some(denied) = denied {
        result["denied_actions"] = denied;
    }
    println!("{}", json!({"event":"result","result":result}));
}
