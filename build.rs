use std::{env, path::PathBuf, process::Command};

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    let Some(root) = env::var_os("CARGO_MANIFEST_DIR").map(PathBuf::from) else {
        return;
    };
    let git = |args: &[&str]| -> Option<String> {
        let mut command = Command::new("git");
        command.current_dir(&root).args(args);
        for (key, _) in
            env::vars_os().filter(|(k, _)| k.to_str().is_some_and(|s| s.starts_with("GIT_")))
        {
            command.env_remove(key);
        }
        let output = command.output().ok()?;
        output
            .status
            .success()
            .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
    };
    // A source archive nested inside somebody else's checkout has no source revision.
    let Some(top) = git(&["rev-parse", "--show-toplevel"]) else {
        return;
    };
    if PathBuf::from(top).canonicalize().ok() != root.canonicalize().ok() {
        return;
    }
    for reference in [
        Some("HEAD".to_owned()),
        Some("packed-refs".to_owned()),
        git(&["symbolic-ref", "-q", "HEAD"]),
    ]
    .into_iter()
    .flatten()
    {
        if let Some(path) = git(&["rev-parse", "--git-path", &reference]) {
            let mut path = root.join(path);
            // Missing loose refs/packed-refs are normal. Track their existing
            // parent until created, rather than forcing a rebuild every time.
            while !path.exists() && path.pop() {}
            println!("cargo:rerun-if-changed={}", path.display());
        }
    }
    if let Some(revision) = git(&["rev-parse", "--short=12", "HEAD"])
        && (7..=40).contains(&revision.len())
        && revision.bytes().all(|b| b.is_ascii_hexdigit())
    {
        println!("cargo:rustc-env=CODECONVOY_GIT_REV={revision}");
    }
}
