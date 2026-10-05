//! Deterministic CLI fixture. Never used by the application.
use std::{
    io::{Read, Write},
    process::Command,
    time::Duration,
};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--help") {
        print!("{}", include_str!("../fixtures/copilot-1.0.65-help.txt"));
        return Ok(());
    }
    if args.iter().any(|a| a == "--version") {
        println!("GitHub Copilot CLI fixture 1.0.65");
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
    std::fs::write("agent-input", &input)?;
    if copilot {
        // Assert the real backend passes its known CLI flags and stdin correctly.
        assert!(args.iter().any(|a| a == "--no-ask-user"));
        assert!(args.windows(2).any(|a| a == ["--stream", "on"]));
        assert!(args.windows(2).any(|a| a == ["--output-format", "text"]));
        assert!(!args.iter().any(|a| a == "--prompt" || a == "-p"));
        print!("live fragment without newline");
        std::io::stdout().flush()?;
    } else {
        println!("{{\"type\":\"turn.started\"}}");
    }
    eprintln!("fixture diagnostic");
    std::thread::sleep(Duration::from_millis(250));
    if std::env::current_dir()?
        .file_name()
        .is_some_and(|n| n == "fail")
    {
        if copilot {
            eprintln!("fixture failure");
        } else {
            println!("{{\"type\":\"turn.failed\",\"error\":\"fixture failure\"}}");
        }
        std::process::exit(7);
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
