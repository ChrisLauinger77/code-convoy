//! Built-in command templates; these have no execution or persistence behavior.
use super::Command;

pub struct Preset {
    pub id: &'static str,
    pub name: &'static str,
    pub executable: &'static str,
    pub arguments: &'static [&'static str],
    pub description: &'static str,
}

impl Preset {
    pub fn command(&self) -> Command {
        Command {
            executable: self.executable.into(),
            arguments: self.arguments.iter().map(|s| (*s).into()).collect(),
        }
    }
}

pub const BUILT_INS: &[Preset] = &[
    Preset {
        id: "gnome-extension-lint",
        name: "npm Lint",
        executable: "npm",
        arguments: &["run", "lint"],
        description: "Run the project's npm lint script.",
    },
    Preset {
        id: "rust-tests",
        name: "Rust Tests",
        executable: "cargo",
        arguments: &["test"],
        description: "Run Rust tests.",
    },
    Preset {
        id: "rust-check",
        name: "Rust Check",
        executable: "cargo",
        arguments: &["check"],
        description: "Check Rust compilation.",
    },
    Preset {
        id: "rust-clippy",
        name: "Rust Clippy",
        executable: "cargo",
        arguments: &["clippy", "--all-targets"],
        description: "Lint all Rust targets with Clippy.",
    },
    Preset {
        id: "npm-test",
        name: "npm Test",
        executable: "npm",
        arguments: &["test"],
        description: "Run the project's npm test script.",
    },
    Preset {
        id: "npm-build",
        name: "npm Build",
        executable: "npm",
        arguments: &["run", "build"],
        description: "Run the project's npm build script.",
    },
    Preset {
        id: "python-pytest",
        name: "Python pytest",
        executable: "pytest",
        arguments: &[],
        description: "Run Python tests with pytest.",
    },
    Preset {
        id: "cmake-build",
        name: "CMake Build",
        executable: "cmake",
        arguments: &["--build", "build"],
        description: "Build the existing build directory.",
    },
    Preset {
        id: "make-test",
        name: "Make Test",
        executable: "make",
        arguments: &["test"],
        description: "Run the project's Make test target.",
    },
];
