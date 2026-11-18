use super::symbols::{adapted, line};
use super::*;
use crate::build::linux::xkb::compose::{ComposeTable, Composer};
use crate::build::linux::xkb::keysym::Keysym;

fn keysyms(names: &[&str]) -> Vec<Keysym> {
    names
        .iter()
        .map(|name| Keysym::from_name(name).unwrap_or_else(|| panic!("{name}")))
        .collect()
}

// [spec:kbdgen:def:ldml.linux.compose+1/test]
#[test]
fn reference_table_holds_every_vendored_dead_sequence() {
    let table = ComposeTable::reference();
    assert_eq!(table.skipped, 0);
    assert_eq!(table.len(), 1916);
    let cases = [
        (&["dead_acute", "a"][..], "á"),
        (&["dead_acute", "space"], "'"),
        (&["dead_diaeresis", "space"], "\""),
        (&["dead_belowdot", "space"], "\u{323}"),
        (&["dead_acute", "dead_diaeresis", "u"], "ǘ"),
        (&["dead_abovedot", "U1E63"], "ṩ"),
    ];
    for (names, output) in cases {
        assert_eq!(table.output(&keysyms(names)), Some(output), "{names:?}");
    }
    assert_eq!(table.output(&keysyms(&["dead_acute", "b"])), None);
}

// [spec:kbdgen:def:ldml.linux.compose+1/test]
#[test]
fn composer_types_what_ibus_1_5_35_typed() {
    let table = ComposeTable::reference();
    let cases: [(&[&str], &str, &[&str]); 29] = [
        (&["dead_acute", "a"], "á", &[]),
        (&["dead_acute", "b"], "", &["dead_acute"]),
        (&["dead_acute", "space"], "'", &[]),
        (&["dead_diaeresis", "space"], "\"", &[]),
        (&["dead_acute", "dead_acute"], "´", &[]),
        (
            &["dead_caron", "dead_acute"],
            "",
            &["dead_caron", "dead_acute"],
        ),
        (&["dead_acute", "dead_diaeresis", "u"], "ǘ", &[]),
        (&["dead_acute", "q", "a"], "á", &[]),
        (&["dead_belowdot", "space"], "\u{323}", &[]),
        (&["dead_abovedot", "j"], "ȷ", &[]),
        (&["dead_belowring", "u"], "ů", &[]),
        (&["dead_belowring", "w"], "ẘ", &[]),
        (&["dead_acute", "Greek_iotadieresis"], "ΐ", &[]),
        (&["dead_acute", "Greek_upsilondieresis"], "ΰ", &[]),
        (
            &["dead_acute", "dead_caron", "a"],
            "",
            &["dead_acute", "dead_caron"],
        ),
        (
            &["dead_caron", "dead_acute", "u"],
            "",
            &["dead_caron", "dead_acute"],
        ),
        (&["dead_currency", "dead_currency"], "¤", &[]),
        (&["dead_acute", "ae"], "ǽ", &[]),
        (&["dead_greek", "a"], "α", &[]),
        (&["dead_belowdot", "U1E61"], "ṩ", &[]),
        (&["dead_breve", "U1EA1"], "ặ", &[]),
        (&["dead_abovedot", "U1E63"], "ṩ", &[]),
        (&["dead_caron", "U01DC"], "", &["dead_caron"]),
        (&["dead_acute", "U1E9E"], "", &["dead_acute"]),
        (&["dead_tilde", "U0306", "o"], "õ", &[]),
        (&["dead_acute", "U0101"], "", &["dead_acute"]),
        (&["dead_acute", "dead_macron", "o"], "ṓ", &[]),
        (&["dead_macron", "dead_acute", "o"], "ṓ", &[]),
        (&["dead_acute", "VoidSymbol", "a"], "á", &[]),
    ];
    for (names, text, pending) in cases {
        let (typed, composer) = Composer::type_keysyms(table, &keysyms(names));
        assert_eq!(typed, text, "{names:?}");
        assert_eq!(composer.pending, keysyms(pending), "{names:?}");
    }
    let (typed, composer) = Composer::type_keysyms(table, &keysyms(&["a", "b"]));
    assert_eq!(typed, "ab", "keys without a pending sequence pass");
    assert!(composer.pending.is_empty());
}

// [spec:kbdgen:sem:ldml.linux.dead-keys+1/test]
#[test]
fn dead_keys_take_the_first_free_dead_keysym() {
    let body = "deadKeys:\n  ´: {standalone: ´, compose: {a: á}}\n  '\\u{301}': {standalone: '\\u{301}', compose: {e: é}}\n  ',': {standalone: ',', compose: {s: ș}}\n  x: {standalone: x, compose: {o: ö}}\n  ¨: {standalone: '', compose: {o: ö}}\n";
    let rows = "\\d{´} \\d{\\u{301}} \\d{,} \\d{x} \\d{¨} 5 6 7 8 9 0 + -\nq w e r t y u i o p å ¨\na s d f g h j k l ö æ '\n< z x c v b n m , . -";
    let layout = adapted("sme", &v4_linux("sme", "Sámi", body, rows));
    let keyboard = &linux_layouts(&[("sme", &v4_linux("sme", "Sámi", body, rows))])[0].keyboard;
    let named: Vec<(String, String)> = layout
        .dead
        .keysyms
        .iter()
        .map(|(marker, keysym)| {
            (
                keyboard.marker_name(*marker).unwrap().to_owned(),
                keysym.name(),
            )
        })
        .collect();
    let find = |marker: &str| {
        named
            .iter()
            .find(|(name, _)| name.contains(marker))
            .map(|(_, keysym)| keysym.as_str())
    };
    assert_eq!(find("00B4"), Some("dead_acute"));
    assert_eq!(find("0301"), None, "dead_acute is taken");
    assert_eq!(find("002C"), Some("dead_belowcomma"));
    assert_eq!(find("00A8"), Some("dead_diaeresis"), "by its display");
    assert_eq!(layout.dead.without.len(), 2);
    assert!(
        line(&layout, "TLDE").contains("[ dead_acute"),
        "{}",
        layout.text
    );
    assert!(line(&layout, "AE01").contains("[ U0301"), "the standalone");
    assert!(line(&layout, "AE03").contains("[ x"), "the standalone");
    assert!(line(&layout, "AE04").contains("[ dead_diaeresis"));
}

// [spec:kbdgen:sem:ldml.linux.dead-keys+1/test]
#[test]
fn compose_results_unlike_the_engine_are_listed() {
    let body = "deadKeys:\n  ´: {standalone: ´, compose: {a: á, b: b́}}\n";
    let rows = ROWS.replacen('§', "\\d{´}", 1);
    let layout = adapted("sme", &v4_linux("sme", "Sámi", body, &rows));
    let lines: Vec<String> = layout.dead.differences.iter().map(|d| d.line()).collect();
    let has = |line: &str| lines.iter().any(|l| l == line);
    assert!(
        has(
            "\\m{dk_00B4} then \"b\": the engine types \"b\\u{301}\", the XKB layout types \"\" and leaves dead_acute pending"
        ),
        "{lines:#?}"
    );
    assert!(
        has("\\m{dk_00B4} then \" \": the engine types \"´\", the XKB layout types \"'\""),
        "{lines:#?}"
    );
    assert!(
        has(
            "\\m{dk_00B4} then \"q\": the engine types \"´q\", the XKB layout types \"\" and leaves dead_acute pending"
        ),
        "{lines:#?}"
    );
    assert!(
        !lines
            .iter()
            .any(|l| l.starts_with("\\m{dk_00B4} then \"a\":")),
        "á agrees: {lines:#?}"
    );
}
