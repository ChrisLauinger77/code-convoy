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
        return super::gate(json, prompt, args, false, false);
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
            println!("{{\"type\":\"system\",\"subtype\":\"init\"}}");
            return Ok(());
        }
        "cursor-fixture:edit" => {
            assert!(args.iter().any(|arg| arg == "--force"));
            assert!(args.iter().any(|arg| arg == "--trust"));
            std::fs::write("tracked.txt", "Cursor fixture edit\n")?;
        }
        _ => {}
    }
    eprintln!("Cursor fixture diagnostic");
    print!("{}", include_str!("../fixtures/cursor-success.jsonl"));
    Ok(())
}
