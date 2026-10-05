use kbd_model::{ClassRange, Fixed};

use super::*;
use crate::escape::Piece;

fn pattern(s: &str) -> PatternSyntax {
    parse_pattern(s).unwrap_or_else(|e| panic!("{s}: {e}"))
}

fn range(lo: char, hi: char) -> ClassRange {
    ClassRange { lo, hi }
}

// [spec:kbdgen:syn:ldml.xml.from/test]
#[test]
fn parses_the_from_grammar() {
    let p = pattern("^\\m{acute}($[vowel])x{0,2}.?[^a-c\\-\\m{m}]\\s(?:ab|\\u{63 64})");
    assert!(p.anchored);
    let seq = &p.alternatives[0];
    assert_eq!(seq[0], Quantified::one(Atom::Marker("acute".into())));
    assert_eq!(
        seq[1],
        Quantified::one(Atom::Capture(vec![Quantified::one(Atom::Var(
            "vowel".into()
        ))]))
    );
    assert_eq!(
        seq[2],
        Quantified {
            atom: Atom::Char('x'),
            min: 0,
            max: 2
        }
    );
    assert_eq!(
        (seq[3].atom.clone(), seq[3].min, seq[3].max),
        (Atom::Any, 0, 1)
    );
    assert_eq!(
        seq[4].atom,
        Atom::Class(ClassSyntax {
            negated: true,
            members: vec![
                ClassMember::Range('a', 'c'),
                ClassMember::Range('-', '-'),
                ClassMember::Marker("m".into())
            ]
        })
    );
    assert_eq!(seq[5].atom, Atom::Fixed(Fixed::Space));
    assert_eq!(
        seq[6].atom,
        Atom::Group(vec![
            vec![
                Quantified::one(Atom::Char('a')),
                Quantified::one(Atom::Char('b'))
            ],
            vec![
                Quantified::one(Atom::Char('c')),
                Quantified::one(Atom::Char('d'))
            ]
        ])
    );
}

// [spec:kbdgen:syn:ldml.xml.from/test]
#[test]
fn rejects_disallowed_regex_features() {
    for bad in [
        "",
        "a|",
        "|a",
        "a||b",
        "a*",
        "a+",
        "a{1,}",
        "a{,2}",
        "a{2,1}",
        "a{0,0}",
        "a??",
        "a{1,2}{1,2}",
        "((a))",
        "(a|b)",
        "(?<n>a)",
        "(?=a)",
        "\\1",
        "\\p{L}",
        "\\b",
        "\\a",
        "a$",
        "a^",
        "[]",
        "[a-]b",
        "[[a]]",
        "[b-a]",
        "(a",
        "a)",
        "]",
        "}",
        "\\u{}",
        "(a)(b)(c)(d)(e)(f)(g)(h)(i)(j)",
        "$[bad id]",
        "\\m{a b}",
    ] {
        assert!(parse_pattern(bad).is_err(), "{bad:?} should be rejected");
    }
    assert!(parse_pattern("(a)(b)(c)(d)(e)(f)(g)(h)(i)").is_ok());
    assert!(
        parse_pattern("x?").is_ok(),
        "empty matches are checked on the model"
    );
    assert!(
        parse_pattern("(?:(a)|b)").is_ok(),
        "a capture may sit in a group"
    );
}

// [spec:kbdgen:syn:ldml.xml.from/test]
#[test]
fn quantified_multi_codepoint_escape_is_grouped() {
    let p = pattern("\\u{61 62}?");
    assert_eq!(
        p.alternatives[0],
        vec![Quantified {
            atom: Atom::Group(vec![vec![
                Quantified::one(Atom::Char('a')),
                Quantified::one(Atom::Char('b'))
            ]]),
            min: 0,
            max: 1
        }]
    );
    assert_eq!(pattern("\\u{61 62}").alternatives[0].len(), 2);
}

// [spec:kbdgen:req:ldml.xml.export.escape/test]
#[test]
fn encoded_patterns_parse_back() {
    for s in [
        "^\\m{a}($[s1])x{0,2}",
        "\\.\\(\\)\\?\\[\\\\\\]\\{\\}\\*\\/\\^\\+\\|\\$",
        "a\\u{0301}\\u{200C} b",
        "[^a-c\\-\\]\\m{m}](?:x|y)\\d\\m{.}",
        "e\\u{0300}|[\\u{0300}-\\u{036F}]",
    ] {
        let p = pattern(s);
        let encoded = encode_pattern(&p);
        assert_eq!(pattern(&encoded), p, "{s} -> {encoded}");
    }
    let marks = PatternSyntax {
        anchored: false,
        alternatives: vec![vec![
            Quantified::one(Atom::Char('\u{301}')),
            Quantified::one(Atom::Char('.')),
            Quantified::one(Atom::Char(' ')),
        ]],
    };
    assert_eq!(encode_pattern(&marks), "\\u{0301}\\. ");
}

// [spec:kbdgen:syn:ldml.xml.from/test]
#[test]
fn string_variables_substitute_textually() {
    let strings = |id: &str| match id {
        "a" => Some("x|y".to_string()),
        "b" => Some("\\u{61}".to_string()),
        _ => None,
    };
    assert_eq!(
        substitute_strings("${a}${b}\\${a}", &strings).unwrap(),
        "x|y\\u{61}\\${a}"
    );
    assert!(substitute_strings("${missing}", &strings).is_err());
    let p = pattern(&substitute_strings("${a}", &strings).unwrap());
    assert_eq!(
        p.alternatives.len(),
        2,
        "the value is parsed as pattern syntax"
    );
}

// [spec:kbdgen:syn:ldml.xml.to/test]
#[test]
fn parses_the_to_grammar() {
    let strings = |id: &str| (id == "z").then(|| vec![Piece::Char('z'), Piece::Marker("m".into())]);
    let items =
        parse_replacement("a$$\\$\\\\$0$9${z}$[1:lower]\\m{k}\\u{301}()", &strings).unwrap();
    assert_eq!(
        items,
        vec![
            ToItem::Char('a'),
            ToItem::Char('$'),
            ToItem::Char('$'),
            ToItem::Char('\\'),
            ToItem::Group(0),
            ToItem::Group(9),
            ToItem::Char('z'),
            ToItem::Marker("m".into()),
            ToItem::MapSet {
                group: 1,
                set: "lower".into()
            },
            ToItem::Marker("k".into()),
            ToItem::Char('\u{301}'),
            ToItem::Char('('),
            ToItem::Char(')'),
        ]
    );
    for bad in [
        "$", "$x", "\\x", "\\", "${nope}", "$[0:x]", "$[1x]", "\\m{.}", "\\u{zz}",
    ] {
        assert!(parse_replacement(bad, &strings).is_err(), "{bad:?}");
    }
    let encoded = encode_replacement(&items);
    assert_eq!(parse_replacement(&encoded, &|_| None).unwrap(), items);
}

// [spec:kbdgen:syn:ldml.xml.sets/test]
#[test]
fn set_values_split_and_splice() {
    let earlier =
        |id: &str| (id == "up").then(|| vec![vec![Piece::Char('A')], vec![Piece::Char('B')]]);
    let items = parse_set("  a \\u{62 20}c  $[up]\t\\m{m}d ", &earlier).unwrap();
    assert_eq!(
        items,
        vec![
            vec![Piece::Char('a')],
            vec![Piece::Char('b'), Piece::Char(' '), Piece::Char('c')],
            vec![Piece::Char('A')],
            vec![Piece::Char('B')],
            vec![Piece::Marker("m".into()), Piece::Char('d')],
        ]
    );
    assert!(parse_set("$[up]$[up]", &earlier).is_err());
    assert!(parse_set("$[later]", &earlier).is_err());
    let encoded = encode_set_items(&items);
    assert_eq!(parse_set(&encoded, &|_| None).unwrap()[1], items[1]);
}

// [spec:kbdgen:syn:ldml.xml.sets/test]
#[test]
fn uset_subset_resolves_to_ranges() {
    let earlier = |id: &str| (id == "range").then(|| vec![range('a', 'z'), range('D', 'G')]);
    assert_eq!(
        parse_uset("[a-z D E F G \\u{200A}]", &|_| None).unwrap(),
        vec![
            range('D', 'G'),
            range('a', 'z'),
            range('\u{200A}', '\u{200A}')
        ]
    );
    assert_eq!(
        parse_uset("[$[range]-[G]]", &earlier).unwrap(),
        vec![range('D', 'F'), range('a', 'z')]
    );
    assert_eq!(
        parse_uset("[[ab][$[range]]]", &earlier).unwrap(),
        vec![range('D', 'G'), range('a', 'z')]
    );
    assert_eq!(
        parse_uset("[^\\u{0}-\\u{FF}]", &|_| None).unwrap(),
        vec![range('\u{100}', char::MAX)]
    );
    for bad in [
        "[{ab}]",
        "[\\p{L}]",
        "[[:L:]]",
        "[a",
        "a",
        "[$[nope]]",
        "[z-a]",
        "[a&b]",
    ] {
        assert!(parse_uset(bad, &earlier).is_err(), "{bad:?}");
    }
    let ranges = parse_uset("[a-c\\-\\[x]", &|_| None).unwrap();
    assert_eq!(
        parse_uset(&encode_uset(&ranges), &|_| None).unwrap(),
        ranges
    );
}

#[test]
fn range_algebra_keeps_surrogates_out() {
    let all = complement_ranges(&[]);
    assert_eq!(
        all,
        vec![range('\0', char::MAX)],
        "a range never holds surrogates"
    );
    assert_eq!(
        normalize_ranges(vec![range('a', '\u{D7FF}'), range('\u{E000}', '\u{E001}')]),
        vec![range('a', '\u{E001}')]
    );
    assert_eq!(complement_ranges(&all), vec![]);
    assert_eq!(
        intersect_ranges(&[range('a', 'z')], &complement_ranges(&[range('c', 'x')])),
        vec![range('a', 'b'), range('y', 'z')]
    );
    assert_eq!(
        intersect_ranges(&[range('a', 'f')], &[range('d', 'z')]),
        vec![range('d', 'f')]
    );
}

#[test]
fn reorder_strings_and_value_lists() {
    let usets = |id: &str| (id == "v").then(|| vec![range('\u{1A75}', '\u{1A79}')]);
    let elems = parse_reorder("\\u{1A60}[\\u{1A75}-\\u{1A79}]$[v]x", &usets).unwrap();
    assert_eq!(
        elems,
        vec![
            ReorderElem::Char('\u{1A60}'),
            ReorderElem::Ranges(vec![range('\u{1A75}', '\u{1A79}')]),
            ReorderElem::Ranges(vec![range('\u{1A75}', '\u{1A79}')]),
            ReorderElem::Char('x'),
        ]
    );
    assert_eq!(
        parse_reorder(&encode_reorder(&elems), &usets).unwrap(),
        elems
    );
    assert!(parse_reorder("\\m{a}", &usets).is_err());
    assert_eq!(
        parse_reorder_ints(Some("10 55"), 3).unwrap(),
        vec![10, 55, 55]
    );
    assert_eq!(parse_reorder_ints(None, 2).unwrap(), vec![0, 0]);
    assert!(parse_reorder_ints(Some("1 2 3"), 2).is_err());
    assert!(parse_reorder_ints(Some("128"), 1).is_err());
    assert_eq!(parse_reorder_bools(Some("1"), 2).unwrap(), vec![true, true]);
    assert!(parse_reorder_bools(Some("yes"), 1).is_err());
}
