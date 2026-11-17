use std::{path::Path, sync::LazyLock};

use anyhow::{Context, Result};
use async_trait::async_trait;
use indexmap::IndexMap;
use language_tags::LanguageTag;
use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::{
    build::{BuildStep, ios::ios_layouts},
    bundle::{
        KbdgenBundle,
        layout::{IOsPlatform, Transform, ios::IOsKbdLayer},
    },
    util::{TRANSFORM_ESCAPE, split_keys},
};

const REPOSITORY: &str = "repo";
const MODELS: &str = "Keyboard/Models";

// [spec:kbdgen:syn:ios.rows]
static SPECIAL_KEY: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\\s\{([^}:]+)(?::(\d+(?:\.\d+)?))?\}$").expect("valid regex"));

#[derive(Serialize, Deserialize)]
pub struct IosInfo {
    name: String,
    locale: String,
    #[serde(rename = "return")]
    return_button: String,
    space: String,
}

// [spec:kbdgen:syn:ios.rows]
pub fn keyboard_component_from_string(input: &str) -> Option<IosButton> {
    let captures = SPECIAL_KEY.captures(input)?;
    let id = &captures[1];
    let id = if id.starts_with('"') {
        id.replace('"', "")
    } else {
        format!("_{}", id.replace('"', ""))
    };
    let width = captures.get(2).map_or(1.0, |width| {
        width
            .as_str()
            .parse()
            .expect("the pattern only matches decimal widths")
    });
    Some(IosButton { id, width })
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

#[derive(Serialize, Deserialize, Default)]
pub struct IosPlatform {
    #[serde(flatten)]
    layer: IndexMap<String, Vec<Vec<IosKeyMapType>>>,
}

// [spec:kbdgen:def:ios.keyboard-json+1]
#[derive(Serialize, Deserialize)]
pub struct IosDeadKeys {
    iphone: IndexMap<IOsKbdLayer, Vec<String>>,
    #[serde(rename = "ipad-9in")]
    i_pad_9in: IndexMap<IOsKbdLayer, Vec<String>>,
    #[serde(rename = "ipad-12in")]
    i_pad_12in: IndexMap<IOsKbdLayer, Vec<String>>,
}

// [spec:kbdgen:def:ios.keyboard-json+1]
#[derive(Serialize, Deserialize)]
pub struct IosKeyboardDefinitions {
    #[serde(flatten)]
    info: IosInfo,
    #[serde(rename = "longPress")]
    longpress: IndexMap<String, Vec<String>>,
    #[serde(rename = "deadKeys")]
    dead_keys: IosDeadKeys,
    transforms: IndexMap<String, IndexMap<String, String>>,
    iphone: IosPlatform,
    #[serde(rename = "ipad-9in")]
    i_pad_9in: IosPlatform,
    #[serde(rename = "ipad-12in")]
    i_pad_12in: IosPlatform,
}

// [spec:kbdgen:syn:ios.rows]
pub fn ios_layer_name(layer: &IOsKbdLayer) -> &'static str {
    match layer {
        IOsKbdLayer::Default => "normal",
        IOsKbdLayer::Shift => "shifted",
        IOsKbdLayer::Caps => "caps",
        IOsKbdLayer::Alt => "alt",
        IOsKbdLayer::AltAndShift => "alt+shift",
        IOsKbdLayer::Symbols1 => "symbols-1",
        IOsKbdLayer::Symbols2 => "symbols-2",
    }
}

// [spec:kbdgen:syn:ios.rows]
pub fn generate_platform(platform: &IOsPlatform) -> IosPlatform {
    let layer = platform
        .layers
        .iter()
        .map(|(layer, rows)| {
            let rows = rows
                .trim()
                .split('\n')
                .map(|row| {
                    split_keys(row)
                        .into_iter()
                        .map(|key| match keyboard_component_from_string(&key) {
                            Some(button) => IosKeyMapType::Button(button),
                            None => IosKeyMapType::Character(key),
                        })
                        .collect()
                })
                .collect();
            (ios_layer_name(layer).to_string(), rows)
        })
        .collect();
    IosPlatform { layer }
}

// [spec:kbdgen:req:ios.transforms]
fn process_transforms(
    tag: &LanguageTag,
    transforms: &IndexMap<String, Transform>,
) -> IndexMap<String, IndexMap<String, String>> {
    let mut output_map: IndexMap<String, IndexMap<String, String>> = IndexMap::new();

    for (dead_key, transform) in transforms {
        let mut transforms_by_char: IndexMap<String, String> = IndexMap::new();
        match transform {
            Transform::End(character) => {
                tracing::warn!(
                    "Transform ended too soon for dead key {dead_key} in {tag}; ignoring its output {character}"
                );
            }
            Transform::More(transforms) => {
                // [spec:kbdgen:req:layout.transforms.dead-key-entries]
                for (next_char, transform) in transforms {
                    if next_char == TRANSFORM_ESCAPE {
                        transforms_by_char.insert(next_char.clone(), dead_key.clone());
                    }
                    match transform {
                        Transform::End(end_char) => {
                            transforms_by_char.insert(next_char.clone(), end_char.clone());
                        }
                        Transform::More(_transform) => {
                            todo!("Recursion required ahead");
                        }
                    }
                }
            }
        }

        output_map.insert(dead_key.clone(), transforms_by_char);
    }

    output_map
}

pub struct GenerateIos;

#[async_trait(?Send)]
impl BuildStep for GenerateIos {
    async fn build(&self, bundle: &KbdgenBundle, output_path: &Path) -> Result<()> {
        let path = output_path
            .join(REPOSITORY)
            .join(MODELS)
            .join("KeyboardDefinitions.json");
        let layouts = ios_layouts(bundle)?;
        tracing::info!(
            "Generating {} for {} layouts",
            path.display(),
            layouts.len()
        );

        let definitions: Vec<IosKeyboardDefinitions> = layouts
            .into_iter()
            .map(|(language_tag, layout)| {
                tracing::debug!("Generating the keyboard definition for {language_tag}");
                let ios = layout
                    .i_os
                    .as_ref()
                    .expect("ios_layouts only returns layouts with an iOS target");
                let key_names = layout
                    .key_names
                    .as_ref()
                    .expect("ios_layouts rejects layouts without keyNames");
                let dead_keys = ios.dead_keys.clone().unwrap_or_default();

                // [spec:kbdgen:def:ios.keyboard-json+1]
                IosKeyboardDefinitions {
                    info: IosInfo {
                        name: layout.autonym().to_string(),
                        locale: language_tag.to_string(),
                        return_button: key_names.r#return.clone(),
                        space: key_names.space.clone(),
                    },
                    longpress: layout.longpress.clone().unwrap_or_default(),
                    dead_keys: IosDeadKeys {
                        iphone: dead_keys.clone(),
                        i_pad_9in: dead_keys.clone(),
                        i_pad_12in: dead_keys,
                    },
                    transforms: process_transforms(
                        language_tag,
                        layout.transforms.as_ref().unwrap_or(&IndexMap::new()),
                    ),
                    iphone: ios
                        .primary
                        .as_ref()
                        .map(generate_platform)
                        .unwrap_or_default(),
                    i_pad_9in: ios
                        .i_pad_9in
                        .as_ref()
                        .map(generate_platform)
                        .unwrap_or_default(),
                    i_pad_12in: ios
                        .i_pad_12in
                        .as_ref()
                        .map(generate_platform)
                        .unwrap_or_default(),
                }
            })
            .collect();

        // [spec:kbdgen:def:ios.keyboard-json+1]
        let json = serde_json::to_string_pretty(&definitions)
            .expect("keyboard definitions serialize to JSON");
        std::fs::write(&path, json).with_context(|| format!("write {}", path.display()))
    }
}
