//! Immutable, renamed test agents without opening their executable for writing.
use std::{fs, path::Path};

const AGENT: &str = env!("CARGO_BIN_EXE_codeconvoy-test-agent");

pub fn directory() -> tempfile::TempDir {
    // Hard links require the fixture and Cargo's executable on the same filesystem;
    // the system temporary directory may be on a separate mount.
    tempfile::Builder::new()
        .prefix(".codeconvoy-test-")
        .tempdir_in(Path::new(AGENT).parent().unwrap())
        .unwrap()
}

pub fn install(path: &Path) {
    // Copying while parallel tests spawn children can leak a writable descriptor
    // across fork until exec, making a subsequent launch fail with Linux ETXTBSY.
    // Link the already-built agent instead. Fixtures must never modify this file.
    fs::hard_link(AGENT, path).unwrap();
}
