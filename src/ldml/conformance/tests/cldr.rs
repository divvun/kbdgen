//! CLDR's keyboardTest3 vectors through the harness (`ldml.test.cldr`).

use kbd_ldml::keyboard_test::{KeyboardTest, Test, TestGroup, TestStep};

use super::super::cldr::{CLDR_OPTIONS, run_cldr, run_keyboard_test};
use super::super::cldr_data::{KEYBOARDS, TESTS};
use super::*;

fn vendored(dir: &str) -> Vec<(String, Vec<u8>)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("crates/kbd-ldml/testdata/cldr/48/keyboards")
        .join(dir);
    let mut files: Vec<(String, Vec<u8>)> = std::fs::read_dir(root)
        .unwrap()
        .map(|e| {
            let path = e.unwrap().path();
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            (name, std::fs::read(&path).unwrap())
        })
        .collect();
    files.sort();
    files
}

// [spec:kbdgen:req:ldml.test.cldr/test]
// [spec:kbdgen:def:ldml.test.harness/test]
#[test]
fn cldr_vectors_pass_on_the_engine() {
    let report = run_cldr().unwrap_or_else(|e| panic!("{e}"));
    let failures: Vec<String> = report.failures.iter().map(|f| f.to_string()).collect();
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    assert_eq!(
        (report.files, report.tests, report.checks),
        (5, 10, 14),
        "every vendored test file, test and check runs"
    );
}

// [spec:kbdgen:req:ldml.test.repertoire/test]
#[test]
fn repertoires_are_noted_and_skipped() {
    let report = run_cldr().unwrap();
    assert_eq!(report.notes.len(), 6, "{:?}", report.notes);
    assert_eq!(
        report.notes[0],
        "cldr/48/keyboards/test/fr-t-k0-test-test.xml: repertoire simple-repertoire (simple) \
         is parsed and not run (ldml.scope.deferred)"
    );
}

// [spec:kbdgen:req:ldml.test.cldr/test]
#[test]
fn embedded_files_are_the_vendored_ones() {
    let embedded = |files: &[(&str, &[u8])]| -> Vec<(String, Vec<u8>)> {
        files
            .iter()
            .map(|(n, b)| (n.to_string(), b.to_vec()))
            .collect()
    };
    assert_eq!(embedded(&TESTS), vendored("test"));
    assert_eq!(embedded(&KEYBOARDS), vendored("3.0"));
}

fn pcm_model() -> Model {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("crates/kbd-ldml/testdata/cldr/48/keyboards/3.0/pcm.xml");
    let keyboard = kbd_ldml::resolve(&kbd_ldml::read_keyboard_file(&path).unwrap())
        .unwrap()
        .keyboard;
    Model::from_keyboard(keyboard, CLDR_OPTIONS).unwrap()
}

fn one_test(steps: Vec<TestStep>) -> KeyboardTest {
    KeyboardTest {
        name: "t".into(),
        author: None,
        keyboard: "pcm.xml".into(),
        repertoires: vec![],
        groups: vec![TestGroup {
            name: "g".into(),
            tests: vec![Test {
                name: "t".into(),
                steps,
            }],
        }],
    }
}

// [spec:kbdgen:req:ldml.test.cldr/test]
#[test]
fn checks_compare_nfd_forms() {
    let model = pcm_model();
    let doc = one_test(vec![
        TestStep::StartContext("\u{E7}".into()),
        TestStep::Emit("e\u{323}".into()),
        TestStep::Check("c\u{327}\u{1EB9}".into()),
        TestStep::Backspace,
        TestStep::Check("\u{E7}e".into()),
    ]);
    let report = run_keyboard_test(&model, &doc, "t.xml");
    assert!(report.failures.is_empty(), "{:?}", report.failures);
    assert_eq!(report.checks, 2, "codePoint backspace deleted one scalar");
}

// [spec:kbdgen:req:ldml.test.cldr/test]
// [spec:kbdgen:req:ldml.cli.test/test]
#[test]
fn failing_checks_report_the_document() {
    let model = pcm_model();
    let doc = one_test(vec![
        TestStep::Keystroke {
            key: "e".into(),
            gesture: kbd_ldml::keyboard_test::TestGesture::Tap,
        },
        TestStep::Check("f".into()),
    ]);
    let report = run_keyboard_test(&model, &doc, "t.xml");
    let shown: Vec<String> = report.failures.iter().map(|f| f.to_string()).collect();
    assert_eq!(
        shown,
        ["FAIL t.xml: g/t: step 2: check \"f\"\n  \
          expected text     \"f\" (NFD)\n  \
          actual   text     \"e\"\n  \
          actual   preedit  \"\"\n  \
          actual   layer    none\n  \
          actual   action   edit: delete 0, insert \"e\", preedit \"\", layer none"]
    );
}
