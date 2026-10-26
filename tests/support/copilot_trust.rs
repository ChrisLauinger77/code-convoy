//! Synthetic trust failures, not provider behavior. Only renamed fixtures opt in.
use std::{
    fs,
    io::{Read, Write},
    path::Path,
    time::{Duration, Instant},
};

pub fn selected() -> bool {
    std::env::current_exe().is_ok_and(|path| {
        path.file_stem()
            .is_some_and(|name| name.to_string_lossy().starts_with("copilot-trust-"))
    })
}

fn locked(path: &Path) -> Result<fs::File, Box<dyn std::error::Error>> {
    let file = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(path)?;
    fs2::FileExt::lock_exclusive(&file)?;
    Ok(file)
}

pub fn run(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    if args.get(1).is_some_and(|arg| arg == "blocked-child") {
        let _lock = locked(Path::new("blocked-child.lock"))?;
        fs::write("blocked-child.ready", "ready")?;
        loop {
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    let name = std::env::current_exe()?
        .file_stem()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let (version, help) = if name.ends_with("sync") {
        (
            "1.0.103",
            include_str!("../fixtures/copilot-1.0.103-help.txt"),
        )
    } else if name.ends_with("new") {
        (
            "1.0.99-1",
            include_str!("../fixtures/copilot-1.0.99-1-help.txt"),
        )
    } else {
        (
            "1.0.65",
            include_str!("../fixtures/copilot-1.0.65-help.txt"),
        )
    };
    // Like the real CLIs, reject options this version's help does not offer
    // (--trust-folder before 1.0.99-1, the remote-session flags across 1.0.103).
    for arg in args.iter().skip(1).filter(|arg| arg.starts_with("--")) {
        let flag = arg.split('=').next().unwrap_or(arg);
        let offered = help.split_whitespace().any(|word| {
            word.strip_prefix(flag)
                .is_some_and(|suffix| suffix.is_empty() || suffix.starts_with(['[', '=', ',']))
        });
        if !offered {
            return Err(format!("unexpected argument '{flag}' for Copilot {version}").into());
        }
    }
    if args.iter().any(|arg| arg == "--help") {
        print!("{help}");
        return Ok(());
    }
    if args.iter().any(|arg| arg == "--version") {
        println!("GitHub Copilot CLI fixture {version}");
        return Ok(());
    }
    let mut input = String::new();
    std::io::stdin().read_to_string(&mut input)?;
    fs::write(
        "copilot-received.json",
        serde_json::to_vec(&serde_json::json!({
            "directory": std::env::current_dir()?, "input": input, "arguments": &args[1..],
        }))?,
    )?;
    match input.lines().next() {
        Some("copilot-trust-fixture:complete") => {
            fs::write("tracked.txt", "Copilot fixture edit\n")?;
            println!("Copilot fixture completed.");
        }
        Some("copilot-trust-fixture:reject") => {
            eprintln!("Error: Workspace trust required for this directory.");
            std::process::exit(2);
        }
        Some("copilot-trust-fixture:confirmation") => {
            print!("Do you trust the files in this folder?");
            std::io::stdout().flush()?;
            let mut confirmation = [0];
            if std::io::stdin().read(&mut confirmation)? != 0 {
                return Err("Unexpected interactive reply after prompt".into());
            }
            eprintln!("Interactive workspace trust confirmation is unavailable: stdin is closed.");
            std::process::exit(1);
        }
        #[cfg(unix)]
        Some("copilot-trust-fixture:terminal") => {
            // A background process group reading its controlling terminal is
            // stopped by SIGTTIN instead of seeing EOF. SIGSTOP has the same
            // effect for the supervisor and needs no terminal.
            let _lock = locked(Path::new("stopped.lock"))?;
            fs::write("stopped.pid", std::process::id().to_string())?;
            eprint!("Do you trust the files in this folder?");
            std::io::stderr().flush()?;
            // SAFETY: raise has no memory-safety preconditions.
            unsafe { libc::raise(libc::SIGSTOP) };
            return Err("Stopped confirmation fixture was continued".into());
        }
        Some("copilot-trust-fixture:block") => {
            let _lock = locked(Path::new("blocked-parent.lock"))?;
            let _child = std::process::Command::new(std::env::current_exe()?)
                .arg("blocked-child")
                .spawn()?;
            let deadline = Instant::now() + Duration::from_secs(10);
            while !Path::new("blocked-child.ready").exists() {
                if Instant::now() > deadline {
                    return Err("Child readiness timed out".into());
                }
                std::thread::sleep(Duration::from_millis(5));
            }
            // No newline: output handling and Stop must not depend on line framing.
            eprint!("Do you trust the files in this folder?");
            std::io::stderr().flush()?;
            loop {
                std::thread::sleep(Duration::from_millis(10));
            }
        }
        _ => return Err("Unknown Copilot trust fixture task".into()),
    }
    Ok(())
}
