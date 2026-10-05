use std::{env, path::PathBuf, process::Command};

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    windows_resources();
    git_revision();
}

#[cfg(windows)]
fn windows_resources() {
    println!("cargo:rerun-if-changed=assets/codeconvoy.ico");
    let mut resource = winresource::WindowsResource::new();
    resource
        .set_icon("assets/codeconvoy.ico")
        .set("ProductName", "CodeConvoy")
        .set(
            "FileDescription",
            "CodeConvoy — local coding-agent task runner",
        )
        .set("CompanyName", "CodeConvoy contributors")
        .set(
            "LegalCopyright",
            "Copyright 2026 CodeConvoy contributors. MIT license.",
        )
        .set("OriginalFilename", "CodeConvoy.exe");
    // winresource derives both numeric and string versions from Cargo metadata.
    if let Err(error) = resource.compile() {
        panic!("Unable to compile Windows application resources: {error}");
    }
}

#[cfg(not(windows))]
fn windows_resources() {}

fn git_revision() {
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
            // Watch existing ref files, or the refs directory for a packed ref
            // becoming loose. Never watch the whole .git directory: index and
            // FETCH_HEAD updates do not change the revision shown in About.
            if reference.starts_with("refs/") {
                while !path.exists() && path.pop() {}
            }
            if path.exists() {
                println!("cargo:rerun-if-changed={}", path.display());
            }
        }
    }
    if let Some(revision) = git(&["rev-parse", "--short=12", "HEAD"])
        && (7..=40).contains(&revision.len())
        && revision.bytes().all(|b| b.is_ascii_hexdigit())
    {
        println!("cargo:rustc-env=CODECONVOY_GIT_REV={revision}");
    }
}
