//! The adapter on the golden v4 layouts and on small layouts for single
//! behaviours.

use std::path::Path;

use super::*;
use crate::build::macos::differential::{fixture_bundle, subject};
use crate::build::macos::input::Binding;

/// The repository root, where fixture paths start.
const WORKSPACE: &str = env!("CARGO_MANIFEST_DIR");

fn read(path: &str) -> String {
    let full = Path::new(WORKSPACE).join(path);
    std::fs::read_to_string(full).unwrap_or_else(|e| panic!("{path}: {e}"))
}

/// The warnings and information messages, one per line.
fn diagnostics(diag: &Diagnostics) -> String {
    let warnings = diag.warnings().iter().map(|w| format!("warning: {w}\n"));
    let infos = diag.infos().iter().map(|i| format!("info: {i}\n"));
    warnings.chain(infos).collect()
}

/// Compares `actual` with the golden file `name`, or rewrites it when
/// `KBDGEN_BLESS` is set.
// [spec:kbdgen:req:ldml.macos.test.golden]
fn golden(name: &str, actual: &str) {
    let path = Path::new(WORKSPACE)
        .join("src/build/macos/testdata/golden")
        .join(name);
    if std::env::var_os("KBDGEN_BLESS").is_some() {
        std::fs::write(&path, actual).unwrap();
    }
    let expected =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    assert_eq!(
        actual, expected,
        "{name} differs from its golden file; rerun with KBDGEN_BLESS=1 to update it"
    );
}

// [spec:kbdgen:req:ldml.macos.test.golden/test]
#[test]
fn golden_layouts_write_their_golden_keylayouts() {
    let layouts = [
        (
            "vro",
            "vro",
            read("crates/kbd-engine/tests/golden/layouts/vro.yaml"),
        ),
        (
            "features",
            "sme",
            read("src/build/macos/testdata/features.yaml"),
        ),
    ];
    for (name, tag, yaml) in layouts {
        let subject = subject(name, tag, &yaml);
        golden(&format!("{name}.keylayout"), &subject.xml);
        golden(&format!("{name}.diag"), &diagnostics(&subject.adapted.diag));
    }
}

fn features() -> Adapted {
    subject(
        "features",
        "sme",
        &read("src/build/macos/testdata/features.yaml"),
    )
    .adapted
}

fn whens(input: &KeylayoutInput, map: usize, code: u16) -> Vec<When> {
    let key = input.maps[map]
        .keys
        .iter()
        .find(|k| k.code == code)
        .unwrap();
    match &key.binding {
        Binding::Action { whens, .. } => whens.clone(),
        Binding::Output(output) => vec![When::output(NONE_STATE, output)],
    }
}

// [spec:kbdgen:sem:ldml.macos.dead-keys/test]
#[test]
fn chained_dead_keys_become_next_states() {
    let input = features().input;
    let states: Vec<&str> = input.terminators.iter().map(|t| t.state.as_str()).collect();
    assert_eq!(states, ["dead_key000", "dead_key001", "dead_key002"]);
    assert_eq!(input.terminators[0].output.as_deref(), Some("´"));
    assert_eq!(
        input.terminators[1].output.as_deref(),
        Some("´¨"),
        "the chained state"
    );
    assert_eq!(
        input.terminators[2].output.as_deref(),
        Some(""),
        "¨ drops its standalone"
    );
    let diaeresis = whens(&input, 0, 30);
    assert_eq!(diaeresis[0], When::next(NONE_STATE, "dead_key002"));
    assert!(
        diaeresis.contains(&When::next("dead_key000", "dead_key001")),
        "{diaeresis:?}"
    );
    let a = whens(&input, 0, 0);
    assert!(a.contains(&When::output("dead_key001", "ǟ")), "{a:?}");
    assert!(a.contains(&When::output("dead_key002", "ä")), "{a:?}");
    let space = whens(&input, 0, 49);
    assert!(
        space.contains(&When::output("dead_key000", "´")),
        "{space:?}"
    );
    assert!(
        space.contains(&When::output("dead_key002", "")),
        "{space:?}"
    );
}

// [spec:kbdgen:sem:ldml.macos.modifiers/test]
#[test]
fn other_layer_is_the_default_map() {
    let input = features().input;
    let other = input
        .maps
        .iter()
        .position(|m| m.modifiers.is_empty())
        .unwrap();
    assert_eq!(input.default_index, other);
    let terms: Vec<&str> = input
        .maps
        .iter()
        .flat_map(|m| m.modifiers.iter().map(String::as_str))
        .collect();
    assert!(terms.contains(&"caps? option command?"), "{terms:?}");
    assert!(terms.contains(&"command"), "{terms:?}");
}

// [spec:kbdgen:sem:ldml.macos.keys/test]
#[test]
fn keys_group_by_value_then_fixed_keys() {
    let input = features().input;
    let codes: Vec<u16> = input.maps[0].keys.iter().map(|k| k.code).collect();
    assert_eq!(&codes[..2], &[10, 18], "E00 then E01");
    assert!(!codes.contains(&44), "B10 has no key");
    let tail = &codes[codes.len() - 2..];
    assert_eq!(tail, [DECIMAL_CODE, SPACE.1]);
    let decimal = input.maps[0]
        .keys
        .iter()
        .find(|k| k.code == DECIMAL_CODE)
        .unwrap();
    assert_eq!(
        decimal.binding,
        Binding::Output(".".into()),
        "no decimal: the keypad types ."
    );
}

// [spec:kbdgen:req:ldml.macos.classify/test]
#[test]
fn lost_features_warn_once_per_feature() {
    let adapted = features();
    let warnings = adapted.diag.warnings().join("\n");
    assert!(
        warnings.contains("cannot express 1 key(s) with long-press"),
        "{warnings}"
    );
    assert!(
        warnings.contains("key display(s) and label(s) (e.g. Å"),
        "{warnings}"
    );
    assert!(
        warnings.contains("layer altL, altL caps: it names a left or right Option"),
        "{warnings}"
    );
    assert_eq!(adapted.classification.gestures.count, 1);
    let modifiers = subject(
        "und-x-mods",
        "und-x-mods",
        &read("crates/kbd-engine/tests/golden/layouts/und-x-mods.yaml"),
    );
    let warnings = modifiers.adapted.diag.warnings().join("\n");
    assert!(
        warnings.contains("macOS has no extra modifiers, so its set extra1 is dropped"),
        "{warnings}"
    );
    assert!(
        warnings.contains("Windows-only option(s) (e.g. extra modifier rightCtrl"),
        "{warnings}"
    );
    let clean = "format: 4\ndisplayNames: {sme: Davvisámegiella}\nhardware:\n  default:\n    layers:\n      none: |\n        § 1 2 3 4 5 6 7 8 9 0 - =\n        q w e r t y u i o p å ¨\n        a s d f g h j k l ö æ '\n        < z x c v b n m , . /\n";
    let clean = subject("clean", "sme", clean).adapted;
    assert!(
        clean.diag.warnings().is_empty(),
        "{:?}",
        clean.diag.warnings()
    );
    assert_eq!(
        clean.diag.infos(),
        ["the .keylayout expresses every feature of the keyboard"]
    );
}

// [spec:kbdgen:def:ldml.macos.adapter/test]
#[test]
fn layouts_without_macos_document_have_none() {
    let yaml = "format: 4\ndisplayNames: {sme: Davvisámegiella}\nhardware:\n  windows:\n    layers:\n      none: |\n        § 1 2 3 4 5 6 7 8 9 0 - =\n        q w e r t y u i o p å ¨\n        a s d f g h j k l ö æ '\n        < z x c v b n m , . /\n";
    let fixture = fixture_bundle("sme", yaml);
    let (tag, path) = &fixture.bundle.v4_layouts[0];
    assert!(load(tag, path).unwrap().is_none());
}

// [spec:kbdgen:def:ldml.macos.adapter/test]
#[test]
fn dead_keys_that_commit_are_fatal() {
    let yaml = "format: 4\ndisplayNames: {sme: Davvisámegiella}\nkeys:\n  dx: {output: '\\m{x}'}\ntransforms: [[{from: '\\m{x}', to: 'y'}]]\nhardware:\n  macOS:\n    layers:\n      none: |\n        \\k{dx} 1 2 3 4 5 6 7 8 9 0 - =\n        q w e r t y u i o p å ¨\n        a s d f g h j k l ö æ '\n        < z x c v b n m , . /\n";
    let fixture = fixture_bundle("sme", yaml);
    let (tag, path) = &fixture.bundle.v4_layouts[0];
    let layout = load(tag, path).unwrap().unwrap();
    let error = adapt(&layout).unwrap_err().to_string();
    assert!(
        error.contains(
            "sme: dead key \\m{x}: pressing it does not leave exactly its marker pending"
        ),
        "{error}"
    );
}
