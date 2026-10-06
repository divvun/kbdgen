//! Reorder groups through the engine.

use super::*;

/// The Myanmar reordering with prebase e-vowel and medial-r.
fn myanmar_keyboard() -> Keyboard {
    let rule = |from: char, order: i8, pre_base: bool| ReorderRule {
        before: vec![],
        from: vec![ReorderClass::Char(from)],
        order: vec![order],
        tertiary: vec![0],
        tertiary_base: vec![false],
        pre_base: vec![pre_base],
    };
    let mut k = keyboard(Normalization::Disabled);
    k.markers = vec!["prebase".to_string()];
    k.simple = vec![
        TransformGroup::Reorder(vec![
            rule('\u{103C}', 20, true),
            rule('\u{103D}', 25, false),
            rule('\u{1031}', 30, true),
            rule('\u{102F}', 40, false),
        ]),
        TransformGroup::Rules(vec![rule_lower_vowels()]),
    ];
    k.backspace = vec![TransformGroup::Rules(vec![
        self::rule(
            vec![TreeItem::one(TreeAtom::Class(0)), ch('\u{1031}')],
            vec![ReplacementItem::Text(Text(vec![
                TextElem::Marker(0),
                TextElem::Char('\u{1031}'),
            ]))],
        ),
        self::rule(vec![mk(0), ch('\u{1031}')], vec![]),
    ])];
    k.classes = vec![Class {
        ranges: vec![ClassRange {
            lo: '\u{1000}',
            hi: '\u{102A}',
        }],
        negated: false,
        markers: vec![],
    }];
    k
}

/// Two lower vowels in one cluster are not allowed: the second is dropped.
fn rule_lower_vowels() -> Rule {
    rule(chars("\u{102F}\u{102F}"), vec![to_text("\u{102F}")])
}

// [spec:kbdgen:sem:ldml.engine.reorder+1/test]
// [spec:kbdgen:def:ldml.scope.v1+1/test]
#[test]
fn prebase_typed_first_is_stored_after_base() {
    let m = model(myanmar_keyboard());
    let mut s = Session::new(&m);
    assert_eq!(s.emit("\u{1031}"), typed("\u{1031}"));
    assert_eq!(s.emit("\u{1000}"), edit(1, "\u{1000}\u{1031}", ""));
    assert_eq!(s.emit("\u{102F}"), typed("\u{102F}"));
    assert_eq!(s.emit("\u{102F}"), typed(""));
    assert_eq!(s.text, "\u{1000}\u{1031}\u{102F}");
}

// [spec:kbdgen:sem:ldml.engine.backspace+1/test]
#[test]
fn burmese_backspace_leaves_prebase_marker() {
    let m = model(myanmar_keyboard());
    let mut s = Session::new(&m);
    s.emit("\u{1031}");
    s.emit("\u{1000}");
    // The base goes and a filler marker stands before the prebase.
    assert_eq!(s.backspace(), edit(2, "\u{1031}", ""));
    assert_eq!(s.pending(), Vec::<&str>::new());
    // The marker and prebase then go as a unit.
    assert_eq!(s.backspace(), edit(1, "", ""));
    assert_eq!(s.text, "");
}
