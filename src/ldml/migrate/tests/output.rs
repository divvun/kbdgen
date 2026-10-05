//! `ldml.migrate.output`, `ldml.migrate.report`, `ldml.migrate.caps-diff`
//! and `ldml.migrate.equivalence`.

use super::super::check;
use super::super::convert::{Cell, DesktopTrace, LayerTrace, Out, Trace, convert};
use super::super::oracle::{CapsState, caps_output};
use super::super::out::{scalar, write};
use super::super::source::{self, Desktop};
use super::*;
use crate::ldml::yaml::load;

fn full(extra: &str) -> String {
    format!(
        "displayNames:\n  se: Davvisámegiella\nlongpress:\n  a: á\nkeyNames:\n  space: space\n  return: enter\ndecimal: ','\n{}{extra}{ACUTE}",
        desktop(
            "windows",
            &[
                ("default", &KEYS.replace("+ ´", "+    ´")),
                ("shift", SHIFTED)
            ],
            "  deadKeys:\n    default: ['´']\n  config:\n    id: SE01\n"
        )
    )
}

const ACUTE: &str = "transforms:\n  ´:\n    ' ': ´\n    a: á\n";

// [spec:kbdgen:req:ldml.migrate.output/test]
#[test]
fn output_is_deterministic_and_loads_cleanly() {
    let first = migrate_as("se", &full(""));
    let second = migrate_as("se", &full(""));
    assert_eq!(yaml(&first), yaml(&second));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("se.yaml");
    std::fs::write(&path, yaml(&first)).unwrap();
    let loaded = load(&path, "se").unwrap();
    assert!(loaded.warnings.is_empty(), "{:?}", loaded.warnings);
    let top: Vec<&str> = yaml(&first)
        .lines()
        .filter(|l| !l.starts_with(' '))
        .map(|l| l.split(':').next().unwrap())
        .collect();
    assert_eq!(
        top,
        [
            "format",
            "displayNames",
            "decimal",
            "keyNames",
            "deadKeys",
            "longPress",
            "hardware",
            "targets"
        ]
    );
}

// [spec:kbdgen:req:ldml.migrate.output/test]
#[test]
fn rows_are_literal_blocks_single_spaced() {
    let m = migrate_as("se", &full(""));
    assert!(
        yaml(&m).contains("      none: |\n        § 1 2 3 4 5 6 7 8 9 0 + \\d{´}\n        q w e"),
        "{}",
        yaml(&m)
    );
}

// [spec:kbdgen:req:ldml.migrate.output/test]
#[test]
fn scalars_are_quoted_only_where_needed() {
    for (s, written) in [
        ("á", "á"),
        ("\\d{´}", "\\d{´}"),
        ("a b", "a b"),
        ("n", "'n'"),
        ("no", "'no'"),
        ("1", "'1'"),
        ("1.5", "'1.5'"),
        ("-", "'-'"),
        ("~", "'~'"),
        ("", "''"),
        ("#", "'#'"),
        ("a: b", "'a: b'"),
        ("a #b", "'a #b'"),
        ("'", "''''"),
        ("`", "'`'"),
        ("\u{1}", "\"\\u0001\""),
    ] {
        assert_eq!(scalar(s), written, "{s:?}");
        let parsed: serde_yaml::Value = serde_yaml::from_str(&format!("k: {written}")).unwrap();
        assert_eq!(parsed["k"].as_str(), Some(s), "{written}");
    }
}

/// A bundle with a v3 layout that migrates, one that blocks, and a v4
/// layout.
fn bundle(root: &Path) -> std::path::PathBuf {
    let bundle = root.join("test.kbdgen");
    let layouts = bundle.join("layouts");
    std::fs::create_dir_all(&layouts).unwrap();
    std::fs::write(layouts.join("se.yaml"), full("")).unwrap();
    std::fs::write(
        layouts.join("fi.yaml"),
        "displayNames:\n  fi: suomi\nmodes: {}\n",
    )
    .unwrap();
    std::fs::write(
        layouts.join("sma.yaml"),
        "format: 4\ndisplayNames:\n  sma: åarjel\n",
    )
    .unwrap();
    bundle
}

fn contents(bundle: &Path) -> Vec<(String, String)> {
    let mut files: Vec<(String, String)> = std::fs::read_dir(bundle.join("layouts"))
        .unwrap()
        .map(|e| {
            let e = e.unwrap();
            (
                e.file_name().to_string_lossy().into_owned(),
                std::fs::read_to_string(e.path()).unwrap(),
            )
        })
        .collect();
    files.sort();
    files
}

// [spec:kbdgen:req:ldml.migrate.output/test]
// [spec:kbdgen:def:ldml.migrate.report/test]
#[test]
fn bundle_writes_unblocked_layouts_in_place() {
    let dir = tempfile::tempdir().unwrap();
    let bundle = bundle(dir.path());
    let before = contents(&bundle);
    let report = migrate_bundle(&bundle, true).unwrap();
    assert_eq!(contents(&bundle), before, "--dry-run writes nothing");
    let outcomes: Vec<(&str, report::Outcome)> = report
        .layouts
        .iter()
        .map(|l| (l.tag.as_str(), l.outcome))
        .collect();
    assert_eq!(
        outcomes,
        [
            ("fi", report::Outcome::Blocked),
            ("se", report::Outcome::Checked)
        ]
    );

    let report = migrate_bundle(&bundle, false).unwrap();
    let after = contents(&bundle);
    assert_eq!(after[0], before[0], "the blocked layout is not rewritten");
    assert!(after[1].1.starts_with("format: 4\n"));
    assert_eq!(after[2], before[2], "v4 layouts are left alone");
    assert_eq!(report.layouts[1].outcome, report::Outcome::Written);
    assert_eq!(report.blocked().len(), 1);
}

// [spec:kbdgen:def:ldml.migrate.report/test]
#[test]
fn report_lists_defects_then_summary() {
    let dir = tempfile::tempdir().unwrap();
    let report = migrate_bundle(&bundle(dir.path()), true).unwrap();
    let text = report.text();
    let lines: Vec<&str> = text.lines().collect();
    assert!(
        lines[0].starts_with("M07 ") && lines[0].ends_with("(blocked)"),
        "{text}"
    );
    assert!(text.contains("summary: 2 layout(s), "), "{text}");
    assert!(text.contains("M07 1"), "{text}");
    assert!(
        text.lines().last().unwrap().starts_with("blocked: "),
        "{text}"
    );

    let yaml: serde_yaml::Value = serde_yaml::from_str(&report.yaml()).unwrap();
    let first = &yaml["defects"][0];
    for field in ["code", "file", "path", "message", "action"] {
        assert!(first.get(field).is_some(), "{field}");
    }
    assert_eq!(first["action"].as_str(), Some("blocked"));
    assert_eq!(yaml["summary"]["counts"]["M07"].as_u64(), Some(1));
    assert_eq!(yaml["summary"]["blocked"].as_sequence().unwrap().len(), 1);
}

fn trace(platform: Desktop, layers: &[(&str, Vec<Out>)]) -> DesktopTrace {
    DesktopTrace {
        platform,
        layers: layers
            .iter()
            .map(|(name, cells)| LayerTrace {
                name: name.to_string(),
                key: String::new(),
                native: false,
                has_dead_list: false,
                path: String::new(),
                cells: cells
                    .iter()
                    .map(|out| Cell {
                        out: out.clone(),
                        explained: Vec::new(),
                    })
                    .collect(),
            })
            .collect(),
    }
}

fn text(s: &str) -> Out {
    Out::Text(s.to_string())
}

// [spec:kbdgen:req:ldml.migrate.caps-diff/test]
#[test]
fn windows_caps_follow_key_attributes() {
    let t = trace(
        Desktop::Windows,
        &[
            ("default", vec![text("a"), text("1"), text("x"), text("é")]),
            ("shift", vec![text("A"), text("!"), text("x"), text("É")]),
            ("caps", vec![Out::NoKey, Out::NoKey, Out::NoKey, text("Ê")]),
            (
                "caps+shift",
                vec![Out::NoKey, Out::NoKey, Out::NoKey, text("ê")],
            ),
            ("alt", vec![text("ä"), Out::NoKey, Out::NoKey, Out::NoKey]),
            (
                "alt+shift",
                vec![text("Ä"), Out::NoKey, Out::NoKey, Out::NoKey],
            ),
        ],
    );
    let at = |state, i| caps_output(&t, state, i);
    assert_eq!(at(CapsState::Caps, 0), text("A"), "CAPLOK");
    assert_eq!(at(CapsState::CapsShift, 0), text("a"), "CAPLOK inverts");
    assert_eq!(at(CapsState::Caps, 1), text("!"), "CAPLOK on digits");
    assert_eq!(at(CapsState::Caps, 2), text("x"));
    assert_eq!(at(CapsState::Caps, 3), text("Ê"), "SGCAPS");
    assert_eq!(at(CapsState::CapsShift, 3), text("ê"), "SGCAPS");
    assert_eq!(at(CapsState::AltCaps, 0), text("Ä"), "CAPLOKALTGR");
}

// [spec:kbdgen:req:ldml.migrate.caps-diff/test]
#[test]
fn macos_and_chromeos_caps_select_layers() {
    let mac = trace(
        Desktop::MacOs,
        &[
            ("shift", vec![text("A")]),
            ("default", vec![text("a")]),
            ("caps+shift", vec![text("ä")]),
        ],
    );
    assert_eq!(
        caps_output(&mac, CapsState::Caps, 0),
        text("A"),
        "first layer"
    );
    assert_eq!(caps_output(&mac, CapsState::CapsShift, 0), text("A"));
    assert_eq!(caps_output(&mac, CapsState::AltCaps, 0), text("A"));
    let chrome = trace(
        Desktop::ChromeOs,
        &[
            ("default", vec![text("a")]),
            ("shift", vec![text("A")]),
            ("caps", vec![text("Â")]),
        ],
    );
    assert_eq!(caps_output(&chrome, CapsState::CapsShift, 0), text("Â"));
    assert_eq!(caps_output(&chrome, CapsState::AltCaps, 0), text("Â"));
}

/// Converts and loads `yaml`, then runs the equivalence check after
/// `tamper` changes the trace of what v3 typed.
fn check_tampered(yaml: &str, tamper: impl Fn(&mut Trace)) -> Vec<Defect> {
    let value: serde_yaml::Value = serde_yaml::from_str(yaml).unwrap();
    let mut defects = defect::Defects::new("se.yaml");
    let src = source::read(&value, &mut defects).unwrap().unwrap();
    let mut converted = convert(&src, &mut defects);
    let text = write(&converted.out);
    let (loaded, _) = check::load(&text, Path::new("se.yaml"), "se").unwrap();
    tamper(&mut converted.trace);
    check::run(&loaded, &converted.trace, true, &mut defects);
    defects.into_list()
}

// [spec:kbdgen:req:ldml.migrate.equivalence/test]
#[test]
fn equivalence_flags_unexplained_key_differences() {
    let clean = check_tampered(&full(""), |_| {});
    assert!(!clean.iter().any(|d| d.code == Code::M99), "{clean:?}");
    let defects = check_tampered(&full(""), |t| {
        t.desktop[0].layers[0].cells[1].out = text("¹");
    });
    let m99: Vec<&Defect> = defects.iter().filter(|d| d.code == Code::M99).collect();
    assert_eq!(m99.len(), 1, "{defects:?}");
    assert!(
        m99[0].message.contains("default E01: v3 typed \"¹\""),
        "{}",
        m99[0]
    );
    let explained = check_tampered(&full(""), |t| {
        let cell = &mut t.desktop[0].layers[0].cells[1];
        cell.out = text("¹");
        cell.explained.push(Code::M13);
    });
    assert!(!explained.iter().any(|d| d.code == Code::M99));
}

// [spec:kbdgen:req:ldml.migrate.equivalence/test]
#[test]
fn equivalence_presses_dead_keys_then_inputs() {
    let defects = check_tampered(&full(""), |t| {
        let leaf = t.dead[0].compose[0].1.as_mut().unwrap();
        leaf.raw = "à".to_string();
        leaf.text = "à".to_string();
    });
    let m99: Vec<&Defect> = defects.iter().filter(|d| d.code == Code::M99).collect();
    assert_eq!(m99.len(), 1, "{defects:?}");
    assert!(
        m99[0].message.contains("´ then a: v3 typed \"à\""),
        "{}",
        m99[0]
    );
    let defects = check_tampered(&full(""), |t| t.dead[0].standalone.raw = "x".to_string());
    assert!(defects.iter().any(|d| d.message.contains("´ then space")));
}

// [spec:kbdgen:req:ldml.migrate.equivalence/test]
#[test]
fn equivalence_taps_touch_keys_and_flicks() {
    let ios = "iOS:\n  iPad-9in:\n    layers:\n      default: |\n        q w\n      alt: |\n        1 2\n";
    let clean = check_tampered(&full(ios), |_| {});
    assert!(!clean.iter().any(|d| d.code == Code::M99), "{clean:?}");
    let defects = check_tampered(&full(ios), |t| {
        let layer = &mut t.touch[0].sizes[0].layers[0];
        layer.rows[0][0].as_mut().unwrap().out = text("Q");
        layer.flicks[0][1].as_mut().unwrap().out = text("3");
    });
    let messages: Vec<&str> = defects
        .iter()
        .filter(|d| d.code == Code::M99)
        .map(|d| d.message.as_str())
        .collect();
    assert_eq!(messages.len(), 2, "{defects:?}");
    assert!(messages[0].contains("tablet base row 1 key 1"));
    assert!(messages[1].contains("row 1 key 2 flick s"));
}

// [spec:kbdgen:req:ldml.migrate.equivalence/test]
#[test]
fn unloadable_or_unexplained_migration_blocks() {
    let m = migrate("iOS:\n  primary:\n    layers:\n      default: a \\s{symbols}\n");
    let m99 = with_code(&m, Code::M99);
    assert_eq!(m99.len(), 1, "{:?}", m.defects);
    assert!(m99[0].message.contains("does not load"), "{}", m99[0]);
    assert!(m.blocked());

    let keys = KEYS.replace("+ ´", "+ \\u{B4}");
    let m = migrate(&format!(
        "{}{ACUTE}",
        desktop(
            "windows",
            &[("default", &keys), ("shift", SHIFTED)],
            "  deadKeys:\n    default: ['\\u{B4}']\n"
        )
    ));
    assert!(
        with_code(&m, Code::M99).iter().any(|d| d
            .message
            .contains("v3 typed \"´\", the migrated layout types dead")),
        "an escaped Windows entry was never dead in v3: {:?}",
        m.defects
    );
}

// [spec:kbdgen:req:ldml.migrate.equivalence/test]
#[test]
fn in_memory_migration_writes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("se.yaml");
    std::fs::write(&path, full("")).unwrap();
    let before = std::fs::read(&path).unwrap();
    let m = migrate_file(&path, "se").unwrap();
    assert!(!m.blocked(), "{:?}", m.defects);
    assert!(windows_model(&m).unwrap().is_some());
    assert_eq!(std::fs::read(&path).unwrap(), before);
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
}
