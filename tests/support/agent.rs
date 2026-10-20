//! Deterministic CLI fixture. Never used by the application.
use std::{
    io::{Read, Write},
    process::Command,
    time::Duration,
};
mod antigravity;
mod cursor;
mod workflow;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    if let Some(result) = git_client_probe(&args) {
        return result;
    }
    if args.get(1).is_some_and(|a| a.starts_with("validation-")) {
        return validation_fixture(&args);
    }
    if args.get(1).is_some_and(|a| a == "passthrough") {
        std::io::copy(&mut std::io::stdin(), &mut std::io::stdout())?;
        return Ok(());
    }
    if args.get(1).is_some_and(|a| a == "invocation-probe") {
        let mut input = String::new();
        std::io::stdin().read_to_string(&mut input)?;
        serde_json::to_writer(
            std::io::stdout(),
            &serde_json::json!({
                "arguments": &args[2..],
                "directory": std::env::current_dir()?,
                "input": input,
            }),
        )?;
        return Ok(());
    }
    if args.get(1).is_some_and(|a| a == "smudge-gate") {
        let mut input = Vec::new();
        std::io::stdin().read_to_end(&mut input)?;
        let status = Command::new(std::env::current_exe()?)
            .args(["smudge-child", &args[2]])
            .status()?;
        if !status.success() {
            return Err("filter child stopped".into());
        }
        std::io::stdout().write_all(&input)?;
        return Ok(());
    }
    if args.get(1).is_some_and(|a| a == "smudge-child") {
        let control = std::path::Path::new(&args[2]);
        let lock = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(false)
            .open(control.join("filter.lock"))?;
        fs2::FileExt::lock_exclusive(&lock)?;
        // Keep readiness after lock acquisition: cancellation tests use it to
        // know the descendant holds an OS lock before cancelling or aborting.
        std::fs::write(control.join("filter.ready"), "ready")?;
        let deadline = std::time::Instant::now() + Duration::from_secs(60);
        while !control.join("filter.release").exists() {
            if std::time::Instant::now() > deadline {
                return Err("filter gate timeout".into());
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        std::fs::write(
            control.join("filter.survived"),
            "unexpected after cancellation",
        )?;
        return Ok(());
    }
    if args.iter().any(|arg| arg.starts_with("--workspace=")) {
        return cursor::run(&args);
    }
    if is_antigravity(&args) {
        return antigravity::run(&args);
    }
    let mut _probe_lock = None;
    if args.iter().any(|arg| arg == "--help" || arg == "--version") {
        let directory = std::env::current_dir()?;
        if directory.join("probe-tracking").exists() {
            let executable = std::env::current_exe()?;
            let name = executable
                .file_stem()
                .ok_or("missing fixture name")?
                .to_string_lossy();
            let phase = if args.iter().any(|arg| arg == "--help") {
                "help"
            } else {
                "version"
            };
            let lock = std::fs::OpenOptions::new()
                .create(true)
                .truncate(false)
                .write(true)
                .open(directory.join(format!("{name}.probe-lock")))?;
            if fs2::FileExt::try_lock_exclusive(&lock).is_err() {
                std::fs::write(directory.join(format!("{name}.overlap")), "duplicate check")?;
                return Err("overlapping CLI probes".into());
            }
            _probe_lock = Some(lock);
            let mut log = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(directory.join(format!("{name}.probes")))?;
            writeln!(log, "{phase}")?;
            std::fs::write(directory.join(format!("{name}.{phase}.ready")), "ready")?;
            let deadline = std::time::Instant::now() + Duration::from_secs(60);
            while directory.join(format!("{name}.{phase}.block")).exists() {
                if std::time::Instant::now() > deadline {
                    return Err("probe gate timed out".into());
                }
                std::thread::sleep(Duration::from_millis(5));
            }
            if directory.join(format!("{name}.invalid")).exists() {
                println!("unrelated executable");
                return Ok(());
            }
            let version = directory.join(format!("{name}.version"));
            if phase == "version" && version.exists() {
                print!("{}", std::fs::read_to_string(version)?);
                return Ok(());
            }
        }
    }
    let opencode = args.get(1).is_some_and(|a| a == "run");
    // A link named opencode2 (OpenCode 2's alternative bin name) stands in for
    // OpenCode 2; every other name keeps the OpenCode 1 interface.
    let opencode2 = std::env::current_exe()?
        .file_stem()
        .is_some_and(|stem| stem.to_string_lossy().starts_with("opencode2"));
    if opencode && args.iter().any(|a| a == "--help") {
        if opencode2 {
            print!("{}", include_str!("../fixtures/opencode-2-run-help.txt"));
        } else {
            print!("{}", include_str!("../fixtures/opencode-run-help.txt"));
        }
        return Ok(());
    }
    if opencode {
        opencode_arguments(&args[2..], opencode2);
    }
    if args.iter().any(|a| a == "--help") {
        println!("Codex fixture: --no-daemon --ask-for-approval exec");
        if workflow::marker().is_some() {
            println!("--image PATH (disposable workflow fixture only)");
        }
        print!("{}", include_str!("../fixtures/copilot-1.0.65-help.txt"));
        print!("{}", include_str!("../fixtures/claude-help-contract.txt"));
        print!("{}", include_str!("../fixtures/cursor-help-contract.txt"));
        // agy 1.4.2 prints help only to stderr; keep it off stdout here so
        // detection must read the stream the backend selects.
        eprint!(
            "{}",
            include_str!("../fixtures/antigravity-help-contract.txt")
        );
        return Ok(());
    }
    if args.iter().any(|a| a == "--version") {
        if args.iter().any(|a| a == "--no-auto-update") {
            println!("GitHub Copilot CLI fixture 1.0.65");
        } else if opencode2 {
            println!("opencode v2.0.32 (CodeConvoy fixture)");
        } else {
            println!("1.18.34 (CodeConvoy fixture)");
        }
        return Ok(());
    }
    let claude = args.iter().any(|a| a == "--print");
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
    if workflow::marker().is_some() {
        return workflow::run(&input, &args, copilot, opencode, claude);
    }
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
        if opencode2 {
            // OpenCode 2 runs where it starts; never via the shared service.
            assert!(args.iter().any(|a| a == "--standalone"));
        } else {
            let dir = args
                .windows(2)
                .find(|a| a[0] == "--dir")
                .expect("missing --dir");
            assert_eq!(
                std::path::Path::new(&dir[1]).canonicalize()?,
                std::env::current_dir()?.canonicalize()?
            );
        }
        assert!(std::env::var_os("PWD").is_none());
    }
    if claude {
        assert!(args.windows(2).any(|a| a == ["--input-format", "text"]));
        assert!(
            args.windows(2)
                .any(|a| a == ["--output-format", "stream-json"])
        );
        assert!(args.iter().any(|a| a == "--verbose"));
        assert!(args.iter().any(|a| a == "--no-session-persistence"));
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
    } else if claude {
        println!("{{\"type\":\"system\",\"subtype\":\"init\",\"session_id\":\"fixture\"}}");
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
        if claude {
            claude_result(true);
        } else if opencode {
            println!("{{\"type\":\"error\",\"error\":{{\"name\":\"fixture failure\"}}}}");
        } else if copilot {
            eprintln!("fixture failure");
        } else {
            println!("{{\"type\":\"turn.failed\",\"error\":\"fixture failure\"}}");
        }
        std::process::exit(7);
    }
    if claude {
        claude_result(false);
        return Ok(());
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
    let cursor = args.iter().any(|a| a.starts_with("--workspace="));
    let antigravity = is_antigravity(args);
    let claude = !cursor && args.iter().any(|a| a == "--print");
    // Attachment context follows the one-line control record. Capture the full
    // stdin below while keeping this test-only handshake independent of it.
    let config: serde_json::Value = serde_json::from_str(json.lines().next().unwrap_or(json))?;
    let root = std::path::Path::new(config["control"].as_str().ok_or("missing control path")?);
    let ticket = config["ticket"].as_str().ok_or("missing ticket")?;
    let cwd = std::env::current_dir()?;
    let name = cwd
        .file_name()
        .ok_or("missing repo name")?
        .to_string_lossy();
    let marker = format!("{ticket}-{name}");
    std::fs::write(
        root.join(format!("{marker}.cwd")),
        cwd.to_string_lossy().as_bytes(),
    )?;
    let git_dir = Command::new("git")
        .args(["rev-parse", "--absolute-git-dir"])
        .output()?;
    if !git_dir.status.success() {
        return Err("fixture Git directory unavailable".into());
    }
    let git_dir = std::path::PathBuf::from(String::from_utf8(git_dir.stdout)?.trim());
    if config["edit_before"].as_bool() == Some(true) {
        std::fs::write("tracked.txt", ticket)?;
    }
    let lock = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(git_dir.join("codeconvoy-fixture.lock"))?;
    lock.try_lock_exclusive()
        .map_err(|_| "same repository overlap")?;
    std::fs::write(root.join(format!("{marker}.input")), input)?;
    std::fs::write(
        root.join(format!("{marker}.args")),
        serde_json::to_vec(args)?,
    )?;
    if antigravity {
        antigravity::init()?;
        antigravity::text(&format!("fixture gate ready {marker}"));
    } else if cursor {
        cursor::init()?;
        println!(
            "{}",
            serde_json::json!({"type":"assistant", "session_id":"cursor-fixture", "message":{"role":"assistant", "content":[{"type":"text","text":format!("fixture gate ready {marker}")}]}})
        );
    } else if claude {
        if let Some(init) = config.get("claude_init") {
            println!("{init}");
        }
        println!(
            "{}",
            serde_json::json!({"type":"assistant", "message":{"content":[{"type":"text","text":format!("fixture gate ready {marker}")}]}})
        );
    } else if opencode {
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
        if antigravity {
            // A turn error may still exit 0; the result status must govern.
            antigravity::result("ERROR", "controlled failure", None);
            std::process::exit(0);
        } else if cursor {
            eprintln!("Cursor fixture failure");
        } else if claude {
            claude_result(true);
        } else if opencode {
            println!("{{\"type\":\"error\",\"error\":{{\"name\":\"controlled failure\"}}}}");
        } else if !copilot {
            println!("{{\"type\":\"turn.failed\",\"error\":\"controlled failure\"}}");
        }
        std::process::exit(7);
    }
    if config["edit"].as_bool() == Some(true) {
        std::fs::write("fixture-change.txt", "controlled repository edit")?;
    }
    if claude {
        match config["claude_outcome"].as_str() {
            Some("result_error") => {
                claude_result(true);
                return Ok(());
            }
            Some("malformed") => {
                println!("{{broken JSON");
                return Ok(());
            }
            Some("incomplete") => return Ok(()),
            _ => {}
        }
    }
    if antigravity {
        match config["antigravity_outcome"].as_str() {
            Some("missing_auth") => {
                eprintln!("Error: authentication required. Run agy in a terminal to sign in.");
                std::process::exit(1);
            }
            Some("interrupted") => {
                antigravity::result("INTERRUPTED", "", None);
                std::process::exit(130);
            }
            Some("malformed") => {
                println!("{{broken JSON");
                return Ok(());
            }
            Some("incomplete") => return Ok(()),
            Some("active_step") => antigravity::active_tool(),
            Some("agy_error") => eprintln!("AGY_ERROR: {{\"status\":\"UNAVAILABLE\"}}"),
            _ => {}
        }
        antigravity::result("SUCCESS", "", None);
    } else if cursor {
        match config["cursor_outcome"].as_str() {
            Some("missing_auth") => {
                eprint!(
                    "{}",
                    include_str!("../fixtures/cursor-2026.10.16-authentication.stderr.txt")
                );
                std::process::exit(1);
            }
            Some("interrupted") => {
                eprintln!("Cursor fixture interrupted");
                std::process::exit(130);
            }
            Some("malformed") => {
                println!("{{broken JSON");
                return Ok(());
            }
            Some("incomplete") => return Ok(()),
            Some("permission") => {
                eprintln!("Permission denied: approval required for writeToolCall")
            }
            Some("wrong_session") => {
                println!(
                    "{{\"type\":\"assistant\",\"session_id\":\"other\",\"message\":{{\"content\":[{{\"type\":\"text\",\"text\":\"wrong session\"}}]}}}}"
                );
            }
            _ => {}
        }
        cursor::complete()?;
    } else if claude {
        claude_result(false);
    } else if opencode {
        opencode_complete();
    } else if copilot {
        println!("done");
    } else {
        println!("{{\"type\":\"turn.completed\",\"usage\":{{}}}}");
    }
    Ok(())
}

// Antigravity streams stdin without --print; Claude's stream-json input uses it.
fn is_antigravity(args: &[String]) -> bool {
    args.windows(2)
        .any(|pair| pair == ["--input-format", "stream-json"])
        && !args.iter().any(|arg| arg == "--print")
}

/// Emulates each OpenCode CLI's strict parser before the prompt is read:
/// OpenCode 1 (strict yargs) has no --standalone, and OpenCode 2 (effect/cli)
/// rejects unknown flags such as --dir and --variant. Both exit 1.
fn opencode_arguments(args: &[String], opencode2: bool) {
    const V1: &[&str] = &[
        "format", "dir", "model", "agent", "variant", "auto", "file", "continue", "session",
        "fork", "title", "attach", "port", "thinking",
    ];
    const V2: &[&str] = &[
        "standalone",
        "server",
        "format",
        "model",
        "agent",
        "auto",
        "file",
        "continue",
        "session",
        "fork",
        "title",
        "thinking",
    ];
    for name in args.iter().filter_map(|arg| arg.strip_prefix("--")) {
        let name = name.split_once('=').map_or(name, |(name, _)| name);
        if opencode2 && !V2.contains(&name) {
            eprintln!("\nERROR\n  Unrecognized flag: --{name} in command opencode run");
            std::process::exit(1);
        }
        if !opencode2 && !V1.contains(&name) {
            eprintln!("Unknown argument: {name}");
            std::process::exit(1);
        }
    }
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

fn claude_result(failed: bool) {
    println!(
        "{}",
        serde_json::json!({
            "type":"result", "subtype":if failed {"error_during_execution"} else {"success"},
            "is_error":failed, "result":if failed {"fixture failure"} else {"done"},
            "duration_ms":1, "duration_api_ms":1, "num_turns":1, "session_id":"fixture"
        })
    );
}

fn validation_fixture(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let mode = &args[1];
    if matches!(
        mode.as_str(),
        "validation-gate" | "validation-child" | "validation-crash-tree" | "validation-orphan-tree"
    ) {
        let control = std::path::Path::new(&args[2]);
        let parent = mode != "validation-child";
        let name = if parent { "parent" } else { "child" };
        let lock = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(false)
            .open(control.join(format!("{name}.lock")))?;
        fs2::FileExt::lock_exclusive(&lock)?;
        let mut child = if parent {
            let mut command = Command::new(std::env::current_exe()?);
            command.args(["validation-child", &args[2]]);
            if mode == "validation-orphan-tree" {
                command
                    .stdin(std::process::Stdio::null())
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null());
            }
            Some(command.spawn()?)
        } else {
            None
        };
        std::fs::write(
            control.join(format!("{name}.pid")),
            std::process::id().to_string(),
        )?;
        std::fs::write(control.join(format!("{name}.ready")), "ready")?;
        println!("validation {name} ready");
        let deadline = std::time::Instant::now() + Duration::from_secs(30);
        if mode == "validation-crash-tree" || mode == "validation-orphan-tree" {
            while !control.join("child.ready").exists() {
                if std::time::Instant::now() > deadline {
                    return Err("crash fixture child timed out".into());
                }
                std::thread::sleep(Duration::from_millis(5));
            }
            // Let the test choose the exact exit moment after observing locks.
            while !control.join("exit-parent").exists() {
                if std::time::Instant::now() > deadline {
                    return Err("parent exit gate timed out".into());
                }
                std::thread::sleep(Duration::from_millis(5));
            }
            std::process::exit(args.get(3).and_then(|s| s.parse().ok()).unwrap_or(7));
        }
        while !control.join("release").exists() {
            if std::time::Instant::now() > deadline {
                return Err("validation gate timed out".into());
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        if let Some(child) = &mut child {
            let _ = child.wait()?;
        }
        return Ok(());
    }
    if mode == "validation-flood" {
        for _ in 0..32 {
            std::io::stdout().write_all(&[b'x'; 8192])?;
        }
    }
    if mode == "validation-edit" {
        std::fs::write("tracked.txt", "validation changed this file\n")?;
    }
    println!("arguments={}", serde_json::to_string(&args[2..])?);
    println!("directory={}", std::env::current_dir()?.display());
    println!("validation stdout Grüße 日本語");
    eprintln!("validation stderr");
    if mode == "validation-fail" {
        std::process::exit(7);
    }
    Ok(())
}

/// Stands in for an external Git client: records how it was started next to
/// itself. The file name selects the outcome, because the launcher, not the
/// test, chooses the arguments.
fn git_client_probe(args: &[String]) -> Option<Result<(), Box<dyn std::error::Error>>> {
    let executable = std::env::current_exe().ok()?;
    let stem = executable.file_stem()?.to_str()?.to_owned();
    let behavior = stem.strip_prefix("git-client-probe")?.to_owned();
    Some((|| {
        let record = serde_json::json!({
            "arguments": &args[1..],
            "directory": std::env::current_dir()?,
        });
        std::fs::write(
            executable.with_file_name(format!("{stem}.json")),
            serde_json::to_vec(&record)?,
        )?;
        match behavior.as_str() {
            "-fail" => std::process::exit(3),
            "-wait" => {
                std::thread::sleep(Duration::from_secs(4));
                std::fs::write(executable.with_file_name(format!("{stem}.done")), "")?;
            }
            _ => {}
        }
        Ok(())
    })())
}
