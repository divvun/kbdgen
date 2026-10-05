//! `ldml.migrate.defects`: each code on a small v3 layout, with what the
//! migrator does about it.

use super::*;

fn windows(layers: &[(&str, &str)], dead: &str) -> String {
    desktop("windows", layers, dead)
}

const ACUTE: &str = "transforms:\n  ´:\n    ' ': ´\n    a: á\n";

// [spec:kbdgen:req:ldml.migrate.defects/test]
#[test]
fn m01_absent_dead_key_is_dropped() {
    let m = migrate(&format!(
        "{}{ACUTE}  ˇ:\n    c: č\n",
        windows(
            &[("default", KEYS), ("shift", SHIFTED)],
            "  deadKeys:\n    default: ['ˇ', '´']\n"
        )
    ));
    let m01 = with_code(&m, Code::M01);
    assert_eq!(m01.len(), 1, "{:?}", m.defects);
    assert_eq!(m01[0].path, "windows.deadKeys.default[1]");
    assert_eq!(m01[0].token.as_deref(), Some("ˇ"));
    assert!(!m.blocked());
    assert!(yaml(&m).contains("\\d{´}") && !yaml(&m).contains("\\d{ˇ}"));
}

// [spec:kbdgen:req:ldml.migrate.defects/test]
#[test]
fn m02_dead_keys_of_missing_layer_dropped() {
    let m = migrate(&format!(
        "{}{ACUTE}",
        windows(
            &[("default", KEYS)],
            "  deadKeys:\n    caps+shift: ['´']\n    default: []\n"
        )
    ));
    let m02 = with_code(&m, Code::M02);
    assert_eq!(m02.len(), 1, "{:?}", m.defects);
    assert_eq!(m02[0].path, "windows.deadKeys.caps+shift");
    assert!(!m.blocked());
}

// [spec:kbdgen:req:ldml.migrate.defects/test]
#[test]
fn m03_orphan_root_is_kept_unreferenced() {
    let m = migrate(&format!(
        "{}{ACUTE}",
        windows(&[("default", KEYS), ("shift", SHIFTED)], "")
    ));
    assert_eq!(with_code(&m, Code::M03).len(), 1, "{:?}", m.defects);
    assert!(yaml(&m).contains("deadKeys:\n  ´:"));
    assert!(
        !m.blocked(),
        "the load warning is explained: {:?}",
        m.defects
    );
}

// [spec:kbdgen:req:ldml.migrate.defects/test]
#[test]
fn m04_dead_key_without_transform_blocks() {
    let m = migrate(&windows(
        &[("default", KEYS)],
        "  deadKeys:\n    default: ['´']\n",
    ));
    let m04 = with_code(&m, Code::M04);
    assert_eq!(m04.len(), 1, "{:?}", m.defects);
    assert!(m04[0].message.contains("plain key or a dead key"));
    assert!(m.blocked());
    assert_eq!(m.blocking_codes(), [Code::M04]);
}

// [spec:kbdgen:req:ldml.migrate.defects/test]
#[test]
fn m05_desktop_layer_miscount_blocks() {
    for keys in [format!("{KEYS} Š"), KEYS.replacen("§ ", "", 1)] {
        let m = migrate(&windows(&[("default", &keys)], ""));
        let m05 = with_code(&m, Code::M05);
        assert_eq!(m05.len(), 1, "{:?}", m.defects);
        assert_eq!(m05[0].path, "windows.primary.layers.default");
        assert!(m.blocked());
    }
}

// [spec:kbdgen:req:ldml.migrate.defects/test]
#[test]
fn m06_unbraced_escape_is_rewritten() {
    let keys = KEYS.replacen("§", "\\u00E1", 1);
    let m = migrate(&format!(
        "keyNames:\n  space: space\n  return: linnjá\\u00ADmolsun\n{}",
        windows(&[("default", &keys), ("shift", SHIFTED)], "")
    ));
    let m06 = with_code(&m, Code::M06);
    assert_eq!(m06.len(), 2, "{:?}", m.defects);
    assert!(yaml(&m).contains("return: linnjá\\u{00AD}molsun"));
    assert!(yaml(&m).contains("\\u{00E1} 1 2"));
    assert!(
        !m.blocked(),
        "v3's literal text is explained: {:?}",
        m.defects
    );
    let label = m.layout.as_ref().unwrap().return_label.clone();
    assert_eq!(label.as_deref(), Some("linnjá\u{AD}molsun"));
}

// [spec:kbdgen:req:ldml.migrate.defects/test]
#[test]
fn m07_v2_layout_blocks_with_no_text() {
    let m = migrate("modes:\n  mobile-default: q w e\nstrings:\n  space: space\n");
    assert_eq!(codes(&m), [Code::M07]);
    assert!(m.blocked() && m.yaml.is_none());
}

// [spec:kbdgen:req:ldml.migrate.defects/test]
#[test]
fn m08_multi_scalar_windows_dead_key_is_info() {
    let keys = KEYS.replacen("§", "\\u{A0}\\u{330}", 1);
    let m = migrate(&format!(
        "{}transforms:\n  '\\u{{A0}}\\u{{330}}':\n    ' ': '\\u{{A0}}\\u{{330}}'\n    l: 'l\\u{{330}}'\n",
        windows(
            &[("default", &keys), ("shift", SHIFTED)],
            "  deadKeys:\n    default: ['\\u{A0}\\u{330}']\n"
        )
    ));
    assert_eq!(with_code(&m, Code::M08).len(), 1, "{:?}", m.defects);
    assert_eq!(
        with_code(&m, Code::M14).len(),
        1,
        "the leaves were written raw"
    );
    assert!(yaml(&m).contains("\\d{\\u{A0}\\u{330}} 1 2"));
    assert!(!m.blocked(), "{:?}", m.defects);
}

// [spec:kbdgen:req:ldml.migrate.defects/test]
// [spec:kbdgen:req:ldml.migrate.caps-diff/test]
#[test]
fn m09_lists_caps_shift_letters_turning_uppercase() {
    let m = migrate(&windows(&[("default", KEYS), ("shift", SHIFTED)], ""));
    let m09 = with_code(&m, Code::M09);
    let caps_shift = m09
        .iter()
        .find(|d| d.message.contains("caps+shift"))
        .unwrap_or_else(|| panic!("{m09:?}"));
    assert!(
        caps_shift.message.contains("D01 \"q\" → \"Q\""),
        "{caps_shift}"
    );
    assert!(
        caps_shift.message.contains("E01 \"1\" → \"!\""),
        "CAPLOK inverted shift on digits too"
    );
    let diff = m
        .caps_diffs
        .iter()
        .find(|d| d.state.name() == "caps+shift")
        .unwrap();
    assert_eq!(diff.platform, "windows");
    assert!(diff.positions.contains(&"D01"));
    assert!(!m.blocked());
}

// [spec:kbdgen:req:ldml.migrate.defects/test]
#[test]
fn m10_names_every_unread_field() {
    let m = migrate(&format!(
        "{}  config:\n    languageName: Sámegiella\n    legacyName: Sami\n  space:\n    default: x\nandroid:\n  deadKeys:\n    default: ['´']\n  primary:\n    layers:\n      default: a\n  tablet-600:\n    layers:\n      default: a\nsentryDsn: x\n",
        windows(&[("default", KEYS), ("shift", SHIFTED)], "")
    ));
    let paths: Vec<&str> = with_code(&m, Code::M10)
        .iter()
        .map(|d| d.path.as_str())
        .collect();
    assert_eq!(
        paths,
        [
            "sentryDsn",
            "windows.space",
            "windows.config.languageName",
            "windows.config.legacyName",
            "android.deadKeys"
        ]
    );
    assert!(!m.blocked());
}

// [spec:kbdgen:req:ldml.migrate.defects/test]
#[test]
fn m11_counts_comment_lines_outside_blocks() {
    let m = migrate(
        "# heading\nandroid:\n  primary:\n    layers:\n      default: |\n        # a b\n        c '#'\n      shift: \"x # y\" # trailing\n  tablet-600:\n    layers:\n      default: a\n",
    );
    let m11 = with_code(&m, Code::M11);
    assert_eq!(m11.len(), 1, "{:?}", m.defects);
    assert!(m11[0].message.contains("lines 3, 10"), "{}", m11[0]);
}

// [spec:kbdgen:req:ldml.migrate.defects/test]
#[test]
fn m12_ios_and_android_dead_keys_differ() {
    let m = migrate(&format!(
        "iOS:\n  primary:\n    layers:\n      default: ´ ¨ a\n  deadKeys:\n    default: ['´']\nandroid:\n  primary:\n    layers:\n      default: ´ ¨ a\n  tablet-600:\n    layers:\n      default: a\n{ACUTE}  ¨:\n    a: ä\n"
    ));
    let m12 = with_code(&m, Code::M12);
    assert_eq!(m12.len(), 1, "{:?}", m.defects);
    assert!(
        m12[0].message.contains("only Android [\"¨\"]"),
        "{}",
        m12[0]
    );
    assert!(!m.blocked(), "{:?}", m.defects);
}

// [spec:kbdgen:req:ldml.migrate.defects/test]
#[test]
fn m13_macos_nul_now_means_no_key() {
    let keys = KEYS.replacen("§", "\\u{0}", 1);
    let m = migrate(&desktop(
        "macOS",
        &[("default", &keys), ("shift", SHIFTED)],
        "",
    ));
    let m13 = with_code(&m, Code::M13);
    assert_eq!(m13.len(), 1, "{:?}", m.defects);
    assert_eq!(m13[0].path, "macOS.primary.layers.default");
    assert!(!m.blocked(), "{:?}", m.defects);
}

// [spec:kbdgen:req:ldml.migrate.defects/test]
#[test]
fn m14_escapes_shown_raw_are_now_decoded() {
    let keys = KEYS.replacen("§", "\\u{E1}", 1);
    let m = migrate(&format!(
        "{}iOS:\n  primary:\n    layers:\n      default: \\u{{E1}} b\n",
        desktop("chromeOS", &[("default", &keys), ("shift", SHIFTED)], "")
    ));
    let paths: Vec<&str> = with_code(&m, Code::M14)
        .iter()
        .map(|d| d.path.as_str())
        .collect();
    assert_eq!(
        paths,
        [
            "chromeOS.primary.layers.default",
            "iOS.primary.layers.default"
        ]
    );
    assert!(!m.blocked(), "{:?}", m.defects);
}

// [spec:kbdgen:req:ldml.migrate.defects/test]
#[test]
fn m15_unaligned_ipad_alt_rows_are_dropped() {
    let m = migrate(
        "iOS:\n  iPad-9in:\n    layers:\n      default: |\n        q w\n        \\s{backspace} a\n      alt: |\n        1 2 3\n        @ #\n      alt+shift: |\n        !\n",
    );
    let m15 = with_code(&m, Code::M15);
    assert_eq!(m15.len(), 2, "{:?}", m.defects);
    assert!(m15.iter().any(|d| d.message.contains("row 1, row 2 key 1")));
    assert!(m15.iter().any(|d| d.message.contains("no shift layer")));
    assert!(yaml(&m).contains("s: |\n                \\u{0} \\u{0}\n                \\u{0} #\n"));
    assert!(!m.blocked(), "{:?}", m.defects);
}

// [spec:kbdgen:req:ldml.migrate.defects/test]
#[test]
fn m16_standalone_differing_from_root_is_info() {
    let keys = KEYS.replacen("§", "ˆ", 1);
    let m = migrate(&format!(
        "{}transforms:\n  ˆ:\n    ' ': '^'\n    a: â\n",
        windows(
            &[("default", &keys), ("shift", SHIFTED)],
            "  deadKeys:\n    default: ['ˆ']\n"
        )
    ));
    let m16 = with_code(&m, Code::M16);
    assert_eq!(m16.len(), 1, "{:?}", m.defects);
    assert!(yaml(&m).contains("  ˆ:\n    standalone: ^\n"));
    assert!(!m.blocked(), "{:?}", m.defects);
}

// [spec:kbdgen:req:ldml.migrate.defects/test]
// [spec:kbdgen:req:ldml.migrate.survey/test]
#[test]
fn codes_are_those_the_spec_lists() {
    let spec = include_str!("../../../../docs/spec/ldml/migrate.md");
    let listed: Vec<String> = spec
        .lines()
        .filter_map(|l| l.strip_prefix("> | M"))
        .map(|l| format!("M{}", &l[..2]))
        .collect();
    let ours: Vec<String> = Code::ALL
        .iter()
        .filter(|c| **c != Code::M99)
        .map(|c| c.name().to_string())
        .collect();
    assert_eq!(listed, ours);
    for code in Code::ALL {
        let blocks = matches!(code, Code::M04 | Code::M05 | Code::M07 | Code::M99);
        assert_eq!(code.blocks(), blocks, "{code}");
    }
}
