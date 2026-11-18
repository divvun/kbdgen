//! The component file template (`linux.ibus.component`) and the `--xml`
//! lister run as ibus-daemon runs it (`linux.ibus.lister`).

// Clippy counts only `#[test]` functions of a test binary as tests, not
// the helpers beside them.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::error::Error;
use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use kbd_model::{Info, Keyboard};

const PROGRAM: &str = env!("CARGO_BIN_EXE_ibus-engine-divvun");
const TEMPLATE: &str = include_str!("../data/divvun.xml.in");

fn description(name: &str) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<engine>\n  <name>{name}</name>\n  \
         <longname>Sámi</longname>\n  <description>Sámi</description>\n  \
         <language>se</language>\n  <author>Test</author>\n  <layout>{name}</layout>\n  \
         <symbol>se</symbol>\n  <version>1.0</version>\n</engine>\n"
    )
}

fn install(root: &Path, name: &str) -> Result<(), Box<dyn Error>> {
    let dir = root.join("no.divvun");
    fs::create_dir_all(&dir)?;
    fs::write(dir.join(format!("{name}.xml")), description(name))?;
    let model = Keyboard::new("se", 45, Info::default()).to_bytes()?;
    fs::write(dir.join(format!("{name}.dvkb")), model)?;
    Ok(())
}

fn run(home: &Path, args: &[&str]) -> std::io::Result<Output> {
    Command::new(PROGRAM).args(args).env("HOME", home).output()
}

// [spec:kbdgen:def:linux.ibus.component/test]
#[test]
fn template_names_program_and_data_dirs() {
    let lines: Vec<&str> = TEMPLATE.lines().map(str::trim).collect();
    for line in [
        "<name>org.freedesktop.IBus.Divvun</name>",
        "<exec>@LIBEXECDIR@/ibus-engine-divvun --ibus --data-dir @DATADIR@/divvun/keyboards</exec>",
        "<version>@VERSION@</version>",
        "<license>MIT OR Apache-2.0</license>",
        "<path>@DATADIR@/divvun/keyboards/</path>",
        "<path>~/.local/share/divvun/keyboards/</path>",
        "<engines exec=\"@LIBEXECDIR@/ibus-engine-divvun --xml --data-dir @DATADIR@/divvun/keyboards\"/>",
    ] {
        assert!(lines.contains(&line), "{line}");
    }
    for element in ["description", "author", "homepage"] {
        assert!(
            lines
                .iter()
                .any(|line| line.starts_with(&format!("<{element}>"))),
            "{element}"
        );
    }
    assert_eq!(
        lines.first(),
        Some(&"<?xml version=\"1.0\" encoding=\"UTF-8\"?>")
    );
    assert!(TEMPLATE.ends_with("</component>\n"));
}

// [spec:kbdgen:req:linux.ibus.lister/test]
#[test]
fn lister_prints_engines_and_nothing_else() {
    let home = tempfile::tempdir().unwrap();
    let system = tempfile::tempdir().unwrap();
    install(
        &home.path().join(".local/share/divvun/keyboards"),
        "divvun-se",
    )
    .unwrap();
    install(system.path(), "divvun-sma").unwrap();
    fs::write(system.path().join("no.divvun/broken.xml"), "<engine>").unwrap();
    let output = run(
        home.path(),
        &["--xml", "--data-dir", system.path().to_str().unwrap()],
    )
    .unwrap();
    assert!(output.status.success());
    assert_eq!(output.stderr, b"");
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.starts_with("<engines>\n  <engine>\n    <name>divvun-se</name>\n"));
    assert!(stdout.contains("  </engine>\n  <engine>\n    <name>divvun-sma</name>\n"));
    assert_eq!(stdout.matches("<engine>").count(), 2, "{stdout}");
    assert!(stdout.ends_with("</engines>\n"));
}

// [spec:kbdgen:req:linux.ibus.lister/test]
#[test]
fn lister_without_keyboards_prints_empty_list() {
    let home = tempfile::tempdir().unwrap();
    let output = run(home.path(), &["--xml"]).unwrap();
    assert!(output.status.success());
    assert_eq!(output.stdout, b"<engines>\n</engines>\n");
    assert_eq!(output.stderr, b"");
}

#[test]
fn unknown_arguments_print_usage() {
    let home = tempfile::tempdir().unwrap();
    for args in [
        &[][..],
        &["--xml", "--data-dir"],
        &["--xml", "--xml"],
        &["-x"],
    ] {
        let output = run(home.path(), args).unwrap();
        assert_eq!(output.status.code(), Some(2), "{args:?}");
        assert!(output.stdout.is_empty());
        assert!(String::from_utf8_lossy(&output.stderr).starts_with("usage: "));
    }
}
