//! The Võro layout: the v4 golden fixture adapted to `kbdl.input` against
//! the v3 bundle's Windows section, through the tables both produce.

use std::collections::BTreeSet;

use crate::build::windows::kbdl::{
    bundle,
    input::DeadKeyNode,
    tables::{CAPLOK, DeadKeyEntry, POSITION_KEYS, Row, SGCAPS, VK_DEAD_ROW},
};
use crate::bundle::layout::Layout as V3Layout;

use super::*;

fn vro3() -> LayoutInput {
    let layout: V3Layout = serde_yaml::from_str(VRO3).unwrap();
    let tag = layout.language_tag.clone();
    let mut layouts = IndexMap::new();
    layouts.insert(tag.clone(), layout);
    let bundle = KbdgenBundle::new_test("vro".into(), layouts);
    let layout = bundle.layouts.get(&tag).unwrap();
    let target = layout.windows.as_ref().unwrap();
    bundle::layout_input(&bundle, &tag, layout, target)
        .unwrap()
        .0
}

/// The fixture authors these cells differently from the v3 layout: `€`
/// for `¤`, a combining acute for a dead grave, `§` for a dead caron and a
/// dead `ę` for a dead acute.
const AUTHORED: [(Layer, &str); 4] = [
    (Layer::Shift, "E04"),
    (Layer::Shift, "E12"),
    (Layer::Caps, "E00"),
    (Layer::Caps, "E12"),
];

const LAYERS: [Layer; 8] = [
    Layer::Default,
    Layer::Shift,
    Layer::Ctrl,
    Layer::Alt,
    Layer::AltShift,
    Layer::Caps,
    Layer::CapsShift,
    Layer::AltCaps,
];

fn cell(input: &LayoutInput, layer: Layer, position: usize) -> Option<KeyValue> {
    input
        .layers
        .get(&layer)
        .and_then(|values| values[position].clone())
}

/// Why the v4 cell may differ from the v3 cell, if it may.
fn explained(v4: &LayoutInput, layer: Layer, index: usize) -> bool {
    let name = POSITION_NAMES[index];
    let ours = cell(v4, layer, index);
    match layer {
        Layer::Ctrl | Layer::AltShift => !v4.layers.contains_key(&layer),
        Layer::Alt => ours.is_none() && index >= position("C01"),
        Layer::CapsShift => ours == cell(v4, Layer::Shift, index),
        Layer::AltCaps => ours == cell(v4, Layer::Alt, index),
        _ => AUTHORED.contains(&(layer, name)),
    }
}

/// The input cells where v3 and v4 differ, each checked against the
/// documented differences.
fn input_differences(v3: &LayoutInput, v4: &LayoutInput) -> BTreeSet<(Layer, usize)> {
    let mut differences = BTreeSet::new();
    for layer in LAYERS {
        for (position, name) in POSITION_NAMES.iter().enumerate() {
            if cell(v3, layer, position) != cell(v4, layer, position) {
                assert!(
                    explained(v4, layer, position),
                    "{layer} {name}: v3 {:?}, v4 {:?}",
                    cell(v3, layer, position),
                    cell(v4, layer, position)
                );
                differences.insert((layer, position));
            }
        }
    }
    differences
}

/// The main row of each virtual key and the dead row after it.
fn rows_by_vk(tables: &Tables) -> IndexMap<u8, (&Row, Option<&Row>)> {
    let mut rows = IndexMap::new();
    for (i, row) in tables.rows.iter().enumerate() {
        if row.vk == VK_DEAD_ROW || rows.contains_key(&row.vk) {
            continue;
        }
        let dead = tables.rows.get(i + 1).filter(|next| next.vk == VK_DEAD_ROW);
        rows.insert(row.vk, (row, dead));
    }
    rows
}

const ATTRIBUTE_LAYERS: [Layer; 6] = [
    Layer::Default,
    Layer::Shift,
    Layer::Caps,
    Layer::Alt,
    Layer::AltShift,
    Layer::AltCaps,
];

const COLUMN_LAYERS: [Layer; 5] = [
    Layer::Default,
    Layer::Shift,
    Layer::Ctrl,
    Layer::Alt,
    Layer::AltShift,
];

// [spec:kbdgen:def:ldml.kbdl.adapter/test]
// [spec:kbdgen:sem:ldml.kbdl.layers/test]
// [spec:kbdgen:sem:ldml.kbdl.positions+2/test]
// [spec:kbdgen:sem:ldml.kbdl.values+1/test]
// [spec:kbdgen:sem:ldml.kbdl.dead-tree+2/test]
#[test]
fn vro_v4_tables_match_v3_where_they_overlap() {
    let v3 = vro3();
    let v4 = adapted("vro", VRO4).input;
    let differences = input_differences(&v3, &v4);
    for (layer, name) in AUTHORED {
        assert!(
            differences.contains(&(layer, position(name))),
            "{layer} {name}"
        );
    }
    let (t3, _) = tables_of(&v3);
    let (t4, _) = tables_of(&v4);
    let (r3, r4) = (rows_by_vk(&t3), rows_by_vk(&t4));
    assert_eq!(
        r3.keys().collect::<Vec<_>>(),
        r4.keys().collect::<Vec<_>>(),
        "both have the same keys in the same order"
    );
    let mut compared = 0;
    for (position, (_, vk)) in POSITION_KEYS.iter().enumerate().take(48) {
        let ((main3, dead3), (main4, dead4)) = (r3[vk], r4[vk]);
        for (column, layer) in COLUMN_LAYERS.iter().enumerate() {
            if differences.contains(&(*layer, position)) {
                continue;
            }
            compared += 1;
            assert_eq!(
                main3.wch[column], main4.wch[column],
                "{vk:#x} column {column}"
            );
            if let (Some(dead3), Some(dead4)) = (dead3, dead4) {
                assert_eq!(dead3.wch[column], dead4.wch[column], "dead id of {vk:#x}");
            }
        }
        let attributes_overlap = ATTRIBUTE_LAYERS
            .iter()
            .all(|layer| !differences.contains(&(*layer, position)));
        if attributes_overlap {
            assert_eq!(main3.attributes, main4.attributes, "attributes of {vk:#x}");
        }
    }
    assert!(compared > 150, "only {compared} cells overlap");
    for ligature in &t4.ligatures {
        assert!(t3.ligatures.contains(ligature), "{ligature:?}");
    }
    assert!(!t4.ligatures.is_empty());
    let entries3: BTreeSet<(u16, u16, u16, u16)> = t3.dead_keys.iter().map(key).collect();
    for entry in &t4.dead_keys {
        assert!(entries3.contains(&key(entry)), "{entry:?}");
    }
    assert_eq!(
        t4.dead_keys.len(),
        9,
        "´ a A, ˇ c C, ~ o O, each with space"
    );
}

fn key(entry: &DeadKeyEntry) -> (u16, u16, u16, u16) {
    (entry.base, entry.id, entry.composed, entry.flags)
}

fn branch<'a>(
    tree: &'a IndexMap<String, DeadKeyNode>,
    identity: &str,
) -> Vec<(&'a str, &'a DeadKeyNode)> {
    match &tree[identity] {
        DeadKeyNode::Branch(children) => children.iter().map(|(k, v)| (k.as_str(), v)).collect(),
        DeadKeyNode::Leaf(_) => panic!("{identity} is a leaf"),
    }
}

fn leaf(output: &str) -> DeadKeyNode {
    DeadKeyNode::Leaf(output.into())
}

// [spec:kbdgen:sem:ldml.kbdl.dead-tree+2/test]
// [spec:kbdgen:sem:ldml.kbdl.values+1/test]
#[test]
fn vro_dead_tree_comes_from_the_engine() {
    let input = adapted("vro", VRO4).input;
    let identities: Vec<&str> = input.dead_key_tree.keys().map(String::as_str).collect();
    assert_eq!(identities, ["ˇ", "´", "~", "ę"]);
    assert_eq!(
        value(&input, Layer::Default, "E12"),
        Some(KeyValue::dead("´"))
    );
    assert_eq!(value(&input, Layer::Alt, "E12"), Some(KeyValue::dead("´")));
    let acute = branch(&input.dead_key_tree, "´");
    assert_eq!(
        acute,
        [
            ("a", &leaf("á")),
            ("b", &leaf("b\u{301}")),
            ("A", &leaf("Á")),
            ("B", &leaf("B\u{301}")),
            (" ", &leaf("´")),
        ],
        "´ j and ´ ˇ are unmatched, so Windows' own fallback applies"
    );
    assert_eq!(
        branch(&input.dead_key_tree, "~"),
        [("o", &leaf("õ")), ("O", &leaf("Õ")), (" ", &leaf("~"))]
    );
    assert_eq!(
        branch(&input.dead_key_tree, "ę"),
        [("a", &leaf("á")), (" ", &leaf("ę"))]
    );
}

// [spec:kbdgen:sem:ldml.kbdl.caps/test]
#[test]
fn vro_caps_keeps_the_windows_convention() {
    let adapted = adapted("vro", VRO4);
    let input = &adapted.input;
    assert_eq!(value(input, Layer::Caps, "D01"), Some(KeyValue::new("Q")));
    assert_eq!(
        value(input, Layer::CapsShift, "D01"),
        Some(KeyValue::new("Q")),
        "the engine's Caps+Shift is uppercase"
    );
    let (tables, diag) = tables_of(input);
    let rows = rows_by_vk(&tables);
    let (q, _) = rows[&b'Q'];
    assert_eq!(q.attributes & CAPLOK, CAPLOK, "Caps+Shift on Q types q");
    assert_eq!(q.attributes & SGCAPS, 0);
    assert_eq!(
        tables.rows.iter().filter(|row| row.vk == b'Q').count(),
        1,
        "no SGCAPS row expresses Caps+Shift = Shift"
    );
    for name in ["E00", "E12"] {
        assert!(
            diag.warnings()
                .iter()
                .any(|w| w.contains(&format!("key {name}: SGCAPS"))),
            "{name}: caps differs from both default and shift on a dead key"
        );
    }
}

// [spec:kbdgen:req:ldml.kbdl.classify/test]
#[test]
fn vro_reports_long_dead_key_outputs() {
    let adapted = adapted("vro", VRO4);
    let classification = &adapted.classification;
    assert_eq!(classification.long_leaves.count, 2);
    assert_eq!(
        classification.long_leaves.examples,
        ["´ b → b\u{301}", "´ B → B\u{301}"]
    );
    assert_eq!(classification.text_transforms.count, 0);
    assert_eq!(classification.backspace_rules.count, 0);
    assert_eq!(classification.mixed_outputs.count, 0);
    assert!(!classification.normalization);
    assert_eq!(adapted.diag.infos().len(), 1);
    assert!(adapted.diag.infos()[0].contains("2 dead-key output(s)"));
    assert!(
        adapted
            .diag
            .warnings()
            .iter()
            .any(|w| w.contains("space bar in layer caps") && w.contains("a0")),
        "the NBSP of space: {{caps: \\u{{A0}}}} cannot reach the space row"
    );
}

// [spec:kbdgen:req:ldml.kbdl.metadata/test]
#[test]
fn vro_metadata_reads_targets_windows() {
    let metadata = adapted("vro", VRO4).input.metadata;
    assert_eq!(metadata.name, "kbdvro");
    assert_eq!(metadata.description, "Võro");
    assert_eq!(metadata.language_name, "Võro");
    assert_eq!(metadata.locale_name, "vro-Latn");
    assert_eq!(metadata.lcid, 0x2000);
    assert_eq!(metadata.company, "Test");
    assert_eq!(metadata.version.as_deref(), Some("1.0.0"));
    assert_eq!(metadata.build.as_deref(), Some("2"));
    assert_eq!(vro3().metadata.locale_name, metadata.locale_name);
}
