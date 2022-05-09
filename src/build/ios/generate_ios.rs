use std::path::Path;

use async_trait::async_trait;
use indexmap::IndexMap;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::to_string_pretty;

use crate::{
    build::BuildStep,
    bundle::{layout::ios::IOsKbdLayer, KbdgenBundle},
    util::split_keys,
};

#[derive(Serialize, Deserialize)]
pub struct IosInfo {
    name: String,
    locale: String,
    enter: String,
    space: String,
}

// #[derive(Serialize, Deserialize)]
// pub struct PlatformDeadKeys;

// #[derive(Serialize, Deserialize)]
// pub struct IosDeadKeys {
//     #[serde(flatten)]
//     dead_keys: IndexMap<String, PlatformDeadKeys>
// }

pub fn keyboard_entity_from_string(input: String) -> Option<IosButton> {
    let regex = Regex::new(r"\{(\w+):([\d | \.]+)\}").expect("valid regex");
    let captures = regex.captures(input.as_str());

    if let Some(captures) = captures {
        if let Some(id) = captures.get(1) {
            if let Some(width) = captures.get(2) {
                Some(IosButton {
                    id: id.as_str().to_string(),
                    width: width.as_str().parse::<f32>().expect("invalid float value"),
                })
            } else {
                return None;
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
pub enum IosKeyMapTypes {
    Character(String),
    Button(IosButton),
}

#[derive(Serialize, Deserialize)]
pub struct IosNormalLayer {
    #[serde(flatten)]
    layer: IndexMap<IOsKbdLayer, Vec<Vec<IosKeyMapTypes>>>,
}

#[derive(Serialize, Deserialize)]
pub struct IosKeyboardDefinitions {
    #[serde(flatten)]
    info: IosInfo,
    longpress: IndexMap<String, Vec<String>>,
    // #[serde(rename = "camelCase")]
    // dead_keys: IosDeadKeys,
    // transforms: ????
    iphone: IosNormalLayer,
}

pub struct GenerateIos;

#[async_trait(?Send)]
impl BuildStep for GenerateIos {
    async fn build(&self, bundle: &KbdgenBundle, output_path: &Path) {
        let keyboard_definitions_file_path = output_path.join("KeyboardDefinitions.json");

        let info = IosInfo {
            name: "test".to_string(),
            locale: "te".to_string(),
            enter: "Enter".to_string(),
            space: "mellanslag".to_string(),
        };

        let mut longpress: IndexMap<String, Vec<String>> = IndexMap::new();
        let mut json_layers: IndexMap<IOsKbdLayer, Vec<Vec<IosKeyMapTypes>>> = IndexMap::new();
        // let mut dead_keys_list: IndexMap<String, PlatformDeadKeys> = IndexMap::new();
        for (_language_tag, layout) in &bundle.layouts {
            if let Some(ios_target) = &layout.i_os {
                if let Some(layout_longpress) = &layout.longpress {
                    for (key, value) in layout_longpress {
                        longpress.insert(key.clone(), value.clone());
                    }
                }

                if let Some(primary_platform) = &ios_target.primary {
                    for (layer_name, layer_key_map) in &primary_platform.layers {
                        let key_map_rows: Vec<&str> =
                            layer_key_map.split("\n").map(|x| x.clone()).collect();
                        let mut layer_rows: Vec<Vec<IosKeyMapTypes>> = Vec::new();
                        for key_map in key_map_rows {
                            let key_map = split_keys(key_map);
                            let mut new_key_map: Vec<IosKeyMapTypes> = Vec::new();
                            for key in key_map {
                                if let Some(keyboard_entity) =
                                    keyboard_entity_from_string(key.clone())
                                {
                                    new_key_map.push(IosKeyMapTypes::Button(keyboard_entity));
                                } else {
                                    new_key_map.push(IosKeyMapTypes::Character(key));
                                }
                            }
                            layer_rows.push(new_key_map)
                        }
                        json_layers.insert(layer_name.clone(), layer_rows);
                    }
                }
            }
        }

        std::fs::write(
            output_path.join(keyboard_definitions_file_path),
            serde_json::to_string_pretty(&[IosKeyboardDefinitions {
                info: info,
                longpress: longpress,
                // dead_keys: IosDeadKeys { dead_keys: dead_keys_list },
                iphone: IosNormalLayer { layer: json_layers },
            }])
            .unwrap(),
        )
        .unwrap();
    }
}
