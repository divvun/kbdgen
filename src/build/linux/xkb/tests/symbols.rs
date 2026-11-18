use super::*;
use crate::build::linux::xkb::keycodes::{DECIMAL, KEYS, evdev_code, scan_code};
use crate::build::linux::xkb::keysym::Keysym;
use crate::build::linux::xkb::types::{KeyType, LEVEL3, LOCK, NUM_LOCK, SHIFT, choose};
use crate::build::linux::xkb::{Adapted, Tree, Value, XkbState, adapt};

/// Rows of `\u{0}` but for `keys`, each (row, column, output).
pub fn sparse(keys: &[(usize, usize, &str)]) -> String {
    let lengths = [13, 12, 12, 11];
    let mut rows: Vec<Vec<String>> = lengths
        .iter()
        .map(|n| vec!["\\u{0}".to_owned(); *n])
        .collect();
    for (row, col, output) in keys {
        rows[*row][*col] = (*output).to_owned();
    }
    rows.iter()
        .map(|row| row.join(" "))
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn adapted(tag: &str, yaml: &str) -> Adapted {
    let layouts = linux_layouts(&[(tag, yaml)]);
    adapt(&ids(), &layouts[0]).unwrap_or_else(|e| panic!("{e:#}"))
}

/// The line of key `name` in the symbols file.
pub fn line<'a>(adapted: &'a Adapted, name: &str) -> &'a str {
    let prefix = format!("    key <{name}> ");
    adapted
        .text
        .lines()
        .find(|line| line.starts_with(&prefix))
        .unwrap_or_else(|| panic!("no <{name}> in\n{}", adapted.text))
}

fn values(adapted: &Adapted, name: &str) -> [Value; 8] {
    adapted
        .levels
        .keys
        .iter()
        .find(|key| key.key.name == name)
        .unwrap_or_else(|| panic!("no <{name}>"))
        .values
        .clone()
}

// [spec:kbdgen:def:ldml.linux.keycodes/test]
#[test]
fn evdev_codes_pair_as_chromium_pairs_them() {
    assert_eq!(evdev_code(0x29), Some(41));
    assert_eq!(evdev_code(0x58), Some(88));
    assert_eq!(evdev_code(0x45), None, "Pause shares 45 with Num Lock");
    assert_eq!(evdev_code(0x54), None);
    assert_eq!(evdev_code(0x55), None);
    for (scan, evdev) in [
        (0x59, 117),
        (0x70, 93),
        (0x71, 123),
        (0x72, 122),
        (0x73, 89),
        (0x79, 92),
        (0x7B, 94),
        (0x7D, 124),
        (0x7E, 121),
    ] {
        assert_eq!(evdev_code(scan), Some(evdev));
        assert_eq!(scan_code(evdev), Some(scan));
    }
    assert_eq!(evdev_code(0x7F), None);
    for scan in 0..=0xFF {
        if let Some(evdev) = evdev_code(scan) {
            assert_eq!(scan_code(evdev), Some(scan), "{scan:02X}");
        }
    }
    let names: Vec<&str> = KEYS.iter().map(|key| key.name).collect();
    assert_eq!(names.len(), 51);
    assert_eq!(&names[..2], ["TLDE", "AE01"]);
    assert_eq!(&names[12..15], ["AE12", "AE13", "AD01"]);
    assert_eq!(&names[36..40], ["AC11", "BKSL", "LSGT", "AB01"]);
    assert_eq!(&names[48..], ["AB10", "AB11", "SPCE"]);
    let codes: Vec<u32> = ["TLDE", "AE13", "LSGT", "AB11", "SPCE"]
        .iter()
        .map(|name| KEYS.iter().find(|key| key.name == *name).unwrap().keycode())
        .collect();
    assert_eq!(codes, [49, 132, 94, 97, 65]);
    assert_eq!(DECIMAL.keycode(), 91);
}

// [spec:kbdgen:def:ldml.linux.keycodes/test]
#[test]
fn only_keys_of_the_form_are_written() {
    let features = fixture_text("src/build/windows/kbdl/testdata/features.yaml")
        .replace("hardware:\n  windows:", "hardware:\n  linux:");
    let abnt2 = adapted("sme", &features);
    let written: Vec<&str> = abnt2.levels.keys.iter().map(|key| key.key.name).collect();
    let expected: Vec<&str> = KEYS
        .iter()
        .map(|key| key.name)
        .filter(|name| *name != "AE13")
        .collect();
    assert_eq!(written, expected, "abnt2 has B11 but no yen key");
    let iso = adapted("sme", &v4_linux("sme", "Sámi", "", ROWS));
    assert!(iso.text.contains("<LSGT>") && !iso.text.contains("<AB11>"));
}

// [spec:kbdgen:sem:ldml.linux.levels+1/test]
#[test]
fn alt_layers_type_with_altgr_left_alt_never() {
    let yaml = v4_layers(
        "sme",
        "Sámi",
        "",
        &[
            ("none", ROWS),
            ("shift", &ROWS.to_uppercase()),
            ("altR", &sparse(&[(0, 2, "@")])),
            ("altL", &sparse(&[(0, 3, "#")])),
            ("ctrl", &sparse(&[(0, 4, "$")])),
        ],
    );
    let layout = adapted("sme", &yaml);
    assert!(layout.levels.uses_level3);
    assert_eq!(
        line(&layout, "AE02"),
        "    key <AE02> { type[Group1] = \"FOUR_LEVEL\", symbols[Group1] = [ 2, 2, at, VoidSymbol ] };"
    );
    assert_eq!(
        values(&layout, "AE03"),
        [
            Value::Char('3'),
            Value::Char('3'),
            Value::Char('3'),
            Value::Char('3'),
            Value::Empty,
            Value::Empty,
            Value::Empty,
            Value::Empty,
        ],
        "Left Alt's layer is not written"
    );
    assert_eq!(
        values(&layout, "AE04")[4],
        Value::Empty,
        "native layers are not"
    );
    assert!(
        layout
            .text
            .ends_with("    include \"level3(ralt_switch)\"\n};\n")
    );

    let alt = v4_layers(
        "sme",
        "Sámi",
        "",
        &[("none", ROWS), ("alt", &sparse(&[(0, 2, "@")]))],
    );
    let layout = adapted("sme", &alt);
    assert!(layout.levels.uses_level3, "alt layers are typed with AltGr");
    assert_eq!(values(&layout, "AE02")[4], Value::Char('@'));

    let left = v4_layers(
        "sme",
        "Sámi",
        "",
        &[
            ("none", ROWS),
            ("shift", &ROWS.to_uppercase()),
            ("altL", &sparse(&[(0, 3, "#")])),
        ],
    );
    let layout = adapted("sme", &left);
    assert!(!layout.levels.uses_level3);
    assert!(!layout.text.contains("level3"), "{}", layout.text);
    assert_eq!(
        line(&layout, "AE03"),
        "    key <AE03> { type[Group1] = \"ONE_LEVEL\", symbols[Group1] = [ 3 ] };"
    );
}

// [spec:kbdgen:sem:ldml.linux.levels+1/test]
#[test]
fn keys_without_any_value_hide_pc_keysyms() {
    let layout = adapted("sme", &v4_linux("sme", "Sámi", "", &sparse(&[(2, 0, "a")])));
    assert_eq!(
        line(&layout, "LSGT"),
        "    key <LSGT> { type[Group1] = \"ONE_LEVEL\", symbols[Group1] = [ VoidSymbol ] };"
    );
    assert_eq!(
        line(&layout, "BKSL"),
        "    key <BKSL> { type[Group1] = \"ONE_LEVEL\", symbols[Group1] = [ VoidSymbol ] };"
    );
}

// [spec:kbdgen:sem:ldml.linux.values+1/test]
#[test]
fn values_become_legacy_unicode_or_void_keysyms() {
    let body = "deadKeys:\n  x: {standalone: x, compose: {a: ä}}\n  y: {standalone: '', compose: {a: ÿ}}\n  z: {standalone: ab, compose: {a: ž}}\n";
    let rows = sparse(&[
        (1, 0, "å"),
        (1, 1, "ǧ"),
        (1, 2, "ŋ̈"),
        (1, 3, "\\d{x}"),
        (1, 4, "\\d{y}"),
        (1, 5, "\\d{z}"),
        (1, 6, "ẞ"),
        (1, 7, "\\u{1B}"),
    ]);
    let layout = adapted("sme", &v4_linux("sme", "Sámi", body, &rows));
    let first = |name: &str| {
        let line = line(&layout, name);
        let start = line.find("[ ").unwrap() + 2;
        line[start..].split([',', ' ']).next().unwrap().to_owned()
    };
    let names: Vec<String> = [
        "AD01", "AD02", "AD03", "AD04", "AD05", "AD06", "AD07", "AD08", "AD09",
    ]
    .iter()
    .map(|name| first(name))
    .collect();
    assert_eq!(
        names,
        [
            "aring",
            "U01E7",
            "VoidSymbol",
            "x",
            "VoidSymbol",
            "VoidSymbol",
            "U1E9E",
            "Escape",
            "VoidSymbol"
        ]
    );
    assert_eq!(values(&layout, "AD03")[0], Value::Long("ŋ̈".to_owned()));
}

// [spec:kbdgen:sem:ldml.linux.values+1/test]
#[test]
fn keysyms_follow_libxkbcommon_names_and_case() {
    assert_eq!(Keysym::from_char('a'), Keysym(0x61));
    assert_eq!(Keysym::from_char('å').name(), "aring");
    assert_eq!(Keysym::from_char('ŋ').name(), "eng");
    assert_eq!(Keysym::from_char('—').name(), "emdash");
    assert_eq!(Keysym::from_char('ǧ'), Keysym(0x0100_01e7));
    assert_eq!(Keysym::from_char('\u{301}').name(), "U0301");
    assert_eq!(Keysym::from_char('😀').name(), "U1F600");
    assert_eq!(Keysym::from_char('\u{8}').name(), "BackSpace");
    assert_eq!(Keysym::from_char('\u{fdd0}'), Keysym::NO_SYMBOL);
    for name in ["aring", "U01E7", "dead_acute", "VoidSymbol", "KP_Separator"] {
        assert_eq!(Keysym::from_name(name).unwrap().name(), name);
    }
    assert_eq!(Keysym::from_name("Ooblique"), Keysym::from_name("Oslash"));
    assert_eq!(Keysym::from_name("U00E5"), Some(Keysym(0xe5)));
    assert_eq!(Keysym::from_name("U0001"), None);
    assert_eq!(Keysym(0xe5).to_char(), Some('å'));
    assert_eq!(Keysym(0x0100_01e7).to_char(), Some('ǧ'));
    assert_eq!(Keysym::KP_DECIMAL.to_char(), Some('.'));
    assert_eq!(Keysym::from_name("dead_acute").unwrap().to_char(), None);
    let upper = |c: char| Keysym::from_char(c).to_upper().to_char();
    assert_eq!(upper('ß'), Some('ẞ'));
    assert_eq!(upper('ı'), Some('I'));
    assert_eq!(upper('ς'), Some('Σ'));
    assert_eq!(upper('µ'), Some('Μ'));
    assert_eq!(upper('ÿ'), Some('Ÿ'));
    assert_eq!(upper('ǅ'), Some('Ǆ'));
    assert_eq!(upper('ᾀ'), Some('ᾈ'));
    assert_eq!(upper('ŉ'), Some('ŉ'));
    assert_eq!(upper('1'), Some('1'));
    let dead = Keysym::from_name("dead_acute").unwrap();
    assert_eq!(dead.to_upper(), dead);
    assert!(dead.is_dead());
    assert_eq!(Keysym::VOID_SYMBOL.to_upper(), Keysym::VOID_SYMBOL);
}

fn k(c: char) -> Keysym {
    Keysym::from_char(c)
}

// [spec:kbdgen:sem:ldml.linux.semantics+1/test]
#[test]
fn key_types_resolve_as_libxkbcommon_does() {
    let four = [k('a'), k('A'), k('æ'), k('Æ')];
    let plus = [k('a'), k('A'), k('æ'), k('Æ'), k('č')];
    let keypad = [Keysym::KP_DELETE, Keysym::KP_SEPARATOR];
    let cases: [(KeyType, &[Keysym], u8, Keysym); 14] = [
        (KeyType::OneLevel, &[k('a')], SHIFT | LOCK, k('A')),
        (KeyType::TwoLevel, &[k('a'), k('A')], LOCK, k('A')),
        (KeyType::TwoLevel, &[k('1'), k('!')], SHIFT | LOCK, k('!')),
        (KeyType::Alphabetic, &[k('a'), k('A')], SHIFT | LOCK, k('a')),
        (KeyType::Alphabetic, &[k('a'), k('A')], LOCK, k('A')),
        (KeyType::FourLevel, &four, LOCK | LEVEL3, k('Æ')),
        (KeyType::FourLevel, &four, LEVEL3, k('æ')),
        (
            KeyType::FourLevelSemialphabetic,
            &four,
            LOCK | LEVEL3,
            k('Æ'),
        ),
        (
            KeyType::FourLevelSemialphabetic,
            &four,
            SHIFT | LOCK,
            k('a'),
        ),
        (KeyType::FourLevelAlphabetic, &four, LOCK | LEVEL3, k('Æ')),
        (
            KeyType::FourLevelAlphabetic,
            &four,
            SHIFT | LOCK | LEVEL3,
            k('æ'),
        ),
        (KeyType::FourLevelPlusLock, &plus, LOCK, k('č')),
        (KeyType::FourLevelPlusLock, &plus, SHIFT | LOCK, k('A')),
        (
            KeyType::Keypad,
            &keypad,
            NUM_LOCK | SHIFT,
            Keysym::KP_SEPARATOR,
        ),
    ];
    for (key_type, levels, mods, expected) in cases {
        assert_eq!(
            key_type.resolve(levels, mods),
            expected,
            "{key_type:?} {mods:#b}"
        );
    }
    assert_eq!(
        KeyType::TwoLevel.resolve(&[k('a')], SHIFT),
        Keysym::NO_SYMBOL,
        "a missing level types nothing"
    );
}

/// The values of states (s, c, g) in [`XkbState::ALL`] order, `-` for
/// none.
fn states(values: [char; 8]) -> [Keysym; 8] {
    values.map(|c| {
        if c == '-' {
            Keysym::VOID_SYMBOL
        } else {
            Keysym::from_char(c)
        }
    })
}

// [spec:kbdgen:sem:ldml.linux.types+1/test]
#[test]
fn types_are_the_first_that_give_every_state() {
    use KeyType::*;
    let cases: [([char; 8], bool, KeyType); 9] = [
        (['a', 'A', 'A', 'A', '-', '-', '-', '-'], false, TwoLevel),
        (['a', 'A', 'A', 'a', '-', '-', '-', '-'], false, Alphabetic),
        (['1', '!', '1', '!', '-', '-', '-', '-'], false, TwoLevel),
        (['-', '-', '-', '-', '-', '-', '-', '-'], true, OneLevel),
        (
            ['q', 'Q', 'č', 'Q', '-', '-', '-', '-'],
            false,
            FourLevelPlusLock,
        ),
        (['a', 'A', 'A', 'A', 'æ', 'Æ', 'Æ', 'Æ'], true, FourLevel),
        (
            ['a', 'A', 'A', 'a', 'æ', 'Æ', 'Æ', 'Æ'],
            true,
            FourLevelSemialphabetic,
        ),
        (
            ['a', 'A', 'A', 'a', 'æ', 'Æ', 'Æ', 'æ'],
            true,
            FourLevelAlphabetic,
        ),
        (
            ['a', 'A', 'a', 'A', 'æ', 'Æ', 'æ', 'Æ'],
            true,
            FourLevelPlusLock,
        ),
    ];
    for (values, uses_level3, expected) in cases {
        let typed = choose(&states(values), uses_level3);
        assert_eq!(typed.key_type, expected, "{values:?}");
        assert!(typed.wrong.is_empty(), "{values:?}: {:?}", typed.wrong);
        assert_eq!(typed.levels.len(), expected.levels());
    }
    let typed = choose(&states(['m', 'M', 'M', 'm', 'µ', '-', 'µ', '-']), true);
    assert_eq!(typed.key_type, FourLevelSemialphabetic);
    assert_eq!(typed.wrong, [XkbState::ALL[6]], "Lock uppercases µ");
}

// [spec:kbdgen:sem:ldml.linux.decimal/test]
#[test]
fn decimal_selects_its_keypad_keysym_with_num_lock() {
    let cases = [
        ("'.'", "KP_Decimal", None),
        ("','", "KP_Separator", None),
        ("'·'", "periodcentered", None),
        ("'٫'", "U066B", None),
        ("'.,'", "KP_Decimal", Some(".,")),
    ];
    for (decimal, keysym, long) in cases {
        let body = format!("decimal: {decimal}\n");
        let layout = adapted("sme", &v4_linux("sme", "Sámi", &body, ROWS));
        let derived = layout.decimal.as_ref().unwrap();
        assert_eq!(derived.keysym.name(), keysym);
        assert_eq!(derived.long.as_deref(), long);
        assert_eq!(
            line(&layout, "KPDL"),
            format!(
                "    key <KPDL> {{ type[Group1] = \"KEYPAD\", symbols[Group1] = [ KP_Delete, {keysym} ] }};"
            )
        );
    }
    let layout = adapted("sme", &v4_linux("sme", "Sámi", "", ROWS));
    assert!(layout.decimal.is_none());
    assert!(!layout.text.contains("<KPDL>"));
}

// [spec:kbdgen:syn:linux.xkb.symbols/test]
// [spec:kbdgen:sem:linux.xkb.naming/test]
#[test]
fn symbols_file_has_one_default_section() {
    let yaml = v4_layers(
        "se",
        "Sámi \"giella\" \\ 1",
        "decimal: ','\n",
        &[("none", ROWS), ("shift", &ROWS.to_uppercase())],
    );
    let layouts = linux_layouts(&[("se-FI", &yaml)]);
    let tree = Tree::new(&ids(), &layouts).unwrap();
    let paths: Vec<&str> = tree.files.iter().map(|(path, _)| path.as_str()).collect();
    assert_eq!(paths, ["symbols/divvun-se-fi", "rules/evdev.xml"]);
    let text = &tree.files[0].1;
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(
        &lines[..4],
        [
            "// divvun-se-fi: generated by kbdgen from layouts/se-FI.yaml. Do not edit.",
            "default partial alphanumeric_keys",
            "xkb_symbols \"basic\" {",
            "    name[Group1] = \"Sámi \\\"giella\\\" \\\\ 1\";",
        ]
    );
    assert_eq!(
        lines[4],
        "    key <TLDE> { type[Group1] = \"ONE_LEVEL\", symbols[Group1] = [ section ] };"
    );
    assert_eq!(lines.len(), 4 + 49 + 1 + 1, "{text}");
    assert_eq!(
        &lines[lines.len() - 3..],
        [
            "    key <SPCE> { type[Group1] = \"ONE_LEVEL\", symbols[Group1] = [ space ] };",
            "    key <KPDL> { type[Group1] = \"KEYPAD\", symbols[Group1] = [ KP_Delete, KP_Separator ] };",
            "};",
        ]
    );
    assert!(text.ends_with("};\n"));
    for line in &lines[4..lines.len() - 1] {
        assert!(line.starts_with("    key <"), "{line}");
    }
}
