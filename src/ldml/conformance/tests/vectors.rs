//! Golden vector files: the format (`ldml.test.vectors`), loading their
//! keyboards, and how failures are reported (`ldml.cli.test`).

use kbd_engine::{BackspacePolicy, Gesture, ModifierState, OutputForm};
use kbd_model::{Direction, Host};

use super::super::run::run_tests;
use super::super::steps::{Expect, Step};
use super::super::vectors::Source;
use super::*;

const EVERY_STEP: &str = r#"
keyboard: keyboards/gestures.xml
options: {backspace: codePoint, outputForm: nfd}
tests:
  - name: all
    context: "a\\u{301}"
    atStart: false
    steps:
      - press: {key: E12, mods: shift caps}
      - press: {key: space}
      - press: {key: '0x56', mods: altgr}
      - touch: {size: phone, layer: base, row: 0, col: 1, gesture: {flick: ne s}}
      - id: ka
        gesture: {multiTap: 2}
      - id: ka
      - emit: "\\u{1F600}"
      - backspace
      - backspace: {mods: shiftR}
      - decimal
      - decimal: {mods: ctrl}
      - commit
      - reset
      - context: x
      - expect: {text: x, preedit: '', pass: false, layer: null}
      - expect: {layer: shift}
"#;

// [spec:kbdgen:def:ldml.test.vectors+1/test]
#[test]
fn every_step_kind_is_read() {
    let file = vector_file(EVERY_STEP).unwrap();
    assert_eq!(
        file.source,
        Source::Keyboard("keyboards/gestures.xml".into())
    );
    assert_eq!(file.host, None);
    assert_eq!(file.options.backspace, BackspacePolicy::CodePoint);
    assert_eq!(file.options.output_form, OutputForm::Nfd);
    let test = &file.tests[0];
    assert_eq!((test.context.as_str(), test.at_start), ("a\u{301}", false));
    let shift_caps = ModifierState {
        caps: true,
        ..ModifierState::shift()
    };
    let shift_r = ModifierState {
        shift_r: true,
        ..ModifierState::default()
    };
    let ctrl = ModifierState {
        ctrl_l: true,
        ..ModifierState::default()
    };
    let none = ModifierState::default();
    assert_eq!(
        test.steps,
        [
            Step::Press {
                scan: 0x0D,
                modifiers: shift_caps
            },
            Step::Press {
                scan: 0x39,
                modifiers: none
            },
            Step::Press {
                scan: 0x56,
                modifiers: ModifierState::altgr()
            },
            Step::Touch {
                size: "phone".into(),
                layer: "base".into(),
                row: 0,
                col: 1,
                gesture: Gesture::Flick(vec![Direction::Ne, Direction::S]),
            },
            Step::Id {
                id: "ka".into(),
                gesture: Gesture::MultiTap(2)
            },
            Step::Id {
                id: "ka".into(),
                gesture: Gesture::Tap
            },
            Step::Emit("\u{1F600}".into()),
            Step::Backspace(none),
            Step::Backspace(shift_r),
            Step::Decimal(none),
            Step::Decimal(ctrl),
            Step::Commit,
            Step::Reset,
            Step::Context("x".into()),
            Step::Expect(Expect {
                text: Some("x".into()),
                preedit: Some(String::new()),
                pass: Some(false),
                layer: Some(None),
            }),
            Step::Expect(Expect {
                layer: Some(Some("shift".into())),
                ..Expect::default()
            }),
        ]
    );
}

// [spec:kbdgen:def:ldml.test.vectors+1/test]
#[test]
fn modifier_names_follow_the_rule() {
    let mods = |names: &str| {
        let yaml = format!(
            "keyboard: k.xml\ntests: [{{name: t, steps: [{{press: {{key: C01, mods: '{names}'}}}}]}}]"
        );
        match &vector_file(&yaml).unwrap().tests[0].steps[0] {
            Step::Press { modifiers, .. } => *modifiers,
            other => panic!("{other:?}"),
        }
    };
    let all = mods(
        "shift shiftL shiftR caps ctrl ctrlL ctrlR alt altL altR altgr cmd extra1 extra2 extra3",
    );
    assert!(all.shift_l && all.shift_r && all.caps && all.ctrl_l && all.ctrl_r);
    assert!(all.alt_l && all.alt_r && all.altgr && all.cmd && all.extra == [true; 3]);
    assert_eq!(mods("ctrl alt"), {
        ModifierState {
            ctrl_l: true,
            alt_l: true,
            ..ModifierState::default()
        }
    });
    assert_eq!(mods("altgr"), ModifierState::altgr());
    assert_eq!(
        mods("altR"),
        ModifierState {
            alt_r: true,
            ..ModifierState::default()
        },
        "altR alone is not AltGr"
    );
}

// [spec:kbdgen:def:ldml.test.vectors+1/test]
#[test]
fn malformed_vector_files_name_the_path() {
    let cases = [
        ("tests: []", "layout or keyboard"),
        ("layout: l.yaml\ntests: []", "needs host"),
        (
            "layout: l.yaml\nkeyboard: k.xml\nhost: web\ntests: []",
            "not both",
        ),
        (
            "layout: l.yaml\nhost: amiga\ntests: []",
            "host: amiga is not a host",
        ),
        (
            "keyboard: k.xml\noptions: {backspace: word}\ntests: []",
            "options.backspace",
        ),
        (
            "keyboard: k.xml\nextra: 1\ntests: []",
            "unknown field extra",
        ),
        (
            "keyboard: k.xml\ntests: [{steps: []}]",
            "tests[1]: the field name is required",
        ),
        (
            "keyboard: k.xml\ntests: [{name: t, steps: [{press: {key: Q99}}]}]",
            "tests[1].steps[1].press.key: Q99 is not an ISO position",
        ),
        (
            "keyboard: k.xml\ntests: [{name: t, steps: [{press: {key: C01, mods: hyper}}]}]",
            "hyper is not a modifier name",
        ),
        (
            "keyboard: k.xml\ntests: [{name: t, steps: [wiggle]}]",
            "wiggle is not a step",
        ),
        (
            "keyboard: k.xml\ntests: [{name: t, steps: [{emit: a, id: b}]}]",
            "exactly one of",
        ),
        (
            "keyboard: k.xml\ntests: [{name: t, steps: [{emit: a, gesture: tap}]}]",
            "unknown field gesture",
        ),
        (
            "keyboard: k.xml\ntests: [{name: t, steps: [{id: a, gesture: {swipe: 1}}]}]",
            "a gesture is tap",
        ),
        (
            "keyboard: k.xml\ntests: [{name: t, steps: [{expect: {}}]}]",
            "at least one of",
        ),
        (
            "keyboard: k.xml\ntests: [{name: t, steps: [{emit: '\\u{D800}'}]}]",
            "steps[1].emit",
        ),
    ];
    for (yaml, needle) in cases {
        let err = vector_file(yaml).unwrap_err();
        assert!(err.starts_with("t.yaml"), "{err}");
        assert!(err.contains(needle), "{yaml}: {err}");
    }
}

// [spec:kbdgen:def:ldml.test.vectors+1/test]
#[test]
fn keyboards_load_for_the_named_host() {
    let (file, model) = golden_model("layout: layouts/vro.yaml\nhost: windows\ntests: []");
    assert_eq!(file.host, Some(Host::Windows));
    assert_eq!(model.keyboard().host, Some(Host::Windows));
    let (_, model) = golden_model("keyboard: keyboards/gestures.xml\nhost: iOS\ntests: []");
    assert_eq!(model.keyboard().host, Some(Host::Ios));
    let load = |yaml: &str| {
        let file = vector_file(yaml).unwrap();
        vector_model(&file, &golden()).unwrap_err().to_string()
    };
    let err = load(
        "layout: layouts/vro.yaml\nhost: windows\ntests: []"
            .replace("vro", "nope")
            .as_str(),
    );
    assert!(err.contains("nope.yaml"), "{err}");
    let err = load("layout: layouts/und-x-mods.yaml\nhost: iOS\ntests: []");
    assert!(err.contains("has no iOS document"), "{err}");
}

// [spec:kbdgen:def:ldml.test.vectors+1/test]
#[test]
fn xml_host_must_match_the_document() {
    let dir = tempfile::tempdir().unwrap();
    let xml = std::fs::read_to_string(golden().join("keyboards/gestures.xml"))
        .unwrap()
        .replace(
            "</keyboard3>",
            "<special><kbdgen:keyboard host=\"windows\" /></special></keyboard3>",
        );
    std::fs::write(dir.path().join("k.xml"), xml).unwrap();
    let file = vector_file("keyboard: k.xml\nhost: macOS\ntests: []").unwrap();
    let err = vector_model(&file, dir.path()).unwrap_err().to_string();
    assert!(err.contains("the windows document, not macOS"), "{err}");
    let file = vector_file("keyboard: k.xml\nhost: windows\ntests: []").unwrap();
    assert_eq!(
        vector_model(&file, dir.path()).unwrap().keyboard().host,
        Some(Host::Windows)
    );
}

// [spec:kbdgen:def:ldml.test.vectors+1/test]
#[test]
fn v3_layouts_migrate_in_memory() {
    let dir = tempfile::tempdir().unwrap();
    let keys = "a b c d e f g h i j k l m n o p q r s t u v w x y z 1 2 3 4 5 6 7 8 9 0 A B C D E F G H I J K L";
    std::fs::write(
        dir.path().join("sme.yaml"),
        format!("displayNames: {{sme: Sámegiella}}\nwindows:\n  primary:\n    layers:\n      default: {keys}\n"),
    )
    .unwrap();
    let file = vector_file("layout: sme.yaml\nhost: windows\ntests: []").unwrap();
    let model = vector_model(&file, dir.path()).unwrap();
    assert_eq!(model.keyboard().host, Some(Host::Windows));
    assert_eq!(
        std::fs::read_dir(dir.path()).unwrap().count(),
        1,
        "nothing written"
    );

    std::fs::write(
        dir.path().join("fi.yaml"),
        "displayNames: {fi: suomi}\nmodes: {}\n",
    )
    .unwrap();
    let file = vector_file("layout: fi.yaml\nhost: windows\ntests: []").unwrap();
    let err = vector_model(&file, dir.path()).unwrap_err().to_string();
    assert!(err.contains("blocked by defects M07"), "{err}");
}

// [spec:kbdgen:req:ldml.cli.test/test]
#[test]
fn failures_show_expected_and_actual() {
    let (file, model) = golden_model(
        r#"
keyboard: keyboards/gestures.xml
tests:
  - name: wrong
    steps:
      - id: to-shift
      - expect: {text: x, layer: base}
      - id: ka
      - expect: {text: a}
      - touch: {size: tablet, layer: base, row: 0, col: 0}
"#,
    );
    let report = run_tests(&model, &file, "tests/t.yaml");
    assert_eq!((report.tests, report.checks), (1, 2));
    let shown: Vec<String> = report.failures.iter().map(|f| f.to_string()).collect();
    assert_eq!(
        shown,
        [
            "FAIL tests/t.yaml: wrong: step 2: expect\n  \
             expected text     \"x\"\n  \
             expected layer    \"base\"\n  \
             actual   text     \"\"\n  \
             actual   preedit  \"\"\n  \
             actual   layer    \"shift\"\n  \
             actual   action   edit: delete 0, insert \"\", preedit \"\", layer \"shift\"",
            "FAIL tests/t.yaml: wrong: step 5: touch tablet base row 0 col 0\n  \
             expected key      the keyboard has no touch set named tablet\n  \
             actual   text     \"a\"\n  \
             actual   preedit  \"\"\n  \
             actual   layer    none\n  \
             actual   action   edit: delete 0, insert \"a\", preedit \"\", layer none",
        ]
    );
}
