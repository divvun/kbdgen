//! The text service's sources call nothing that talks to other processes,
//! the network or the file system, starts processes, writes the registry
//! outside `DllRegisterServer`, creates windows, or records anything but
//! the ETW events of `win/trace.rs` (`tsf.component.self-contained`,
//! `tsf.component.logging`, `tsf.security.secure-mode`). Test code is
//! exempt.
//!
//! `OutputDebugString` and the event log stay forbidden: the first hands
//! its text to any debugger or to the session-wide `DBWIN` buffer that any
//! process can read, the second stores records through another process;
//! neither waits for a diagnostic switch. ETW keeps an event only while an
//! administrator's trace session enables the provider.

use std::path::{Path, PathBuf};

const FORBIDDEN: &[&str] = &[
    "std::fs",
    "std::net",
    "std::process",
    "std::env",
    "println!",
    "eprintln!",
    "print!",
    "dbg!",
    "OutputDebugString",
    "EventLog",
    "ReportEvent",
    "RegisterEventSource",
    // A leaked `String` is a `&'static str`, which a diagnostic event
    // accepts (`crate::diag`).
    "leak(",
    "transmute",
    "CreateFile",
    "WriteFile",
    "CreateProcess",
    "ShellExecute",
    "CreatePipe",
    "CreateNamedPipe",
    "CreateFileMapping",
    "PostMessage",
    "SendMessage",
    "CreateWindow",
    "WinHttp",
    "socket",
];

/// What only registration (`DllRegisterServer`) may do.
const REGISTRATION_ONLY: &[&str] = &["RegSetValue", "RegCreateKey", "RegDeleteTree"];

/// What only the diagnostics writer may call, so that every event passes
/// the gate of `crate::diag`.
const TRACE_ONLY: &[&str] = &[
    "tracelogging",
    "Diagnostics_Etw",
    "EventRegister",
    "EventWrite",
    "EventUnregister",
    "EventProviderEnabled",
    "EventSetInformation",
    "EventEnabled",
    "TraceLogging",
];

fn sources(dir: &Path, found: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let path = entry.path();
        let test_code = path
            .file_name()
            .is_some_and(|n| n == "tests" || n == "tests.rs");
        if test_code {
            continue;
        }
        if path.is_dir() {
            sources(&path, found);
        } else if path.extension().is_some_and(|e| e == "rs") {
            found.push(path);
        }
    }
}

// [spec:kbdgen:req:tsf.component.self-contained/test]
// [spec:kbdgen:req:tsf.component.logging+1/test]
// [spec:kbdgen:req:tsf.security.secure-mode+1/test]
// [spec:kbdgen:sem:tsf.security.integrity/test]
#[test]
fn sources_avoid_io_ipc_windows_and_logging() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    sources(&root, &mut files);
    assert!(files.len() > 10, "found the sources");
    let mut violations = Vec::new();
    for file in &files {
        let text = std::fs::read_to_string(file).unwrap();
        let code: String = text
            .lines()
            .filter(|line| !line.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n");
        let registration = file.ends_with("win/register.rs");
        let writer = file.ends_with("win/trace.rs");
        for token in FORBIDDEN {
            if code.contains(token) {
                violations.push(format!("{} uses {token}", file.display()));
            }
        }
        for token in REGISTRATION_ONLY {
            if !registration && code.contains(token) {
                violations.push(format!("{} uses {token}", file.display()));
            }
        }
        for token in TRACE_ONLY {
            if !writer && code.contains(token) {
                violations.push(format!("{} uses {token}", file.display()));
            }
        }
    }
    assert!(violations.is_empty(), "{}", violations.join("\n"));
}

/// The writer is reached only through `diag::record`, behind the gate.
// [spec:kbdgen:req:tsf.component.logging+1/test]
#[test]
fn only_the_gate_reaches_the_writer() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    sources(&root, &mut files);
    let callers: Vec<_> = files
        .iter()
        .filter(|file| {
            std::fs::read_to_string(file)
                .unwrap()
                .contains("trace::write")
        })
        .collect();
    assert_eq!(callers, [&root.join("diag.rs")]);
}
