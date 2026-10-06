//! `tsf.test.differential`, `tsf.test.engine` and `tsf.data.version` on
//! every fixture layout.

use std::collections::BTreeSet;

use kbd_model::{Host, Keyboard, MAGIC, MAJOR_VERSION, MINOR_VERSION};

use super::compare::compare;
use super::exceptions::{Exception, LISTED};
use super::simulate::{Dll, Typed, WinKeys};
use super::*;
use crate::build::windows::kbdl::adapter;
use crate::build::windows::kbdl::input::{KeyValue, Layer};
use crate::build::windows::kbdl::tables::tests::{base, branch, leaf, set};

const FIXTURE_EXCEPTIONS: [Exception; 8] = [
    Exception::CapsShiftCaplok,
    Exception::CtrlLayer,
    Exception::LongDeadKeyLeaf,
    Exception::DroppedPosition,
    Exception::SgcapsDeadKey,
    Exception::MigratedCaps,
    Exception::UnmatchedDeadKeys,
    Exception::LigatureAfterDeadKey,
];

/// Small layouts for the exceptions no fixture exercises.
const SYNTHETIC: [(Exception, &str); 4] = [
    (
        Exception::TextTransform,
        "transforms: [[{from: q, to: Q}]]\n",
    ),
    (
        Exception::Reorder,
        "transforms:\n  - reorder: [{from: '\\u{301}', order: 20}, {from: '\\u{328}', order: 10}]\n",
    ),
    (Exception::MixedOutput, "keys:\n  mx: {output: 'a\\m{x}'}\n"),
    (Exception::Normalization, "normalization: enabled\n"),
];

/// A one-layer `sme` layout with `body` before its hardware, with `\k{mx}`
/// at E00 and `a\u{301}\u{328}` and `é` at E01 and E02.
fn synthetic(body: &str) -> String {
    format!(
        "format: 4\ndisplayNames: {{sme: Davvisámegiella}}\n{body}hardware:\n  windows:\n    layers:\n      none: |\n        {}\n        q w e r t y u i o p å ¨\n        a s d f g h j k l ö æ '\n        < z x c v b n m , . /\n",
        if body.contains("mx:") {
            "\\k{mx} 1 2 3 4 5 6 7 8 9 0 - ="
        } else {
            "§ a\\u{301}\\u{328} é 3 4 5 6 7 8 9 0 - ="
        }
    )
}

// [spec:kbdgen:req:tsf.test.differential/test]
#[test]
fn fixture_layouts_type_what_the_engine_types() {
    let mut unexplained = Vec::new();
    let mut exercised = BTreeSet::new();
    for fixture in fixture_bundles() {
        for subject in subjects(&fixture) {
            let report = compare(&subject);
            eprintln!("{} ({}): {}", fixture.label, subject.name, report.summary());
            assert!(report.matched * 2 > report.cases, "{}", fixture.label);
            exercised.extend(report.excepted.keys().copied());
            unexplained.extend(report.unexplained);
        }
    }
    assert!(unexplained.is_empty(), "{}", unexplained.join("\n"));
    assert_eq!(exercised, BTreeSet::from(FIXTURE_EXCEPTIONS));
}

// [spec:kbdgen:req:tsf.test.differential/test]
#[test]
fn each_listed_exception_explains_a_difference() {
    for (exception, body) in SYNTHETIC {
        let fixture = fixture_bundle(&format!("{exception:?}"), "sme", &synthetic(body));
        let subject = subjects(&fixture).remove(0);
        let report = compare(&subject);
        assert!(report.unexplained.is_empty(), "{:#?}", report.unexplained);
        assert!(
            report.excepted.contains_key(&exception),
            "{exception:?}: {}",
            report.summary()
        );
    }
    let listed: BTreeSet<Exception> = LISTED.iter().map(|(e, _)| *e).collect();
    let exercised: BTreeSet<Exception> = FIXTURE_EXCEPTIONS
        .into_iter()
        .chain(SYNTHETIC.map(|(e, _)| e))
        .chain([Exception::BackspaceRule])
        .collect();
    assert_eq!(
        listed, exercised,
        "every listed exception is exercised; no case presses Backspace"
    );
}

// [spec:kbdgen:req:tsf.test.differential/test]
#[test]
fn corrupted_tables_are_not_explained() {
    let fixture = fixture_bundles()
        .into_iter()
        .find(|f| f.label.ends_with("layouts/vro.yaml"))
        .unwrap();
    let mut subject = subjects(&fixture).remove(0);
    let row = subject
        .tables
        .rows
        .iter_mut()
        .find(|r| r.vk == b'Q')
        .unwrap();
    row.wch[0] = u16::from(b'x');
    let acute = subject
        .tables
        .dead_keys
        .iter_mut()
        .find(|e| e.base == u16::from(b'a') && e.id == 0xb4)
        .unwrap();
    acute.composed = u16::from(b'y');
    let report = compare(&subject);
    let lines = report.unexplained.join("\n");
    assert!(
        lines.contains("kbdvro: D01 in default: engine \"q\", DLL \"x\""),
        "{lines}"
    );
    assert!(
        lines.contains("E12 in default, then C01 in default: engine \"á\", DLL \"y\""),
        "{lines}"
    );
}

// [spec:kbdgen:req:tsf.test.differential/test]
#[test]
fn simulated_dead_keys_compose_chain_and_fall_back() {
    let mut input = base();
    set(&mut input, Layer::Default, "E00", KeyValue::dead("^"));
    set(&mut input, Layer::Default, "E12", KeyValue::dead("´"));
    set(&mut input, Layer::Default, "C01", KeyValue::new("a"));
    set(&mut input, Layer::Default, "D01", KeyValue::new("q"));
    set(&mut input, Layer::Default, "D02", KeyValue::new("wz"));
    set(&mut input, Layer::Default, "D03", KeyValue::new("az"));
    input.dead_key_tree.insert(
        "^".into(),
        branch(&[
            ("a", leaf("â")),
            ("q", branch(&[("a", leaf("ǎ")), (" ", leaf("ˇ"))])),
            (" ", leaf("^")),
        ]),
    );
    input
        .dead_key_tree
        .insert("´".into(), branch(&[(" ", leaf("´"))]));
    let mut diag = Diagnostics::new("kbdtest");
    let tables = super::super::tables::build(&input, &mut diag).unwrap();
    let mut dll = Dll::new(&tables);
    let keys = WinKeys::default();
    let mut press = |vk: u8| dll.press(vk, keys);
    let text = |s: &str| Typed::Text(s.encode_utf16().collect());
    assert_eq!(press(0xc0), Typed::Dead(u16::from(b'^')));
    assert_eq!(press(b'A'), text("â"));
    assert_eq!(press(0xc0), Typed::Dead(u16::from(b'^')));
    assert_eq!(press(b'Q'), Typed::Dead(0x02c7));
    assert_eq!(press(b'A'), text("ǎ"));
    assert_eq!(press(0xc0), Typed::Dead(u16::from(b'^')));
    assert_eq!(press(b'W'), text("^wz"));
    assert_eq!(press(0xc0), Typed::Dead(u16::from(b'^')));
    assert_eq!(press(b'E'), text("âz"), "a ligature's first unit composes");
    assert_eq!(press(0xc0), Typed::Dead(u16::from(b'^')));
    assert_eq!(press(0xbb), text("^´"), "an unmatched dead key is typed");
    assert_eq!(press(b'A'), text("a"));
    assert_eq!(press(0x20), text(" "));
    let ctrl = WinKeys {
        ctrl: true,
        ..WinKeys::default()
    };
    assert_eq!(
        dll.press(b'Q', ctrl),
        Typed::Text(vec![0x11]),
        "Ctrl+Q is DC1"
    );
}

/// The `windows` keyboard of the layout tagged `tag` in `fixture`: the
/// compiled v4 layout's, or that of a v3 layout's in-memory migration.
fn windows_keyboard(fixture: &FixtureBundle, tag: &str) -> Keyboard {
    let bundle = &fixture.bundle;
    let layout = match bundle.v4_layouts.iter().find(|(t, _)| t.as_str() == tag) {
        Some((tag, path)) => adapter::load(tag, path).unwrap().unwrap().layout,
        None => migrate_bundle_layout(&bundle.path, tag)
            .unwrap()
            .unwrap()
            .compiled()
            .unwrap()
            .unwrap(),
    };
    layout.keyboard_for(Host::Windows).unwrap().clone()
}

// [spec:kbdgen:req:tsf.test.engine/test]
// [spec:kbdgen:req:tsf.data.resource/test]
// [spec:kbdgen:def:tsf.engine.model/test]
// [spec:kbdgen:req:tsf.data.version/test]
#[tokio::test]
async fn built_res_files_decode_to_the_windows_model() {
    for fixture in fixture_bundles() {
        let out = tempfile::tempdir().unwrap();
        write_crates(&fixture, out.path()).await;
        let layouts = source_layouts(&fixture.bundle).unwrap();
        assert!(!layouts.is_empty(), "{}", fixture.label);
        for layout in layouts {
            let name = &layout.input.metadata.name;
            let res_path = out
                .path()
                .join("build")
                .join(name)
                .join(format!("{name}.res"));
            let res = std::fs::read(&res_path).unwrap();
            let bytes = embedded_model(&res).unwrap_or_else(|| panic!("{}", fixture.label));
            assert_eq!(bytes[..4], MAGIC);
            assert_eq!(bytes[4..8], [1, 0, 0, 0], "DVKB 1.0");
            assert_eq!(
                (MAJOR_VERSION, MINOR_VERSION),
                (1, 0),
                "the reader expects what kbdgen writes"
            );
            let model = Model::from_bytes(&bytes).unwrap();
            let expected = windows_keyboard(&fixture, layout.tag.as_str());
            assert_eq!(model.keyboard(), &expected, "{}", fixture.label);
        }
    }
}

// [spec:kbdgen:req:tsf.data.version/test]
#[test]
fn embedded_minor_versions_load_and_majors_do_not() {
    let fixture = fixture_bundles().remove(0);
    let layout = source_layouts(&fixture.bundle).unwrap().remove(0);
    let mut bytes = layout.model.unwrap();
    bytes.extend_from_slice(b"appended by a later minor version");
    let mut minor = bytes.clone();
    minor[6..8].copy_from_slice(&7u16.to_le_bytes());
    assert!(Model::from_bytes(&minor).is_ok());
    let mut major = bytes;
    major[4..6].copy_from_slice(&(MAJOR_VERSION + 1).to_le_bytes());
    let error = Model::from_bytes(&major).unwrap_err().to_string();
    assert!(error.contains("version 2.0 is not supported"), "{error}");
}
