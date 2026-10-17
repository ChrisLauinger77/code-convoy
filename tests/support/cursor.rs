//! Synthetic direct-CLI fixture; never invokes Cursor or reads credentials.
use std::path::Path;

pub fn run(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    assert_eq!(args[1], "agent");
    let separator = args
        .iter()
        .position(|arg| arg == "--")
        .ok_or("missing option terminator")?;
    let prompt = args.get(separator + 1).ok_or("missing positional prompt")?;
    assert_eq!(args.len(), separator + 2);
    assert!(args.iter().any(|arg| arg == "--print"));
    assert!(
        args.windows(2)
            .any(|pair| pair == ["--output-format", "stream-json"])
    );
    let workspace = args
        .iter()
        .find_map(|arg| arg.strip_prefix("--workspace="))
        .ok_or("missing workspace")?;
    let cwd = std::env::current_dir()?;
    assert_eq!(Path::new(workspace).canonicalize()?, cwd.canonicalize()?);
    assert!(std::env::var_os("PWD").is_none());
    assert!(std::env::vars_os().all(|(key, _)| !key.to_string_lossy().starts_with("GIT_")));
    if let Some(json) = prompt.strip_prefix("codeconvoy-fixture-gate\n") {
        let config: serde_json::Value = serde_json::from_str(json.lines().next().unwrap_or(json))?;
        let _exit_lock = if config["observe_exit"].as_bool() == Some(true) {
            let control = Path::new(config["control"].as_str().ok_or("missing exit control")?);
            let lock = std::fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(false)
                .open(control.join("parent.lock"))?;
            fs2::FileExt::lock_exclusive(&lock)?;
            std::fs::write(control.join("parent.ready"), "ready")?;
            Some(lock)
        } else {
            None
        };
        let mut child = if config["spawn_child"].as_bool() == Some(true) {
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
        let result = super::gate(json, prompt, args, false, false);
        if let Some(child) = child.as_mut() {
            let _ = child.wait()?;
        }
        if config["observe_exit"].as_bool() == Some(true) && result.is_ok() {
            std::io::Write::flush(&mut std::io::stdout())?;
            // Keep the lock until OS exit rather than releasing it in Drop.
            std::process::exit(0);
        }
        return result;
    }
    std::fs::write("agent-input", prompt)?;
    std::fs::write("cursor-cwd", cwd.to_string_lossy().as_bytes())?;
    match prompt.as_str() {
        "cursor-fixture:missing-auth" => {
            eprintln!("Not authenticated. Run agent login.");
            std::process::exit(1);
        }
        "cursor-fixture:failure" => {
            eprintln!("Cursor fixture execution failed");
            std::process::exit(7);
        }
        "cursor-fixture:malformed" => {
            println!("{{broken-json");
            return Ok(());
        }
        "cursor-fixture:incomplete" => {
            init()?;
            return Ok(());
        }
        "cursor-fixture:permission" => {
            init()?;
            eprintln!("Permission denied: approval required for writeToolCall");
            complete()?;
            return Ok(());
        }
        "cursor-fixture:interrupted" => {
            init()?;
            eprintln!("Cursor fixture interrupted before completion");
            std::process::exit(130);
        }
        "cursor-fixture:edit" => {
            assert!(args.iter().any(|arg| arg == "--force"));
            assert!(args.iter().any(|arg| arg == "--trust"));
            std::fs::write("tracked.txt", "Cursor fixture edit\n")?;
        }
        _ => {}
    }
    eprintln!("Cursor fixture diagnostic");
    init()?;
    complete()?;
    Ok(())
}

pub fn init() -> Result<(), Box<dyn std::error::Error>> {
    let mut init: serde_json::Value = serde_json::from_str(
        include_str!("../fixtures/cursor-success.jsonl")
            .lines()
            .next()
            .ok_or("missing init fixture")?,
    )?;
    init["cwd"] = serde_json::json!(std::env::current_dir()?);
    println!("{init}");
    Ok(())
}

pub fn complete() -> Result<(), Box<dyn std::error::Error>> {
    for line in include_str!("../fixtures/cursor-success.jsonl")
        .lines()
        .skip(1)
    {
        println!("{line}");
    }
    Ok(())
}
