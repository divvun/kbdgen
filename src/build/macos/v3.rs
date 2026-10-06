//! The v3 path: a v3 layout's `macOS` section turned into the keylayout
//! input as the `keylayout.*` rules read its tokens.

use indexmap::IndexMap;
use language_tags::LanguageTag;

use super::input::{Binding, Key, KeyMap, KeylayoutInput, NONE_STATE, When};
use super::keymap::{MACOS_HARDCODED, MACOS_KEYS};
use super::layers::layer_attributes;
use super::util::keyboard_name;
use crate::bundle::layout::Layout;
use crate::bundle::layout::Transform;
use crate::bundle::layout::macos::MacOsKbdLayer;
use crate::util::{TRANSFORM_ESCAPE, decode_unicode_escapes, split_keys};

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

// [spec:kbdgen:sem:keylayout.actions]
impl TransformIdManager {
    fn new() -> Self {
        TransformIdManager {
            dead_key_counter: 0,
            action_counter: 0,
        }
    }

    fn next_dead_key(&mut self) -> DeadKeyId {
        let old_counter = self.dead_key_counter;

        self.dead_key_counter += 1;

        format!("dead_key{:03}", old_counter)
    }

    fn next_action(&mut self) -> ActionId {
        let old_counter = self.action_counter;

        self.action_counter += 1;

        format!("action{:03}", old_counter)
    }
}

// [spec:kbdgen:req:keylayout.keymaps.tokens]
fn initialize_key_transition_map(
    language_tag: &LanguageTag,
    layers: &IndexMap<MacOsKbdLayer, String>,
    layered_key_transition_map: &mut IndexMap<MacOsKbdLayer, IndexMap<String, Vec<KeyTransition>>>,
) {
    for (layer, key_map) in layers.iter() {
        layered_key_transition_map.insert(*layer, IndexMap::new());

        let key_transition_map = layered_key_transition_map
            .get_mut(layer)
            .expect("getting back the value that was just inserted");

        let key_map: Vec<String> = split_keys(key_map);

        for (cursor, (_iso_key, key_code)) in MACOS_KEYS.iter().enumerate() {
            tracing::debug!(
                "iso len: {}; keymap len: {}",
                MACOS_KEYS.len(),
                key_map.len()
            );
            // [spec:kbdgen:req:keys.iso-order.desktop-layers]
            if MACOS_KEYS.len() > key_map.len() {
                panic!(
                    r#"Provided layer does not have enough keys, expected {} keys but`` got {}, in {}:{}:{}:{:?}: \n{:?}"#,
                    MACOS_KEYS.len(),
                    key_map.len(),
                    language_tag,
                    "MacOS",
                    "Primary",
                    layer,
                    key_map
                );
            }

            let key = key_map[cursor].clone();

            let key_transition = KeyTransition::Output(KeyOutput {
                code: *key_code,
                output: key.clone(),
            });

            // Add to existing Vec or create new Vec if key doesn't exist
            key_transition_map
                .entry(key.clone())
                .or_default()
                .push(key_transition);
        }
    }
}

// [spec:kbdgen:req:keylayout.transforms]
fn process_transforms(
    layers: &IndexMap<MacOsKbdLayer, String>,
    transforms: &IndexMap<String, Transform>,
    target_dead_keys: &IndexMap<MacOsKbdLayer, Vec<String>>,
    dead_keys: &mut IndexMap<String, DeadKeyOutput>,
    layered_key_transition_map: &mut IndexMap<MacOsKbdLayer, IndexMap<String, Vec<KeyTransition>>>,
    id_manager: &mut TransformIdManager,
) {
    for (layer, key_map) in layers.iter() {
        let mut cursor = 0;

        let layer_dead_keys = target_dead_keys.get(layer);

        if let Some(_layer_dead_keys) = layer_dead_keys {
            let key_transition_map = layered_key_transition_map
                .get_mut(layer)
                .expect("this map should be prefilled by now");

            for (_iso_key, _code) in MACOS_KEYS.iter() {
                let key_map: Vec<String> = split_keys(key_map);

                for (dead_key, value) in transforms {
                    //if !layer_dead_keys.contains(dead_key) {
                    //    continue;
                    //}

                    // [spec:kbdgen:req:layout.transforms.dead-key-entries]
                    match value {
                        Transform::End(_character) => {
                            tracing::error!("Transform ended too soon for dead key {}", dead_key);
                        }
                        Transform::More(map) => {
                            let escape_transform = map.get(TRANSFORM_ESCAPE).unwrap_or_else(|| {
                                panic!(
                                    "The escape transform `{}` not found for dead key `{}`",
                                    TRANSFORM_ESCAPE, dead_key
                                )
                            });

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

                                        if *next_char == key_map[cursor] {
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
                                        panic!(
                                            "nested transforms under `{dead_key}` are not supported on macOS"
                                        );
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

// [spec:kbdgen:sem:keylayout.actions]
fn update_key_transition_map_with_transform(
    key_transition_map: &mut IndexMap<String, Vec<KeyTransition>>,
    key: &str,
    transform: DeadKeyOutput,
    id_manager: &mut TransformIdManager,
) {
    if key_transition_map.contains_key(key) {
        let transitions = key_transition_map.get_mut(key).unwrap();

        // Apply transform to all transitions for this key
        for transition in transitions.iter_mut() {
            match transition {
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

                    *transition = KeyTransition::Action(action);
                }
                KeyTransition::Action(action) => {
                    // Only add the transform if it doesn't already exist to prevent duplicates
                    if !action
                        .states
                        .iter()
                        .any(|state| state.id == transform.id && state.output == transform.output)
                    {
                        action.states.push(transform.clone());
                    }
                }
                KeyTransition::Next(_) => {
                    panic!("Next states shouldn't exist yet!?!??!!??!?!");
                }
            }
        }
    } else {
        panic!(
            "The key_transition_map must already have Output entries for all keys by this point."
        )
    }
}

// [spec:kbdgen:req:keylayout.actions.dead-key-next]
fn create_dead_key_actions(
    layers: &IndexMap<MacOsKbdLayer, String>,
    layered_key_transition_map: &mut IndexMap<MacOsKbdLayer, IndexMap<String, Vec<KeyTransition>>>,
    target_dead_keys: &IndexMap<MacOsKbdLayer, Vec<String>>,
    dead_keys: &IndexMap<String, DeadKeyOutput>,
    id_manager: &mut TransformIdManager,
) {
    for (layer, key_map) in layers.iter() {
        let mut cursor = 0;

        let layer_dead_keys = target_dead_keys.get(layer);

        if let Some(layer_dead_keys) = layer_dead_keys {
            let key_transition_map = layered_key_transition_map
                .get_mut(layer)
                .expect("this map should be prefilled by now");

            for (_iso_key, key_code) in MACOS_KEYS.iter() {
                let key_map: Vec<String> = split_keys(key_map);

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
                            code: *key_code,
                            states: vec![none_state],
                        };

                        if let Some(transitions) = key_transition_map.get_mut(&key_map[cursor]) {
                            for transition in transitions.iter_mut() {
                                *transition = KeyTransition::Next(action.clone());
                            }
                        }
                    } else {
                        panic!(
                            "dead key `{}` in target list but not the transforms.",
                            key_map[cursor]
                        );
                    }
                }

                cursor += 1;
            }
        }
    }
}

type TransitionMap = IndexMap<MacOsKbdLayer, IndexMap<String, Vec<KeyTransition>>>;

/// The key element of one transition, decoded.
// [spec:kbdgen:req:keylayout.keymaps.tokens]
// [spec:kbdgen:sem:keylayout.actions]
fn key_of(transition: &KeyTransition) -> Key {
    match transition {
        KeyTransition::Output(output) => Key {
            code: output.code as u16,
            binding: Binding::Output(decode_unicode_escapes(&output.output)),
        },
        KeyTransition::Action(action) => Key {
            code: action.code as u16,
            binding: Binding::Action {
                id: action.id.clone(),
                whens: action
                    .states
                    .iter()
                    .map(|state| When::output(&state.id, decode_unicode_escapes(&state.output)))
                    .collect(),
            },
        },
        KeyTransition::Next(action) => Key {
            code: action.code as u16,
            binding: Binding::Action {
                id: action.id.clone(),
                whens: action
                    .states
                    .iter()
                    .map(|state| When::next(NONE_STATE, &state.next))
                    .collect(),
            },
        },
    }
}

/// One key map per layer, in YAML order: the layer's keys grouped by
/// token, then the fixed keys, the decimal key and the space bar.
// [spec:kbdgen:req:keylayout.keymaps]
fn key_maps(
    layers: &IndexMap<MacOsKbdLayer, String>,
    transitions: &TransitionMap,
    decimal: &str,
) -> Vec<KeyMap> {
    let fixed = MACOS_HARDCODED
        .iter()
        .map(|(code, output)| (*code, *output))
        .chain([(65, decimal), (49, " ")]);
    layers
        .keys()
        .map(|layer| {
            let mut keys: Vec<Key> = transitions[layer].values().flatten().map(key_of).collect();
            keys.extend(fixed.clone().map(|(code, output)| Key {
                code: code as u16,
                binding: Binding::Output(decode_unicode_escapes(output)),
            }));
            KeyMap {
                modifiers: vec![layer_attributes(layer)],
                keys,
            }
        })
        .collect()
}

/// The keylayout input of a v3 layout with a `macOS` section.
// [spec:kbdgen:def:ldml.macos.input]
// [spec:kbdgen:def:keylayout.document]
// [spec:kbdgen:req:keylayout.transforms]
pub fn layout_input(tag: &LanguageTag, layout: &Layout) -> Option<KeylayoutInput> {
    let target = layout.mac_os.as_ref()?;
    let layers = &target.primary.layers;
    let mut transitions = TransitionMap::new();
    let mut dead_keys: IndexMap<String, DeadKeyOutput> = IndexMap::new();
    initialize_key_transition_map(tag, layers, &mut transitions);
    let mut id_manager = TransformIdManager::new();
    match (&layout.transforms, &target.dead_keys) {
        (Some(transforms), Some(target_dead_keys)) => {
            process_transforms(
                layers,
                transforms,
                target_dead_keys,
                &mut dead_keys,
                &mut transitions,
                &mut id_manager,
            );
            create_dead_key_actions(
                layers,
                &mut transitions,
                target_dead_keys,
                &dead_keys,
                &mut id_manager,
            );
        }
        (Some(_), None) => tracing::warn!("No dead keys in {tag}:MacOS:Primary"),
        (None, _) => tracing::warn!("No transforms in {tag}:MacOS:Primary"),
    }
    let decimal = layout.decimal.as_deref().unwrap_or(".");
    Some(KeylayoutInput {
        name: keyboard_name(tag.as_str()),
        tag: tag.clone(),
        display_names: layout.display_names.clone(),
        default_index: 0,
        maps: key_maps(layers, &transitions, decimal),
        terminators: dead_keys
            .values()
            .map(|dead| When::output(&dead.id, decode_unicode_escapes(&dead.output)))
            .collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bundle::KbdgenBundle;
    use crate::bundle::layout::macos::MacOsKbdLayer;
    use crate::bundle::layout::{Layout, MacOsPrimaryPlatform, MacOsTarget};
    use indexmap::IndexMap;
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
        // Create a layout with duplicate 'a' keys
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

    fn documents(bundle: &KbdgenBundle) -> Vec<String> {
        bundle
            .layouts
            .iter()
            .filter_map(|(tag, layout)| layout_input(tag, layout))
            .map(|input| crate::build::macos::writer::write(&input))
            .collect()
    }

    // [spec:kbdgen:req:keylayout.keymaps.tokens/test]
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
        if let Some(transitions) = base_layer_map.get("q") {
            assert_eq!(transitions.len(), 1);
            if let KeyTransition::Output(output) = &transitions[0] {
                assert_eq!(output.output, "q");
                // Key code should be set correctly (checking actual mapping)
                assert!(output.code > 0, "Key code should be set");
            } else {
                panic!("Expected Output transition for 'q'");
            }
        } else {
            panic!("Expected transitions for 'q'");
        }
    }

    // [spec:kbdgen:req:keylayout.keymaps.tokens/test]
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

    // [spec:kbdgen:req:keylayout.keymaps.tokens/test]
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
        let layer_keys: Vec<String> = split_keys(layers.get(&MacOsKbdLayer::Default).unwrap());
        let a_count = layer_keys.iter().filter(|&k| k == "a").count();
        assert_eq!(a_count, 2); // We have 2 'a' keys

        assert!(base_layer_map.contains_key("a"));
        if let Some(a_transitions) = base_layer_map.get("a") {
            assert_eq!(a_transitions.len(), 2, "Should have 2 'a' key transitions");
        } else {
            panic!("Expected 'a' key to exist");
        }

        // The total number of unique keys should be less than MACOS_KEYS
        // because we have duplicates, but all transitions should be preserved
        let total_transitions: usize = base_layer_map.values().map(|v| v.len()).sum();
        assert_eq!(
            total_transitions,
            MACOS_KEYS.len(),
            "Should preserve all key positions"
        );
    }

    // [spec:kbdgen:req:keylayout.transforms/test]
    // [spec:kbdgen:sem:keylayout.actions/test]
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
        // 'a' should now have Action transitions instead of Output
        if let Some(a_transitions) = base_layer_map.get("a") {
            // All 'a' keys should have been transformed to Actions
            for transition in a_transitions {
                if let KeyTransition::Action(action) = transition {
                    assert_eq!(action.states.len(), 2); // none state + transform state
                    assert_eq!(action.states[0].id, "none");
                    assert_eq!(action.states[1].output, "á");
                } else {
                    panic!("Expected Action transition for 'a' after transform processing");
                }
            }
        } else {
            panic!("Expected 'a' transitions after transform processing");
        }
    }

    // [spec:kbdgen:sem:keylayout.actions/test]
    #[test]
    fn test_update_key_transition_map_with_transform() {
        let mut key_transition_map = IndexMap::new();
        let mut id_manager = TransformIdManager::new();

        // Insert initial outputs (simulate duplicate keys)
        key_transition_map.insert(
            "a".to_string(),
            vec![
                KeyTransition::Output(KeyOutput {
                    code: 0,
                    output: "a".to_string(),
                }),
                KeyTransition::Output(KeyOutput {
                    code: 1,
                    output: "a".to_string(),
                }),
            ],
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

        // Both transitions should now be Actions
        if let Some(transitions) = key_transition_map.get("a") {
            assert_eq!(transitions.len(), 2);
            for transition in transitions {
                if let KeyTransition::Action(action) = transition {
                    assert_eq!(action.states.len(), 2);
                    assert_eq!(action.states[0].id, "none");
                    assert_eq!(action.states[1].id, "dead_key000");
                    assert_eq!(action.states[1].output, "á");
                } else {
                    panic!("Expected Action transition after transform update");
                }
            }
        } else {
            panic!("Expected transitions for 'a'");
        }

        // Test adding another transform to existing Actions
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

        if let Some(transitions) = key_transition_map.get("a") {
            for transition in transitions {
                if let KeyTransition::Action(action) = transition {
                    assert_eq!(action.states.len(), 3);
                } else {
                    panic!("Expected Action transition with multiple states");
                }
            }
        } else {
            panic!("Expected transitions for 'a'");
        }
    }

    // [spec:kbdgen:req:keylayout.actions.dead-key-next/test]
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

        // The apostrophe key should now have Next actions
        if let Some(transitions) = base_layer_map.get("'") {
            for transition in transitions {
                if let KeyTransition::Next(next_action) = transition {
                    assert_eq!(next_action.states.len(), 1);
                    assert_eq!(next_action.states[0].next, "dead_key000");
                } else {
                    panic!("Expected Next transition for dead key");
                }
            }
        } else {
            panic!("Expected transitions for dead key");
        }
    }

    #[test]
    fn test_generate_key_layout_files() {
        let bundle = create_test_bundle();
        let key_layouts = documents(&bundle);
        assert_eq!(key_layouts.len(), 1);
        assert!(key_layouts[0].contains("<keyMapSet id=\"default\">"));
        assert!(key_layouts[0].contains("<modifierMap defaultIndex=\"0\" id=\"modifiers\">"));
        assert!(!key_layouts[0].contains("<actions>"));
    }

    #[test]
    fn test_generate_key_layout_files_with_transforms() {
        let bundle = create_test_bundle_with_transforms();
        let key_layouts = documents(&bundle);
        assert_eq!(key_layouts.len(), 1);
        assert!(key_layouts[0].contains("<actions>"));
        assert!(key_layouts[0].contains("<terminators>"));
    }

    // [spec:kbdgen:sem:keylayout.actions/test]
    #[test]
    fn duplicate_keys_get_no_duplicate_when_statements() {
        // This test specifically verifies that duplicate <when> statements are not generated within actions
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

        // Generate the key layout files
        let xml_string = documents(&bundle).remove(0);

        // For each action, extract all the when statements and check for duplicates
        let actions: Vec<&str> = xml_string.split(r#"<action id=""#).skip(1).collect();

        for action_content in actions {
            let action_end = action_content
                .find("</action>")
                .unwrap_or(action_content.len());
            let action_xml = &action_content[..action_end];

            // Extract all when statements from this action
            let when_statements: Vec<&str> = action_xml
                .split("<when ")
                .skip(1) // Skip the part before the first when
                .map(|s| {
                    s.split("/>")
                        .next()
                        .unwrap_or(s.split(">").next().unwrap_or(""))
                })
                .collect();

            // Check for duplicate when statements
            let mut seen_whens = std::collections::HashSet::new();
            for when_stmt in when_statements {
                if seen_whens.contains(&when_stmt) {
                    panic!("Found duplicate when statement in action: {}", when_stmt);
                }
                seen_whens.insert(when_stmt);
            }
        }
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
