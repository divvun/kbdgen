//! Each rule of `ldml.kbdl.*` on small v4 layouts.

use kbd_engine::{Action, BackspacePolicy, Context, KeyEvent, Model, Options, OutputForm, State};

use crate::build::windows::kbdl::{
    input::{DeadKeyNode, ExtraModifierKey},
    tables::{DKF_DEAD, EXTRA_MODIFIER_VKS, KBDEXT, POSITION_KEYS},
};

use super::super::layers::{Selection, layer_states};
use super::*;

fn windows(layers: &str) -> String {
    sme(&format!("hardware:\n  windows:\n    layers:\n{layers}"))
}

fn layer(name: &str, rows: &str) -> String {
    format!("      {name}: |\n        {rows}")
}

// [spec:kbdgen:sem:ldml.kbdl.layers/test]
#[test]
fn ctrl_column_comes_from_the_native_ctrl_layer() {
    let yaml = windows(&format!(
        "{}{}{}",
        layer("none", &iso_rows("§")),
        layer("ctrl", &iso_rows("\\u{1B}")),
        layer("cmd", &iso_rows("c"))
    ));
    let input = adapted("sme", &yaml).input;
    assert_eq!(
        value(&input, Layer::Ctrl, "E00"),
        Some(KeyValue::new("\u{1B}"))
    );
    assert_eq!(value(&input, Layer::Ctrl, "D01"), Some(KeyValue::new("q")));
    let (tables, _) = tables_of(&input);
    let row = tables.rows.iter().find(|row| row.vk == 0xc0).unwrap();
    assert_eq!(row.wch[2], 0x1b);

    let yaml = windows(&format!(
        "{}{}",
        layer("none", &iso_rows("§")),
        layer("cmd", &iso_rows("c"))
    ));
    let input = adapted("sme", &yaml).input;
    assert!(
        !input.layers.contains_key(&Layer::Ctrl),
        "cmd is native-only but is not Ctrl"
    );
    assert!(!input.layers.contains_key(&Layer::AltShift));
}

// [spec:kbdgen:sem:ldml.kbdl.layers/test]
#[test]
fn altgr_reaches_ctrl_alt_and_other_layers() {
    let yaml = windows(&format!(
        "{}{}{}",
        layer("none", &iso_rows("§")),
        layer("ctrl alt", &iso_rows("@")),
        layer("other", &iso_rows("o"))
    ));
    let input = adapted("sme", &yaml).input;
    assert_eq!(value(&input, Layer::Alt, "E00"), Some(KeyValue::new("@")));
    assert_eq!(value(&input, Layer::Shift, "E00"), Some(KeyValue::new("o")));
    assert_eq!(
        value(&input, Layer::AltShift, "E00"),
        Some(KeyValue::new("o"))
    );
    assert!(
        !input.layers.contains_key(&Layer::Ctrl),
        "Other never serves a native-only state"
    );
}

// [spec:kbdgen:sem:ldml.kbdl.layers/test]
// [spec:kbdgen:req:ldml.kbdl.windows-inputs/test]
#[test]
fn extra_modifiers_select_their_own_columns() {
    let yaml = sme(&format!(
        "hardware:\n  windows:\n    extraModifiers: [rightCtrl, capsLock]\n    layers:\n{}{}{}{}",
        layer("none", &iso_rows("§")),
        layer("shift", &iso_rows("½")),
        layer("extra1", &iso_rows("x")),
        layer("extra2 shift", &iso_rows("Y"))
    ));
    let input = adapted("sme", &yaml).input;
    assert_eq!(
        input.extra_modifiers,
        [ExtraModifierKey::RightCtrl, ExtraModifierKey::CapsLock]
    );
    assert_eq!(
        value(&input, Layer::Extra(0), "E00"),
        Some(KeyValue::new("x"))
    );
    assert!(!input.layers.contains_key(&Layer::ExtraShift(0)));
    assert!(!input.layers.contains_key(&Layer::Extra(1)));
    assert_eq!(
        value(&input, Layer::ExtraShift(1), "E00"),
        Some(KeyValue::new("Y"))
    );
    assert_eq!(
        value(&input, Layer::Caps, "E00"),
        Some(KeyValue::new("§")),
        "a capsLock binding clears Caps before selection"
    );
    let (tables, _) = tables_of(&input);
    assert_eq!(tables.columns, 9);
    assert_eq!(tables.vk_to_bits[3], (EXTRA_MODIFIER_VKS[0], 0x08));
    assert!(
        tables
            .e0_vsc_to_vk
            .contains(&(0x1d, u16::from(EXTRA_MODIFIER_VKS[0]) | KBDEXT))
    );
    assert_eq!(tables.aus_vk[0x3a], u16::from(EXTRA_MODIFIER_VKS[1]));
    let row = tables.rows.iter().find(|row| row.vk == 0xc0).unwrap();
    assert_eq!(
        row.wch[5..9],
        [u16::from(b'x'), 0xf000, 0xf000, u16::from(b'Y')]
    );
}

// [spec:kbdgen:sem:ldml.kbdl.positions/test]
// [spec:kbdgen:req:ldml.kbdl.windows-inputs/test]
#[test]
fn abnt2_form_fills_the_49th_key() {
    let rows = iso_rows("§").replace(". /\n", ". / ?\n");
    let yaml = sme(&format!(
        "hardware:\n  windows:\n    form: abnt2\n    layers:\n{}",
        layer("none", &rows)
    ));
    let input = adapted("sme", &yaml).input;
    assert_eq!(
        value(&input, Layer::Default, "B11"),
        Some(KeyValue::new("?"))
    );
    assert_eq!(
        value(&input, Layer::Default, "B00"),
        Some(KeyValue::new("<"))
    );
    let (tables, _) = tables_of(&input);
    let row = tables.rows.iter().find(|row| row.vk == 0xc1).unwrap();
    assert_eq!(row.wch[0], u16::from(b'?'));

    let input = adapted("sme", &windows(&layer("none", &iso_rows("§")))).input;
    assert_eq!(value(&input, Layer::Default, "B11"), None);
    let (tables, _) = tables_of(&input);
    assert!(!tables.rows.iter().any(|row| row.vk == 0xc1));
}

// [spec:kbdgen:sem:ldml.kbdl.positions/test]
#[test]
fn keys_outside_the_49_positions_warn() {
    let rows = "§ 1 2 3 4 5 6 7 8 9 0 - = ¥\n        q w e r t y u i o p å ¨\n        a s d f g h j k l ö æ '\n        z x c v b n m , . / \\\n";
    let yaml = sme(&format!(
        "hardware:\n  windows:\n    form: jis\n    layers:\n{}    space: {{none: '\\u{{A0}}'}}\n",
        layer("none", rows)
    ));
    let adapted = adapted("sme", &yaml);
    let warnings = adapted.diag.warnings();
    assert!(
        warnings
            .iter()
            .any(|w| w.contains("scan code 7D") && w.contains("layer default")),
        "{warnings:?}"
    );
    assert!(
        warnings
            .iter()
            .any(|w| w.contains("space bar in layer default")),
        "{warnings:?}"
    );
    let input = adapted.input;
    assert_eq!(
        value(&input, Layer::Default, "B11"),
        Some(KeyValue::new("\\"))
    );
    assert_eq!(value(&input, Layer::Default, "B00"), None);
}

fn chained() -> String {
    sme(&format!(
        "deadKeys:\n  ´:\n    compose:\n      a: á\n      ¨: {{standalone: ¨, compose: {{u: ǘ, '-': {{standalone: '-', compose: {{u: ǖ}}}}}}}}\nhardware:\n  windows:\n    layers:\n{}",
        layer("none", &iso_rows("\\d{´}"))
    ))
}

// [spec:kbdgen:sem:ldml.kbdl.dead-tree/test]
#[test]
fn chained_dead_keys_become_dkf_dead_entries() {
    let input = adapted("sme", &chained()).input;
    let DeadKeyNode::Branch(acute) = &input.dead_key_tree["´"] else {
        panic!("´ is a branch");
    };
    let DeadKeyNode::Branch(diaeresis) = &acute["¨"] else {
        panic!("´ ¨ is a chained state");
    };
    assert_eq!(diaeresis["u"], DeadKeyNode::Leaf("ǘ".into()));
    assert_eq!(diaeresis[" "], DeadKeyNode::Leaf("¨".into()));
    let DeadKeyNode::Branch(macron) = &diaeresis["-"] else {
        panic!("´ ¨ - is a chained state of depth three");
    };
    assert_eq!(macron["u"], DeadKeyNode::Leaf("ǖ".into()));
    let (tables, _) = tables_of(&input);
    let chain = |base: char, id: char, composed: char| {
        tables.dead_keys.iter().any(|entry| {
            entry.base == base as u16
                && entry.id == id as u16
                && entry.composed == composed as u16
                && entry.flags == DKF_DEAD
        })
    };
    assert!(chain('¨', '´', '¨'));
    assert!(chain('-', '¨', '-'));
    assert!(
        tables
            .dead_keys
            .iter()
            .any(|e| e.id == '-' as u16 && e.base == u16::from(b'u') && e.composed == 'ǖ' as u16)
    );
}

fn explicit_dead_key(extra: &str) -> String {
    sme(&format!(
        "keys:\n  dk: {{output: '\\m{{x}}'}}\n{extra}hardware:\n  windows:\n    layers:\n{}",
        layer("none", &iso_rows("\\k{dk}"))
    ))
}

// [spec:kbdgen:sem:ldml.kbdl.dead-tree/test]
#[test]
fn dead_key_cycles_are_fatal() {
    let yaml = explicit_dead_key(
        "displays: [{output: '\\m{x}', display: '^'}]\ntransforms: [[{from: '\\m{x}a', to: '\\m{x}'}]]\n",
    );
    let error = adapt_error("sme", &yaml);
    assert!(
        error.contains("cycle") && error.contains("\\m{x}"),
        "{error}"
    );
}

// [spec:kbdgen:sem:ldml.kbdl.values/test]
#[test]
fn dead_identity_needs_a_display_or_flush() {
    let error = adapt_error("sme", &explicit_dead_key(""));
    assert!(
        error.contains("key E00 in layer default") && error.contains("\\m{x}"),
        "{error}"
    );
    let yaml = explicit_dead_key("displays: [{output: '\\m{x}', display: '^'}]\n");
    let input = adapted("sme", &yaml).input;
    assert_eq!(
        value(&input, Layer::Default, "E00"),
        Some(KeyValue::dead("^"))
    );
    let DeadKeyNode::Branch(children) = &input.dead_key_tree["^"] else {
        panic!("^ is a branch");
    };
    assert_eq!(
        children.keys().collect::<Vec<_>>(),
        [" "],
        "without a flush output the marker is dropped, so nothing composes"
    );
}

// [spec:kbdgen:sem:ldml.kbdl.values/test]
// [spec:kbdgen:req:ldml.kbdl.classify/test]
#[test]
fn mixed_outputs_warn_and_are_classified() {
    let yaml = sme(&format!(
        "keys:\n  mx: {{output: 'a\\m{{x}}'}}\n  two: {{output: '\\m{{x}}\\m{{y}}'}}\nhardware:\n  windows:\n    layers:\n{}",
        layer("none", &iso_rows("\\k{mx}").replace(" 1 ", " \\k{two} "))
    ));
    let adapted = adapted("sme", &yaml);
    assert_eq!(value(&adapted.input, Layer::Default, "E00"), None);
    assert_eq!(value(&adapted.input, Layer::Default, "E01"), None);
    assert!(
        adapted
            .diag
            .warnings()
            .iter()
            .any(|w| w.contains("key E00 in layer default") && w.contains("mixes"))
    );
    assert_eq!(adapted.classification.mixed_outputs.count, 4);
    assert_eq!(
        adapted.classification.mixed_outputs.examples[..2],
        ["E00 in default", "E01 in default"]
    );
}

// [spec:kbdgen:req:ldml.kbdl.classify/test]
#[test]
fn classification_counts_what_only_the_service_does() {
    let yaml = sme(&format!(
        "normalization: enabled\ntransforms:\n  - [{{from: ae, to: æ}}, {{from: '\\m{{x}}o', to: ø}}]\n  - reorder: [{{from: '\\u{{301}}', order: 10}}]\nbackspace: [[{{from: æ, to: ae}}]]\nhardware:\n  windows:\n    layers:\n{}",
        layer("none", &iso_rows("§"))
    ));
    let result = adapted("sme", &yaml);
    let c = &result.classification;
    assert_eq!(c.text_transforms.count, 1);
    assert_eq!(c.text_transforms.examples, ["group 1 rule 1: ae"]);
    assert_eq!(c.reorder_groups.count, 1);
    assert_eq!(c.backspace_rules.count, 1);
    assert!(c.normalization);
    let info = &result.diag.infos()[0];
    for needle in [
        "1 transform rule(s)",
        "1 reorder group(s)",
        "1 backspace rule(s)",
        "normalization is enabled",
    ] {
        assert!(info.contains(needle), "{info}");
    }

    let plain = adapted("sme", &windows(&layer("none", &iso_rows("§"))));
    assert_eq!(plain.classification, Classification::default());
    assert!(plain.diag.infos()[0].contains("reproduces the whole keyboard"));
}

// [spec:kbdgen:def:ldml.kbdl.adapter/test]
// [spec:kbdgen:req:ldml.kbdl.windows-inputs/test]
// [spec:kbdgen:req:ldml.kbdl.metadata/test]
#[test]
fn windows_options_reach_the_input() {
    let yaml = sme(&format!(
        "decimal: ','\ndeadKeys: {{´: {{name: AKUHTTA, compose: {{a: á}}}}}}\ntargets: {{windows: {{id: sme-x, shiftLock: true, lrmRlm: true, keyNames: {{Caps Lock: Stuorrabustávat}}}}}}\nhardware:\n  windows:\n    layers:\n{}",
        layer("none", &iso_rows("\\d{´}"))
    ));
    let input = adapted("sme", &yaml).input;
    assert_eq!(input.metadata.name, "kbdsme-x");
    assert!(input.shift_lock && input.lrm_rlm);
    assert_eq!(input.decimal.as_deref(), Some(","));
    assert_eq!(input.dead_key_names["´"], "AKUHTTA");
    assert_eq!(
        input.key_name_overrides[&KeyNameEntry {
            table: KeyNameTable::Normal,
            scan_code: 0x3a
        }],
        "Stuorrabustávat"
    );
    let (tables, _) = tables_of(&input);
    assert_eq!(tables.dead_key_names.len(), 1);
}

// [spec:kbdgen:def:ldml.kbdl.adapter/test]
#[test]
fn layouts_without_a_windows_document_have_no_output() {
    let yaml = sme(&format!(
        "hardware:\n  macOS:\n    layers:\n{}",
        layer("none", &iso_rows("§"))
    ));
    let fixture = fixture(&[("sme", &yaml)]);
    assert!(fixture.model_layout("sme").is_none());
    let (tag, path) = &fixture.bundle.v4_layouts[0];
    assert_eq!(layout_name(tag, path).unwrap(), None);
    assert!(
        crate::build::windows::kbdl::generate_bundle(&fixture.bundle)
            .unwrap()
            .is_empty()
    );
}

/// The state's selection, checked against the engine itself: for every
/// layer whose state the engine handles and every position, a plain value
/// is what the engine types for that scan code, a dead value leaves its
/// marker pending, and an absent layer passes.
// [spec:kbdgen:sem:ldml.kbdl.layers/test]
#[test]
fn selection_agrees_with_the_engine() {
    let fixture = fixture(&[("vro", VRO4)]);
    let layout = fixture.model_layout("vro").unwrap();
    let input = adapt(&fixture.bundle, &layout).unwrap().input;
    let keyboard = layout
        .layout
        .keyboard_for(kbd_model::Host::Windows)
        .unwrap();
    let options = Options {
        output_form: OutputForm::Nfc,
        backspace: BackspacePolicy::CancelOrPass,
        host: Some(kbd_model::Host::Windows),
    };
    let model = Model::from_keyboard(keyboard.clone(), options).unwrap();
    let mut checked = 0;
    for (layer, state, selection) in layer_states(0) {
        if selection == Selection::Native {
            continue;
        }
        for (position, (scan_code, _)) in POSITION_KEYS.iter().enumerate() {
            let event = KeyEvent::with(kbd_engine::Key::Scan(*scan_code), state);
            let (action, next) = model.key(&State::default(), &Context::default(), &event);
            let value = input.layers.get(&layer).map(|values| &values[position]);
            match (value, action) {
                (None, action) => assert_eq!(action, Action::Pass, "{layer}"),
                (Some(Some(value)), Action::Edit { insert, .. }) if !value.dead => {
                    assert_eq!(insert, value.text, "{layer} {}", POSITION_NAMES[position]);
                    checked += 1;
                }
                (Some(Some(_)), Action::Edit { insert, .. }) => {
                    assert!(insert.is_empty() && model.pending_markers(&next).len() == 1);
                }
                (Some(None), action) => assert!(
                    matches!(&action, Action::Pass)
                        || matches!(&action, Action::Edit { insert, .. } if insert.is_empty()),
                    "{layer} {}: {action:?}",
                    POSITION_NAMES[position]
                ),
                (value, action) => panic!("{layer}: {value:?} but {action:?}"),
            }
        }
    }
    assert!(checked > 200, "{checked}");
}
