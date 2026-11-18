//! The planner against a fake client (`linux.test.planner`): the typing
//! cases of `ldml.chromeos.test.typing` for every fixture, sent as IBus key
//! codes and state bits, with GNOME Shell's virtual keys, its on-screen
//! Backspace past the engine, Left Alt and a password field, in a field
//! that reports its surrounding text on time, one key late or not at all,
//! with or without a preedit; and for the model that rewrites committed
//! text, every pair of positions. What the field shows and which keys the
//! engine handles agree with the engine run natively.

// Clippy counts only `#[test]` functions of a test binary as tests, not
// the helpers beside them.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use common::cases::{BACKSPACE, Case, Step, Stroke, Typed, cases, pairs};
use common::field::{Client, Report};
use common::fixture;
use common::native::Native;
use kbd_engine::{Key, KeyEvent, Model};
use kbd_ibus::planner::PURPOSE_PASSWORD;

/// How the field is set up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Setup {
    report: Report,
    preedit: bool,
    password: bool,
}

const GTK: Setup = Setup {
    report: Report::OnTime,
    preedit: true,
    password: false,
};
const GNOME_SHELL: Setup = Setup {
    report: Report::Late,
    ..GTK
};
const TERMINAL: Setup = Setup {
    report: Report::Never,
    ..GTK
};
const BARE: Setup = Setup {
    report: Report::Never,
    preedit: false,
    password: false,
};
const PASSWORD: Setup = Setup {
    password: true,
    ..GTK
};

/// The engine's Backspace, as GNOME Shell presses it.
fn backspace() -> Stroke {
    Stroke {
        name: "Backspace".to_owned(),
        taps: Vec::new(),
        held: Vec::new(),
        keyval: BACKSPACE.0,
        code: BACKSPACE.1,
        state: 0,
        event: Some(KeyEvent::new(Key::Backspace)),
    }
}

/// Runs `case` in a field set up as `setup` and natively, and says where
/// they differ.
// [spec:kbdgen:req:linux.test.planner]
fn run(model: &Model, setup: Setup, case: &Case) -> Result<(), String> {
    let mut client = Client::new(model.clone(), setup.report, setup.preedit);
    if setup.password {
        client.session.set_content_type(PURPOSE_PASSWORD, 0);
    }
    let mut native = if setup.password {
        Native::blind(model)
    } else {
        Native::new(model)
    };
    for step in &case.steps {
        let (stroke, handled) = match step {
            Step::Key(stroke) => (stroke.clone(), client.stroke(stroke)),
            Step::OnScreenBackspace
                if client.field.preedit.is_empty() && setup.report != Report::Never =>
            {
                let mut before = client.field.before.clone();
                before.pop();
                client.replace_text(&before);
                native.delete_outside();
                continue;
            }
            Step::OnScreenBackspace => {
                let stroke = backspace();
                let handled = client.stroke(&stroke);
                (stroke, handled)
            }
        };
        let expected = native.key(stroke.event.as_ref(), stroke.typed());
        if handled != expected {
            return Err(format!(
                "{}: {} handled {handled}, natively {expected}",
                case.name, stroke.name
            ));
        }
    }
    let (got, expected) = if setup.preedit && !setup.password {
        (
            client.field.text(),
            format!("{}{}", native.document, native.preedit),
        )
    } else {
        (client.field.before.clone(), native.document.clone())
    };
    if got != expected {
        return Err(format!(
            "{}: the field shows {got:?}, natively {expected:?}",
            case.name
        ));
    }
    Ok(())
}

/// Every case of `cases` in every setup, the setups in parallel; at most
/// ten failures reported.
fn check(name: &str, model: &Model, cases: &[Case], setups: &[Setup]) {
    assert!(cases.len() > 40, "{name}: {} cases", cases.len());
    let mut failures: Vec<String> = std::thread::scope(|scope| {
        let runs: Vec<_> = setups
            .iter()
            .map(|setup| {
                scope.spawn(move || {
                    cases
                        .iter()
                        .filter(|case| {
                            setup.preedit
                                || !case
                                    .steps
                                    .iter()
                                    .any(|step| matches!(step, Step::OnScreenBackspace))
                        })
                        .filter_map(|case| run(model, *setup, case).err())
                        .map(|failure| format!("{name} {setup:?}: {failure}"))
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        runs.into_iter()
            .flat_map(|run| run.join().unwrap())
            .collect()
    });
    let count = failures.len();
    failures.truncate(10);
    assert!(
        failures.is_empty(),
        "{count} failure(s):\n{}",
        failures.join("\n")
    );
}

/// The cases of the fixture `name` in every setup that gives the engine
/// a context; a model that rewrites committed text gets none without a
/// preedit or in a password field, so it is not compared there.
// [spec:kbdgen:req:linux.test.planner/test]
// [spec:kbdgen:req:linux.ibus.planner/test]
// [spec:kbdgen:req:linux.ibus.keys/test]
// [spec:kbdgen:sem:linux.ibus.modifiers/test]
// [spec:kbdgen:sem:linux.ibus.virtual-keys/test]
// [spec:kbdgen:sem:linux.ibus.semantics/test]
// [spec:kbdgen:req:linux.ibus.edit/test]
// [spec:kbdgen:sem:linux.ibus.context/test]
// [spec:kbdgen:sem:linux.ibus.context.fallback/test]
// [spec:kbdgen:sem:linux.ibus.osk-backspace/test]
// [spec:kbdgen:req:linux.ibus.security/test]
fn fixture_types_what_the_engine_types(name: &str) {
    let (_, tag, text) = fixture::all()
        .into_iter()
        .find(|(fixture, _, _)| *fixture == name)
        .unwrap();
    let model = fixture::model(tag, &text);
    let rewrites = !model.keyboard().rewrites().is_empty();
    let setups: &[Setup] = if rewrites {
        &[GTK, GNOME_SHELL, TERMINAL]
    } else {
        &[GTK, GNOME_SHELL, TERMINAL, BARE, PASSWORD]
    };
    check(name, &model, &cases(&model), setups);
}

#[test]
fn vro_types_what_the_engine_types() {
    fixture_types_what_the_engine_types("vro");
}

#[test]
fn mods_type_what_the_engine_types() {
    fixture_types_what_the_engine_types("mods");
}

#[test]
fn v3_layout_types_what_the_engine_types() {
    fixture_types_what_the_engine_types("se-NO");
}

#[test]
fn v3_chromeos_vro_types_what_engine_types() {
    fixture_types_what_the_engine_types("vro-v3");
}

#[test]
fn macos_features_type_what_the_engine_types() {
    fixture_types_what_the_engine_types("macos-features");
}

#[test]
fn windows_features_type_what_the_engine_types() {
    fixture_types_what_the_engine_types("windows-features");
}

#[test]
fn context_fixture_types_what_the_engine_types() {
    fixture_types_what_the_engine_types("context");
}

// [spec:kbdgen:req:linux.test.planner/test]
// [spec:kbdgen:sem:linux.ibus.context.fallback/test]
#[test]
fn rewriting_model_types_every_pair_of_positions() {
    let model = fixture::model("und", &fixture::context());
    assert!(!model.keyboard().rewrites().is_empty());
    check("context", &model, &pairs(&model), &[TERMINAL, GTK]);
}

#[test]
fn passed_keys_are_typed_as_gtk_types_them() {
    assert_eq!(Typed::of(0x61, 30, 0), Typed::Text('a'));
    assert_eq!(Typed::of(0x61, 30, 1 << 2), Typed::Nothing, "a Ctrl chord");
    assert_eq!(Typed::of(0xff0d, 28, 0), Typed::Nothing);
    assert_eq!(Typed::of(0, 14, 0), Typed::Backspace);
}
