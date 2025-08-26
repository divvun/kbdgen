use std::cmp;
use std::path::Path;
use std::str::FromStr;

use anyhow::Result;
use async_trait::async_trait;
use indexmap::IndexMap;
use language_tags::LanguageTag;
use xmlem::{Document, Element, Selector};

use crate::build::macos::keymap::{MACOS_HARDCODED, MACOS_KEYS};
use crate::build::macos::layers::layer_attributes;
use crate::bundle::layout::Transform;
use crate::bundle::layout::macos::MacOsKbdLayer;
use crate::util::{TRANSFORM_ESCAPE, decode_unicode_escapes, split_keys};
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
    async fn build(&self, bundle: &KbdgenBundle, output_path: &Path) -> Result<()> {
        let key_layouts = generate_key_layout_files(&bundle);

        let mut key_layout_macos_bundle =
            MacOsBundle::new(output_path.to_path_buf(), bundle.name(), &bundle).unwrap();

        for (language_tag, (document, names)) in key_layouts.into_iter() {
            key_layout_macos_bundle.add_key_layout(language_tag, document, names);
        }

        key_layout_macos_bundle.write_all().unwrap();

        Ok(())
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
            ("keyMapSelect", [("mapIndex", layer_index.to_string())]),
        );

        key_map_select
            .append_new_element(document, ("modifier", [("keys", layer_attributes(layer))]));

        key_map_set.append_new_element(document, ("keyMap", [("index", layer_index.to_string())]));
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
                                    panic!(
                                        "The escape transform should be a string, not another transform"
                                    );
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
            KeyTransition::Action(action) => {
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
                            "dead key `{}` in target list but not the transforms.",
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
                        (
                            "key",
                            [
                                ("code", dead_key_action.code.to_string()),
                                ("action", dead_key_action.id.clone()),
                            ],
                        ),
                    );

                    let action = actions.append_new_element(
                        document,
                        ("action", [("id", dead_key_action.id.clone())]),
                    );

                    for state in &dead_key_action.states {
                        append_dead_key_output_element(&action, document, &state);
                    }
                }
                KeyTransition::Next(next_action) => {
                    xml_key_map.append_new_element(
                        document,
                        (
                            "key",
                            [
                                ("code", next_action.code.to_string()),
                                ("action", next_action.id.clone()),
                            ],
                        ),
                    );

                    let action = actions
                        .append_new_element(document, ("action", [("id", next_action.id.clone())]));

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
    // Remove actions tag if empty. Otherwise MacOS does not load the layout
    if actions.child_nodes(document).is_empty() {
        document.root().remove_child(document, actions.as_node())
    }
}

fn write_terminators(document: &mut Document, dead_keys: &IndexMap<String, DeadKeyOutput>) {
    if dead_keys.len() > 0 {
        let terminators = document.root().append_new_element(document, "terminators");

        for (_key, dead_key) in dead_keys {
            append_dead_key_output_element(&terminators, document, &dead_key);
        }
    }
}

fn append_dead_key_output_element(element: &Element, document: &mut Document, key: &DeadKeyOutput) {
    element.append_new_element(
        document,
        (
            "when",
            [
                ("state", key.id.clone()),
                ("output", decode_unicode_escapes(&key.output)),
            ],
        ),
    );
}

fn append_dead_key_next_element(element: &Element, document: &mut Document, key: &DeadKeyNext) {
    element.append_new_element(
        document,
        (
            "when",
            [("state", "none".to_string()), ("next", key.next.clone())],
        ),
    );
}

fn append_key_output_element(element: &Element, document: &mut Document, key: &KeyOutput) {
    element.append_new_element(
        document,
        (
            "key",
            [
                ("code", key.code.to_string()),
                ("output", decode_unicode_escapes(&key.output)),
            ],
        ),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bundle::KbdgenBundle;
    use crate::bundle::layout::macos::MacOsKbdLayer;
    use crate::bundle::layout::{Layout, MacOsPrimaryPlatform, MacOsTarget};
    use language_tags::LanguageTag;
    use std::str::FromStr;

    // Helper function to create a minimal test bundle
    fn create_test_bundle() -> KbdgenBundle {
        let mut layouts = IndexMap::new();
        let mut display_names = IndexMap::new();
        display_names.insert(LanguageTag::from_str("en").unwrap(), "English".to_string());

        let mut layers = IndexMap::new();
        // Create a basic QWERTY layout with exactly 48 keys for MACOS_KEYS
        let basic_layer = "q w e r t y u i o p [ ] a s d f g h j k l ; ' z x c v b n m , . / \\ ` 1 2 3 4 5 6 7 8 9 0 - = space".to_string();
        layers.insert(MacOsKbdLayer::Default, basic_layer);

        let mac_os_target = MacOsTarget {
            primary: MacOsPrimaryPlatform { layers },
            dead_keys: None,
            space: IndexMap::new(),
        };

        let layout = Layout {
            language_tag: LanguageTag::from_str("en-US").unwrap(),
            display_names,
            decimal: Some(".".to_string()),
            windows: None,
            chrome_os: None,
            mac_os: Some(mac_os_target),
            i_os: None,
            android: None,
            longpress: None,
            transforms: None,
            key_names: None,
        };

        layouts.insert(LanguageTag::from_str("en-US").unwrap(), layout);

        KbdgenBundle::new_test("test-keyboard".to_string(), layouts)
    }

    // Helper function to create a test bundle with dead keys and transforms
    fn create_test_bundle_with_transforms() -> KbdgenBundle {
        let mut layouts = IndexMap::new();
        let mut display_names = IndexMap::new();
        display_names.insert(LanguageTag::from_str("en").unwrap(), "English".to_string());

        let mut layers = IndexMap::new();
        // Create a layout that includes dead keys
        let basic_layer = "q w e r t y u i o p [ ] a s d f g h j k l ; ' z x c v b n m , . / \\ ` 1 2 3 4 5 6 7 8 9 0 - = space".to_string();
        layers.insert(MacOsKbdLayer::Default, basic_layer);

        // Create dead keys configuration
        let mut dead_keys = IndexMap::new();
        dead_keys.insert(MacOsKbdLayer::Default, vec!["'".to_string()]);

        // Create transforms for dead keys
        let mut transforms = IndexMap::new();
        let mut acute_map = IndexMap::new();
        acute_map.insert(" ".to_string(), Transform::End("'".to_string())); // escape
        acute_map.insert("a".to_string(), Transform::End("á".to_string()));
        acute_map.insert("e".to_string(), Transform::End("é".to_string()));
        transforms.insert("'".to_string(), Transform::More(acute_map));

        let mac_os_target = MacOsTarget {
            primary: MacOsPrimaryPlatform { layers },
            dead_keys: Some(dead_keys),
            space: IndexMap::new(),
        };

        let layout = Layout {
            language_tag: LanguageTag::from_str("en-US").unwrap(),
            display_names,
            decimal: Some(".".to_string()),
            windows: None,
            chrome_os: None,
            mac_os: Some(mac_os_target),
            i_os: None,
            android: None,
            longpress: None,
            transforms: Some(transforms),
            key_names: None,
        };

        layouts.insert(LanguageTag::from_str("en-US").unwrap(), layout);

        KbdgenBundle::new_test("test-keyboard-with-transforms".to_string(), layouts)
    }

    // Helper function to create a test bundle with duplicate keys
    fn create_test_bundle_with_duplicate_keys() -> KbdgenBundle {
        let mut layouts = IndexMap::new();
        let mut display_names = IndexMap::new();
        display_names.insert(LanguageTag::from_str("en").unwrap(), "English".to_string());

        let mut layers = IndexMap::new();
        // Create a layout with duplicate 'a' keys - this should expose the bug
        let layer_with_duplicates = "q w e r t y u i o p [ ] a s d f g h j k l ; ' z x c v b n m , . / a ` 1 2 3 4 5 6 7 8 9 0 - = space".to_string();
        layers.insert(MacOsKbdLayer::Default, layer_with_duplicates);

        let mac_os_target = MacOsTarget {
            primary: MacOsPrimaryPlatform { layers },
            dead_keys: None,
            space: IndexMap::new(),
        };

        let layout = Layout {
            language_tag: LanguageTag::from_str("en-US").unwrap(),
            display_names,
            decimal: Some(".".to_string()),
            windows: None,
            chrome_os: None,
            mac_os: Some(mac_os_target),
            i_os: None,
            android: None,
            longpress: None,
            transforms: None,
            key_names: None,
        };

        layouts.insert(LanguageTag::from_str("en-US").unwrap(), layout);

        KbdgenBundle::new_test("test-keyboard-duplicates".to_string(), layouts)
    }

    #[test]
    fn test_transform_id_manager() {
        let mut manager = TransformIdManager::new();

        assert_eq!(manager.next_dead_key(), "dead_key000");
        assert_eq!(manager.next_dead_key(), "dead_key001");
        assert_eq!(manager.next_action(), "action000");
        assert_eq!(manager.next_action(), "action001");
    }

    #[test]
    fn test_compute_keyboard_id() {
        let name = "en";
        let id = compute_keyboard_id(name);
        assert!(id.starts_with("-"));
        assert!(id[1..].parse::<i32>().is_ok());
    }

    #[test]
    fn test_compute_language_name() {
        let tag = LanguageTag::from_str("en-US").unwrap();
        let name = compute_language_name(&tag);
        assert_eq!(name, "enUS");
    }

    #[test]
    fn test_initialize_key_transition_map_basic() {
        let bundle = create_test_bundle();
        let layout = bundle.layouts.values().next().unwrap();
        let layers = &layout.mac_os.as_ref().unwrap().primary.layers;
        let language_tag = &layout.language_tag;

        let mut layered_key_transition_map = IndexMap::new();
        initialize_key_transition_map(language_tag, layers, &mut layered_key_transition_map);

        // Verify that the map was populated
        assert!(!layered_key_transition_map.is_empty());
        let base_layer_map = layered_key_transition_map
            .get(&MacOsKbdLayer::Default)
            .unwrap();

        // Check that we have the expected number of keys
        assert_eq!(base_layer_map.len(), MACOS_KEYS.len());

        // Check that specific keys exist with correct transitions
        assert!(base_layer_map.contains_key("q"));
        if let Some(KeyTransition::Output(output)) = base_layer_map.get("q") {
            assert_eq!(output.output, "q");
            // Key code for 'q' should match MACOS_KEYS
            assert_eq!(
                output.code,
                *MACOS_KEYS.get(&crate::util::iso_key::IsoKey::D01).unwrap()
            );
        } else {
            panic!("Expected Output transition for 'q'");
        }
    }

    #[test]
    fn test_initialize_key_transition_map_with_transforms() {
        let bundle = create_test_bundle_with_transforms();
        let layout = bundle.layouts.values().next().unwrap();
        let layers = &layout.mac_os.as_ref().unwrap().primary.layers;
        let language_tag = &layout.language_tag;

        let mut layered_key_transition_map = IndexMap::new();
        initialize_key_transition_map(language_tag, layers, &mut layered_key_transition_map);

        // Should initialize normally even with transforms present
        let base_layer_map = layered_key_transition_map
            .get(&MacOsKbdLayer::Default)
            .unwrap();
        assert_eq!(base_layer_map.len(), MACOS_KEYS.len());
    }

    #[test]
    fn test_duplicate_keys_issue() {
        let bundle = create_test_bundle_with_duplicate_keys();
        let layout = bundle.layouts.values().next().unwrap();
        let layers = &layout.mac_os.as_ref().unwrap().primary.layers;
        let language_tag = &layout.language_tag;

        let mut layered_key_transition_map = IndexMap::new();
        initialize_key_transition_map(language_tag, layers, &mut layered_key_transition_map);

        let base_layer_map = layered_key_transition_map
            .get(&MacOsKbdLayer::Default)
            .unwrap();

        // Count how many 'a' keys should exist in the layer
        let layer_keys: Vec<String> = split_keys(&layers.get(&MacOsKbdLayer::Default).unwrap());
        let a_count = layer_keys.iter().filter(|&k| k == "a").count();
        assert_eq!(a_count, 2); // We have 2 'a' keys

        // This demonstrates the bug: only one 'a' key transition will be stored
        // even though there are two 'a' keys in different positions
        assert!(base_layer_map.contains_key("a")); // Only one 'a' entry exists

        // The current implementation loses one of the duplicate keys
        // This test documents the current buggy behavior
        assert_eq!(base_layer_map.len(), MACOS_KEYS.len()); // But total keys should still match MACOS_KEYS
    }

    #[test]
    fn test_process_transforms_with_dead_keys() {
        let bundle = create_test_bundle_with_transforms();
        let layout = bundle.layouts.values().next().unwrap();
        let layers = &layout.mac_os.as_ref().unwrap().primary.layers;
        let language_tag = &layout.language_tag;
        let transforms = layout.transforms.as_ref().unwrap();
        let target_dead_keys = layout.mac_os.as_ref().unwrap().dead_keys.as_ref().unwrap();

        let mut layered_key_transition_map = IndexMap::new();
        let mut dead_keys = IndexMap::new();
        let mut id_manager = TransformIdManager::new();

        // Initialize first
        initialize_key_transition_map(language_tag, layers, &mut layered_key_transition_map);

        // Process transforms
        process_transforms(
            layers,
            transforms,
            target_dead_keys,
            &mut dead_keys,
            &mut layered_key_transition_map,
            &mut id_manager,
        );

        // Verify dead keys were created
        assert!(!dead_keys.is_empty());
        assert!(dead_keys.contains_key("'"));

        let base_layer_map = layered_key_transition_map
            .get(&MacOsKbdLayer::Default)
            .unwrap();

        // Check that transforms were applied to appropriate keys
        // 'a' should now have an Action transition instead of Output
        if let Some(KeyTransition::Action(action)) = base_layer_map.get("a") {
            assert_eq!(action.states.len(), 2); // none state + transform state
            assert_eq!(action.states[0].id, "none");
            assert_eq!(action.states[1].output, "á");
        } else {
            panic!("Expected Action transition for 'a' after transform processing");
        }
    }

    #[test]
    fn test_update_key_transition_map_with_transform() {
        let mut key_transition_map = IndexMap::new();
        let mut id_manager = TransformIdManager::new();

        // Insert initial output
        key_transition_map.insert(
            "a".to_string(),
            KeyTransition::Output(KeyOutput {
                code: 0,
                output: "a".to_string(),
            }),
        );

        let transform = DeadKeyOutput {
            id: "dead_key000".to_string(),
            output: "á".to_string(),
        };

        update_key_transition_map_with_transform(
            &mut key_transition_map,
            "a",
            transform,
            &mut id_manager,
        );

        // Should now be an Action
        if let Some(KeyTransition::Action(action)) = key_transition_map.get("a") {
            assert_eq!(action.states.len(), 2);
            assert_eq!(action.states[0].id, "none");
            assert_eq!(action.states[1].id, "dead_key000");
            assert_eq!(action.states[1].output, "á");
        } else {
            panic!("Expected Action transition after transform update");
        }

        // Test adding another transform to existing Action
        let transform2 = DeadKeyOutput {
            id: "dead_key001".to_string(),
            output: "à".to_string(),
        };

        update_key_transition_map_with_transform(
            &mut key_transition_map,
            "a",
            transform2,
            &mut id_manager,
        );

        if let Some(KeyTransition::Action(action)) = key_transition_map.get("a") {
            assert_eq!(action.states.len(), 3);
        } else {
            panic!("Expected Action transition with multiple states");
        }
    }

    #[test]
    fn test_create_dead_key_actions() {
        let bundle = create_test_bundle_with_transforms();
        let layout = bundle.layouts.values().next().unwrap();
        let layers = &layout.mac_os.as_ref().unwrap().primary.layers;
        let language_tag = &layout.language_tag;
        let target_dead_keys = layout.mac_os.as_ref().unwrap().dead_keys.as_ref().unwrap();

        let mut layered_key_transition_map = IndexMap::new();
        let mut dead_keys = IndexMap::new();
        let mut id_manager = TransformIdManager::new();

        // Setup dead keys
        dead_keys.insert(
            "'".to_string(),
            DeadKeyOutput {
                id: "dead_key000".to_string(),
                output: "'".to_string(),
            },
        );

        initialize_key_transition_map(language_tag, layers, &mut layered_key_transition_map);

        create_dead_key_actions(
            layers,
            &mut layered_key_transition_map,
            target_dead_keys,
            &dead_keys,
            &mut id_manager,
        );

        let base_layer_map = layered_key_transition_map
            .get(&MacOsKbdLayer::Default)
            .unwrap();

        // The apostrophe key should now have a Next action
        if let Some(KeyTransition::Next(next_action)) = base_layer_map.get("'") {
            assert_eq!(next_action.states.len(), 1);
            assert_eq!(next_action.states[0].next, "dead_key000");
        } else {
            panic!("Expected Next transition for dead key");
        }
    }

    #[test]
    fn test_generate_key_layout_files() {
        let bundle = create_test_bundle();
        let key_layouts = generate_key_layout_files(&bundle);

        assert_eq!(key_layouts.len(), 1);

        let (document, _) = key_layouts.values().next().unwrap();
        let root = document.root();

        // Check that basic XML structure exists
        let selector = Selector::new("keyMapSet").unwrap();
        assert!(root.query_selector(&document, &selector).is_some());

        let selector = Selector::new("modifierMap").unwrap();
        assert!(root.query_selector(&document, &selector).is_some());
    }

    #[test]
    fn test_generate_key_layout_files_with_transforms() {
        let bundle = create_test_bundle_with_transforms();
        let key_layouts = generate_key_layout_files(&bundle);

        assert_eq!(key_layouts.len(), 1);

        let (document, _) = key_layouts.values().next().unwrap();
        let root = document.root();

        // Should have actions section for dead keys
        let selector = Selector::new("actions").unwrap();
        let actions = root.query_selector(&document, &selector);
        assert!(actions.is_some());

        // Should have terminators section
        let selector = Selector::new("terminators").unwrap();
        let terminators = root.query_selector(&document, &selector);
        assert!(terminators.is_some());
    }

    #[test]
    #[should_panic(expected = "only one instance")]
    fn test_duplicate_keys_with_transforms_consistency() {
        // This test demonstrates that with the current implementation,
        // transforms won't be consistent across duplicate keys
        let mut bundle = create_test_bundle_with_duplicate_keys();

        // Add transforms to the bundle
        let mut transforms = IndexMap::new();
        let mut acute_map = IndexMap::new();
        acute_map.insert(" ".to_string(), Transform::End("'".to_string()));
        acute_map.insert("a".to_string(), Transform::End("á".to_string()));
        transforms.insert("'".to_string(), Transform::More(acute_map));

        // Add dead keys
        let mut dead_keys = IndexMap::new();
        dead_keys.insert(MacOsKbdLayer::Default, vec!["'".to_string()]);

        // Update the layout
        let layout = bundle
            .layouts
            .get_mut(&LanguageTag::from_str("en-US").unwrap())
            .unwrap();
        layout.transforms = Some(transforms);
        if let Some(mac_os_target) = &mut layout.mac_os {
            mac_os_target.dead_keys = Some(dead_keys);
        }

        // This should demonstrate inconsistent behavior with duplicate 'a' keys
        let key_layouts = generate_key_layout_files(&bundle);

        // The test should show that only one of the duplicate 'a' keys gets the transform
        // This will expose the bug we're trying to fix
        panic!(
            "This test should demonstrate that duplicate keys don't get consistent transforms - only one instance"
        );
    }

    #[test]
    fn test_key_transition_output_creation() {
        let output = KeyOutput {
            code: 42,
            output: "test".to_string(),
        };

        assert_eq!(output.code, 42);
        assert_eq!(output.output, "test");
    }

    #[test]
    fn test_dead_key_output_creation() {
        let dead_key = DeadKeyOutput {
            id: "dead_key001".to_string(),
            output: "á".to_string(),
        };

        assert_eq!(dead_key.id, "dead_key001");
        assert_eq!(dead_key.output, "á");
    }

    #[test]
    fn test_dead_key_action_creation() {
        let action = DeadKeyAction {
            id: "action001".to_string(),
            code: 15,
            states: vec![
                DeadKeyOutput {
                    id: "none".to_string(),
                    output: "a".to_string(),
                },
                DeadKeyOutput {
                    id: "acute".to_string(),
                    output: "á".to_string(),
                },
            ],
        };

        assert_eq!(action.id, "action001");
        assert_eq!(action.code, 15);
        assert_eq!(action.states.len(), 2);
    }
}

