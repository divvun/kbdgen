//! `ldml.macos.semantics`, `ldml.macos.test.typing` and
//! `ldml.macos.test.differential` on every fixture layout and on small
//! layouts for the exceptions no fixture exercises.

#![cfg(test)]

use std::collections::BTreeSet;

use super::compare::compare;
use super::exceptions::{Exception, LISTED};
use super::migrated::EXPLANATIONS;
use super::*;
use crate::build::macos::input::{Binding, Key, KeyMap, When};

/// Small layouts for the exceptions no fixture exercises.
const SYNTHETIC: [(Exception, &str); 5] = [
    (
        Exception::TextTransform,
        "transforms: [[{from: q, to: Q}]]\n",
    ),
    (
        Exception::Reorder,
        "transforms:\n  - reorder: [{from: '\\u{301}', order: 20}, {from: '\\u{328}', order: 10}]\n",
    ),
    (Exception::Normalization, "normalization: enabled\n"),
    (Exception::MixedOutput, "keys:\n  mx: {output: 'a\\m{x}'}\n"),
    (
        Exception::DeadKeyResult,
        "keys:\n  dx: {output: '\\m{x}'}\ntransforms: [[{from: '\\m{x}q', to: 'x\\m{x}'}]]\n",
    ),
];

/// A one-layer `sme` macOS layout with `body` before its hardware: `\k{mx}`
/// or `\k{dx}` at E00 when `body` defines it, and `a\u{301}\u{328}` and `é`
/// at E01 and E02.
fn synthetic(body: &str) -> String {
    let first = if body.contains("mx:") {
        "\\k{mx} 1 2 3 4 5 6 7 8 9 0 - ="
    } else if body.contains("dx:") {
        "\\k{dx} 1 2 3 4 5 6 7 8 9 0 - ="
    } else {
        "§ a\\u{301}\\u{328} é 3 4 5 6 7 8 9 0 - ="
    };
    format!(
        "format: 4\ndisplayNames: {{sme: Davvisámegiella}}\n{body}hardware:\n  macOS:\n    layers:\n      none: |\n        {first}\n        q w e r t y u i o p å ¨\n        a s d f g h j k l ö æ '\n        < z x c v b n m , . /\n"
    )
}

// [spec:kbdgen:req:ldml.macos.test.typing/test]
#[test]
fn fixture_keylayouts_type_what_the_engine_types() {
    let mut unexplained = Vec::new();
    for (label, tag, yaml) in fixture_layouts() {
        let subject = subject(&label, &tag, &yaml);
        let report = compare(&subject);
        eprintln!("{label}: {}", report.summary());
        assert!(report.matched * 2 > report.cases, "{label}");
        assert!(report.excepted.is_empty(), "{label}: {}", report.summary());
        unexplained.extend(report.unexplained);
    }
    assert!(unexplained.is_empty(), "{}", unexplained.join("\n"));
}

// [spec:kbdgen:req:ldml.macos.test.typing/test]
#[test]
fn each_listed_exception_explains_a_difference() {
    for (exception, body) in SYNTHETIC {
        let subject = subject(&format!("{exception:?}"), "sme", &synthetic(body));
        let report = compare(&subject);
        assert!(report.unexplained.is_empty(), "{:#?}", report.unexplained);
        assert!(
            report.excepted.contains_key(&exception),
            "{exception:?}: {}",
            report.summary()
        );
    }
    let listed: BTreeSet<Exception> = LISTED.iter().map(|(e, _)| *e).collect();
    let exercised: BTreeSet<Exception> = SYNTHETIC.iter().map(|(e, _)| *e).collect();
    assert_eq!(listed, exercised);
}

// [spec:kbdgen:req:ldml.macos.test.typing/test]
#[test]
fn corrupted_keylayouts_are_not_explained() {
    let yaml = std::fs::read_to_string(
        std::path::Path::new(WORKSPACE).join("crates/kbd-engine/tests/golden/layouts/vro.yaml"),
    )
    .unwrap();
    let mut subject = subject("vro", "vro", &yaml);
    for key in &mut subject.parsed.maps[0].keys {
        match &mut key.binding {
            Binding::Output(output) if output == "q" => *output = "x".to_owned(),
            Binding::Action { whens, .. } => {
                for when in whens
                    .iter_mut()
                    .filter(|w| w.output.as_deref() == Some("á"))
                {
                    when.output = Some("y".to_owned());
                }
            }
            Binding::Output(_) => {}
        }
    }
    let lines = compare(&subject).unexplained.join("\n");
    assert!(
        lines.contains("D01 with []: engine \"q\", keylayout \"x\""),
        "{lines}"
    );
    assert!(
        lines.contains("then C01 with []: engine \"á\", keylayout \"y\""),
        "{lines}"
    );
}

fn action(id: &str, whens: &[When]) -> Binding {
    Binding::Action {
        id: id.to_owned(),
        whens: whens.to_vec(),
    }
}

// [spec:kbdgen:sem:ldml.macos.semantics/test]
#[test]
fn simulated_dead_keys_compose_chain_and_terminate() {
    let maps = vec![
        KeyMap {
            modifiers: vec!["command?".into()],
            keys: vec![
                Key {
                    code: 10,
                    binding: action("a0", &[When::next("none", "d0")]),
                },
                Key {
                    code: 0,
                    binding: action(
                        "a1",
                        &[
                            When::output("none", "a"),
                            When::output("d0", "â"),
                            When::output("d1", "ǎ"),
                        ],
                    ),
                },
                Key {
                    code: 12,
                    binding: action("a2", &[When::output("none", "q"), When::next("d0", "d1")]),
                },
                Key {
                    code: 13,
                    binding: Binding::Output("w".into()),
                },
            ],
        },
        KeyMap {
            modifiers: vec!["anyShift caps? command?".into(), "option".into()],
            keys: vec![Key {
                code: 0,
                binding: Binding::Output("A".into()),
            }],
        },
    ];
    let input = KeylayoutInput {
        name: "sme".to_owned(),
        tag: "sme".parse().unwrap(),
        display_names: Default::default(),
        default_index: 0,
        maps,
        terminators: vec![When::output("d0", "^"), When::output("d1", "ˇ")],
    };
    let mut keylayout = Keylayout::new(&input).unwrap();
    let none = Mods::default();
    let mut press = |code| keylayout.press(code, none);
    assert_eq!(press(10), "");
    assert_eq!(press(0), "â");
    assert_eq!(press(10), "");
    assert_eq!(press(12), "", "q chains to the second state");
    assert_eq!(press(0), "ǎ");
    assert_eq!(press(10), "");
    assert_eq!(press(13), "^w", "a plain key types the terminator first");
    assert_eq!(press(10), "");
    assert_eq!(
        press(10),
        "^",
        "a dead key with no when of its own terminates, then enters"
    );
    assert_eq!(press(14), "", "a key the map lacks types nothing");
    assert_eq!(keylayout.pending().as_deref(), Some("^"));
    let shift = Mods {
        shift_r: true,
        caps: true,
        ..none
    };
    assert_eq!(keylayout.select(shift).unwrap().keys.len(), 1);
    let left = Mods {
        option_l: true,
        ..none
    };
    let right = Mods {
        option_r: true,
        ..none
    };
    assert_eq!(keylayout.match_count(left), 1);
    assert_eq!(
        keylayout.match_count(right),
        0,
        "option names the left key only"
    );
    assert_eq!(
        keylayout.select(right).unwrap().keys.len(),
        4,
        "the default index"
    );
}

// [spec:kbdgen:def:ldml.macos.input/test]
#[test]
fn written_keylayouts_read_back_unchanged() {
    for (label, tag, yaml) in fixture_layouts() {
        let subject = subject(&label, &tag, &yaml);
        let mut expected = subject.adapted.input.clone();
        expected.display_names.clear();
        assert_eq!(subject.parsed, expected, "{label}");
    }
}

// [spec:kbdgen:req:ldml.macos.test.differential/test]
#[test]
fn v3_fixtures_match_their_migrations() {
    let golden = std::path::Path::new(WORKSPACE).join("crates/kbd-engine/tests/golden/v3");
    let mut seen = BTreeSet::new();
    let row = "´ 1 2 3 4 5 6 7 8 9 0 + ¨ q w e r t y u i o p å ^ a s d f g h j k l ö æ ' < z x c v b n m , . -";
    let dead_after_dead = format!(
        "displayNames: {{sme: Davvisámegiella}}\nmacOS:\n  primary:\n    layers:\n      default: {row}\n      shift: {upper}\n      caps: {upper}\n  deadKeys:\n    default: ['´', '¨']\ntransforms:\n  ´:\n    ' ': ´\n    a: á\n    ¨: ˝\n  ¨:\n    ' ': ¨\n    a: ä\n",
        upper = row.to_uppercase()
    );
    let mut cases: Vec<(String, String)> = ["vro", "se-NO"]
        .iter()
        .map(|tag| {
            (
                tag.to_string(),
                std::fs::read_to_string(golden.join(format!("{tag}.yaml"))).unwrap(),
            )
        })
        .collect();
    cases.push(("sme".to_owned(), dead_after_dead));
    for (tag, yaml) in cases {
        let comparison = migrated::compare(&tag, &yaml);
        for (why, lines) in &comparison.explained {
            eprintln!("{tag}: {} × {why:?}, e.g. {}", lines.len(), lines[0]);
        }
        assert!(
            comparison.unexplained.is_empty(),
            "{tag}: {:#?}",
            comparison.unexplained
        );
        seen.extend(comparison.explained.keys().copied());
    }
    assert_eq!(seen, BTreeSet::from(EXPLANATIONS));
}

// [spec:kbdgen:req:ldml.macos.test.differential/test]
#[test]
fn plain_v3_layouts_migrate_byte_for_byte() {
    let rows = |shift: bool| {
        let row = "§ 1 2 3 4 5 6 7 8 9 0 + ´ q w e r t y u i o p å ¨ a s d f g h j k l ö æ ' < z x c v b n m , . -";
        if shift {
            row.to_uppercase()
        } else {
            row.to_owned()
        }
    };
    let layers: Vec<String> = [
        ("default", false),
        ("shift", true),
        ("caps", true),
        ("alt", false),
        ("alt+shift", true),
        ("alt+caps", true),
    ]
    .iter()
    .map(|(name, upper)| format!("      {name}: {}\n", rows(*upper)))
    .collect();
    let yaml = format!(
        "displayNames: {{sme: Davvisámegiella}}\ndecimal: ','\nmacOS:\n  primary:\n    layers:\n{}",
        layers.concat()
    );
    let comparison = migrated::compare("sme", &yaml);
    assert!(comparison.identical, "{comparison:#?}");
    assert!(comparison.explained.is_empty() && comparison.unexplained.is_empty());
}
