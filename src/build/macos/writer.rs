//! Turns a keylayout input into the `.keylayout` XML text.

use std::str::FromStr;

use xmlem::{Document, Element, Selector, display::Config, display::EntityMode};

use super::input::{Binding, KeyMap, KeylayoutInput, When};
use super::util::keyboard_id;

// [spec:kbdgen:def:keylayout.document]
const LAYOUT_TEMPLATE: &str = include_str!("../../../resources/template-macos-layout.xml");

fn child(document: &Document, name: &str) -> Element {
    let selector = Selector::new(name).expect("a valid selector");
    document
        .root()
        .query_selector(document, &selector)
        .unwrap_or_else(|| panic!("the template has a '{name}' element"))
}

fn append_when(parent: &Element, document: &mut Document, when: &When) {
    let element = parent.append_new_element(document, ("when", [("state", when.state.clone())]));
    if let Some(output) = &when.output {
        element.set_attribute(document, "output", output);
    }
    if let Some(next) = &when.next {
        element.set_attribute(document, "next", next);
    }
}

/// Appends one key map's `<keyMapSelect>`, `<keyMap>` and the `<action>`
/// of each of its action keys, in key order. A map no modifiers select,
/// reached only as the default index, has no `<keyMapSelect>`, which the
/// DTD requires to hold a `<modifier>`.
// [spec:kbdgen:req:keylayout.keymaps]
// [spec:kbdgen:req:keylayout.keymaps.tokens]
// [spec:kbdgen:sem:keylayout.actions]
fn append_map(document: &mut Document, index: usize, map: &KeyMap, actions: &Element) {
    let modifier_map = child(document, "modifierMap");
    let key_map_set = child(document, "keyMapSet");
    if !map.modifiers.is_empty() {
        let select = modifier_map.append_new_element(
            document,
            ("keyMapSelect", [("mapIndex", index.to_string())]),
        );
        for keys in &map.modifiers {
            select.append_new_element(document, ("modifier", [("keys", keys.clone())]));
        }
    }
    let key_map =
        key_map_set.append_new_element(document, ("keyMap", [("index", index.to_string())]));
    for key in &map.keys {
        let code = key.code.to_string();
        match &key.binding {
            Binding::Output(output) => {
                key_map.append_new_element(
                    document,
                    ("key", [("code", code), ("output", output.clone())]),
                );
            }
            Binding::Action { id, whens } => {
                key_map.append_new_element(
                    document,
                    ("key", [("code", code), ("action", id.clone())]),
                );
                let action = actions.append_new_element(document, ("action", [("id", id.clone())]));
                for when in whens {
                    append_when(&action, document, when);
                }
            }
        }
    }
}

/// The `.keylayout` text of `input`: pretty-printed with two-space indent
/// and hex entities, the `<actions>` element left out when it would be
/// empty and `<terminators>` written last when any dead-key state exists.
// [spec:kbdgen:def:ldml.macos.input]
// [spec:kbdgen:def:keylayout.document]
// [spec:kbdgen:req:keylayout.transforms]
// [spec:kbdgen:req:macbundle.plist.bundle-resources]
pub fn write(input: &KeylayoutInput) -> String {
    let mut document = Document::from_str(LAYOUT_TEMPLATE).expect("invalid template");
    let root = document.root();
    root.set_attribute(&mut document, "id", &keyboard_id(&input.name));
    root.set_attribute(&mut document, "name", &input.name);
    child(&document, "modifierMap").set_attribute(
        &mut document,
        "defaultIndex",
        &input.default_index.to_string(),
    );
    let actions = child(&document, "actions");
    for (index, map) in input.maps.iter().enumerate() {
        append_map(&mut document, index, map, &actions);
    }
    if actions.child_nodes(&document).is_empty() {
        root.remove_child(&mut document, actions.as_node());
    }
    if !input.terminators.is_empty() {
        let terminators = root.append_new_element(&mut document, "terminators");
        for when in &input.terminators {
            append_when(&terminators, &mut document, when);
        }
    }
    let config = Config::default_pretty().entity_mode(EntityMode::Hex);
    document.to_string_pretty_with_config(&config)
}
