use std::str::FromStr;
use std::{path::Path, sync::Arc};

use async_trait::async_trait;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use xmlem::{Document, Element, NewElement, Selector};

use crate::build::macos::keymap::{MACOS_HARDCODED, MACOS_KEYS};
use crate::build::macos::layers::layer_attributes;
use crate::bundle::layout::macos::MacOsKbdLayer;
use crate::bundle::layout::Transform;
use crate::util::{split_keys, TRANSFORM_ESCAPE};
use crate::{build::BuildStep, bundle::KbdgenBundle};

pub const KEY_LAYOUT_EXT: &str = "keylayout";

const TOP_FOLDER: &str = "Contents";
const RESOURCES_FOLDER: &str = "Resources";
const PLIST_FILENAME: &str = "Info.plist";

const PLIST_TEMPLATE: &str = include_str!("../../../resources/template-macos-plist.xml");
const LAYOUT_TEMPLATE: &str = include_str!("../../../resources/template-macos-layout.xml");

#[derive(Serialize, Deserialize)]
pub struct InfoPlist {
    #[serde(rename = "CFBundleIdentifier")]
    pub cf_bundle_identifier: String,
    #[serde(rename = "CFBundleName")]
    pub cf_bundle_name: String,
    #[serde(rename = "CFBundleVersion")]
    pub cf_bundle_version: String,
    #[serde(rename = "CFBundleShortVersionString")]
    pub cf_bundle_short_version_string: String,
}

#[derive(Debug)]
pub enum KeyTransition {
    Output(KeyOutput),
    Action(DeadKeyAction),
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

pub struct GenerateMacOs {}

#[async_trait(?Send)]
impl BuildStep for GenerateMacOs {
    async fn build(&self, bundle: Arc<KbdgenBundle>, output_path: &Path) {
        let contents_path = output_path.join(TOP_FOLDER);
        let cloned_contents_path = contents_path.clone();
        let resources_path = contents_path.join(RESOURCES_FOLDER);

        std::fs::create_dir_all(contents_path).unwrap();
        std::fs::create_dir_all(resources_path.clone()).unwrap();

        let mut plist: InfoPlist = plist::from_bytes(PLIST_TEMPLATE.as_bytes()).unwrap();
        println!(
            "what's my CFBundleIdentifier: {}",
            plist.cf_bundle_identifier
        );
        plist.cf_bundle_name = "MyAmazingKbdgenBundle".to_string();

        plist::to_file_xml(cloned_contents_path.join(PLIST_FILENAME), &plist).unwrap();

        // One .keylayout file in Resources folder per language with MacOS primary platform
        for (language_tag, layout) in &bundle.layouts {
            if let Some(mac_os_target) = &layout.mac_os {
                let mut id_manager = TransformIdManager::new();

                let layers = &mac_os_target.primary.layers;

                let dead_key_count = 0;
                let state_count = 0;

                let mut key_transition_map: IndexMap<String, KeyTransition> = IndexMap::new();
                let mut dead_keys: IndexMap<String, _> = IndexMap::new();

                let mut document = Document::from_str(LAYOUT_TEMPLATE).unwrap();

                let root = document.root();

                let selector = Selector::new("keyMapSet").unwrap();
                let key_map_set = root
                    .query_selector(&document, &selector)
                    .expect("The template document should have a 'keyMapSet' tag");

                add_layer_tags(&layers, &mut document, &key_map_set);

                let mut cursor = 0;
                for (_iso_key, code) in MACOS_KEYS.iter() {
                    for (layer_index, (layer, key_map)) in layers.iter().enumerate() {
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

                        key_transition_map.insert(
                            key_map[cursor].clone(),
                            KeyTransition::Output(KeyOutput {
                                code: *code,
                                output: key_map[cursor].clone(),
                            }),
                        );

                        if let Some(transforms) = &layout.transforms {
                            for (dead_key, value) in transforms {
                                match value {
                                    Transform::End(_character) => {
                                        tracing::error!(
                                            "Transform ended too soon for dead key {}",
                                            dead_key
                                        );
                                    }
                                    Transform::More(map) => {
                                        let escape_transform =
                                            map.get(TRANSFORM_ESCAPE).expect(&format!(
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
                                                        let transition = key_transition_map
                                                            .get_mut(&key_map[cursor])
                                                            .unwrap();

                                                        let key_transform = DeadKeyOutput {
                                                            id: id.clone(),
                                                            output: end_char.to_string(),
                                                        };

                                                        match transition {
                                                            KeyTransition::Output(output) => {
                                                                key_transition_map.insert(
                                                                    key_map[cursor].clone(),
                                                                    KeyTransition::Action(
                                                                        DeadKeyAction {
                                                                            id: id_manager
                                                                                .next_action(),
                                                                            code: *code,
                                                                            states: vec![
                                                                                key_transform
                                                                                    .clone(),
                                                                            ],
                                                                        },
                                                                    ),
                                                                );
                                                            }
                                                            KeyTransition::Action(
                                                                ref mut action,
                                                            ) => {
                                                                action
                                                                    .states
                                                                    .push(key_transform.clone());
                                                            }
                                                        };
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
                        }

                        let key = key_transition_map.get(&key_map[cursor]).unwrap();
                        let selector =
                            Selector::new(&format!("keyMap[index=\"{}\"]", layer_index)).unwrap();
                        let xml_key_map = key_map_set
                            .query_selector(&mut document, &selector)
                            .expect("keymap to have right index");

                        match key {
                            KeyTransition::Output(output_key) => {
                                append_key_output_element(&xml_key_map, &mut document, &output_key);
                            }
                            KeyTransition::Action(action) => {
                                xml_key_map.append_new_element(
                                    &mut document,
                                    NewElement {
                                        name: "key".into(),
                                        attrs: [
                                            ("code".into(), action.code.to_string()),
                                            ("action".into(), action.id.clone()),
                                        ]
                                        .into(),
                                    },
                                );
                            }
                        };
                    }

                    cursor += 1;
                }

                // Add all the hardcoded keys on top
                for (layer_index, (layer, _)) in layers.iter().enumerate() {
                    let selector =
                        Selector::new(&format!("keyMap[index=\"{}\"]", layer_index)).unwrap();
                    let xml_key_map = key_map_set
                        .query_selector(&mut document, &selector)
                        .expect("keymap to have right index");

                    // Why does Mac need these? nobody knows
                    for (code, output) in MACOS_HARDCODED.iter() {
                        let output_key = KeyOutput {
                            code: *code,
                            output: output.to_string(),
                        };

                        append_key_output_element(&xml_key_map, &mut document, &output_key);
                    }
                }

                let selector = Selector::new("actions").unwrap();
                let actions = root
                    .query_selector(&document, &selector)
                    .expect("The template document should have an 'actions' tag");

                for (key, transition) in key_transition_map {
                    //println!(
                    //    "{}: xx key: {:?}, transition: {:?}",
                    //    language_tag, key, transition
                    //);

                    match transition {
                        KeyTransition::Output(_) => {}
                        KeyTransition::Action(dead_key_action) => {
                            let action = actions.append_new_element(
                                &mut document,
                                NewElement {
                                    name: "action".into(),
                                    attrs: [("id".into(), dead_key_action.id.clone())].into(),
                                },
                            );

                            for state in dead_key_action.states {
                                append_dead_key_output_element(&action, &mut document, &state);
                            }
                        }
                    };
                }

                if dead_keys.len() > 0 {
                    let terminators = root.append_new_element(
                        &mut document,
                        NewElement {
                            name: "terminators".into(),
                            attrs: [].into(),
                        },
                    );

                    for (key, dead_key) in dead_keys {
                        append_dead_key_output_element(&terminators, &mut document, &dead_key);
                    }
                }

                let key_layout_path =
                    resources_path.join(format!("{}.{}", language_tag.to_string(), KEY_LAYOUT_EXT));
                std::fs::write(key_layout_path, document.to_string_pretty()).unwrap();
            }
        }
    }
}

fn compute_keyboard_id(language_name: &str) -> String {
    "-8045".to_string()
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
                attrs: [("mapIndex".into(), layer_attributes(layer))].into(),
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

fn append_dead_key_output_element(element: &Element, document: &mut Document, key: &DeadKeyOutput) {
    element.append_new_element(
        document,
        NewElement {
            name: "when".into(),
            attrs: [
                ("state".to_string(), key.id.clone()),
                ("output".to_string(), key.output.clone()),
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
                ("output".into(), key.output.clone()),
            ]
            .into(),
        },
    );
}

/*
    let document = Document::from_str(LAYOUT_TEMPLATE).unwrap();

    let doc_children = document.children().unwrap();
    let children = RefCell::borrow(&*doc_children);

    let root = document.root();

    let borrowed_root = RefCell::borrow(&*root);
    let modifier_map_elem = borrowed_root.find_child_tag_with_name("modifierMap").unwrap();

    let borrowed_modifier_map = RefCell::borrow(&*modifier_map_elem);
    let key_map_select = borrowed_modifier_map.find_child_tag_with_name("keyMapSelect").unwrap();
    let borrowed_key_map_select = RefCell::borrow(&*key_map_select);

    let modifier = Element::new_child(&key_map_select, "modifier").unwrap();
    {
        let el = modifier.borrow();
        el.add_attr(QName::without_namespace("keys"), "command?");
    }

    let key_layout_path =
        resources_path.join(format!("{}.{}", language_tag.to_string(), KEY_LAYOUT_EXT));
    std::fs::write(key_layout_path, document.to_string()).unwrap();
*/

// return str(-min(max(binascii.crc_hqx(name.encode("utf-8"), 0) // 2, 1), 32768,))
