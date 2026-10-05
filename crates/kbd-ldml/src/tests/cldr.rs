//! The vendored CLDR 48 keyboards and keyboardTest3 files
//! (`testdata/cldr/48`). kbdgen runs the vectors through the engine
//! (`kbdgen::ldml::conformance`); here they are read and their keyboards
//! resolved.

use super::*;
use crate::keyboard_test::{Repertoire, TestGesture, TestStep};
use crate::{read_keyboard_file, read_keyboard_test, read_keyboard_test_file};
use kbd_model::Direction;

fn cldr_test_files() -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = std::fs::read_dir(testdata("cldr/48/keyboards/test"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    paths.sort();
    paths
}

#[test]
fn cldr_keyboards_resolve_and_load() {
    for path in cldr_keyboards() {
        let source = read_keyboard_file(&path).unwrap();
        let resolved = resolve(&source).unwrap_or_else(|e| panic!("{e}"));
        engine(resolved.keyboard);
    }
}

// [spec:kbdgen:req:ldml.test.cldr/test]
// [spec:kbdgen:req:ldml.test.repertoire/test]
#[test]
fn cldr_test_files_read_and_name_keyboards() {
    let mut summary = Vec::new();
    for path in cldr_test_files() {
        let test = read_keyboard_test_file(&path).unwrap_or_else(|e| panic!("{e}"));
        let keyboard = testdata(&format!("cldr/48/keyboards/3.0/{}", test.keyboard));
        resolve(&read_keyboard_file(&keyboard).unwrap()).unwrap_or_else(|e| panic!("{e}"));
        let tests: usize = test.groups.iter().map(|g| g.tests.len()).sum();
        summary.push((test.keyboard.clone(), tests, test.repertoires.len()));
    }
    assert_eq!(
        summary,
        [
            ("bn.xml".to_string(), 2, 0),
            ("fr-t-k0-test.xml".to_string(), 1, 2),
            ("ja-Latn.xml".to_string(), 2, 1),
            ("pcm.xml".to_string(), 2, 1),
            ("pt-t-k0-abnt2.xml".to_string(), 3, 2),
        ]
    );
}

const STEPS: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<keyboardTest3 conformsTo="techpreview">
  <info keyboard="x.xml" author="A" name="x-test"/>
  <repertoire name="r" chars="[a b \u{22}]" type="gesture"/>
  <special/>
  <tests name="g">
    <test name="t">
      <startContext to="a\u{0300}"/>
      <keystroke key="e"/>
      <keystroke key="f" flick="ne s"/>
      <keystroke key="g" longPress="2"/>
      <keystroke key="h" tapCount="3"/>
      <special><other/></special>
      <emit to="\u{0301}"/>
      <backspace/>
      <check result="x\u{1F600}"/>
    </test>
  </tests>
</keyboardTest3>
"#;

// [spec:kbdgen:req:ldml.test.cldr/test]
// [spec:kbdgen:req:ldml.test.repertoire/test]
#[test]
fn steps_gestures_and_repertoires_are_read() {
    let test = read_keyboard_test("x-test.xml", None, STEPS.as_bytes()).unwrap();
    assert_eq!(
        (
            test.name.as_str(),
            test.author.as_deref(),
            test.keyboard.as_str()
        ),
        ("x-test", Some("A"), "x.xml")
    );
    assert_eq!(
        test.repertoires,
        [Repertoire {
            name: "r".to_string(),
            chars: r"[a b \u{22}]".to_string(),
            kind: Some("gesture".to_string()),
        }],
        "repertoire chars stay as written"
    );
    let key = |key: &str, gesture: TestGesture| TestStep::Keystroke {
        key: key.to_string(),
        gesture,
    };
    assert_eq!(
        test.groups[0].tests[0].steps,
        [
            TestStep::StartContext("a\u{300}".to_string()),
            key("e", TestGesture::Tap),
            key("f", TestGesture::Flick(vec![Direction::Ne, Direction::S])),
            key("g", TestGesture::LongPress(2)),
            key("h", TestGesture::TapCount(3)),
            TestStep::Emit("\u{301}".to_string()),
            TestStep::Backspace,
            TestStep::Check("x\u{1F600}".to_string()),
        ]
    );
}

// [spec:kbdgen:req:ldml.test.cldr/test]
#[test]
fn malformed_test_documents_are_refused() {
    let cases = [
        (
            r#"<keyboard3 "#,
            r#"<keyboardTest3 "#,
            "root of a keyboard test",
        ),
        (r#" name="x-test"/>"#, r#"/>"#, "name is required"),
        (r#"flick="ne s""#, r#"flick="up""#, "not a direction"),
        (r#"longPress="2""#, r#"longPress="0""#, "from 1 to 999"),
        (r#"tapCount="3""#, r#"tapCount="1""#, "from 2 to 999"),
        (r#"key="h""#, r#"key="h" longPress="1""#, "one gesture"),
        (
            r#"type="gesture""#,
            r#"type="swipe""#,
            "not a repertoire type",
        ),
        (r#"<backspace/>"#, r#"<startContext to=""/>"#, "first step"),
        (r#"<backspace/>"#, r#"<press/>"#, "press is not a test step"),
        (
            r#"<emit to="\u{0301}"/>"#,
            r#"<emit to="\u{D800}"/>"#,
            "emit",
        ),
        (
            r#"<check result="x\u{1F600}"/>"#,
            r#"<check/>"#,
            "result is required",
        ),
    ];
    for (from, to, needle) in cases {
        let xml = if from.starts_with("<keyboard3") {
            STEPS
                .replace("<keyboardTest3 ", from)
                .replace("</keyboardTest3>", "</keyboard3>")
        } else {
            assert!(STEPS.contains(from), "{from}");
            STEPS.replacen(from, to, 1)
        };
        let err = read_keyboard_test("x-test.xml", None, xml.as_bytes())
            .unwrap_err()
            .to_string();
        assert!(err.contains(needle), "{to}: {err}");
    }
}

#[test]
fn cldr_keyboards_survive_export() {
    for path in cldr_keyboards() {
        let resolved = resolve(&read_keyboard_file(&path).unwrap()).unwrap();
        let xml = crate::write(&crate::export(&resolved.keyboard, &resolved.extensions));
        let again = resolve(&read_keyboard("exported.xml", None, xml.as_bytes()).unwrap())
            .unwrap_or_else(|e| panic!("{}: {e}\n{xml}", path.display()));
        assert_eq!(
            again.keyboard,
            resolved.keyboard,
            "{}\n{xml}",
            path.display()
        );
        assert_eq!(again.extensions, resolved.extensions);
    }
}
