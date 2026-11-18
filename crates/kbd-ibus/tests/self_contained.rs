//! The program's sources write nothing but diagnostics, through
//! `crate::diag`, and `--xml`'s list and the usage line, in `main.rs`;
//! they write no file, start no process and open no network connection
//! (`linux.ibus.logging`). Test code is exempt.

// Clippy counts only `#[test]` functions of a test binary as tests, not
// the helpers beside them.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::path::{Path, PathBuf};

const FORBIDDEN: &[&str] = &[
    "println!",
    "eprintln!",
    "print!",
    "eprint!",
    "dbg!",
    "std::process::Command",
    "Command::new",
    "std::net",
    "TcpStream",
    "UdpSocket",
    "fs::write",
    "File::create",
    "OpenOptions",
    "create_dir",
    "remove_file",
    "fs::rename",
    "tracing::",
    "log::",
    "syslog",
    // A leaked `String` is a `&'static str`, which a diagnostic accepts.
    "leak(",
    "transmute",
];

/// What only `diag.rs` and `main.rs` may write to.
const STREAMS: &[&str] = &["stderr()", "stdout()"];

fn sources(dir: &Path, found: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let path = entry.path();
        if path.file_name().is_some_and(|name| name == "tests") {
            continue;
        }
        if path.is_dir() {
            sources(&path, found);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            found.push(path);
        }
    }
}

fn code(file: &Path) -> String {
    let text = std::fs::read_to_string(file).unwrap();
    let code = text.split("#[cfg(test)]\npub mod recorder").next().unwrap();
    code.lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n")
}

// [spec:kbdgen:req:linux.ibus.logging/test]
#[test]
fn sources_write_only_diagnostics() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    sources(&root, &mut files);
    assert!(files.len() > 8, "found the sources");
    let mut violations = Vec::new();
    for file in &files {
        let code = code(file);
        let writer = file.ends_with("diag.rs") || file.ends_with("main.rs");
        for token in FORBIDDEN {
            if code.contains(token) {
                violations.push(format!("{} uses {token}", file.display()));
            }
        }
        for token in STREAMS {
            if !writer && code.contains(token) {
                violations.push(format!("{} uses {token}", file.display()));
            }
        }
        if !file.ends_with("diag.rs") && code.contains("FileName(") {
            violations.push(format!("{} makes a FileName", file.display()));
        }
    }
    assert!(violations.is_empty(), "{}", violations.join("\n"));
}
