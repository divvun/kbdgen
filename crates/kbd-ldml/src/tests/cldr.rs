//! The vendored CLDR 48 keyboards (`testdata/cldr/48`).

use super::*;
use crate::read_keyboard_file;

#[test]
fn cldr_keyboards_resolve_and_load() {
    for path in cldr_keyboards() {
        let source = read_keyboard_file(&path).unwrap();
        let resolved = resolve(&source).unwrap_or_else(|e| panic!("{e}"));
        kbd_engine::Model::from_keyboard(resolved.keyboard, kbd_engine::Options::default())
            .unwrap_or_else(|e| panic!("{}: {e:?}", path.display()));
    }
}

fn run_vectors(test_file: &str) -> usize {
    let path = testdata(&format!("cldr/48/keyboards/test/{test_file}"));
    let bytes = std::fs::read(&path).unwrap();
    let doc = crate::read_document(test_file, Some(&path), &bytes).unwrap();
    let tree = crate::tree::El::from_document(test_file, &doc.document).unwrap();
    let keyboard_file = tree.child("info").unwrap().attr("keyboard").unwrap();
    let keyboard_path = testdata(&format!("cldr/48/keyboards/3.0/{keyboard_file}"));
    let resolved = resolve(&read_keyboard_file(&keyboard_path).unwrap()).unwrap();
    let mut checks = 0;
    for tests in tree.children_named("tests") {
        for test in tests.children_named("test") {
            let mut session = Session::new(resolved.keyboard.clone());
            for step in &test.children {
                let decode =
                    |a: &str| crate::escape::decode_plain(step.attr(a).unwrap_or("")).unwrap();
                match step.name.as_str() {
                    "startContext" => session.text = decode("to"),
                    "keystroke" => {
                        let key = step.attr("key").unwrap();
                        let gesture = if let Some(f) = step.attr("flick") {
                            kbd_engine::Gesture::Flick(
                                f.split_whitespace()
                                    .map(|d| kbd_model::Direction::from_name(d).unwrap())
                                    .collect(),
                            )
                        } else if let Some(n) = step.attr("longPress") {
                            kbd_engine::Gesture::LongPress(n.parse().unwrap())
                        } else if let Some(n) = step.attr("tapCount") {
                            kbd_engine::Gesture::MultiTap(n.parse().unwrap())
                        } else {
                            kbd_engine::Gesture::Tap
                        };
                        session.id(key, gesture);
                    }
                    "emit" => {
                        session.send(kbd_engine::KeyEvent::new(kbd_engine::Key::Emit(decode(
                            "to",
                        ))));
                    }
                    "backspace" => {
                        session.send(kbd_engine::KeyEvent::new(kbd_engine::Key::Backspace));
                    }
                    "check" => {
                        assert_eq!(
                            crate::nfd::nfd_str(&session.text),
                            crate::nfd::nfd_str(&decode("result")),
                            "{test_file} {}",
                            test.path
                        );
                        checks += 1;
                    }
                    other => panic!("unknown step {other}"),
                }
            }
        }
    }
    checks
}

#[test]
fn cldr_test_vectors_pass() {
    let mut files: Vec<String> = std::fs::read_dir(testdata("cldr/48/keyboards/test"))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    files.sort();
    let checks: usize = files.iter().map(|f| run_vectors(f)).sum();
    assert!(checks >= 10, "{checks} checks");
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
