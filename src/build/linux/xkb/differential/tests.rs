//! `ldml.linux.test.typing` on every fixture layout, and on small layouts
//! for the explanations no fixture exercises.

use std::collections::BTreeSet;

use super::compare::compare;
use super::exceptions::{Exception, LISTED};
use super::*;
use crate::build::linux::xkb::SymbolsKey;
use crate::build::linux::xkb::keysym::Keysym;
use crate::build::linux::xkb::tests::{ROWS, v4_layers};

const SHIFT_ROWS: &str = "½ ! \" # ¤ % & / ( ) = ? `\nQ W E R T Y U I O P Å ^\nA S D F G H J K L Ö Æ *\n> Z X C V B N M ; : _";

/// A two-layer `sme` layout with `body` before its hardware and `none` as
/// its unshifted rows.
fn synthetic(body: &str, none: &str) -> String {
    v4_layers(
        "sme",
        "Sámi",
        body,
        &[("none", none), ("shift", SHIFT_ROWS)],
    )
}

/// Small layouts for the explanations, each with its explanation.
fn synthetic_layouts() -> Vec<(Exception, String)> {
    let with = |first: &str| ROWS.replacen('§', first, 1);
    let caps = ROWS.to_uppercase().replacen('Q', "Č", 1);
    let caps_shift = SHIFT_ROWS.replacen('Q', "x", 1);
    let wrong = v4_layers(
        "sme",
        "Sámi",
        "",
        &[
            ("none", ROWS),
            ("shift", SHIFT_ROWS),
            ("caps", &caps),
            ("caps shift", &caps_shift),
        ],
    );
    vec![
        (Exception::LongOutput, synthetic("", &with("ŋ̈"))),
        (
            Exception::MixedOutput,
            synthetic("keys:\n  mx: {output: 'a\\m{x}'}\n", &with("\\k{mx}")),
        ),
        (Exception::WrongState, wrong),
        (
            Exception::DeadKeyWithoutKeysym,
            synthetic(
                "deadKeys:\n  x: {standalone: x, compose: {o: ö}}\n",
                &with("\\d{x}"),
            ),
        ),
        (
            Exception::DeadKeyResult,
            synthetic(
                "deadKeys:\n  ´: {standalone: ´, compose: {a: á, b: b́}}\n",
                &with("\\d{´}"),
            ),
        ),
        (
            Exception::TextTransform,
            synthetic("transforms: [[{from: q, to: Q}]]\n", ROWS),
        ),
        (Exception::LongDecimal, synthetic("decimal: '.,'\n", ROWS)),
    ]
}

// [spec:kbdgen:req:ldml.linux.test.typing+1/test]
#[test]
fn fixture_xkb_layouts_type_what_the_engine_types() {
    let mut unexplained = Vec::new();
    for (label, tag, yaml) in fixture_layouts() {
        let subject = subject(&label, &tag, &yaml);
        let report = compare(&subject);
        eprintln!("{label}: {}", report.summary());
        assert!(report.matched > 0, "{label}");
        unexplained.extend(report.unexplained);
    }
    assert!(unexplained.is_empty(), "{}", unexplained.join("\n"));
}

// [spec:kbdgen:req:ldml.linux.test.typing+1/test]
#[test]
fn each_listed_explanation_explains_a_difference() {
    let synthetic = synthetic_layouts();
    for (exception, yaml) in &synthetic {
        let subject = subject(&format!("{exception:?}"), "sme", yaml);
        let report = compare(&subject);
        assert!(report.unexplained.is_empty(), "{:#?}", report.unexplained);
        assert!(
            report.excepted.contains_key(exception),
            "{exception:?}: {}",
            report.summary()
        );
    }
    let listed: BTreeSet<Exception> = LISTED.iter().map(|(e, _)| *e).collect();
    let exercised: BTreeSet<Exception> = synthetic.iter().map(|(e, _)| *e).collect();
    assert_eq!(listed, exercised);
}

// [spec:kbdgen:req:ldml.linux.test.typing+1/test]
#[test]
fn corrupted_symbols_files_are_not_explained() {
    let yaml = fixture_text("crates/kbd-engine/tests/golden/layouts/vro.yaml");
    let mut subject = subject("vro", "vro", &yaml);
    let q = Keysym::from_char('q');
    for key in &mut subject.parsed.keys {
        for level in key.levels.iter_mut().filter(|level| **level == q) {
            *level = Keysym::from_char('x');
        }
    }
    let lines = compare(&subject).unexplained.join("\n");
    assert!(
        lines.contains("vro: <AD01>: engine \"q\", XKB \"x\""),
        "{lines}"
    );
}

// [spec:kbdgen:syn:linux.xkb.symbols/test]
#[test]
fn written_symbols_files_read_back_unchanged() {
    for (label, tag, yaml) in fixture_layouts() {
        let subject = subject(&label, &tag, &yaml);
        let symbols = &subject.adapted.symbols;
        let written: Vec<SymbolsKey> = symbols.keys.clone();
        assert_eq!(
            subject.parsed.keys.len(),
            written.len() + usize::from(symbols.decimal.is_some())
        );
        for (parsed, key) in subject.parsed.keys.iter().zip(&written) {
            assert_eq!(parsed.name, key.key.name, "{label}");
            assert_eq!(parsed.key_type, key.key_type, "{label}");
            assert_eq!(parsed.levels, key.levels, "{label}");
        }
        assert_eq!(subject.parsed.level3, symbols.uses_level3, "{label}");
        assert_eq!(subject.parsed.description, symbols.description, "{label}");
    }
}
