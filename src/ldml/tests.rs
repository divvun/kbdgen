use std::path::{Path, PathBuf};

use clap::Parser;
use kbd_model::Host;

use super::LdmlError;
use super::cli::LdmlCommand;
use super::compile::{Selection, compile};
use super::export::{export, render};
use super::import::{destinations, group, report};
use super::layouts::{HostDocument, compiled_layouts, host_documents, layout_files};
use super::yaml::LayoutFormat;

fn keyboard_xml(tag: &str, output: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<!-- a comment that v4 YAML drops -->
<keyboard3 locale="{tag}" conformsTo="45" xmlns:kbdgen="https://divvun.no/ns/kbdgen-ldml/1">
  <info name="Test" />
  <settings normalization="disabled" />
  <keys><key id="x1" output="{output}" /></keys>
  <layers formId="iso"><layer modifiers="none"><row keys="x1" /></layer></layers>
  <special><kbdgen:displayName lang="{tag}" name="Autonym" /></special>
</keyboard3>
"#
    )
}

fn document(tag: &str, host: Host, output: &str) -> HostDocument {
    let xml = keyboard_xml(tag, output);
    HostDocument {
        tag: tag.to_string(),
        host,
        source: kbd_ldml::read_keyboard("t.xml", None, xml.as_bytes()).unwrap(),
    }
}

fn documents() -> Vec<HostDocument> {
    vec![
        document("sme", Host::Windows, "á"),
        document("sme", Host::MacOs, "á"),
        document("sme", Host::Ios, "a"),
        document("smj", Host::Windows, "á"),
    ]
}

fn names(paths: &[PathBuf]) -> Vec<String> {
    paths
        .iter()
        .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
        .collect()
}

// [spec:kbdgen:req:ldml.cli.export/test]
#[test]
fn export_writes_one_file_per_layout_host() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("deep/out");
    let paths = export(&documents(), &Selection::default(), &out).unwrap();
    assert_eq!(
        names(&paths),
        [
            "sme.windows.xml",
            "sme.macOS.xml",
            "sme.iOS.xml",
            "smj.windows.xml"
        ]
    );
    let read = |n: &str| std::fs::read_to_string(out.join(n)).unwrap();
    let (windows, macos) = (read("sme.windows.xml"), read("sme.macOS.xml"));
    assert!(
        windows.contains(r#"<kbdgen:keyboard host="windows" />"#),
        "{windows}"
    );
    assert_eq!(windows.replace("host=\"windows\"", "host=\"macOS\""), macos);
    let back =
        kbd_ldml::resolve(&kbd_ldml::read_keyboard("x", None, macos.as_bytes()).unwrap()).unwrap();
    assert_eq!(back.keyboard.host, Some(Host::MacOs));
}

// [spec:kbdgen:req:ldml.cli.export/test]
#[test]
fn export_selection_and_failures() {
    let selection = Selection {
        layouts: vec!["sme".into()],
        hosts: vec![Host::Ios, Host::Windows],
    };
    let files = render(&documents(), &selection).unwrap();
    let names: Vec<&str> = files.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(names, ["sme.windows.xml", "sme.iOS.xml"]);
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("out");
    let unknown = Selection {
        layouts: vec!["fkv".into()],
        ..Selection::default()
    };
    let err = export(&documents(), &unknown, &out).unwrap_err();
    assert!(matches!(err, LdmlError::UnknownLayout(t) if t == "fkv"));
    assert!(!out.exists(), "a failure writes nothing");
}

// [spec:kbdgen:req:ldml.cli.compile/test]
#[test]
fn shared_hosts_compile_to_identical_models() {
    let layouts = compiled_layouts(&documents()).unwrap();
    assert_eq!(layouts.len(), 2);
    let sme = &layouts[0];
    assert_eq!(sme.keyboards.len(), 2, "windows and macOS share a keyboard");
    assert_eq!(
        sme.display_names.get("sme").map(String::as_str),
        Some("Autonym")
    );
    let dir = tempfile::tempdir().unwrap();
    let paths = compile(&layouts, &Selection::default(), dir.path()).unwrap();
    assert_eq!(paths.len(), 4);
    let read = |n: &str| std::fs::read(dir.path().join(n)).unwrap();
    assert_eq!(read("sme.windows.dvkb"), read("sme.macOS.dvkb"));
    assert_ne!(read("sme.windows.dvkb"), read("sme.iOS.dvkb"));
}

fn bundle(layouts: &[(&str, &str)]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("layouts")).unwrap();
    for (stem, yaml) in layouts {
        std::fs::write(
            dir.path().join("layouts").join(format!("{stem}.yaml")),
            yaml,
        )
        .unwrap();
    }
    dir
}

// [spec:kbdgen:def:ldml.cli.commands/test]
// [spec:kbdgen:def:ldml.yaml.detect/test]
// [spec:kbdgen:req:ldml.yaml.coexistence/test]
#[test]
fn layouts_load_in_bundle_order_by_format() {
    let dir = bundle(&[
        ("sme", "format: 4\ndisplayNames: {sme: Davvisámegiella}\n"),
        ("se-fi", "displayNames: {se: Davvisámegiella}\n"),
        ("smj", "format: 4\n"),
    ]);
    let files = layout_files(dir.path()).unwrap();
    let summary: Vec<(&str, LayoutFormat)> =
        files.iter().map(|f| (f.tag.as_str(), f.format)).collect();
    assert_eq!(
        summary,
        [
            ("se-FI", LayoutFormat::V3),
            ("sme", LayoutFormat::V4),
            ("smj", LayoutFormat::V4)
        ]
    );
    let err = host_documents(&files, &[]).unwrap_err();
    assert!(err.to_string().contains("kbdgen ldml migrate"), "{err}");
    let err = host_documents(&files, &["smj".into()]).unwrap_err();
    assert!(
        matches!(err, LdmlError::Layout4(ref e) if e.to_string().ends_with("smj.yaml: the field displayNames is required")),
        "{err}"
    );
    assert!(host_documents(&files, &["sme".into()]).unwrap().is_empty());
    assert!(matches!(
        host_documents(&files, &["fkv".into()]),
        Err(LdmlError::UnknownLayout(_))
    ));
    let bad = bundle(&[("sme", "format: 3\n")]);
    assert!(
        layout_files(bad.path())
            .unwrap_err()
            .to_string()
            .contains("format 3")
    );
}

fn write_xml(dir: &Path, name: &str, xml: &str) -> PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, xml).unwrap();
    path
}

// [spec:kbdgen:req:ldml.cli.import/test]
#[test]
fn import_groups_by_tag_and_host() {
    let dir = tempfile::tempdir().unwrap();
    let a = write_xml(dir.path(), "a.xml", &keyboard_xml("se-fi", "a"));
    let tagged = keyboard_xml("und", "b").replace(
        "<kbdgen:displayName",
        r#"<kbdgen:keyboard tag="se-FI" host="iOS" /><kbdgen:displayName"#,
    );
    let b = write_xml(dir.path(), "b.xml", &tagged);
    let c = write_xml(dir.path(), "c.xml", &keyboard_xml("sma", "c"));
    let groups = group(&[c.clone(), a.clone(), b.clone()]).unwrap();
    let summary: Vec<(&str, Vec<Option<Host>>)> = groups
        .iter()
        .map(|g| (g.tag.as_str(), g.files.iter().map(|f| f.host).collect()))
        .collect();
    assert_eq!(
        summary,
        [("se-FI", vec![None, Some(Host::Ios)]), ("sma", vec![None])]
    );
    let lines = report(&groups);
    assert!(
        lines
            .iter()
            .any(|l| l.ends_with("a.xml: 1 comment(s) dropped")),
        "{lines:?}"
    );
    assert!(
        lines.iter().any(|l| l.starts_with("warning: ")),
        "{lines:?}"
    );
    let dup = write_xml(dir.path(), "d.xml", &keyboard_xml("se-FI", "d"));
    let err = group(&[a, dup]).unwrap_err().to_string();
    assert!(
        err.contains("a.xml") && err.contains("d.xml") && err.contains("default"),
        "{err}"
    );
}

// [spec:kbdgen:req:ldml.cli.import/test]
#[test]
fn import_refuses_existing_layouts_without_force() {
    let dir = bundle(&[("sma", "displayNames: {sma: Åarjelsaemien}\n")]);
    let c = write_xml(dir.path(), "c.xml", &keyboard_xml("sma", "c"));
    let groups = group(&[c]).unwrap();
    let err = destinations(dir.path(), &groups, false).unwrap_err();
    assert!(err.to_string().contains("--force"), "{err}");
    let paths = destinations(dir.path(), &groups, true).unwrap();
    assert_eq!(paths, [dir.path().join("layouts/sma.yaml")]);
    let broken = write_xml(dir.path(), "broken.xml", "<keyboard3 locale=\"sma\"");
    assert!(group(&[broken]).is_err());
}

#[derive(Parser)]
struct Wrapper {
    #[command(subcommand)]
    command: LdmlCommand,
}

// [spec:kbdgen:def:ldml.cli.commands/test]
#[test]
fn command_line_shapes_parse() {
    let parse = |args: &[&str]| {
        Wrapper::try_parse_from(std::iter::once("ldml").chain(args.iter().copied()))
    };
    let export = parse(&[
        "export", "-b", "B", "-o", "O", "--layout", "sme", "--host", "macOS", "--host", "iOS",
    ])
    .unwrap();
    let LdmlCommand::Export(args) = export.command else {
        panic!("export");
    };
    assert_eq!(
        (args.layouts.as_slice(), args.hosts.as_slice()),
        (&["sme".to_string()][..], &[Host::MacOs, Host::Ios][..])
    );
    assert!(matches!(
        parse(&["compile", "-b", "B", "-o", "O"]).unwrap().command,
        LdmlCommand::Compile(_)
    ));
    let import = parse(&["import", "-b", "B", "--force", "x.xml", "y.xml"]).unwrap();
    let LdmlCommand::Import(args) = import.command else {
        panic!("import");
    };
    assert!(args.force && args.files.len() == 2);
    assert!(
        parse(&["import", "-b", "B"]).is_err(),
        "import needs XML files"
    );
    let bad = parse(&["export", "-b", "B", "-o", "O", "--host", "amiga"])
        .err()
        .unwrap()
        .to_string();
    assert!(bad.contains("unknown host amiga"), "{bad}");
}
