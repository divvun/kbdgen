use std::cmp;
use std::path::Path;
use std::str::FromStr;

use async_trait::async_trait;
use indexmap::IndexMap;
use language_tags::LanguageTag;
use xmlem::{Document, Element, NewElement, Selector};

use crate::build::macos::keymap::{MACOS_HARDCODED, MACOS_KEYS};
use crate::build::macos::layers::layer_attributes;
use crate::bundle::layout::macos::MacOsKbdLayer;
use crate::bundle::layout::Transform;
use crate::util::{decode_unicode_escapes, split_keys, TRANSFORM_ESCAPE};
use crate::{build::BuildStep, bundle::KbdgenBundle};

use super::macos_bundle::MacOsBundle;
use super::util::crc_hqx;

const LAYOUT_TEMPLATE: &str = include_str!("../../../resources/template-macos-layout.xml");

#[derive(Debug)]
pub enum KeyTransition {
    Output(KeyOutput),
    Action(DeadKeyAction),
    Next(DeadKeyNextAction),
}

#[derive(Debug, Clone)]
pub struct KeyOutput {
    code: usize,
    output: String,
}

#[derive(Debug, Clone)]
pub struct DeadKeyOutput {
    id: DeadKeyId,
    output: String,
}

#[derive(Debug, Clone)]
pub struct DeadKeyAction {
    id: ActionId,
    code: usize,
    states: Vec<DeadKeyOutput>,
}

#[derive(Debug, Clone)]
pub struct DeadKeyNext {
    next: DeadKeyId,
}

#[derive(Debug, Clone)]
pub struct DeadKeyNextAction {
    id: ActionId,
    code: usize,
    states: Vec<DeadKeyNext>,
}

type DeadKeyId = String;
type ActionId = String;

struct TransformIdManager {
    dead_key_counter: usize,
    action_counter: usize,
}

impl TransformIdManager {
    fn new() -> Self {
        TransformIdManager {
            dead_key_counter: 0,
            action_counter: 0,
        }
    }

    fn next_dead_key(&mut self) -> DeadKeyId {
        let old_counter = self.dead_key_counter;

        self.dead_key_counter = self.dead_key_counter + 1;

        format!("dead_key{:03}", old_counter)
    }

    fn next_action(&mut self) -> ActionId {
        let old_counter = self.action_counter;

        self.action_counter = self.action_counter + 1;

        format!("action{:03}", old_counter)
    }
}

pub struct GenerateMacOs;

fn generate_key_layout_files(
    bundle: &KbdgenBundle,
) -> IndexMap<LanguageTag, (Document, &IndexMap<LanguageTag, String>)> {
    let mut key_layouts = IndexMap::new();

    // One .keylayout file in Resources folder per language with MacOS primary platform
    for (language_tag, layout) in &bundle.layouts {
        if let Some(mac_os_target) = &layout.mac_os {
            let layers = &mac_os_target.primary.layers;

            let mut layered_key_transition_map: IndexMap<
                MacOsKbdLayer,
                IndexMap<String, KeyTransition>,
            > = IndexMap::new();
            let mut dead_keys: IndexMap<String, _> = IndexMap::new();

            let mut document = Document::from_str(LAYOUT_TEMPLATE).expect("invalid template");

            let root = document.root();

            let keyboard_name = compute_language_name(language_tag);
            let keyboard_id = compute_keyboard_id(&keyboard_name);
            root.set_attribute(&mut document, "id", &keyboard_id);
            root.set_attribute(&mut document, "name", &keyboard_name);

            let selector = Selector::new("keyMapSet").unwrap();
            let key_map_set = root
                .query_selector(&document, &selector)
                .expect("The template document should have a 'keyMapSet' tag");

            add_layer_tags(&layers, &mut document, &key_map_set);

            initialize_key_transition_map(&language_tag, &layers, &mut layered_key_transition_map);

            let mut id_manager = TransformIdManager::new();

            if let Some(transforms) = &layout.transforms {
                if let Some(target_dead_keys) = &mac_os_target.dead_keys {
                    process_transforms(
                        &layers,
                        transforms,
                        target_dead_keys,
                        &mut dead_keys,
                        &mut layered_key_transition_map,
                        &mut id_manager,
                    );
                } else {
                    tracing::warn!(
                        r#"No dead keys in {}:{}:{}"#,
                        language_tag.to_string(),
                        "MacOS",
                        "Primary",
                    );
                }
            } else {
                tracing::warn!(
                    r#"No transforms in {}:{}:{}"#,
                    language_tag.to_string(),
                    "MacOS",
                    "Primary",
                );
            }

            if let Some(_transforms) = &layout.transforms {
                if let Some(target_dead_keys) = &mac_os_target.dead_keys {
                    create_dead_key_actions(
                        &layers,
                        &mut layered_key_transition_map,
                        &target_dead_keys,
                        &mut dead_keys,
                        &mut id_manager,
                    );
                }
            }

            let decimal = layout.decimal.as_deref().unwrap_or(".");

            write_key_transition_map(
                &layers,
                &layered_key_transition_map,
                &mut document,
                &key_map_set,
                decimal,
            );

            write_terminators(&mut document, &dead_keys);

            key_layouts.insert(language_tag.clone(), (document, &layout.display_names));
        }
    }

    key_layouts
}

#[async_trait(?Send)]
impl BuildStep for GenerateMacOs {
    async fn build(&self, bundle: &KbdgenBundle, output_path: &Path) {
        let key_layouts = generate_key_layout_files(&bundle);

        let mut key_layout_macos_bundle =
            MacOsBundle::new(output_path.to_path_buf(), bundle.name(), &bundle).unwrap();

        for (language_tag, (document, names)) in key_layouts.into_iter() {
            key_layout_macos_bundle.add_key_layout(language_tag, document, names);
        }

        key_layout_macos_bundle.write_all().unwrap();
    }
}

fn compute_keyboard_id(language_name: &str) -> String {
    let crc = crc_hqx(language_name.as_bytes()) / 2;
    let crc = cmp::max(1, crc);
    let crc = cmp::min(crc, 32768);
    format!("-{}", crc)
}

fn compute_language_name(tag: &LanguageTag) -> String {
    tag.to_string().replace("-", "").replace("_", "")
}

fn add_layer_tags(
    layers: &IndexMap<MacOsKbdLayer, String>,
    document: &mut Document,
    key_map_set: &Element,
) {
    let root = document.root();

    let selector = Selector::new("modifierMap").unwrap();
    let modifier_map = root
        .query_selector(&document, &selector)
        .expect("The template document should have a 'modifierMap' tag");

    for (layer_index, (layer, _)) in layers.iter().enumerate() {
        let key_map_select = modifier_map.append_new_element(
            document,
            NewElement {
                name: "keyMapSelect".into(),
                attrs: [("mapIndex".into(), layer_index.to_string())].into(),
            },
        );

        key_map_select.append_new_element(
            document,
            NewElement {
                name: "modifier".into(),
                attrs: [("keys".into(), layer_attributes(layer))].into(),
            },
        );

        key_map_set.append_new_element(
            document,
            NewElement {
                name: "keyMap".into(),
                attrs: [("index".into(), layer_index.to_string())].into(),
            },
        );
    }
}

fn initialize_key_transition_map(
    language_tag: &LanguageTag,
    layers: &IndexMap<MacOsKbdLayer, String>,
    layered_key_transition_map: &mut IndexMap<MacOsKbdLayer, IndexMap<String, KeyTransition>>,
) {
    for (_layer_index, (layer, key_map)) in layers.iter().enumerate() {
        let mut cursor = 0;

        layered_key_transition_map.insert(*layer, IndexMap::new());

        let key_transition_map = layered_key_transition_map
            .get_mut(layer)
            .expect("getting back the value that was just inserted");

        for (_iso_key, key_code) in MACOS_KEYS.iter() {
            let key_map: Vec<String> = split_keys(&key_map);

            tracing::debug!(
                "iso len: {}; keymap len: {}",
                MACOS_KEYS.len(),
                key_map.len()
            );
            if MACOS_KEYS.len() > key_map.len() {
                panic!(
                    r#"Provided layer does not have enough keys, expected {} keys but`` got {}, in {}:{}:{}:{:?}: \n{:?}"#,
                    MACOS_KEYS.len(),
                    key_map.len(),
                    language_tag.to_string(),
                    "MacOS",
                    "Primary",
                    layer,
                    key_map
                );
            }

            let key = key_map[cursor].clone();

            key_transition_map.insert(
                key.clone(),
                KeyTransition::Output(KeyOutput {
                    code: *key_code,
                    output: key.clone(),
                }),
            );

            cursor += 1;
        }
    }
}

fn process_transforms(
    layers: &IndexMap<MacOsKbdLayer, String>,
    transforms: &IndexMap<String, Transform>,
    target_dead_keys: &IndexMap<MacOsKbdLayer, Vec<String>>,
    dead_keys: &mut IndexMap<String, DeadKeyOutput>,
    layered_key_transition_map: &mut IndexMap<MacOsKbdLayer, IndexMap<String, KeyTransition>>,
    id_manager: &mut TransformIdManager,
) {
    for (_layer_index, (layer, key_map)) in layers.iter().enumerate() {
        let mut cursor = 0;

        let layer_dead_keys = target_dead_keys.get(layer);

        if let Some(_layer_dead_keys) = layer_dead_keys {
            let key_transition_map = layered_key_transition_map
                .get_mut(layer)
                .expect("this map should be prefilled by now");

            for (_iso_key, _code) in MACOS_KEYS.iter() {
                let key_map: Vec<String> = split_keys(&key_map);

                for (dead_key, value) in transforms {
                    //if !layer_dead_keys.contains(dead_key) {
                    //    continue;
                    //}

                    match value {
                        Transform::End(_character) => {
                            tracing::error!("Transform ended too soon for dead key {}", dead_key);
                        }
                        Transform::More(map) => {
                            let escape_transform = map.get(TRANSFORM_ESCAPE).expect(&format!(
                                "The escape transform `{}` not found for dead key `{}`",
                                TRANSFORM_ESCAPE, &dead_key
                            ));

                            match escape_transform {
                                Transform::End(end_char) => {
                                    if !dead_keys.contains_key(dead_key) {
                                        let id = id_manager.next_dead_key();

                                        dead_keys.insert(
                                            dead_key.clone(),
                                            DeadKeyOutput {
                                                id,
                                                output: end_char.clone(),
                                            },
                                        );
                                    }
                                }
                                Transform::More(_transform) => {
                                    panic!("The escape transform should be a string, not another transform");
                                }
                            };

                            let dead_key_transform = dead_keys[dead_key].clone();
                            let id = dead_key_transform.id.clone();

                            for (next_char, transform) in map {
                                match transform {
                                    Transform::End(end_char) => {
                                        if next_char == TRANSFORM_ESCAPE {
                                            continue;
                                        }

                                        if next_char.to_string() == key_map[cursor] {
                                            let key_transform = DeadKeyOutput {
                                                id: id.clone(),
                                                output: end_char.to_string(),
                                            };

                                            update_key_transition_map_with_transform(
                                                key_transition_map,
                                                next_char,
                                                key_transform,
                                                id_manager,
                                            );

                                            break;
                                        }
                                    }
                                    Transform::More(_transform) => {
                                        todo!("Recursion required ahead");
                                    }
                                };
                            }
                        }
                    };
                }

                cursor += 1;
            }
        }
    }
}

fn update_key_transition_map_with_transform(
    key_transition_map: &mut IndexMap<String, KeyTransition>,
    key: &str,
    transform: DeadKeyOutput,
    id_manager: &mut TransformIdManager,
) {
    if key_transition_map.contains_key(key) {
        let entry = key_transition_map.get_mut(key).unwrap();

        match entry {
            KeyTransition::Output(output) => {
                let code = output.code;

                let none_state = DeadKeyOutput {
                    id: "none".to_string(),
                    output: output.output.clone(),
                };

                let action = DeadKeyAction {
                    id: id_manager.next_action(),
                    code,
                    states: vec![none_state, transform.clone()],
                };

                key_transition_map.insert(key.to_string(), KeyTransition::Action(action));
            }
            KeyTransition::Action(ref mut action) => {
                action.states.push(transform.clone());
            }
            KeyTransition::Next(_) => {
                panic!("Next states shouldn't exist yet!?!??!!??!?!");
            }
        };
    } else {
        panic!(
            "The key_transition_map must already have Output entries for all keys by this point."
        )
    }
}

fn create_dead_key_actions(
    layers: &IndexMap<MacOsKbdLayer, String>,
    layered_key_transition_map: &mut IndexMap<MacOsKbdLayer, IndexMap<String, KeyTransition>>,
    target_dead_keys: &IndexMap<MacOsKbdLayer, Vec<String>>,
    dead_keys: &IndexMap<String, DeadKeyOutput>,
    id_manager: &mut TransformIdManager,
) {
    for (_layer_index, (layer, key_map)) in layers.iter().enumerate() {
        let mut cursor = 0;

        let layer_dead_keys = target_dead_keys.get(layer);

        if let Some(layer_dead_keys) = layer_dead_keys {
            let key_transition_map = layered_key_transition_map
                .get_mut(layer)
                .expect("this map should be prefilled by now");

            for (_iso_key, key_code) in MACOS_KEYS.iter() {
                let key_map: Vec<String> = split_keys(&key_map);

                tracing::debug!(
                    "layer dead keys: {:?}, key: {}",
                    layer_dead_keys,
                    &key_map[cursor]
                );

                if layer_dead_keys.contains(&key_map[cursor]) {
                    if let Some(dead_key_in_list) = dead_keys.get(&key_map[cursor]) {
                        let none_state = DeadKeyNext {
                            next: dead_key_in_list.id.clone(),
                        };

                        let action = DeadKeyNextAction {
                            id: id_manager.next_action(),
                            code: key_code.clone(),
                            states: vec![none_state],
                        };

                        key_transition_map
                            .insert(key_map[cursor].clone(), KeyTransition::Next(action));
                    } else {
                        panic!(
                            "dead key {} in target list but not the transforms noooooo",
                            &key_map[cursor]
                        );
                    }
                }

                cursor += 1;
            }
        }
    }
}

fn write_key_transition_map(
    layers: &IndexMap<MacOsKbdLayer, String>,
    layered_key_transition_map: &IndexMap<MacOsKbdLayer, IndexMap<String, KeyTransition>>,
    document: &mut Document,
    key_map_set: &Element,
    decimal: &str,
) {
    let selector = Selector::new("actions").unwrap();
    let actions = document
        .root()
        .query_selector(&document, &selector)
        .expect("The template document should have an 'actions' tag");

    for (layer_index, (layer, _key_map)) in layers.iter().enumerate() {
        let selector = Selector::new(&format!("keyMap[index=\"{}\"]", layer_index)).unwrap();
        let xml_key_map = key_map_set
            .query_selector(document, &selector)
            .expect("keymap to have right index");

        let key_transition_map = layered_key_transition_map
            .get(layer)
            .expect("this map should be prefilled by now");

        for (_key, transition) in key_transition_map {
            match transition {
                KeyTransition::Output(output) => {
                    append_key_output_element(&xml_key_map, document, &output);
                }
                KeyTransition::Action(dead_key_action) => {
                    xml_key_map.append_new_element(
                        document,
                        NewElement {
                            name: "key".into(),
                            attrs: [
                                ("code".into(), dead_key_action.code.to_string()),
                                ("action".into(), dead_key_action.id.clone()),
                            ]
                            .into(),
                        },
                    );

                    let action = actions.append_new_element(
                        document,
                        NewElement {
                            name: "action".into(),
                            attrs: [("id".into(), dead_key_action.id.clone())].into(),
                        },
                    );

                    for state in &dead_key_action.states {
                        append_dead_key_output_element(&action, document, &state);
                    }
                }
                KeyTransition::Next(next_action) => {
                    xml_key_map.append_new_element(
                        document,
                        NewElement {
                            name: "key".into(),
                            attrs: [
                                ("code".into(), next_action.code.to_string()),
                                ("action".into(), next_action.id.clone()),
                            ]
                            .into(),
                        },
                    );

                    let action = actions.append_new_element(
                        document,
                        NewElement {
                            name: "action".into(),
                            attrs: [("id".into(), next_action.id.clone())].into(),
                        },
                    );

                    for state in &next_action.states {
                        append_dead_key_next_element(&action, document, &state);
                    }
                }
            };
        }

        for (key_code, output) in MACOS_HARDCODED.iter() {
            let key = KeyOutput {
                code: *key_code,
                output: output.to_string(),
            };

            append_key_output_element(&xml_key_map, document, &key);
        }

        // Special case for our favourite decimal key on the keypad
        let key = KeyOutput {
            code: 65,
            output: decimal.to_string(),
        };

        append_key_output_element(&xml_key_map, document, &key);

        // Special case for our favourite spacebar key
        let key = KeyOutput {
            code: 49,
            output: " ".to_string(),
        };

        append_key_output_element(&xml_key_map, document, &key);
    }
}

fn write_terminators(document: &mut Document, dead_keys: &IndexMap<String, DeadKeyOutput>) {
    if dead_keys.len() > 0 {
        let terminators = document.root().append_new_element(
            document,
            NewElement {
                name: "terminators".into(),
                attrs: [].into(),
            },
        );

        for (_key, dead_key) in dead_keys {
            append_dead_key_output_element(&terminators, document, &dead_key);
        }
    }
}

fn append_dead_key_output_element(element: &Element, document: &mut Document, key: &DeadKeyOutput) {
    element.append_new_element(
        document,
        NewElement {
            name: "when".into(),
            attrs: [
                ("state".to_string(), key.id.clone()),
                ("output".to_string(), decode_unicode_escapes(&key.output)),
            ]
            .into(),
        },
    );
}

fn append_dead_key_next_element(element: &Element, document: &mut Document, key: &DeadKeyNext) {
    element.append_new_element(
        document,
        NewElement {
            name: "when".into(),
            attrs: [
                ("state".to_string(), "none".to_string()),
                ("next".to_string(), key.next.clone()),
            ]
            .into(),
        },
    );
}

fn append_key_output_element(element: &Element, document: &mut Document, key: &KeyOutput) {
    element.append_new_element(
        document,
        NewElement {
            name: "key".into(),
            attrs: [
                ("code".into(), key.code.to_string()),
                ("output".into(), decode_unicode_escapes(&key.output)),
            ]
            .into(),
        },
    );
}
