use super::symbols::{adapted, sparse, sparse_rows};
use super::*;
use crate::build::linux::xkb::{Adapted, rewrites};

/// The messages of a layout, warnings first.
fn messages(layout: &Adapted) -> Vec<String> {
    let warnings = layout
        .diag
        .warnings()
        .iter()
        .map(|w| format!("warning: {w}"));
    let infos = layout.diag.infos().iter().map(|i| format!("info: {i}"));
    warnings.chain(infos).collect()
}

/// Asserts that the messages start with `prefixes`, one each, in order.
fn assert_messages(layout: &Adapted, prefixes: &[&str]) {
    let messages = messages(layout);
    assert_eq!(messages.len(), prefixes.len(), "{messages:#?}");
    for (message, prefix) in messages.iter().zip(prefixes) {
        assert!(
            message.starts_with(prefix),
            "{message}\nshould start with\n{prefix}"
        );
    }
}

// [spec:kbdgen:req:ldml.linux.classify+1/test]
#[test]
fn layouts_losing_nothing_get_one_message() {
    let yaml = v4_layers(
        "sme",
        "Sámi",
        "",
        &[("none", ROWS), ("shift", &ROWS.to_uppercase())],
    );
    let layout = adapted("sme", &yaml);
    assert_eq!(
        messages(&layout),
        ["info: the XKB layout types every feature of the keyboard"]
    );
}

// [spec:kbdgen:req:ldml.linux.classify+1/test]
// [spec:kbdgen:def:ldml.linux.rewrites/test]
#[test]
fn outputs_dead_keys_and_rewrites_are_reported_once() {
    let body = "decimal: '.,'\nnormalization: enabled\ndeadKeys:\n  ´: {standalone: ´, compose: {a: á}}\n  x: {standalone: x, compose: {o: ö}}\nlongPress: {a: á ä}\nkeys:\n  mx: {output: 'a\\m{x}'}\ntransforms:\n  - [{from: q, to: Q}, {from: w, to: W}]\n  - reorder: [{from: '\\u{301}', order: 20}]\nbackspace: [[{from: e}]]\n";
    let rows = "\\d{´} \\d{x} \\k{mx} ŋ̈ 4 5 6 7 8 9 0 + -\nq w e r t y u i o p å ¨\na s d f g h j k l ö æ '\n< z x c v b n m , . -";
    let shift = "½ ! \" # ¤ % & / ( ) = ? `\nQ W E R T Y U I O P Å ^\nA S D F G H J K L Ö Æ *\n> Z X C V B N M ; : _";
    let yaml = v4_layers("sme", "Sámi", body, &[("none", rows), ("shift", shift)]);
    let layout = adapted("sme", &yaml);
    let r = &layout.classification.rewrites;
    assert!(r.any());
    assert_eq!(r.text_transforms.count, 2);
    assert_eq!(r.reorder_groups.count, 1);
    assert_eq!(r.backspace_rules.count, 1);
    assert!(r.normalization);
    assert_eq!(*r, rewrites(&linux_layouts(&[("sme", &yaml)])[0].keyboard));
    assert_messages(
        &layout,
        &[
            "info: the XKB layout cannot type 1 output(s) of more than one scalar value (e.g. <AE03>: \"ŋ\\u{308}\"); the IBus engine types them",
            "info: the XKB layout cannot type 1 output(s) mixing text and markers (e.g. <AE02>: key mx); the IBus engine types them",
            "info: the XKB layout cannot type 1 dead key(s) without a dead keysym, which it types as their standalone (e.g. \\m{dk_0078}, typed as \"x\"); the IBus engine types them",
            "info: the XKB layout composes 95 dead-key result(s) (e.g. \\m{dk_00B4} then \\m{dk_00B4}: the engine types \"´\" and leaves \\m{dk_00B4} pending, the XKB layout types \"´\";",
            "info: the XKB layout cannot type 2 transform rule(s) that do not start with a marker (e.g. group 3 rule 1; group 3 rule 2); the IBus engine types them",
            "info: the XKB layout cannot type 1 reorder group(s) (e.g. group 4 (1 rules)); the IBus engine types them",
            "info: the XKB layout cannot type 1 backspace rule(s) besides the generated one (e.g. group 1 rule 1); the IBus engine types them",
            "info: the XKB layout cannot normalize its output; the IBus engine does",
            "info: the XKB layout cannot type the decimal \".,\", which is more than one scalar value, and types KP_Decimal; the IBus engine types it",
            "info: the XKB layout ignores the long-press, multi-tap and flick gestures of 1 key(s) (e.g. u-0061), which no hardware key makes",
        ],
    );
    let dead = layout
        .diag
        .infos()
        .iter()
        .find(|info| info.contains("dead-key result"))
        .unwrap();
    assert!(
        dead.ends_with(
            "because IBus's simple engine composes by the reference Compose table, drops a key that continues no sequence and keeps the dead key pending, where the engine types the dead key's standalone and the key; the IBus engine types them as the engine does"
        ),
        "{dead}"
    );
}

/// The rows of a form with one more key, scan code 70, at the end of the
/// top row.
const WIDE: [usize; 4] = [14, 12, 12, 11];

// [spec:kbdgen:req:ldml.linux.classify+1/test]
#[test]
fn layers_positions_and_types_are_reported_once() {
    let form = "    form:\n      id: wide\n      rows: ['29 02 03 04 05 06 07 08 09 0A 0B 0C 0D 70', '10 11 12 13 14 15 16 17 18 19 1A 1B', '1E 1F 20 21 22 23 24 25 26 27 28 2B', '56 2C 2D 2E 2F 30 31 32 33 34 35', '39']\n    extraModifiers: [rightCtrl]\n    impliedLayers: none\n";
    let wide = |row0: &str| {
        format!("{row0}\nq w e r t y u i o p å ¨\na s d f g h j k l ö æ '\n< z x c v b n m , . -")
    };
    let none = wide("§ 1 2 3 4 5 6 7 8 9 0 + ´ ¥");
    let shift = wide("½ ! \" # ¤ % & / ( ) = ? ` ¥").to_uppercase();
    let mut text = v4_layers(
        "sme",
        "Sámi",
        "",
        &[
            ("none", &none),
            ("shift", &shift),
            ("caps", &none.to_uppercase()),
            ("caps shift", &shift.replacen('M', "m", 1)),
            ("altR", &sparse_rows(&WIDE, &[(3, 7, "µ")])),
            ("altR caps", &sparse_rows(&WIDE, &[(3, 7, "µ")])),
            ("altL", &sparse_rows(&WIDE, &[(0, 1, "¹")])),
            ("extra1", &sparse_rows(&WIDE, &[(0, 1, "₁")])),
        ],
    );
    text = text.replace(
        "hardware:\n  linux:\n",
        &format!("hardware:\n  linux:\n{form}"),
    );
    let layout = adapted("sme", &text);
    assert_messages(
        &layout,
        &[
            "warning: nothing on Linux types 1 layer(s) that only Left Alt or a Left Ctrl selects (e.g. layer altL): the XKB layout leaves Left Alt and Left Ctrl to shortcuts, and the IBus engine passes their chords",
            "info: the XKB layout cannot type 1 key state(s) that no key type gives (e.g. <AB07> with Caps Lock+AltGr should type 'µ' (FOUR_LEVEL_SEMIALPHABETIC)); the IBus engine types them",
            "info: the XKB layout cannot type 1 layer(s) selected by an extra modifier (e.g. layer extra1); the IBus engine types them",
            "info: the XKB layout cannot type 4 output(s) at scan codes it does not write (e.g. scan code 70 in layer none; scan code 70 in layer shift;",
        ],
    );

    let alt = v4_layers(
        "sme",
        "Sámi",
        "",
        &[
            ("none", ROWS),
            ("shift", &ROWS.to_uppercase()),
            ("alt", &sparse(&[(0, 2, "@")])),
        ],
    );
    let messages_alt = messages(&adapted("sme", &alt));
    assert!(
        messages_alt.contains(
            &"info: Linux types 1 alt layer(s) (e.g. layer alt, alt caps) with AltGr alone; Left Alt stays Alt for shortcuts".to_owned()
        ),
        "{messages_alt:#?}"
    );
    let mods = adapted(
        "und",
        &fixture_text("crates/kbd-engine/tests/golden/layouts/und-x-mods.yaml"),
    );
    assert!(
        !messages(&mods).iter().any(|m| m.contains("alt layer")),
        "a ctrl alt layer is AltGr's on Windows too"
    );
}
