use std::{cmp::Ordering, path::Path};

use async_trait::async_trait;
use indexmap::IndexMap;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json;

use crate::{
    build::BuildStep,
    bundle::{
        layout::{ios::IOsKbdLayer, IOsPlatform},
        KbdgenBundle,
    },
    util::split_keys,
};

const REPOSITORY: &str = "repo";
const MODELS: &str = "Keyboard/Models";

#[derive(Serialize, Deserialize)]
pub struct IosInfo {
    name: String,
    locale: String,
    enter: String,
    space: String,
}

/// Removes all occurrences of `character` in `input`
pub fn remove_all_occurrences(input: String, character: char) -> String {
    input
        .as_str()
        .chars()
        .filter(|x| x.cmp(&character) != Ordering::Equal)
        .into_iter()
        .collect::<String>()
}

pub fn keyboard_component_from_string(input: String) -> Option<IosButton> {
    let regex = Regex::new(r"^\\s\{([^}:]+)(?::(\d+(?:\.\d+)?))?\}$").expect("valid regex");
    let captures = regex.captures(input.as_str());

    if let Some(captures) = captures {
        if let Some(id) = captures.get(1) {
            let id = match id.as_str().chars().next().unwrap().cmp(&'\"') {
                Ordering::Equal => remove_all_occurrences(id.as_str().to_string(), '\"'),
                _ => format!("_{}", remove_all_occurrences(id.as_str().to_string(), '\"')),
            };
            if let Some(width) = captures.get(2) {
                Some(IosButton {
                    id: id,
                    width: width.as_str().parse::<f32>().expect("invalid float value"),
                })
            } else {
                Some(IosButton {
                    id: id,
                    width: 1.0f32,
                })
            }
        } else {
            return None;
        }
    } else {
        return None;
    }
}

#[derive(Serialize, Deserialize)]
pub struct IosButton {
    id: String,
    width: f32,
}

#[derive(Serialize, Deserialize)]
#[serde(untagged)]
pub enum IosKeyMapType {
    Character(String),
    Button(IosButton),
}

#[derive(Serialize, Deserialize)]
pub struct IosPlatform {
    #[serde(flatten)]
    layer: IndexMap<String, Vec<Vec<IosKeyMapType>>>,
}

#[derive(Serialize, Deserialize)]
pub struct IosDeadKeys {
    iphone: IndexMap<String, String>,
    #[serde(rename = "ipad-9in")]
    i_pad_9in: IndexMap<String, String>,
    #[serde(rename = "ipad-12in")]
    i_pad_12in: IndexMap<String, String>,
}

#[derive(Serialize, Deserialize)]
pub struct IosKeyboardDefinitions {
    #[serde(flatten)]
    info: IosInfo,
    longpress: IndexMap<String, Vec<String>>,
    dead_keys: IosDeadKeys,
    transforms: serde_json::value::Value,
    iphone: IosPlatform,
    i_pad_9in: IosPlatform,
    i_pad_12in: IosPlatform,
}

pub fn ios_layer_name(layer: &IOsKbdLayer) -> String {
    match layer {
        IOsKbdLayer::Default => "normal",
        IOsKbdLayer::Shift => "shifted",
        IOsKbdLayer::Alt => "alt",
        IOsKbdLayer::AltAndShift => "alt+shift",
        IOsKbdLayer::Symbols1 => "symbols-1",
        IOsKbdLayer::Symbols2 => "symbols-2",
    }
    .to_string()
}

pub fn generate_platform(platform: &IOsPlatform) -> IndexMap<String, Vec<Vec<IosKeyMapType>>> {
    let mut layers: IndexMap<String, Vec<Vec<IosKeyMapType>>> = IndexMap::new();
    for (layer_name, layer_key_map) in &platform.layers {
        let layer_name = ios_layer_name(layer_name);
        let key_map_rows: Vec<&str> = layer_key_map
            .trim()
            .split("\n")
            .map(|x| x.clone())
            .collect();
        let mut layer_rows: Vec<Vec<IosKeyMapType>> = Vec::new();
        for key_map in key_map_rows {
            let key_map = split_keys(key_map);
            let mut new_key_map: Vec<IosKeyMapType> = Vec::new();
            for key in key_map {
                if let Some(keyboard_entity) = keyboard_component_from_string(key.clone()) {
                    new_key_map.push(IosKeyMapType::Button(keyboard_entity));
                } else {
                    new_key_map.push(IosKeyMapType::Character(key));
                }
            }
            layer_rows.push(new_key_map)
        }
        layers.insert(layer_name, layer_rows);
    }
    layers
}

pub struct GenerateIos;

#[async_trait(?Send)]
impl BuildStep for GenerateIos {
    async fn build(&self, bundle: &KbdgenBundle, output_path: &Path) {
        let repository_path = output_path.join(REPOSITORY);
        let models_path = repository_path.join(MODELS);
        let keyboard_definitions_file_path = models_path.join("KeyboardDefinitions.json");

        for (language_tag, layout) in &bundle.layouts {
            let mut longpress: IndexMap<String, Vec<String>> = IndexMap::new();
            let mut iphone_layers: IndexMap<String, Vec<Vec<IosKeyMapType>>> = IndexMap::new();
            let mut i_pad_9in_layers: IndexMap<String, Vec<Vec<IosKeyMapType>>> = IndexMap::new();
            let mut i_pad_12in_layers: IndexMap<String, Vec<Vec<IosKeyMapType>>> = IndexMap::new();

            if let Some(ios_target) = &layout.i_os {
                if let Some(layout_longpress) = &layout.longpress {
                    for (key, value) in layout_longpress {
                        longpress.insert(key.clone(), value.clone());
                    }
                }

                if let Some(primary_platform) = &ios_target.primary {
                    iphone_layers.extend(generate_platform(&primary_platform));
                }
                if let Some(i_pad_9in_platform) = &ios_target.i_pad_9in {
                    i_pad_9in_layers.extend(generate_platform(&i_pad_9in_platform));
                }
                if let Some(i_pad_12in_platform) = &ios_target.i_pad_12in {
                    i_pad_12in_layers.extend(generate_platform(&i_pad_12in_platform));
                }

                if let Some(key_names) = &layout.key_names {
                    std::fs::write(
                        output_path.join(keyboard_definitions_file_path.clone()),
                        serde_json::to_string_pretty(&[IosKeyboardDefinitions {
                            info: IosInfo {
                                name: layout
                                    .display_names
                                    .get(language_tag)
                                    .expect("can't evaluate language tag of layout")
                                    .to_string(),
                                locale: language_tag.to_string(),
                                enter: key_names.r#return.to_string(),
                                space: key_names.space.to_string(),
                            },
                            longpress: longpress,
                            dead_keys: IosDeadKeys {
                                iphone: IndexMap::new(),
                                i_pad_9in: IndexMap::new(),
                                i_pad_12in: IndexMap::new(),
                            },
                            transforms: serde_json::value::Value::Null,
                            iphone: IosPlatform {
                                layer: iphone_layers,
                            },
                            i_pad_9in: IosPlatform {
                                layer: i_pad_9in_layers,
                            },
                            i_pad_12in: IosPlatform {
                                layer: i_pad_12in_layers,
                            },
                        }])
                        .unwrap(),
                    )
                    .unwrap();
                }
            }
        }
    }
}
