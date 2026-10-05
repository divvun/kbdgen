//! CLDR's keyboardTest3 vectors through the harness (`ldml.test.cldr`).
//!
//! The vectors check kbdgen's engine alone. Differential runs against
//! Keyman Core, the other LDML runtime, are deferred
//! (`ldml.test.oracle`, `ldml.scope.deferred`): there is no second engine
//! here to compare with.

use kbd_engine::harness::Harness;
use kbd_engine::{BackspacePolicy, Gesture, Key, KeyEvent, Model, Options, OutputForm};
use kbd_ldml::keyboard_test::{KeyboardTest, TestGesture, TestStep};
use kbd_ldml::{nfd_str, read_keyboard, read_keyboard_test};
use kbd_model::Keyboard;

use super::cldr_data::{KEYBOARDS, RELEASE, TESTS};
use super::{Failure, Observed, Report};
use crate::ldml::LdmlError;

/// The options CLDR's vectors assume: LDML's default backspace and NFC
/// output.
pub const CLDR_OPTIONS: Options = Options {
    backspace: BackspacePolicy::CodePoint,
    output_form: OutputForm::Nfc,
    host: None,
};

/// The vendored keyboard `name` (`pcm.xml`), resolved.
fn cldr_keyboard(name: &str) -> Result<Keyboard, LdmlError> {
    let (_, bytes) =
        KEYBOARDS
            .iter()
            .find(|(n, _)| *n == name)
            .ok_or_else(|| LdmlError::Vectors {
                path: format!("cldr/{RELEASE}/keyboards/3.0/{name}").into(),
                message: "no vendored CLDR keyboard has this name".to_string(),
            })?;
    let label = format!("cldr/{RELEASE}/keyboards/3.0/{name}");
    Ok(kbd_ldml::resolve(&read_keyboard(&label, None, bytes)?)?.keyboard)
}

fn gesture(g: &TestGesture) -> Gesture {
    match g {
        TestGesture::Tap => Gesture::Tap,
        TestGesture::Flick(directions) => Gesture::Flick(directions.clone()),
        TestGesture::LongPress(n) => Gesture::LongPress(*n),
        TestGesture::TapCount(n) => Gesture::MultiTap(*n),
    }
}

/// Applies a step; for `check`, returns the expected result when NFD of
/// the document differs from NFD of it.
fn apply(h: &mut Harness, step: &TestStep) -> Option<String> {
    let event = match step {
        TestStep::StartContext(text) => {
            h.set_document(text);
            return None;
        }
        TestStep::Check(result) => {
            return (nfd_str(h.document()) != nfd_str(result)).then(|| result.clone());
        }
        TestStep::Keystroke { key, gesture: g } => KeyEvent::new(Key::Id {
            id: key.clone(),
            gesture: gesture(g),
        }),
        TestStep::Emit(text) => KeyEvent::new(Key::Emit(text.clone())),
        TestStep::Backspace => KeyEvent::new(Key::Backspace),
    };
    h.send(&event);
    None
}

fn written(step: &TestStep) -> String {
    match step {
        TestStep::StartContext(text) => format!("startContext {text:?}"),
        TestStep::Keystroke { key, gesture } => format!("keystroke {key} {gesture:?}"),
        TestStep::Emit(text) => format!("emit {text:?}"),
        TestStep::Backspace => "backspace".to_string(),
        TestStep::Check(result) => format!("check {result:?}"),
    }
}

// [spec:kbdgen:req:ldml.test.repertoire]
/// Runs every test of a keyboardTest3 document on `model`, each from an
/// empty document in the reset state, comparing NFD forms at each
/// `check`. Repertoires are noted and skipped (`ldml.scope.deferred`).
pub fn run_keyboard_test(model: &Model, doc: &KeyboardTest, label: &str) -> Report {
    let mut report = Report {
        files: 1,
        ..Report::default()
    };
    for r in &doc.repertoires {
        report.notes.push(format!(
            "{label}: repertoire {} ({}) is parsed and not run (ldml.scope.deferred)",
            r.name,
            r.kind.as_deref().unwrap_or("default")
        ));
    }
    for group in &doc.groups {
        for test in &group.tests {
            report.tests += 1;
            let mut h = Harness::new(model);
            for (i, step) in test.steps.iter().enumerate() {
                report.checks += usize::from(matches!(step, TestStep::Check(_)));
                if let Some(result) = apply(&mut h, step) {
                    report.failures.push(Failure {
                        file: label.to_string(),
                        test: format!("{}/{}", group.name, test.name),
                        step: i + 1,
                        written: written(step),
                        expected: vec![("text".to_string(), format!("{result:?} (NFD)"))],
                        observed: Observed::of(&h),
                    });
                }
            }
        }
    }
    report
}

fn engine(keyboard: Keyboard, label: &str) -> Result<Model, LdmlError> {
    Model::from_keyboard(keyboard, CLDR_OPTIONS).map_err(|e| LdmlError::Vectors {
        path: label.into(),
        message: format!("the keyboard does not load in the engine: {e}"),
    })
}

// [spec:kbdgen:req:ldml.test.cldr+1]
// [spec:kbdgen:req:ldml.test.oracle]
/// Resolves every vendored CLDR keyboard, then runs every vendored
/// keyboardTest3 file with [`CLDR_OPTIONS`] on kbdgen's engine.
pub fn run_cldr() -> Result<Report, LdmlError> {
    for (name, _) in KEYBOARDS {
        engine(cldr_keyboard(name)?, name)?;
    }
    let mut report = Report::default();
    for (name, bytes) in TESTS {
        let label = format!("cldr/{RELEASE}/keyboards/test/{name}");
        let doc = read_keyboard_test(&label, None, bytes)?;
        let model = engine(cldr_keyboard(&doc.keyboard)?, &label)?;
        report.absorb(run_keyboard_test(&model, &doc, &label));
    }
    Ok(report)
}
