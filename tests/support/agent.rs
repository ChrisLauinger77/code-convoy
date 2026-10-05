//! Deterministic CLI fixture. Never used by the application.
use std::{
    io::{Read, Write},
    process::Command,
    time::Duration,
};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let opencode = args.get(1).is_some_and(|a| a == "run");
    if opencode && args.iter().any(|a| a == "--help") {
        print!("{}", include_str!("../fixtures/opencode-run-help.txt"));
        return Ok(());
    }
    if args.iter().any(|a| a == "--help") {
        println!("Codex fixture: --no-daemon --ask-for-approval exec");
        print!("{}", include_str!("../fixtures/copilot-1.0.65-help.txt"));
        return Ok(());
    }
    if args.iter().any(|a| a == "--version") {
        if args.iter().any(|a| a == "--no-auto-update") {
            println!("GitHub Copilot CLI fixture 1.0.65");
        } else {
            println!("1.18.34 (CodeConvoy fixture)");
        }
        return Ok(());
    }
    let copilot = args.iter().any(|a| a == "--no-auto-update");
    if args.get(1).is_some_and(|a| a == "descendant") {
        std::fs::write("descendant-ready", "ready")?;
        std::thread::sleep(Duration::from_millis(1200));
        std::fs::write("orphan-survived", "unexpected")?;
        return Ok(());
    }
    if args.get(1).is_some_and(|a| a == "flood") {
        for _ in 0..400 {
            std::io::stdout().write_all(&[b'x'; 8192])?;
        }
        return Ok(());
    }
    let mut input = String::new();
    std::io::stdin().read_to_string(&mut input)?;
    if let Some(json) = input.strip_prefix("codeconvoy-fixture-gate\n") {
        return gate(json, &input, &args, copilot, opencode);
    }
    if input == "spawn-child"
        || (input == "cancel-test"
            && std::env::current_dir()?
                .file_name()
                .is_some_and(|n| n == "tree"))
    {
        let mut child = Command::new(std::env::current_exe()?)
            .arg("descendant")
            .spawn()?;
        println!("child started");
        let _ = child.wait()?;
        return Ok(());
    }
    if opencode {
        assert!(args.windows(2).any(|a| a == ["--format", "json"]));
        let dir = args
            .windows(2)
            .find(|a| a[0] == "--dir")
            .expect("missing --dir");
        assert_eq!(std::path::Path::new(&dir[1]), std::env::current_dir()?);
        assert!(std::env::var_os("PWD").is_none());
    }
    std::fs::write("agent-input", &input)?;
    if copilot {
        // Assert the real backend passes its known CLI flags and stdin correctly.
        assert!(args.iter().any(|a| a == "--no-ask-user"));
        assert!(args.windows(2).any(|a| a == ["--stream", "on"]));
        assert!(args.windows(2).any(|a| a == ["--output-format", "text"]));
        assert!(!args.iter().any(|a| a == "--prompt" || a == "-p"));
        print!("live fragment without newline");
        std::io::stdout().flush()?;
    } else if opencode {
        println!("{{\"type\":\"step_start\",\"part\":{{\"type\":\"step-start\"}}}}");
    } else {
        println!("{{\"type\":\"turn.started\"}}");
    }
    eprintln!("fixture diagnostic");
    std::thread::sleep(Duration::from_millis(250));
    if std::env::current_dir()?
        .file_name()
        .is_some_and(|n| n == "fail")
    {
        if opencode {
            println!("{{\"type\":\"error\",\"error\":{{\"name\":\"fixture failure\"}}}}");
        } else if copilot {
            eprintln!("fixture failure");
        } else {
            println!("{{\"type\":\"turn.failed\",\"error\":\"fixture failure\"}}");
        }
        std::process::exit(7);
    }
    if opencode {
        opencode_complete();
        return Ok(());
    }
    if copilot {
        println!("\ndone");
        return Ok(());
    }
    println!(
        "{{\"type\":\"item.completed\",\"item\":{{\"type\":\"agent_message\",\"text\":\"done\"}}}}"
    );
    println!("{{\"type\":\"turn.completed\",\"usage\":{{}}}}");
    Ok(())
}

// Handshake-controlled integration jobs. The test releases a named gate; elapsed
// sleeps never determine whether a scheduling assertion passes. All files are in
// disposable test directories, and the lock file is Git-internal test metadata.
fn gate(
    json: &str,
    input: &str,
    args: &[String],
    copilot: bool,
    opencode: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    use fs2::FileExt;
    let config: serde_json::Value = serde_json::from_str(json)?;
    let root = std::path::Path::new(config["control"].as_str().ok_or("missing control path")?);
    let ticket = config["ticket"].as_str().ok_or("missing ticket")?;
    let cwd = std::env::current_dir()?;
    let name = cwd
        .file_name()
        .ok_or("missing repo name")?
        .to_string_lossy();
    let marker = format!("{ticket}-{name}");
    let lock = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(".git/codeconvoy-fixture.lock")?;
    lock.try_lock_exclusive()
        .map_err(|_| "same repository overlap")?;
    std::fs::write(root.join(format!("{marker}.input")), input)?;
    std::fs::write(
        root.join(format!("{marker}.args")),
        serde_json::to_vec(args)?,
    )?;
    if opencode {
        println!(
            "{}",
            serde_json::json!({"type":"text", "part":{"text":format!("fixture gate ready {marker}")}})
        );
    } else if copilot {
        println!("fixture gate ready {marker}");
    } else {
        println!(
            "{}",
            serde_json::json!({"type":"item.completed", "item":{"type":"agent_message", "text":format!("fixture gate ready {marker}")}})
        );
    }
    std::io::stdout().flush()?;
    let deadline = std::time::Instant::now() + Duration::from_secs(60);
    while !root.join(format!("{marker}.release")).exists() {
        if std::time::Instant::now() > deadline {
            return Err("fixture gate timed out".into());
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    if config["fail"].as_bool() == Some(true) {
        if opencode {
            println!("{{\"type\":\"error\",\"error\":{{\"name\":\"controlled failure\"}}}}");
        } else if !copilot {
            println!("{{\"type\":\"turn.failed\",\"error\":\"controlled failure\"}}");
        }
        std::process::exit(7);
    }
    if config["edit"].as_bool() == Some(true) {
        std::fs::write("fixture-change.txt", "controlled repository edit")?;
    }
    if opencode {
        opencode_complete();
    } else if copilot {
        println!("done");
    } else {
        println!("{{\"type\":\"turn.completed\",\"usage\":{{}}}}");
    }
    Ok(())
}

fn opencode_complete() {
    println!(
        "{}",
        serde_json::json!({"type":"text", "part":{"type":"text", "text":"done"}})
    );
    println!(
        "{}",
        serde_json::json!({"type":"step_finish", "part":{"type":"step-finish", "reason":"stop", "tokens":{}, "cost":0}})
    );
}
