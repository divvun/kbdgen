//! The v3 path: a v3 layout's `macOS` section turned into the keylayout
//! input as the `keylayout.*` rules read its tokens.

use anyhow::{Result, bail};
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
enum KeyTransition {
    Output(KeyOutput),
    Action(DeadKeyAction),
    Next(DeadKeyNextAction),
}

#[derive(Debug, Clone)]
struct KeyOutput {
    code: usize,
    output: String,
}

#[derive(Debug, Clone)]
struct DeadKeyOutput {
    id: DeadKeyId,
    output: String,
}

#[derive(Debug, Clone)]
struct DeadKeyAction {
    id: ActionId,
    code: usize,
    states: Vec<DeadKeyOutput>,
}

/// A dead key's action: from state `none`, enter state `next`.
#[derive(Debug, Clone)]
struct DeadKeyNextAction {
    id: ActionId,
    code: usize,
    next: DeadKeyId,
}

type DeadKeyId = String;
type ActionId = String;

/// One layer's transitions by raw token, one per position holding it.
type LayerTransitions = IndexMap<String, Vec<KeyTransition>>;
type TransitionMap = IndexMap<MacOsKbdLayer, LayerTransitions>;

#[derive(Default)]
struct TransformIdManager {
    dead_key_counter: usize,
    action_counter: usize,
}

// [spec:kbdgen:sem:keylayout.actions]
impl TransformIdManager {
    fn next_dead_key(&mut self) -> DeadKeyId {
        let id = format!("dead_key{:03}", self.dead_key_counter);
        self.dead_key_counter += 1;
        id
    }

    fn next_action(&mut self) -> ActionId {
        let id = format!("action{:03}", self.action_counter);
        self.action_counter += 1;
        id
    }
}

// [spec:kbdgen:req:keylayout.keymaps.tokens]
fn initialize_key_transition_map(
    layers: &IndexMap<MacOsKbdLayer, String>,
) -> Result<TransitionMap> {
    let mut layered_key_transition_map = TransitionMap::new();
    for (layer, key_map) in layers {
        let keys = split_keys(key_map);
        // [spec:kbdgen:req:keys.iso-order.desktop-layers+2]
        if keys.len() < MACOS_KEYS.len() {
            bail!(
                "macOS layer {layer:?} does not have enough keys, expected {} keys but got {}",
                MACOS_KEYS.len(),
                keys.len()
            );
        }
        let mut key_transition_map = LayerTransitions::new();
        for (key, code) in keys.into_iter().zip(MACOS_KEYS.values()) {
            key_transition_map
                .entry(key.clone())
                .or_default()
                .push(KeyTransition::Output(KeyOutput {
                    code: *code,
                    output: key,
                }));
        }
        layered_key_transition_map.insert(*layer, key_transition_map);
    }
    Ok(layered_key_transition_map)
}

/// Registers each top-level transform as a dead-key state, in YAML order,
/// with its escape output as the terminator.
// [spec:kbdgen:req:layout.transforms.dead-key-entries+1]
// [spec:kbdgen:req:keylayout.transforms+1]
fn register_dead_keys(
    name: &str,
    transforms: &IndexMap<String, Transform>,
    id_manager: &mut TransformIdManager,
) -> Result<IndexMap<String, DeadKeyOutput>> {
    let mut dead_keys = IndexMap::new();
    for (dead_key, value) in transforms {
        let Transform::More(map) = value else {
            tracing::warn!("{name}: transform ended too soon for dead key `{dead_key}`; ignored");
            continue;
        };
        let escape = match map.get(TRANSFORM_ESCAPE) {
            Some(Transform::End(escape)) => escape,
            Some(Transform::More(_)) => bail!(
                "the escape transform of dead key `{dead_key}` should be a string, not another transform"
            ),
            None => bail!(
                "the escape transform `{TRANSFORM_ESCAPE}` not found for dead key `{dead_key}`"
            ),
        };
        dead_keys.insert(
            dead_key.clone(),
            DeadKeyOutput {
                id: id_manager.next_dead_key(),
                output: escape.clone(),
            },
        );
    }
    Ok(dead_keys)
}

// [spec:kbdgen:req:keylayout.transforms+1]
fn process_transforms(
    layers: &IndexMap<MacOsKbdLayer, String>,
    transforms: &IndexMap<String, Transform>,
    target_dead_keys: &IndexMap<MacOsKbdLayer, Vec<String>>,
    dead_keys: &IndexMap<String, DeadKeyOutput>,
    layered_key_transition_map: &mut TransitionMap,
    id_manager: &mut TransformIdManager,
) {
    for (layer, key_map) in layers {
        if !target_dead_keys.contains_key(layer) {
            continue;
        }
        let key_transition_map = layered_key_transition_map
            .get_mut(layer)
            .expect("every layer has its transitions");
        for token in split_keys(key_map).iter().take(MACOS_KEYS.len()) {
            for (dead_key, value) in transforms {
                // [spec:kbdgen:req:layout.transforms.dead-key-entries+1]
                let Transform::More(map) = value else {
                    continue;
                };
                let id = &dead_keys[dead_key].id;
                for (next_char, transform) in map {
                    match transform {
                        Transform::End(_) if next_char == TRANSFORM_ESCAPE => {}
                        Transform::End(end_char) => {
                            if next_char == token {
                                let key_transform = DeadKeyOutput {
                                    id: id.clone(),
                                    output: end_char.clone(),
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
        }
    }
}

// [spec:kbdgen:sem:keylayout.actions]
fn update_key_transition_map_with_transform(
    key_transition_map: &mut LayerTransitions,
    key: &str,
    transform: DeadKeyOutput,
    id_manager: &mut TransformIdManager,
) {
    let transitions = key_transition_map
        .get_mut(key)
        .expect("every token of the layer has its transitions");
    for transition in transitions.iter_mut() {
        match transition {
            KeyTransition::Output(output) => {
                let none_state = DeadKeyOutput {
                    id: NONE_STATE.to_owned(),
                    output: output.output.clone(),
                };
                *transition = KeyTransition::Action(DeadKeyAction {
                    id: id_manager.next_action(),
                    code: output.code,
                    states: vec![none_state, transform.clone()],
                });
            }
            KeyTransition::Action(action) => {
                if !action
                    .states
                    .iter()
                    .any(|state| state.id == transform.id && state.output == transform.output)
                {
                    action.states.push(transform.clone());
                }
            }
            KeyTransition::Next(_) => {
                unreachable!("dead-key next actions are created after every transform is applied")
            }
        }
    }
}

// [spec:kbdgen:req:keylayout.actions.dead-key-next+1]
fn create_dead_key_actions(
    layers: &IndexMap<MacOsKbdLayer, String>,
    layered_key_transition_map: &mut TransitionMap,
    target_dead_keys: &IndexMap<MacOsKbdLayer, Vec<String>>,
    dead_keys: &IndexMap<String, DeadKeyOutput>,
    id_manager: &mut TransformIdManager,
) -> Result<()> {
    for (layer, key_map) in layers {
        let Some(layer_dead_keys) = target_dead_keys.get(layer) else {
            continue;
        };
        let key_transition_map = layered_key_transition_map
            .get_mut(layer)
            .expect("every layer has its transitions");
        for (token, code) in split_keys(key_map).iter().zip(MACOS_KEYS.values()) {
            if !layer_dead_keys.contains(token) {
                continue;
            }
            let Some(dead_key) = dead_keys.get(token) else {
                bail!(
                    "dead key `{token}` of layer {layer:?} is in macOS.deadKeys but not in the transforms"
                );
            };
            let action = DeadKeyNextAction {
                id: id_manager.next_action(),
                code: *code,
                next: dead_key.id.clone(),
            };
            let transitions = key_transition_map
                .get_mut(token)
                .expect("every token of the layer has its transitions");
            for transition in transitions.iter_mut() {
                *transition = KeyTransition::Next(action.clone());
            }
        }
    }
    Ok(())
}

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
                whens: vec![When::next(NONE_STATE, &action.next)],
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

/// The keylayout input of a v3 layout, or `None` when it has no `macOS`
/// section. Dead keys apply only once some layer has a `deadKeys` entry.
// [spec:kbdgen:def:ldml.macos.input]
// [spec:kbdgen:def:keylayout.document]
// [spec:kbdgen:req:keylayout.transforms+1]
pub fn layout_input(tag: &LanguageTag, layout: &Layout) -> Result<Option<KeylayoutInput>> {
    let Some(target) = &layout.mac_os else {
        return Ok(None);
    };
    let name = keyboard_name(tag.as_str());
    let layers = &target.primary.layers;
    let mut transitions = initialize_key_transition_map(layers)?;
    let mut id_manager = TransformIdManager::default();
    let dead_keys = match (&layout.transforms, &target.dead_keys) {
        (Some(transforms), Some(target_dead_keys))
            if layers
                .keys()
                .any(|layer| target_dead_keys.contains_key(layer)) =>
        {
            let dead_keys = register_dead_keys(&name, transforms, &mut id_manager)?;
            process_transforms(
                layers,
                transforms,
                target_dead_keys,
                &dead_keys,
                &mut transitions,
                &mut id_manager,
            );
            create_dead_key_actions(
                layers,
                &mut transitions,
                target_dead_keys,
                &dead_keys,
                &mut id_manager,
            )?;
            dead_keys
        }
        (Some(_), None) => {
            tracing::warn!(
                "{name}: the layout has transforms but no macOS.deadKeys, so it has no dead keys on macOS"
            );
            IndexMap::new()
        }
        (None, Some(_)) => {
            tracing::warn!(
                "{name}: macOS.deadKeys is set but the layout has no transforms, so its dead keys type themselves"
            );
            IndexMap::new()
        }
        _ => IndexMap::new(),
    };
    let decimal = layout.decimal.as_deref().unwrap_or(".");
    Ok(Some(KeylayoutInput {
        name,
        tag: tag.clone(),
        display_names: layout.display_names.clone(),
        default_index: 0,
        maps: key_maps(layers, &transitions, decimal),
        terminators: dead_keys
            .values()
            .map(|dead| When::output(&dead.id, decode_unicode_escapes(&dead.output)))
            .collect(),
    }))
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::str::FromStr;

    use super::*;
    use crate::build::macos::writer;
    use crate::bundle::layout::{MacOsPrimaryPlatform, MacOsTarget};

    /// 48 tokens, one per ISO position from E00.
    const QWERTY: &str = "q w e r t y u i o p [ ] a s d f g h j k l ; ' z x c v b n m , . / \\ ` 1 2 3 4 5 6 7 8 9 0 - = space";
    /// [`QWERTY`] with a second `a` in place of `\`.
    const TWO_AS: &str = "q w e r t y u i o p [ ] a s d f g h j k l ; ' z x c v b n m , . / a ` 1 2 3 4 5 6 7 8 9 0 - = space";

    fn tag() -> LanguageTag {
        LanguageTag::from_str("en-US").unwrap()
    }

    /// The transforms of the `'` dead key: its escape, then `compositions`.
    fn acute(compositions: &[(&str, &str)]) -> IndexMap<String, Transform> {
        let branch = [(TRANSFORM_ESCAPE, "'")]
            .iter()
            .chain(compositions)
            .map(|(next, output)| (next.to_string(), Transform::End(output.to_string())))
            .collect();
        IndexMap::from([("'".to_owned(), Transform::More(branch))])
    }

    /// An `en-US` layout whose only macOS layer is `layer`. With
    /// `transforms`, `'` is listed as a dead key of that layer.
    fn layout(layer: &str, transforms: Option<IndexMap<String, Transform>>) -> Layout {
        let dead_keys = transforms
            .as_ref()
            .map(|_| IndexMap::from([(MacOsKbdLayer::Default, vec!["'".to_owned()])]));
        Layout {
            language_tag: tag(),
            display_names: IndexMap::from([(
                LanguageTag::from_str("en").unwrap(),
                "English".to_owned(),
            )]),
            decimal: Some(".".to_owned()),
            windows: None,
            chrome_os: None,
            mac_os: Some(MacOsTarget {
                primary: MacOsPrimaryPlatform {
                    layers: IndexMap::from([(MacOsKbdLayer::Default, layer.to_owned())]),
                },
                dead_keys,
                space: IndexMap::new(),
            }),
            i_os: None,
            android: None,
            longpress: None,
            transforms,
            key_names: None,
        }
    }

    fn layers(layout: &Layout) -> &IndexMap<MacOsKbdLayer, String> {
        &layout.mac_os.as_ref().unwrap().primary.layers
    }

    fn document(layout: &Layout) -> String {
        writer::write(&layout_input(&tag(), layout).unwrap().unwrap())
    }

    #[test]
    fn test_transform_id_manager() {
        let mut manager = TransformIdManager::default();

        assert_eq!(manager.next_dead_key(), "dead_key000");
        assert_eq!(manager.next_dead_key(), "dead_key001");
        assert_eq!(manager.next_action(), "action000");
        assert_eq!(manager.next_action(), "action001");
    }

    // [spec:kbdgen:req:keylayout.keymaps.tokens/test]
    #[test]
    fn test_initialize_key_transition_map_basic() {
        let layout = layout(QWERTY, None);
        let layered_key_transition_map = initialize_key_transition_map(layers(&layout)).unwrap();

        let base_layer_map = &layered_key_transition_map[&MacOsKbdLayer::Default];
        assert_eq!(base_layer_map.len(), MACOS_KEYS.len());
        let [KeyTransition::Output(output)] = &base_layer_map["q"][..] else {
            panic!("expected one Output transition for 'q'");
        };
        assert_eq!(output.output, "q");
        assert_eq!(output.code, 10, "E00");
    }

    // [spec:kbdgen:req:keylayout.keymaps.tokens/test]
    #[test]
    fn test_duplicate_keys_issue() {
        let layout = layout(TWO_AS, None);
        let layered_key_transition_map = initialize_key_transition_map(layers(&layout)).unwrap();
        let base_layer_map = &layered_key_transition_map[&MacOsKbdLayer::Default];

        assert_eq!(
            base_layer_map["a"].len(),
            2,
            "should have 2 'a' key transitions"
        );
        let total_transitions: usize = base_layer_map.values().map(Vec::len).sum();
        assert_eq!(
            total_transitions,
            MACOS_KEYS.len(),
            "should preserve all key positions"
        );
    }

    // [spec:kbdgen:req:keylayout.transforms+1/test]
    // [spec:kbdgen:sem:keylayout.actions/test]
    #[test]
    fn test_process_transforms_with_dead_keys() {
        let layout = layout(QWERTY, Some(acute(&[("a", "á"), ("e", "é")])));
        let layers = layers(&layout);
        let transforms = layout.transforms.as_ref().unwrap();
        let target_dead_keys = layout.mac_os.as_ref().unwrap().dead_keys.as_ref().unwrap();
        let mut id_manager = TransformIdManager::default();
        let mut layered_key_transition_map = initialize_key_transition_map(layers).unwrap();

        let dead_keys = register_dead_keys("enUS", transforms, &mut id_manager).unwrap();
        assert!(dead_keys.contains_key("'"));

        process_transforms(
            layers,
            transforms,
            target_dead_keys,
            &dead_keys,
            &mut layered_key_transition_map,
            &mut id_manager,
        );

        for transition in &layered_key_transition_map[&MacOsKbdLayer::Default]["a"] {
            let KeyTransition::Action(action) = transition else {
                panic!("expected an Action transition for 'a' after transform processing");
            };
            assert_eq!(action.states.len(), 2, "none state + transform state");
            assert_eq!(action.states[0].id, "none");
            assert_eq!(action.states[1].output, "á");
        }
    }

    // [spec:kbdgen:sem:keylayout.actions/test]
    #[test]
    fn test_update_key_transition_map_with_transform() {
        let mut id_manager = TransformIdManager::default();
        let mut key_transition_map = LayerTransitions::from([(
            "a".to_owned(),
            vec![
                KeyTransition::Output(KeyOutput {
                    code: 0,
                    output: "a".to_owned(),
                }),
                KeyTransition::Output(KeyOutput {
                    code: 1,
                    output: "a".to_owned(),
                }),
            ],
        )]);

        let transform = DeadKeyOutput {
            id: "dead_key000".to_owned(),
            output: "á".to_owned(),
        };
        update_key_transition_map_with_transform(
            &mut key_transition_map,
            "a",
            transform,
            &mut id_manager,
        );

        assert_eq!(key_transition_map["a"].len(), 2);
        for transition in &key_transition_map["a"] {
            let KeyTransition::Action(action) = transition else {
                panic!("expected an Action transition after the transform update");
            };
            assert_eq!(action.states.len(), 2);
            assert_eq!(action.states[0].id, "none");
            assert_eq!(action.states[1].id, "dead_key000");
            assert_eq!(action.states[1].output, "á");
        }

        let transform2 = DeadKeyOutput {
            id: "dead_key001".to_owned(),
            output: "à".to_owned(),
        };
        update_key_transition_map_with_transform(
            &mut key_transition_map,
            "a",
            transform2,
            &mut id_manager,
        );

        for transition in &key_transition_map["a"] {
            let KeyTransition::Action(action) = transition else {
                panic!("expected an Action transition with multiple states");
            };
            assert_eq!(action.states.len(), 3);
        }
    }

    // [spec:kbdgen:req:keylayout.actions.dead-key-next+1/test]
    #[test]
    fn test_create_dead_key_actions() {
        let layout = layout(QWERTY, Some(acute(&[("a", "á"), ("e", "é")])));
        let layers = layers(&layout);
        let target_dead_keys = layout.mac_os.as_ref().unwrap().dead_keys.as_ref().unwrap();
        let mut id_manager = TransformIdManager::default();
        let mut layered_key_transition_map = initialize_key_transition_map(layers).unwrap();
        let dead_keys = IndexMap::from([(
            "'".to_owned(),
            DeadKeyOutput {
                id: "dead_key000".to_owned(),
                output: "'".to_owned(),
            },
        )]);

        create_dead_key_actions(
            layers,
            &mut layered_key_transition_map,
            target_dead_keys,
            &dead_keys,
            &mut id_manager,
        )
        .unwrap();

        for transition in &layered_key_transition_map[&MacOsKbdLayer::Default]["'"] {
            let KeyTransition::Next(next_action) = transition else {
                panic!("expected a Next transition for the dead key");
            };
            assert_eq!(next_action.next, "dead_key000");
        }
    }

    #[test]
    fn test_generate_key_layout_files() {
        let key_layout = document(&layout(QWERTY, None));
        assert!(key_layout.contains("<keyMapSet id=\"default\">"));
        assert!(key_layout.contains("<modifierMap defaultIndex=\"0\" id=\"modifiers\">"));
        assert!(!key_layout.contains("<actions>"));
    }

    #[test]
    fn test_generate_key_layout_files_with_transforms() {
        let key_layout = document(&layout(QWERTY, Some(acute(&[("a", "á"), ("e", "é")]))));
        assert!(key_layout.contains("<actions>"));
        assert!(key_layout.contains("<terminators>"));
    }

    // [spec:kbdgen:sem:keylayout.actions/test]
    #[test]
    fn duplicate_keys_get_no_duplicate_when_statements() {
        let xml_string = document(&layout(TWO_AS, Some(acute(&[("a", "á")]))));

        for action_content in xml_string.split(r#"<action id=""#).skip(1) {
            let action_end = action_content
                .find("</action>")
                .unwrap_or(action_content.len());
            let action_xml = &action_content[..action_end];

            let mut seen_whens = HashSet::new();
            for when_stmt in action_xml.split("<when ").skip(1) {
                let when_stmt = when_stmt.split("/>").next().unwrap();
                assert!(
                    seen_whens.insert(when_stmt),
                    "found duplicate when statement in action: {when_stmt}"
                );
            }
        }
    }

    // [spec:kbdgen:req:keylayout.actions.dead-key-next+1/test]
    #[test]
    fn malformed_layouts_are_errors() {
        let error = |layout: &Layout| layout_input(&tag(), layout).unwrap_err().to_string();

        let short = layout("q w e", None);
        assert!(
            error(&short).contains("expected 48 keys but got 3"),
            "{}",
            error(&short)
        );

        let mut no_escape = layout(QWERTY, Some(acute(&[("a", "á")])));
        let Some(Transform::More(branch)) = no_escape.transforms.as_mut().unwrap().get_mut("'")
        else {
            unreachable!()
        };
        branch.shift_remove(TRANSFORM_ESCAPE);
        assert!(
            error(&no_escape).contains("not found for dead key `'`"),
            "{}",
            error(&no_escape)
        );

        let mut unlisted = layout(QWERTY, Some(acute(&[("a", "á")])));
        unlisted.mac_os.as_mut().unwrap().dead_keys = Some(IndexMap::from([(
            MacOsKbdLayer::Default,
            vec!["q".to_owned()],
        )]));
        assert!(
            error(&unlisted).contains("dead key `q` of layer Default is in macOS.deadKeys"),
            "{}",
            error(&unlisted)
        );
    }
}
